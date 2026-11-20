/* Differential-test wrapper for the in-game UI draft (#215, group lc3, upstream commit 0a05454d), src/drafts/sweep_common_client/in_game_ui.rs. Cut verbatim by build.rs
** from GameEngine/Source/GameClient/InGameUI.cpp (the end of each cut is the next function's banner or start):
**   igu_super.inc   showObjectSuperweaponDisplayByScript, setSuperweaponDisplayEnabledByScript, getSuperweaponDisplayEnabledByScript;
**   igu_draw.inc    SuperweaponInfo::drawName and drawTime;
**   igu_msg.inc     addMessageText, removeMessageAtIndex, beginAreaSelectHint, endAreaSelectHint, createMoveHint;
**   igu_icons.inc   destroyPlacementIcons;   igu_sel.inc  isDrawableSelected, isAnySelectedKindOf, isAllSelectedKindOf;
**   igu_expire.inc  expireHint;   igu_can.inc  canSelectedObjectsNonAttackInteractWithObject;
**   igu_float.inc   updateFloatingText, drawFloatingText, clearFloatingText;
**   igu_idle.inc    findIdleWorker, addIdleWorker, removeIdleWorker;   igu_idle2.inc  getIdleWorkerCount .. resetIdleWorker.
** and GameGetColorComponents from Color.cpp (igu_color.inc), with GameClient/Color.h read unchanged.
**
** Stand-ins (outside the cuts): RGBColor is BaseType.h's with getAsInt (the shim's BaseType.h has the data members only); class InGameUI with only the members the cuts read, as InGameUI.h declares them (UIMessage, MoveHintStruct, the lists and maps), and the
** functions; AsciiString (a map key), UnicodeString (units with clear and assignment); DisplayString / TheDisplayStringManager / TheFontLibrary / TheGlobalLanguageData
** (handles and a log of every call); TheGameLogic and TheGameClient (the op's frames); TheTacticalView->worldToScreen (false for x < 0, else (x*1.5, y*2.5+3) truncated);
** ThePartitionManager (worldToCell = x/10, y/10 truncated; the shroud is clear for the cells a bit mask names); ThePlayerList's local player; Object and Drawable
** (an id, a controlling player, a kind mask, an object or none); GameWindow / TheWindowManager / GadgetButtonSetText for the idle worker window (logged);
** TheTerrainVisual and TheGameClient->destroyDrawable (logged); `canSelectedObjectsDoAction` (a bit mask of actions); REAL_TO_INT is `(Int)(x)` (the real
** one is x87 assembly for a truncation); MemoryPoolObject::deleteInstance is `delete`. ActionType is copied from InGameUI.h (ALLOW_SURRENDER undefined).
**
** Entry point ref_lc3_igu_run(prog, nprog, out): see tests/differential_draft_in_game_ui.rs for the layout. */
#include "Lib/BaseType.h"
#include "GameClient/Color.h"
#include <vector>
#include <list>
#include <map>
#include <string>
#include <string.h>
#ifndef NULL
#define NULL 0
#endif
#ifndef TRUE
#define TRUE true
#define FALSE false
#endif
#undef REAL_TO_INT
#define REAL_TO_INT(x) ((Int)(x))
#define NEW new
#define DEBUG_ASSERTCRASH(a, b)
#define DEBUG_CRASH(a)
#include "igu_color.inc"

// BaseType.h's RGBColor with getAsInt (the shim's BaseType.h keeps only the data members), copied from Libraries/Include/Lib/BaseType.h
struct RGBColorX
{
	Real red, green, blue;
	inline Int getAsInt() const
	{
		return
			((Int)(red * 255.0) << 16) |
			((Int)(green * 255.0) << 8) |
			((Int)(blue * 255.0) << 0);
	}
};
#define RGBColor RGBColorX
typedef unsigned int ObjectID;
typedef unsigned int DrawableID;
enum { MAX_PLAYER_COUNT = 16 };
enum KindOfType { KINDOF_IMMOBILE = 3, KINDOF_OTHER = 5 };
enum CellShroudStatus { CELLSHROUD_CLEAR, CELLSHROUD_FOGGED, CELLSHROUD_SHROUDED };
enum HintType { MOVE_HINT };

