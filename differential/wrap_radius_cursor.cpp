/* Differential-test wrapper for the in-game UI's radius cursor (GitHub #124, upstream commit 0a05454d), cut verbatim by
** build.rs into OUT_DIR/radius_cursor_gen:
**   rc_enum.inc      GameEngine/Include/GameClient/InGameUI.h, `enum RadiusCursorType` (to `#ifdef DEFINE_RADIUSCURSOR_NAMES`);
**   rc_classes.inc   GameEngine/Include/GameClient/RadiusDecal.h, `class RadiusDecal` and `class RadiusDecalTemplate` (to
**                    the include guard's `#endif`);
**   rc_tmpl.inc      GameEngine/Source/GameClient/RadiusDecal.cpp, `RadiusDecalTemplate::RadiusDecalTemplate` and
**                    `createRadiusDecal` (to `xferRadiusDecalTemplate`);
**   rc_ctor.inc      RadiusDecal.cpp, `RadiusDecal::RadiusDecal()` (to the copy constructor);
**   rc_decal.inc     RadiusDecal.cpp, `RadiusDecal::clear`, `~RadiusDecal`, `update`, `setOpacity`, `setPosition` (to the end);
**   rc_set.inc       GameEngine/Source/GameClient/InGameUI.cpp, `InGameUI::setRadiusCursor` and `handleRadiusCursor` (to
**                    `triggerDoubleClickAttackMoveGuardHint`);
**   rc_trigger.inc   InGameUI.cpp, `triggerDoubleClickAttackMoveGuardHint` (to the placement icons' comment).
** No edit. The header's classes are compiled with `private` read as `public` (the setup writes the templates as
** `parseRadiusDecalTemplate` would). Stand-ins: the projected shadow manager makes a recording `Shadow`; the players,
** objects, weapons and special power templates are the setup's; `TheMouse->getMouseStatus()->pos`, the radar, the
** tactical view, `TheGlobalData->m_doubleClickAttackMove` and `TheGameLogic`'s frame and icon flag are the step's
** environment: `screenToTerrain(p)` is (p.x, p.y, 3), the radar's world point (when it hits) (2 p.x, 2 p.y, 7).
**
** Entry point (extern "C", tests/differential_radius_cursor.rs): ref_radius_cursor_seq(in, nin, out, cap).
**   Setup: 30 templates of 8 words [texture 0 none | 1 "RC<i>", style, OpacityMin bits, OpacityMax bits, OpacityThrobTime,
**   Color, OnlyVisibleToOwningPlayer, 0]; 8 words [local player, the colours of players 0..3, 0, 0, 0]; 4 objects (ids
**   1..4) of 24 words [controlling player (-1 none), std guard range bits, then for slots 0..2: has a weapon, primary
**   damage radius bits, scatter radius bits, scatter target scalar bits, continue attack range bits]; 4 special power
**   templates (ids 0..3) of 8 words [type, radius cursor radius bits, the most ready shortcut object of that type (0 none)].
**   Steps of 8 words [kind, a, b, c, d, e, f, g]:
**     kind 0  the InGameUI: getSelectCount() a, getFirstSelectedDrawable() b (0 none, 1..4 the object's, 9 one without
**             an object), the pending command's type c (-1 none)
**     kind 1  setRadiusCursor(a, b < 0 ? NULL : template b, c)
**     kind 2  handleRadiusCursor()
**     kind 3  triggerDoubleClickAttackMoveGuardHint()
**     kind 4  the guard hint's timer = a
**     kind 5  the environment: mouse (a, b), radar on c, radar hits d, frame e, icons f, double-click attack move g
**   After the setup and after each step: [300, m_empty, m_template's type (-1 NULL), m_curRcType, m_decal != NULL, timer,
**   0, 0], [301, texture hash, shadow type, size x bits, size y bits, angle bits, colour, opacity (-1 never set)],
**   [302, decal x, y, z bits, stash x, y, z bits, 0].
**
** (#130, batch #134) Second entry point: ref_logic_decals_seq(in, nin, out, cap), the logic's own decals as the client
** keeps them: six real `RadiusDecal`s (objects 1..3, each with decals 0 and 1, slot = 2 (object - 1) + index), each with
** its module's `RadiusDecalTemplate`, driven by the calls the logic's `GridDecal` events stand for. Setup: 5 words [local
** player, the colours of players 0..3]. Steps of 16 words [kind, slot, ...]:
**     kind 0  m_tmpl[slot] = [texture 0 none | 1 "T<slot>", style, OpacityMin bits, OpacityMax bits, OpacityThrobTime,
**             Color, OnlyVisibleToOwningPlayer] (words 2-8), then m_tmpl[slot].createRadiusDecal((x, y, z) words 10-12
**             bits, radius word 9 bits, player word 13 (-1 NULL), decal[slot])
**     kind 1  decal[slot].clear()
**     kind 2  decal[slot].setPosition((x, y, z) words 2-4 bits)
**     kind 3  decal[slot].setOpacity(word 2 bits)
**     kind 4  TheGameLogic's frame word 2 and getDrawIconUI word 3, then decal[slot].update()
**     kind 5  the slot's object destroyed: ~RadiusDecal() on both its decals (then made anew, as a new object's would be)
**   After each step, per slot: [m_empty, m_decal != NULL, texture hash, type, size x bits, size y bits, angle bits,
**   colour, opacity (-1 never set), x, y, z bits] (12 words; the shadow's words 0 without a decal). The shadows come
**   from a free list of 16 here (the cursor's entry keeps its ring of two). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <new>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
}
/* BaseType.h lines 86 and 231 (the shim leaves the asm helpers to differential/wrap_base_type.cpp) */
#define PI 3.14159265359f
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))