namespace {

std::vector<int> g_ev;
int g_nextDs = 1;
int sumOf(const std::vector<unsigned short> &v) { int s = 0; for (size_t i = 0; i < v.size(); ++i) s = s * 31 + v[i]; return s; }

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	bool operator<(const AsciiString &o) const { return s < o.s; }
};

class UnicodeString
{
public:
	std::vector<unsigned short> v;
	static UnicodeString TheEmptyString;
	void clear() { v.clear(); }
};
UnicodeString UnicodeString::TheEmptyString;

class DisplayString
{
public:
	int id;
	void setFont(void *) { g_ev.push_back(2); g_ev.push_back(id); }
	void setText(const UnicodeString &t) { g_ev.push_back(3); g_ev.push_back(id); g_ev.push_back((int)t.v.size()); g_ev.push_back(sumOf(t.v)); }
	Int getWidth() { return (id * 13) % 40 + 5; }
	void getSize(Int *w, Int *h) { if (w) *w = getWidth(); if (h) *h = 9; }
	void draw(Int x, Int y, Color c, Color d) { g_ev.push_back(5); g_ev.push_back(id); g_ev.push_back(x); g_ev.push_back(y); g_ev.push_back(c); g_ev.push_back(d); }
};
struct DisplayStringManagerStandIn
{
	DisplayString *newDisplayString() { DisplayString *d = new DisplayString; d->id = g_nextDs++; g_ev.push_back(1); g_ev.push_back(d->id); return d; }
	void freeDisplayString(DisplayString *d) { g_ev.push_back(4); g_ev.push_back(d->id); }
} g_dsm;
DisplayStringManagerStandIn *TheDisplayStringManager = &g_dsm;
struct FontLibraryStandIn { void *getFont(const AsciiString &, Int, Bool) { return NULL; } } g_fl;
FontLibraryStandIn *TheFontLibrary = &g_fl;
struct GlobalLanguageDataStandIn { Int adjustFontSize(Int s) { return s; } } g_gld;
GlobalLanguageDataStandIn *TheGlobalLanguageData = &g_gld;

unsigned g_logicFrame = 0, g_clientFrame = 0;
struct GameLogicStandIn { unsigned getFrame() { return g_logicFrame; } } g_gl;
GameLogicStandIn *TheGameLogic = &g_gl;

struct Drawable;
struct Player { int index; Int getPlayerIndex() { return index; } };
Player g_players[MAX_PLAYER_COUNT];
int g_local = 0;
struct PlayerListStandIn { Player *getLocalPlayer() { return &g_players[g_local]; } } g_pl;
PlayerListStandIn *ThePlayerList = &g_pl;

class Object
{
public:
	ObjectID id; int player; unsigned kinds;
	ObjectID getID() const { return id; }
	Player *getControllingPlayer() { return &g_players[player]; }
	bool isKindOf(KindOfType k) const { return (kinds >> k) & 1; }
};
class Drawable
{
public:
	DrawableID id; Object *obj; unsigned kinds;
	DrawableID getID() const { return id; }
	Object *getObject() { return obj; }
	bool isKindOf(KindOfType k) const { return (kinds >> k) & 1; }
};

struct TacticalViewStandIn
{
	bool worldToScreen(const Coord3D *p, ICoord2D *s) { if (p->x < 0) return false; s->x = (Int)(p->x * 1.5f); s->y = (Int)(p->y * 2.5f + 3.0f); return true; }
} g_tv;
TacticalViewStandIn *TheTacticalView = &g_tv;
unsigned long long g_shroud = 0;
struct PartitionManagerStandIn
{
	void worldToCell(Real x, Real y, Int *cx, Int *cy) { *cx = (Int)(x / 10.0f); *cy = (Int)(y / 10.0f); }
	CellShroudStatus getShroudStatusForPlayer(Int, Int cx, Int cy) { int b = (((cx % 8) + 8) % 8) * 8 + (((cy % 8) + 8) % 8); return (g_shroud >> b) & 1 ? CELLSHROUD_CLEAR : CELLSHROUD_SHROUDED; }
} g_pm;
PartitionManagerStandIn *ThePartitionManager = &g_pm;