namespace rcz {

std::vector<int> g_out;
void rec(Int tag, Int a = 0, Int b = 0, Int c = 0, Int d = 0, Int e = 0, Int f = 0, Int g = 0)
{
	Int w[8] = { tag, a, b, c, d, e, f, g };
	g_out.insert(g_out.end(), w, w + 8);
}
Int bitsOf(Real r) { Int b; memcpy(&b, &r, 4); return b; }
Real realOf(Int b) { Real r; memcpy(&r, &b, 4); return r; }

/* GameCommon.h */
enum { LOGICFRAMES_PER_SECOND = 30 };
typedef Int Color;
inline Real Sin(Real x) { return sinf(x); }
enum ShadowType { SHADOW_NONE = 0, SHADOW_ALPHA_DECAL = 0x20, SHADOW_TYPE_MAX = 0x7FFFFFFF };
class Xfer;
class INI;

/* ---- the projected decal ---- */
class Shadow {
public:
	struct ShadowTypeInfo {
		char m_ShadowName[64]; Bool allowUpdates; Bool allowWorldAlign; ShadowType m_type; Real m_sizeX; Real m_sizeY;
	};
	AsciiString m_name; Int m_type; Real m_sizeX, m_sizeY, m_angle; Color m_color; Coord3D m_pos; Int m_opacity;
	void setAngle(Real a) { m_angle = a; }
	void setColor(Color c) { m_color = c; }
	void setPosition(Real x, Real y, Real z) { m_pos.x = x; m_pos.y = y; m_pos.z = z; }
	void setOpacity(Int o) { m_opacity = o; }
	void release() { m_live = FALSE; }
	Bool m_live;
};
Shadow g_shadows[16];
Int g_nextShadow;
Bool g_freeList; /* (#130) the logic decals' entry: the first shadow not live */
struct ProjectedShadowManager {
	Shadow *addDecal(Shadow::ShadowTypeInfo *i)
	{
		Shadow *s = &g_shadows[g_nextShadow++ & 1];
		if (g_freeList) {
			Int k = 0;
			while (k < 15 && g_shadows[k].m_live)
				++k;
			s = &g_shadows[k];
		}
		s->m_name = i->m_ShadowName; s->m_type = i->m_type; s->m_sizeX = i->m_sizeX; s->m_sizeY = i->m_sizeY;
		s->m_angle = 99.0f; s->m_color = 0; s->m_pos.x = s->m_pos.y = s->m_pos.z = -1.0f; s->m_opacity = -1; s->m_live = TRUE;
		return s;
	}
};
ProjectedShadowManager g_psm;
ProjectedShadowManager *TheProjectedShadowManager = &g_psm;

/* ---- players, objects, weapons, special powers ---- */
typedef Int SpecialPowerType;
enum WeaponSlotType { PRIMARY_WEAPON = 0, SECONDARY_WEAPON, TERTIARY_WEAPON, WEAPONSLOT_COUNT };
class Object;
struct Weapon {
	Real m_dmg, m_scatter, m_scalar, m_cont;
	Real getPrimaryDamageRadius(const Object *) const { return m_dmg; }
	Real getScatterRadius() const { return m_scatter; }
	Real getScatterTargetScalar() const { return m_scalar; }
	Real getContinueAttackRange() const { return m_cont; }
};
class Player {
public:
	Int m_index; Color m_color;
	Int getPlayerIndex() const { return m_index; }
	Color getPlayerColor() const { return m_color; }
	Object *findMostReadyShortcutSpecialPowerOfType(SpecialPowerType t);
	Bool hasRadar() const { return TRUE; }
};
Player g_players[4];
class Object {
public:
	Int m_controller; Real m_guard; Bool m_has[3]; Weapon m_w[3];
	Player *getControllingPlayer() const { return m_controller < 0 ? NULL : &g_players[m_controller]; }
	const Weapon *getWeaponInWeaponSlot(WeaponSlotType s) const { return m_has[s] ? &m_w[s] : NULL; }
};
Object g_objects[5];
struct SpecialPowerTemplate {
	SpecialPowerType m_type; Real m_radius; Int m_mostReady;
	SpecialPowerType getSpecialPowerType() const { return m_type; }
	Real getRadiusCursorRadius() const { return m_radius; }
};
SpecialPowerTemplate g_powers[4];
Object *Player::findMostReadyShortcutSpecialPowerOfType(SpecialPowerType t)
{
	for (Int i = 0; i < 4; ++i)
		if (g_powers[i].m_type == t)
			return g_powers[i].m_mostReady ? &g_objects[g_powers[i].m_mostReady] : NULL;
	return NULL;
}
struct PlayerList { Int m_local; Player *getLocalPlayer() { return &g_players[m_local]; } };
PlayerList g_playerList;
PlayerList *ThePlayerList = &g_playerList;
struct AIGuardMachine { static Real getStdGuardRange(const Object *o) { return o->m_guard; } };
class Drawable { public: Object *m_obj; Object *getObject() { return m_obj; } };
Drawable g_drawables[10];

/* ---- the environment ---- */
struct MouseIO { ICoord2D pos; };
struct MouseStub { MouseIO m_io; const MouseIO *getMouseStatus() { return &m_io; } };
MouseStub g_mouse;
MouseStub *TheMouse = &g_mouse;
struct RadarStub {
	Bool m_on, m_hits;
	Bool isRadarForced() { return m_on; }
	Bool isRadarHidden() { return TRUE; }
	Bool screenPixelToWorld(const ICoord2D *p, Coord3D *w) { if (!m_hits) return FALSE; w->x = 2.0f * p->x; w->y = 2.0f * p->y; w->z = 7.0f; return TRUE; }
};
RadarStub g_radar;
RadarStub *TheRadar = &g_radar;
struct ViewStub { void screenToTerrain(const ICoord2D *p, Coord3D *w) { w->x = (Real)p->x; w->y = (Real)p->y; w->z = 3.0f; } };
ViewStub g_view;
ViewStub *TheTacticalView = &g_view;
struct GlobalData { Bool m_doubleClickAttackMove; };
GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;
struct GameLogic { UnsignedInt m_frame; Bool m_icons; UnsignedInt getFrame() { return m_frame; } Bool getDrawIconUI() { return m_icons; } };
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;

class RadiusDecalTemplate;
#define private public
#include "rc_classes.inc"
#undef private

enum GUICommandType { GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT = 31 };
struct CommandButton { Int m_type; GUICommandType getCommandType() const { return (GUICommandType)m_type; } };
CommandButton g_command;

class InGameUI
{
public:
#include "rc_enum.inc"
	void setRadiusCursor(RadiusCursorType r, const SpecialPowerTemplate* sp, WeaponSlotType wslot);
	void handleRadiusCursor();
	void triggerDoubleClickAttackMoveGuardHint( void );
	Int getSelectCount() { return m_selectCount; }
	Drawable *getFirstSelectedDrawable() { return m_first ? &g_drawables[m_first] : NULL; }
	Int m_selectCount; Int m_first;
	const CommandButton *m_pendingGUICommand;
	RadiusDecalTemplate m_radiusCursors[RADIUSCURSOR_COUNT];
	RadiusDecal m_curRadiusCursor;
	RadiusCursorType m_curRcType;
	Int m_duringDoubleClickAttackMoveGuardHintTimer;
	Coord3D m_duringDoubleClickAttackMoveGuardHintStashedPosition;
};

#include "rc_tmpl.inc"
#include "rc_ctor.inc"
#include "rc_decal.inc"
#include "rc_set.inc"
#include "rc_trigger.inc"

alignas(InGameUI) unsigned char g_buf[sizeof(InGameUI)];

void state(InGameUI *ui)
{
	RadiusDecal &d = ui->m_curRadiusCursor;
	Int t = -1;
	for (Int i = 0; i < InGameUI::RADIUSCURSOR_COUNT; ++i)
		if (d.m_template == &ui->m_radiusCursors[i])
			t = i;
	rec(300, d.m_empty, t, ui->m_curRcType, d.m_decal != NULL, ui->m_duringDoubleClickAttackMoveGuardHintTimer);
	Shadow *s = d.m_decal;
	if (s) {
		UnsignedInt h = 2166136261u;
		for (const char *c = s->m_name.str(); *c; ++c) {
			h ^= (UnsignedInt)(unsigned char)*c;
			h *= 16777619u;
		}
		rec(301, (Int)h, s->m_type, bitsOf(s->m_sizeX), bitsOf(s->m_sizeY), bitsOf(s->m_angle), s->m_color, s->m_opacity);
	} else {
		rec(301);
	}
	const Coord3D &st = ui->m_duringDoubleClickAttackMoveGuardHintStashedPosition;
	rec(302, s ? bitsOf(s->m_pos.x) : 0, s ? bitsOf(s->m_pos.y) : 0, s ? bitsOf(s->m_pos.z) : 0, bitsOf(st.x), bitsOf(st.y), bitsOf(st.z));
}

}  // namespace rcz

using namespace rcz;

extern "C" int ref_radius_cursor_seq(const int *in, int nin, int *out, int cap)
{
	const int SETUP = 30 * 8 + 8 + 4 * 24 + 4 * 8;
	g_out.clear();
	if (nin < SETUP)
		return 0;
	g_nextShadow = 0;
	g_freeList = FALSE;
	memset(&g_mouse, 0, sizeof g_mouse);
	g_radar.m_on = g_radar.m_hits = FALSE;
	g_gd.m_doubleClickAttackMove = FALSE;
	g_logic.m_frame = 0;
	g_logic.m_icons = TRUE;
	InGameUI *ui = new (g_buf) InGameUI();
	ui->m_selectCount = 0;
	ui->m_first = 0;
	ui->m_pendingGUICommand = NULL;
	ui->m_curRcType = InGameUI::RADIUSCURSOR_NONE;
	ui->m_duringDoubleClickAttackMoveGuardHintTimer = 0;
	ui->m_duringDoubleClickAttackMoveGuardHintStashedPosition.zero();
	char name[16];
	for (Int i = 0; i < 30; ++i) {
		const int *w = in + i * 8;
		RadiusDecalTemplate &t = ui->m_radiusCursors[i];
		sprintf(name, "RC%d", (int)i);
		t.m_name = w[0] ? name : "";
		t.m_shadowType = (ShadowType)w[1];
		t.m_minOpacity = realOf(w[2]);
		t.m_maxOpacity = realOf(w[3]);
		t.m_opacityThrobTime = (UnsignedInt)w[4];
		t.m_color = w[5];
		t.m_onlyVisibleToOwningPlayer = w[6] != 0;
	}
	const int *g = in + 240;
	g_playerList.m_local = g[0];
	for (Int p = 0; p < 4; ++p) {
		g_players[p].m_index = p;
		g_players[p].m_color = g[1 + p];
	}
	for (Int o = 0; o < 4; ++o) {
		const int *w = in + 248 + o * 24;
		Object &ob = g_objects[o + 1];
		ob.m_controller = w[0];
		ob.m_guard = realOf(w[1]);
		for (Int s = 0; s < 3; ++s) {
			ob.m_has[s] = w[2 + s * 5] != 0;
			ob.m_w[s].m_dmg = realOf(w[3 + s * 5]);
			ob.m_w[s].m_scatter = realOf(w[4 + s * 5]);
			ob.m_w[s].m_scalar = realOf(w[5 + s * 5]);
			ob.m_w[s].m_cont = realOf(w[6 + s * 5]);
		}
		g_drawables[o + 1].m_obj = &ob;
	}
	g_drawables[9].m_obj = NULL;
	for (Int p = 0; p < 4; ++p) {
		const int *w = in + 344 + p * 8;
		g_powers[p].m_type = w[0];
		g_powers[p].m_radius = realOf(w[1]);
		g_powers[p].m_mostReady = w[2];
	}
	state(ui);
	for (int s = SETUP; s + 8 <= nin; s += 8) {
		const int *w = in + s;
		switch (w[0]) {
		case 0:
			ui->m_selectCount = w[1];
			ui->m_first = w[2];
			g_command.m_type = w[3];
			ui->m_pendingGUICommand = w[3] < 0 ? NULL : &g_command;
			break;
		case 1:
			ui->setRadiusCursor((InGameUI::RadiusCursorType)w[1], w[2] < 0 ? NULL : &g_powers[w[2]], (WeaponSlotType)w[3]);
			break;
		case 2: ui->handleRadiusCursor(); break;
		case 3: ui->triggerDoubleClickAttackMoveGuardHint(); break;
		case 4: ui->m_duringDoubleClickAttackMoveGuardHintTimer = w[1]; break;
		case 5:
			g_mouse.m_io.pos.x = w[1];
			g_mouse.m_io.pos.y = w[2];
			g_radar.m_on = w[3] != 0;
			g_radar.m_hits = w[4] != 0;
			g_logic.m_frame = (UnsignedInt)w[5];
			g_logic.m_icons = w[6] != 0;
			g_gd.m_doubleClickAttackMove = w[7] != 0;
			break;
		}
		state(ui);
	}
	ui->~InGameUI();
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}