class GameWindow
{
public:
	void winEnable(Bool e) { g_ev.push_back(6); g_ev.push_back(e); }
};
GameWindow g_idleWin;
int g_idleWinFound = 0;
struct NameKeyGeneratorStandIn { int nameToKey(const char *) { return 1; } } g_nkg;
NameKeyGeneratorStandIn *TheNameKeyGenerator = &g_nkg;
#define NAMEKEY(x) 1
struct WindowManagerStandIn { GameWindow *winGetWindowFromId(GameWindow *, int) { g_ev.push_back(8); return g_idleWinFound ? &g_idleWin : NULL; } } g_wm;
WindowManagerStandIn *TheWindowManager = &g_wm;
void GadgetButtonSetText(GameWindow *, const UnicodeString &) { g_ev.push_back(7); }

struct TerrainVisualStandIn { void removeFactionBibDrawable(Drawable *d) { g_ev.push_back(11); g_ev.push_back((int)d->id); } void removeAllBibs() { g_ev.push_back(10); } } g_tvis;
TerrainVisualStandIn *TheTerrainVisual = &g_tvis;
struct GameClientStandIn { unsigned getFrame() { return g_clientFrame; } void destroyDrawable(Drawable *d) { g_ev.push_back(9); g_ev.push_back((int)d->id); } } g_gc;
GameClientStandIn *TheGameClient = &g_gc;
struct GlobalDataStandIn { Int m_maxLineBuildObjects; } g_gd;
GlobalDataStandIn *TheGlobalData = &g_gd;

class FloatingTextData
{
public:
	Color m_color; Coord3D m_pos3D; UnsignedInt m_frameTimeOut; UnsignedInt m_frameCount; DisplayString *m_dString;
	void deleteInstance() { if (m_dString) TheDisplayStringManager->freeDisplayString(m_dString); delete this; }
};
class SuperweaponInfo
{
public:
	ObjectID m_id; Bool m_hiddenByScript; Color m_color; DisplayString *m_nameDisplayString; DisplayString *m_timeDisplayString;
	void drawName(Int x, Int y, Color color, Color dropColor);
	void drawTime(Int x, Int y, Color color, Color dropColor);
};
typedef std::list<SuperweaponInfo *> SuperweaponList;
typedef std::map<AsciiString, SuperweaponList> SuperweaponMap;
typedef std::list<FloatingTextData *> FloatingTextList;
typedef FloatingTextList::iterator FloatingTextListIt;
typedef std::list<Object *> ObjectList;
typedef ObjectList::iterator ObjectListIt;
typedef std::list<Drawable *> DrawableList;
typedef DrawableList::const_iterator DrawableListCIt;

struct Region2DStandIn { IRegion2D r; };
struct GameMessageArg { IRegion2D pixelRegion; ObjectID objectID; Coord3D location; };
struct GameMessage { GameMessageArg a; const GameMessageArg *getArgument(int) const { return &a; } };

unsigned g_canMask = 0;

class InGameUI
{
public:
	enum { MAX_MOVE_HINTS = 256 };
	struct MoveHintStruct { Coord3D pos; UnsignedInt sourceID; UnsignedInt frame; };
	struct UIMessage { UnicodeString fullText; DisplayString *displayString; UnsignedInt timestamp; Color color; };
	enum { MAX_UI_MESSAGES = 6 };
	enum SelectionRules { SELECTION_ANY, SELECTION_ALL };
	enum ActionType { ACTIONTYPE_NONE, ACTIONTYPE_ATTACK_OBJECT, ACTIONTYPE_GET_REPAIRED_AT, ACTIONTYPE_DOCK_AT, ACTIONTYPE_GET_HEALED_AT, ACTIONTYPE_REPAIR_OBJECT, ACTIONTYPE_RESUME_CONSTRUCTION, ACTIONTYPE_ENTER_OBJECT, ACTIONTYPE_HIJACK_VEHICLE, ACTIONTYPE_CONVERT_OBJECT_TO_CARBOMB, ACTIONTYPE_CAPTURE_BUILDING, ACTIONTYPE_DISABLE_VEHICLE_VIA_HACKING, ACTIONTYPE_STEAL_CASH_VIA_HACKING, ACTIONTYPE_DISABLE_BUILDING_VIA_HACKING, ACTIONTYPE_MAKE_DEFECTOR, ACTIONTYPE_SET_RALLY_POINT, ACTIONTYPE_COMBATDROP_INTO, ACTIONTYPE_SABOTAGE_BUILDING, NUM_ACTIONTYPES };

	UIMessage m_uiMessages[MAX_UI_MESSAGES];
	Color m_messageColor1, m_messageColor2;
	AsciiString m_messageFont; Int m_messagePointSize; Bool m_messageBold;
	Bool m_isDragSelecting; IRegion2D m_dragSelectRegion;
	MoveHintStruct m_moveHint[MAX_MOVE_HINTS]; Int m_nextMoveHint;
	DrawableList m_selectedDrawables;
	Drawable **m_placeIcon;
	FloatingTextList m_floatingTextList;
	Real m_floatingTextMoveVanishRate, m_floatingTextMoveUpSpeed;
	ObjectList m_idleWorkers[MAX_PLAYER_COUNT];
	GameWindow *m_idleWorkerWin; Int m_currentIdleWorkerDisplay; Bool m_inputEnabled;
	SuperweaponMap m_superweapons[MAX_PLAYER_COUNT];
	Bool m_superweaponHiddenByScript;

	InGameUI() : m_messageColor1(0), m_messageColor2(0), m_messagePointSize(0), m_messageBold(false), m_isDragSelecting(false), m_nextMoveHint(0), m_placeIcon(NULL), m_floatingTextMoveVanishRate(0), m_floatingTextMoveUpSpeed(0), m_idleWorkerWin(NULL), m_currentIdleWorkerDisplay(-1), m_inputEnabled(true), m_superweaponHiddenByScript(false)
	{ memset(m_moveHint, 0, sizeof m_moveHint); for (int i = 0; i < MAX_MOVE_HINTS; ++i) {} for (int i = 0; i < MAX_UI_MESSAGES; ++i) { m_uiMessages[i].displayString = NULL; m_uiMessages[i].timestamp = 0; m_uiMessages[i].color = 0; } memset(&m_dragSelectRegion, 0, sizeof m_dragSelectRegion); }
	Int getSelectCount() { return (Int)m_selectedDrawables.size(); }
	Drawable *getFirstSelectedDrawable() { return m_selectedDrawables.empty() ? NULL : m_selectedDrawables.front(); }
	Bool getInputEnabled() { return m_inputEnabled; }
	Bool canSelectedObjectsDoAction(ActionType a, const Object *, SelectionRules) const { return (g_canMask >> a) & 1; }
	void showObjectSuperweaponDisplayByScript(const Object *obj);
	void setSuperweaponDisplayEnabledByScript(Bool enable);
	Bool getSuperweaponDisplayEnabledByScript(void) const;
	void addMessageText(const UnicodeString &formattedMessage, const RGBColor *rgbColor);
	void removeMessageAtIndex(Int i);
	void beginAreaSelectHint(const GameMessage *msg);
	void endAreaSelectHint(const GameMessage *msg);
	void createMoveHint(const GameMessage *msg);
	void destroyPlacementIcons(void);
	Bool isDrawableSelected(DrawableID idToCheck) const;
	Bool isAnySelectedKindOf(KindOfType kindOf) const;
	Bool isAllSelectedKindOf(KindOfType kindOf) const;
	void expireHint(HintType type, UnsignedInt hintIndex);
	Bool canSelectedObjectsNonAttackInteractWithObject(const Object *objectToInteractWith, SelectionRules rule) const;
	void updateFloatingText(void);
	void drawFloatingText(void);
	void clearFloatingText(void);
	Object *findIdleWorker(Object *obj);
	void addIdleWorker(Object *obj);
	void removeIdleWorker(Object *obj, Int playerNumber);
	Int getIdleWorkerCount(void);
	void showIdleWorkerLayout(void);
	void hideIdleWorkerLayout(void);
	void updateIdleWorker(void);
	void resetIdleWorker(void);
};