/* ---- (#130) the logic's own decals ---- */
namespace rcz {
alignas(RadiusDecal) unsigned char g_decalBuf[6][sizeof(RadiusDecal)];
RadiusDecalTemplate g_tmpl[6];
RadiusDecal *decalAt(Int s) { return reinterpret_cast<RadiusDecal *>(g_decalBuf[s]); }
UnsignedInt nameHash(const char *c)
{
	UnsignedInt h = 2166136261u;
	for (; *c; ++c) {
		h ^= (UnsignedInt)(unsigned char)*c;
		h *= 16777619u;
	}
	return h;
}
}  // namespace rcz

extern "C" int ref_logic_decals_seq(const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	if (nin < 5)
		return 0;
	g_freeList = TRUE;
	for (Int k = 0; k < 16; ++k)
		g_shadows[k].m_live = FALSE;
	g_logic.m_frame = 0;
	g_logic.m_icons = TRUE;
	g_playerList.m_local = in[0];
	for (Int p = 0; p < 4; ++p) {
		g_players[p].m_index = p;
		g_players[p].m_color = in[1 + p];
	}
	for (Int s = 0; s < 6; ++s) {
		new (g_decalBuf[s]) RadiusDecal();
		g_tmpl[s] = RadiusDecalTemplate();
	}
	char name[16];
	for (int st = 5; st + 16 <= nin; st += 16) {
		const int *w = in + st;
		Int slot = w[1];
		RadiusDecal *d = decalAt(slot);
		switch (w[0]) {
		case 0: {
			RadiusDecalTemplate &t = g_tmpl[slot];
			sprintf(name, "T%d", (int)slot);
			t.m_name = w[2] ? name : "";
			t.m_shadowType = (ShadowType)w[3];
			t.m_minOpacity = realOf(w[4]);
			t.m_maxOpacity = realOf(w[5]);
			t.m_opacityThrobTime = (UnsignedInt)w[6];
			t.m_color = w[7];
			t.m_onlyVisibleToOwningPlayer = w[8] != 0;
			Coord3D pos;
			pos.x = realOf(w[10]); pos.y = realOf(w[11]); pos.z = realOf(w[12]);
			t.createRadiusDecal(pos, realOf(w[9]), w[13] < 0 ? NULL : &g_players[w[13]], *d);
			break;
		}
		case 1: d->clear(); break;
		case 2: {
			Coord3D pos;
			pos.x = realOf(w[2]); pos.y = realOf(w[3]); pos.z = realOf(w[4]);
			d->setPosition(pos);
			break;
		}
		case 3: d->setOpacity(realOf(w[2])); break;
		case 4:
			g_logic.m_frame = (UnsignedInt)w[2];
			g_logic.m_icons = w[3] != 0;
			d->update();
			break;
		case 5: {
			Int first = slot & ~1;
			for (Int s = first; s < first + 2; ++s) {
				decalAt(s)->~RadiusDecal();
				new (g_decalBuf[s]) RadiusDecal();
			}
			break;
		}
		}
		for (Int s = 0; s < 6; ++s) {
			RadiusDecal *e = decalAt(s);
			Shadow *sh = e->m_decal;
			Int w12[12] = { (Int)e->m_empty, sh != NULL, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 };
			if (sh) {
				w12[2] = (Int)nameHash(sh->m_name.str());
				w12[3] = sh->m_type; w12[4] = bitsOf(sh->m_sizeX); w12[5] = bitsOf(sh->m_sizeY); w12[6] = bitsOf(sh->m_angle);
				w12[7] = sh->m_color; w12[8] = sh->m_opacity;
				w12[9] = bitsOf(sh->m_pos.x); w12[10] = bitsOf(sh->m_pos.y); w12[11] = bitsOf(sh->m_pos.z);
			}
			g_out.insert(g_out.end(), w12, w12 + 12);
		}
	}
	for (Int s = 0; s < 6; ++s)
		decalAt(s)->~RadiusDecal();
	g_freeList = FALSE;
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