#include "igu_super.inc"
#include "igu_draw.inc"
#include "igu_msg.inc"
#include "igu_icons.inc"
#include "igu_sel.inc"
#include "igu_expire.inc"
#include "igu_can.inc"
#include "igu_float.inc"
#include "igu_idle.inc"
#include "igu_idle2.inc"

}

namespace {
int rdi(const int *p, int &k) { return p[k++]; }
float fl(int b) { float f; memcpy(&f, &b, 4); return f; }
int bi(float f) { int b; memcpy(&b, &f, 4); return b; }
Object g_objs[64];
Drawable g_sel[8];
Object g_selObj[8];
Drawable g_icons[8];
}

/* prog: [localPlayer, inputEnabled, idleWinFound, shroudLo, shroudHi, vanishRateBits, upSpeedBits, color1, color2, maxLineBuild, iconMask, canMask, nOps, then nOps records of 16 words:
** kind, logicFrame, clientFrame, a0..a12]. See the test for the ops. */
extern "C" int ref_lc3_igu_run(const int *p, int np, int *out)
{
	int k = 0, n = 0;
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i) g_players[i].index = i;
	for (int i = 0; i < 64; ++i) { g_objs[i].id = i; g_objs[i].player = 0; g_objs[i].kinds = 0; }
	g_ev.clear(); g_nextDs = 1;
	InGameUI ui;
	g_local = rdi(p, k);
	ui.m_inputEnabled = rdi(p, k) != 0;
	g_idleWinFound = rdi(p, k);
	unsigned lo = (unsigned)rdi(p, k), hi = (unsigned)rdi(p, k);
	g_shroud = ((unsigned long long)hi << 32) | lo;
	ui.m_floatingTextMoveVanishRate = fl(rdi(p, k));
	ui.m_floatingTextMoveUpSpeed = fl(rdi(p, k));
	ui.m_messageColor1 = rdi(p, k);
	ui.m_messageColor2 = rdi(p, k);
	g_gd.m_maxLineBuildObjects = rdi(p, k);
	int iconMask = rdi(p, k);
	g_canMask = (unsigned)rdi(p, k);
	Drawable *icons[8];
	for (int i = 0; i < 8; ++i) { g_icons[i].id = 100 + i; g_icons[i].obj = NULL; g_icons[i].kinds = 0; icons[i] = (iconMask >> i) & 1 ? &g_icons[i] : NULL; }
	ui.m_placeIcon = icons;
	ui.m_idleWorkerWin = NULL;
	int nOps = rdi(p, k);
	for (int o = 0; o < nOps; ++o) {
		int kind = rdi(p, k);
		g_logicFrame = (unsigned)rdi(p, k);
		g_clientFrame = (unsigned)rdi(p, k);
		int a[13];
		for (int i = 0; i < 13; ++i) a[i] = rdi(p, k);
		g_ev.clear();
		int ans = 0;
		int lenpos = n++;
		switch (kind) {
		case 0: {
			UnicodeString t; for (int i = 0; i < a[0]; ++i) t.v.push_back((unsigned short)a[1 + i]);
			RGBColor c; c.red = (((a[8] >> 16) & 255) + 0.5f) / 255.0f; c.green = (((a[8] >> 8) & 255) + 0.5f) / 255.0f; c.blue = ((a[8] & 255) + 0.5f) / 255.0f;
			ui.addMessageText(t, a[7] ? &c : NULL);
			break;
		}
		case 1: ui.removeMessageAtIndex(a[0]); break;
		case 2: { GameMessage m; m.a.pixelRegion.lo.x = a[0]; m.a.pixelRegion.lo.y = a[1]; m.a.pixelRegion.hi.x = a[2]; m.a.pixelRegion.hi.y = a[3]; ui.beginAreaSelectHint(&m); break; }
		case 3: { GameMessage m; m.a.objectID = (ObjectID)a[0]; m.a.location.x = fl(a[1]); m.a.location.y = fl(a[2]); m.a.location.z = fl(a[3]); ui.createMoveHint(&m); break; }
		case 4: ui.expireHint(MOVE_HINT, (UnsignedInt)a[0]); break;
		case 5: {
			FloatingTextData *f = new FloatingTextData;
			f->m_color = a[0]; f->m_pos3D.x = fl(a[1]); f->m_pos3D.y = fl(a[2]); f->m_pos3D.z = fl(a[3]); f->m_frameTimeOut = (UnsignedInt)a[4]; f->m_frameCount = (UnsignedInt)a[5];
			if (a[6]) { f->m_dString = new DisplayString; f->m_dString->id = a[6]; } else f->m_dString = NULL;
			ui.m_floatingTextList.push_front(f);
			break;
		}
		case 6: ui.updateFloatingText(); break;
		case 7: ui.drawFloatingText(); break;
		case 8: ui.clearFloatingText(); break;
		case 9: { Object *ob = a[0] ? &g_objs[a[0]] : NULL; if (ob) ob->player = a[1]; ui.addIdleWorker(ob); break; }
		case 10: { Object *ob = a[0] ? &g_objs[a[0]] : NULL; ui.removeIdleWorker(ob, a[1]); break; }
		case 11: { Object *ob = a[0] ? &g_objs[a[0]] : NULL; if (ob) ob->player = a[1]; Object *r = ui.findIdleWorker(ob); ans = r ? (int)r->id : -1; break; }
		case 12: ans = ui.getIdleWorkerCount(); break;
		case 13: ui.showIdleWorkerLayout(); break;
		case 14: ui.hideIdleWorkerLayout(); break;
		case 15: ui.updateIdleWorker(); break;
		case 16: ui.resetIdleWorker(); break;
		case 17: {
			SuperweaponInfo *s = new SuperweaponInfo;
			s->m_id = (ObjectID)a[2]; s->m_hiddenByScript = a[3] != 0; s->m_color = a[4];
			s->m_nameDisplayString = new DisplayString; s->m_nameDisplayString->id = a[5];
			s->m_timeDisplayString = new DisplayString; s->m_timeDisplayString->id = a[6];
			char nm[4] = { 'n', (char)('0' + a[1]), 0, 0 };
			ui.m_superweapons[a[0]][AsciiString(nm)].push_back(s);
			break;
		}
		case 18: ui.showObjectSuperweaponDisplayByScript(&g_objs[a[0]]); break;
		case 19: ui.setSuperweaponDisplayEnabledByScript(a[0] == 0); break;
		case 20: ans = ui.getSuperweaponDisplayEnabledByScript(); break;
		case 21: ui.destroyPlacementIcons(); break;
		case 22: {
			ui.m_selectedDrawables.clear();
			for (int i = 0; i < a[0]; ++i) {
				g_sel[i].id = (DrawableID)a[1 + 3 * i]; g_sel[i].kinds = (unsigned)a[2 + 3 * i];
				int f = a[3 + 3 * i];
				g_selObj[i].id = 200 + i; g_selObj[i].kinds = (f & 2) ? (1u << KINDOF_IMMOBILE) : 0;
				g_sel[i].obj = (f & 1) ? &g_selObj[i] : NULL;
				ui.m_selectedDrawables.push_back(&g_sel[i]);
			}
			break;
		}
		case 23: ans = ui.isDrawableSelected((DrawableID)a[0]); break;
		case 24: ans = ui.isAllSelectedKindOf((KindOfType)a[0]); break;
		case 25: ans = ui.canSelectedObjectsNonAttackInteractWithObject(NULL, InGameUI::SELECTION_ANY); break;
		case 26: case 27: {
			char nm[4] = { 'n', (char)('0' + a[1]), 0, 0 };
			SuperweaponMap::iterator mi = ui.m_superweapons[a[0]].find(AsciiString(nm));
			if (mi != ui.m_superweapons[a[0]].end() && !mi->second.empty()) { SuperweaponInfo *si = mi->second.front(); if (kind == 26) si->drawName(a[2], a[3], a[4], a[5]); else si->drawTime(a[2], a[3], a[4], a[5]); }
			break;
		}
		case 28: ui.m_inputEnabled = a[0] != 0; break;
		case 29: ui.m_nextMoveHint = a[0]; break;
		}
		out[n++] = (int)g_ev.size();
		for (size_t i = 0; i < g_ev.size(); ++i) out[n++] = g_ev[i];
		out[n++] = ans;
		for (int i = 0; i < 6; ++i) {
			InGameUI::UIMessage &m = ui.m_uiMessages[i];
			out[n++] = (int)m.fullText.v.size(); out[n++] = sumOf(m.fullText.v); out[n++] = m.displayString ? m.displayString->id : 0; out[n++] = (int)m.timestamp; out[n++] = m.color;
		}
		out[n++] = ui.m_isDragSelecting; out[n++] = ui.m_dragSelectRegion.lo.x; out[n++] = ui.m_dragSelectRegion.lo.y; out[n++] = ui.m_dragSelectRegion.hi.x; out[n++] = ui.m_dragSelectRegion.hi.y;
		int cnt = 0; for (int i = 0; i < 256; ++i) if (ui.m_moveHint[i].frame || ui.m_moveHint[i].sourceID) ++cnt;
		out[n++] = cnt;
		for (int i = 0; i < 256; ++i) if (ui.m_moveHint[i].frame || ui.m_moveHint[i].sourceID) { out[n++] = i; out[n++] = bi(ui.m_moveHint[i].pos.x); out[n++] = bi(ui.m_moveHint[i].pos.y); out[n++] = bi(ui.m_moveHint[i].pos.z); out[n++] = (int)ui.m_moveHint[i].frame; out[n++] = (int)ui.m_moveHint[i].sourceID; }
		out[n++] = ui.m_nextMoveHint;
		out[n++] = (int)ui.m_floatingTextList.size();
		for (FloatingTextListIt it = ui.m_floatingTextList.begin(); it != ui.m_floatingTextList.end(); ++it) { FloatingTextData *f = *it; out[n++] = f->m_color; out[n++] = bi(f->m_pos3D.x); out[n++] = bi(f->m_pos3D.y); out[n++] = bi(f->m_pos3D.z); out[n++] = (int)f->m_frameTimeOut; out[n++] = (int)f->m_frameCount; out[n++] = f->m_dString ? f->m_dString->id : 0; }
		for (int pl = 0; pl < MAX_PLAYER_COUNT; ++pl) { out[n++] = (int)ui.m_idleWorkers[pl].size(); for (ObjectListIt it = ui.m_idleWorkers[pl].begin(); it != ui.m_idleWorkers[pl].end(); ++it) out[n++] = (int)(*it)->id; }
		out[n++] = ui.m_idleWorkerWin != NULL; out[n++] = ui.m_currentIdleWorkerDisplay; out[n++] = ui.m_inputEnabled; out[n++] = ui.m_superweaponHiddenByScript;
		for (int pl = 0; pl < MAX_PLAYER_COUNT; ++pl) for (int nm = 0; nm < 3; ++nm) {
			char s[4] = { 'n', (char)('0' + nm), 0, 0 };
			SuperweaponMap::iterator mi = ui.m_superweapons[pl].find(AsciiString(s));
			if (mi == ui.m_superweapons[pl].end()) { out[n++] = -1; continue; }
			out[n++] = (int)mi->second.size();
			for (SuperweaponList::iterator li = mi->second.begin(); li != mi->second.end(); ++li) { out[n++] = (int)(*li)->m_id; out[n++] = (*li)->m_hiddenByScript; }
		}
		for (int i = 0; i < 8; ++i) out[n++] = icons[i] ? (int)icons[i]->id : 0;
		out[lenpos] = n - lenpos - 1;
	}
	return n;
}
