/* Differential-test wrapper for the cursors the in-game UI asks for (GitHub #122, upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/cursor_hint_gen:
**   ch_msg_type.inc            GameEngine/Include/Common/MessageStream.h, GameMessage's `enum Type` (the release build's);
**   ch_command_options.inc     GameEngine/Include/GameClient/ControlBar.h, `enum CommandOption` to
**                              `#ifdef DEFINE_GUI_COMMMAND_NAMES` (COMMAND_OPTION_NEED_TARGET, `enum GUICommandType`);
**   ch_mouse_enums.inc         GameEngine/Include/GameClient/Mouse.h, `enum MouseCursor` and `enum RedrawMode`;
**   ch_cursor_index.inc        GameEngine/Source/GameClient/Input/Mouse.cpp, `Mouse::getCursorIndex`;
**   ch_set_mouse_cursor.inc    GameEngine/Source/GameClient/InGameUI.cpp, `InGameUI::setMouseCursor`;
**   ch_guard_hint.inc          InGameUI.cpp, `triggerDoubleClickAttackMoveGuardHint`;
**   ch_hints.inc               InGameUI.cpp, `createMouseoverHint` and `createCommandHint`;
**   ch_set_gui_command.inc     InGameUI.cpp, `setGUICommand`;
**   ch_place_build.inc         InGameUI.cpp, `placeBuildAvailable`;
**   ch_can_select.inc          GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp, `CanSelectDrawable`;
**   ch_selection_mouseover.inc SelectionXlat.cpp, the `MSG_MOUSEOVER_DRAWABLE_HINT` case of `translateGameMessage`
**                              (compiled inside a switch of the wrapper's, `selectionMouseover`).
** No edit. They are compiled inside namespace ch on stand-in classes holding what the cut functions read, answered from
** the case's setup words: the drawables (1..n; drawable k's object, when it has one, is object k, at x = k), their
** objects' kinds, death, hiding, statuses, shroud, containers, stealth disguise, mob slaver and warehouse; four players
** (the local one is 0), four templates, four command buttons, the window chain under the mouse (and one opaque window
** over each obscured drawable: `worldToScreen` puts drawable k at x = 1000 + k), the radar, the recorder and the global
** data. The game's text is a stand-in (`OBJECT:Prop` -> "PROP", `TOOLTIP:SupplyWarehouse` -> " ($%d)", any other label
** itself), and so is UnicodeString (in namespace ch: `format` reads `%s` as a 16-bit string and `%d` as an Int, as
** MSVC's `_vsnwprintf` reads them). The calls on TheMouse and the radius cursor are recorded in order.
**
** Entry point (extern "C", tests/differential_cursor_hint.rs):
**   ref_cursor_hint_seq(in, nin, out, cap): one case; writes its records; returns the count.
**   Setup words (SETUP = 416): globals 0..16 [n drawables, multiplayer, base value per box, double-click attack move,
**   radar forced, radar hidden, playback, mouse moved recently, night, ...]; players 16 + 8p [allied to the local default
**   team, active, colour, night colour, playable side, has radar]; templates 48 + 8t [display name 0 empty | 1 "Name<t>" |
**   2 "PROP", placement view angle bits, kinds mask (of the list in kindBit)]; drawables 80 + 32(k-1) [has object, 0, dead, hidden, obscured,
**   unselectable or masked, selectable, locally controlled, shroud status, cell clear, has contain, apparent player (-1
**   none), garrisonable, controlling player (-1 none), has stealth, disguised, disguised player, disguised template (-1
**   none), template, indicator colour, has mob module, slaver id, has drawable (for getDrawable), has warehouse, boxes];
**   buttons 336 + 16b [command type, options, cursor name, invalid cursor name (indices into kNames), radius cursor,
**   special power id (-1 none), weapon slot]; windows 400 [count, then (left HUD, see-through, radar) per window].
**   Steps of 8 words from SETUP:
**     0 createMouseoverHint(MSG_MOUSEOVER_DRAWABLE_HINT of drawable a)   1 createMouseoverHint(MSG_MOUSEOVER_LOCATION_HINT)
**     2 createCommandHint(message of type a)                            3 setGUICommand(button a, -1 NULL)
**     4 placeBuildAvailable(template a (-1 NULL), drawable b (0 NULL))   5 m_isScrolling = a, m_isSelecting = b
**     6 the selection: a drawables b, c, d (front first)                7 triggerDoubleClickAttackMoveGuardHint()
**     8 double-click attack move a, playback b, mouse moved c, radar forced d, radar hidden e
**     9 the windows: a of them, each (b..) left HUD | see-through << 1 | radar << 2
**     10 SelectionTranslator's MSG_MOUSEOVER_DRAWABLE_HINT of drawable a, evaluateContextCommand answering b
**     11 setMouseCursor(a)                                              12 m_placeAnchorInProgress = a
**   Records of 8 words, as the calls happen: [300, cursor] setCursor; [301, text hash, colour, has colour]
**   setCursorTooltip; [302] resetTooltipDelay; [303] capture; [304] releaseCapture; [305, type, special power (-1),
**   weapon slot] setRadiusCursor; [306, disposition] the selection case's. After each step: [200, mouse mode, mouse mode
**   cursor, moused-over drawable, guard hint timer, pending place type (-1), pending place source, prevent flag],
**   [201, placement anchored, pending GUI command (-1)]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <wchar.h>
#include <list>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

namespace ch {

std::vector<int> g_out;
void rec(Int tag, Int a = 0, Int b = 0, Int c = 0, Int d = 0, Int e = 0, Int f = 0, Int g = 0)
{
	Int w[8] = { tag, a, b, c, d, e, f, g };
	g_out.insert(g_out.end(), w, w + 8);
}

/* ---- UnicodeString (see the header) ---- */
class UnicodeString
{
public:
	static const UnicodeString TheEmptyString;
	std::basic_string<WideChar> m_s;
	UnicodeString() {}
	UnicodeString(const WideChar *s) { if (s) m_s = s; }
	const WideChar *str() const { return m_s.c_str(); }
	Bool isEmpty() const { return m_s.empty(); }
	void concat(const UnicodeString &o) { m_s += o.m_s; }
	int compare(const UnicodeString &o) const { return m_s == o.m_s ? 0 : (m_s < o.m_s ? -1 : 1); }
	template <class C> void vfmt(const C *f, va_list ap)
	{
		m_s.clear();
		for (; *f; ++f) {
			if (*f != '%') { m_s += (WideChar)*f; continue; }
			++f;
			if (*f == 's') { const WideChar *s = va_arg(ap, const WideChar *); if (s) m_s += s; }
			else if (*f == 'd') { char b[16]; snprintf(b, sizeof b, "%d", va_arg(ap, int)); for (char *c = b; *c; ++c) m_s += (WideChar)*c; }
			else if (*f) m_s += (WideChar)*f;
			else break;
		}
	}
	void format(const wchar_t *f, ...) { va_list ap; va_start(ap, f); vfmt(f, ap); va_end(ap); }
	void format(UnicodeString f, ...) { std::basic_string<WideChar> s = f.m_s; va_list ap; va_start(ap, f); vfmt(s.c_str(), ap); va_end(ap); }
};
const UnicodeString UnicodeString::TheEmptyString;
UnicodeString widen(const char *s)
{
	UnicodeString u;
	for (; *s; ++s) u.m_s += (WideChar)(unsigned char)*s;
	return u;
}
Int hashOf(const UnicodeString &s)
{
	UnsignedInt h = 2166136261u;
	for (size_t i = 0; i < s.m_s.size(); ++i) { h ^= (UnsignedInt)s.m_s[i]; h *= 16777619u; }
	return (Int)h;
}

/* ---- the setup's words ---- */
enum { SETUP = 416, G = 0, P = 16, T = 48, D = 80, B = 336, W = 400, MAXD = 8 };
const int *g_in;
Int gi(Int at) { return g_in[at]; }
Int dw(Int k, Int f) { return g_in[D + 32 * (k - 1) + f]; }
Int g_dcam, g_playback, g_moved, g_radarForced, g_radarHidden;

/* ---- the game's types ---- */
typedef Int DrawableID;
typedef Int ObjectID;
typedef Int NameKeyType;
enum { INVALID_DRAWABLE_ID = 0, INVALID_ID = 0 };
enum KindOfType { KINDOF_SELECTABLE = 1, KINDOF_SHRUBBERY = 6, KINDOF_STRUCTURE = 7, KINDOF_IGNORED_IN_GUI = 43, KINDOF_MINE = 50, KINDOF_ALWAYS_SELECTABLE = 53, KINDOF_FORCEATTACKABLE = 63, KINDOF_DISGUISER = 83 };
Int kindBit(KindOfType k)
{
	switch (k) {
	case KINDOF_SELECTABLE: return 1; case KINDOF_SHRUBBERY: return 2; case KINDOF_STRUCTURE: return 4; case KINDOF_IGNORED_IN_GUI: return 8;
	case KINDOF_MINE: return 16; case KINDOF_ALWAYS_SELECTABLE: return 32; case KINDOF_FORCEATTACKABLE: return 64; case KINDOF_DISGUISER: return 128;
	}
	return 0;
}
enum ObjectShroudStatus { OBJECTSHROUD_INVALID = 0, OBJECTSHROUD_CLEAR, OBJECTSHROUD_PARTIAL_CLEAR, OBJECTSHROUD_FOGGED, OBJECTSHROUD_SHROUDED };
enum CellShroudStatus { CELLSHROUD_CLEAR, CELLSHROUD_FOGGED, CELLSHROUD_SHROUDED };
enum Relationship { ENEMIES = 0, NEUTRAL, ALLIES };
enum RadiusCursorType { RADIUSCURSOR_NONE = 0, RADIUSCURSOR_GUARD_AREA = 4 }; /* InGameUI.h's values */
enum WeaponSlotType { PRIMARY_WEAPON = 0 };
enum { OBJECT_STATUS_UNSELECTABLE = 1, OBJECT_STATUS_MASKED = 2 };
#define MAKE_OBJECT_STATUS_MASK2(a, b) ((a) | (b))
enum TimeOfDay { TIME_OF_DAY_AFTERNOON = 2, TIME_OF_DAY_NIGHT = 4 };
enum { DRAWABLE_STATUS_NO_STATE_PARTICLES = 1 };
enum RecorderModeType { RECORDERMODETYPE_RECORD, RECORDERMODETYPE_PLAYBACK };
enum GameMessageDisposition { KEEP_MESSAGE, DESTROY_MESSAGE };
struct RGBColor { Int v; void setFromInt(Int c) { v = c; } };

struct Team { Int owner; };
struct Player
{
	Int idx;
	Team team;
	Int getPlayerIndex() const { return idx; }
	Team *getDefaultTeam() { return &team; }
	Relationship getRelationship(const Team *) const { return g_in[P + 8 * idx] ? ALLIES : ENEMIES; }
	Bool isPlayerActive() const { return g_in[P + 8 * idx + 1] != 0; }
	Int getPlayerColor() const { return g_in[P + 8 * idx + 2]; }
	Int getPlayerNightColor() const { return g_in[P + 8 * idx + 3]; }
	Bool isPlayableSide() const { return g_in[P + 8 * idx + 4] != 0; }
	UnicodeString getPlayerDisplayName() { char b[8]; snprintf(b, sizeof b, "P%d", idx); return widen(b); }
	Bool hasRadar() const { return g_in[P + 8 * idx + 5] != 0; }
};
Player g_players[4];
struct PlayerListStub
{
	Player *getLocalPlayer() { return &g_players[0]; }
	Player *getNthPlayer(Int i) { return i >= 0 && i < 4 ? &g_players[i] : NULL; }
};
PlayerListStub g_playerList;
PlayerListStub *ThePlayerList = &g_playerList;

struct ThingTemplate
{
	Int idx;
	UnicodeString getDisplayName() const
	{
		Int k = g_in[T + 8 * idx];
		if (k == 0) return UnicodeString();
		if (k == 2) return widen("PROP");
		char b[16]; snprintf(b, sizeof b, "Name%d", idx); return widen(b);
	}
	AsciiString getName() const { char b[16]; snprintf(b, sizeof b, "Tmpl%d", idx); return AsciiString(b); }
	Real getPlacementViewAngle() const { Real r; memcpy(&r, &g_in[T + 8 * idx + 1], 4); return r; }
	Bool isKindOf(KindOfType t) const { return (g_in[T + 8 * idx + 2] & kindBit(t)) != 0; }
};
ThingTemplate g_templates[4];

struct SpecialPowerTemplate { Int id; };
SpecialPowerTemplate g_powers[64];

struct ContainModuleInterface
{
	Int k;
	const Player *getApparentControllingPlayer(const Player *) const { Int p = dw(k, 11); return p >= 0 ? &g_players[p] : NULL; }
	Bool isGarrisonable() const { return dw(k, 12) != 0; }
};
struct StealthUpdate
{
	Int k;
	Bool isDisguised() const { return dw(k, 15) != 0; }
	Int getDisguisedPlayerIndex() const { return dw(k, 16); }
	const ThingTemplate *getDisguisedTemplate() const { Int t = dw(k, 17); return t >= 0 ? &g_templates[t] : NULL; }
};
struct MobMemberSlavedUpdate { Int k; ObjectID getSlaverID() const { return dw(k, 21); } };
struct SupplyWarehouseDockUpdate { Int k; Int getBoxesStored() const { return dw(k, 24); } };
struct ProductionUpdateInterface { void setSpecialPowerConstructionCommandButton(const void *) {} };
struct StatusBits { Int v; Bool testForAny(Int m) const { return (v & m) != 0; } };

struct NameKeyGeneratorStub { NameKeyType nameToKey(const char *s) { return strcmp(s, "MobMemberSlavedUpdate") == 0 ? 1 : strcmp(s, "SupplyWarehouseDockUpdate") == 0 ? 2 : 3; } };
NameKeyGeneratorStub g_nameKeys;
NameKeyGeneratorStub *TheNameKeyGenerator = &g_nameKeys;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class Drawable;
class Object
{
public:
	Int k;
	mutable ContainModuleInterface m_contain;
	mutable StealthUpdate m_stealth;
	mutable MobMemberSlavedUpdate m_mob;
	mutable SupplyWarehouseDockUpdate m_warehouse;
	Coord3D m_pos;
	ObjectID getID() const { return k; }
	Bool isKindOf(KindOfType t) const { return getTemplate()->isKindOf(t); }
	Bool isEffectivelyDead() const { return dw(k, 2) != 0; }
	StatusBits getStatusBits() const { StatusBits s; s.v = dw(k, 5) ? (OBJECT_STATUS_UNSELECTABLE | OBJECT_STATUS_MASKED) : 0; return s; }
	Bool isSelectable() const { return dw(k, 6) != 0; }
	Bool isLocallyControlled() const { return dw(k, 7) != 0; }
	ObjectShroudStatus getShroudedStatus(Int) const { return (ObjectShroudStatus)dw(k, 8); }
	ContainModuleInterface *getContain() const { return dw(k, 10) ? &m_contain : NULL; }
	Player *getControllingPlayer() const { Int p = dw(k, 13); return p >= 0 ? &g_players[p] : NULL; }
	StealthUpdate *getStealth() const { return dw(k, 14) ? &m_stealth : NULL; }
	const ThingTemplate *getTemplate() const { return &g_templates[dw(k, 18)]; }
	Int getIndicatorColor() const { return dw(k, 19); }
	void *findUpdateModule(NameKeyType key) const
	{
		if (key == 1) return dw(k, 20) ? (void *)&m_mob : NULL;
		if (key == 2) return dw(k, 23) ? (void *)&m_warehouse : NULL;
		return NULL;
	}
	const Coord3D *getPosition() const { return &m_pos; }
	Drawable *getDrawable() const;
	ProductionUpdateInterface *getProductionUpdateInterface() const { return NULL; }
};
Object g_objects[MAXD + 1];

class Drawable
{
public:
	Int k;
	Coord3D m_pos;
	Object *getObject() const { return dw(k, 0) ? &g_objects[k] : NULL; }
	DrawableID getID() const { return k; }
	const ThingTemplate *getTemplate() const { return &g_templates[dw(k, 18)]; }
	Bool isKindOf(KindOfType t) const { return getTemplate()->isKindOf(t); }
	Bool isDrawableEffectivelyHidden() const { return dw(k, 3) != 0; }
	const Coord3D *getPosition() const { return &m_pos; }
	void setIndicatorColor(Int) {}
	void setOrientation(Real) {}
	void setDrawableOpacity(Real) {}
};
Drawable g_drawables[MAXD + 1];
Drawable g_icon;
Drawable *Object::getDrawable() const { return dw(k, 22) ? &g_drawables[k] : NULL; }
typedef std::list<Drawable *> DrawableList;

struct GameMessageArgument { DrawableID drawableID; Coord3D location; };
class GameMessage
{
public:
#include "ch_msg_type.inc"
	GameMessage(Type t) : m_type(t) { m_arg.drawableID = 0; m_arg.location.zero(); }
	Type getType() const { return m_type; }
	const GameMessageArgument *getArgument(Int) const { return &m_arg; }
	Type m_type;
	GameMessageArgument m_arg;
};

#include "ch_command_options.inc"

class CommandButton
{
public:
	Int b;
	AsciiString m_cursorName, m_invalidCursorName;
	Bool isContextCommand() const { return BitTest(getOptions(), CONTEXTMODE_COMMAND); }
	GUICommandType getCommandType() const { return (GUICommandType)g_in[B + 16 * b]; }
	UnsignedInt getOptions() const { return (UnsignedInt)g_in[B + 16 * b + 1]; }
	const AsciiString &getCursorName() const { return m_cursorName; }
	const AsciiString &getInvalidCursorName() const { return m_invalidCursorName; }
	RadiusCursorType getRadiusCursorType() const { return (RadiusCursorType)g_in[B + 16 * b + 4]; }
	const SpecialPowerTemplate *getSpecialPowerTemplate() const { Int s = g_in[B + 16 * b + 5]; return s >= 0 ? &g_powers[s] : NULL; }
	WeaponSlotType getWeaponSlot() const { return (WeaponSlotType)g_in[B + 16 * b + 6]; }
	AsciiString getName() const { return AsciiString("Button"); }
};
CommandButton g_buttons[4];
const char *kNames[] = { "", "Target", "attackobj", "Nope", "Arrow", "ParticleUplinkCannon", "LaserGuidedMissiles", "WAYPOINT" };

/* ---- the windows ---- */
class GameWindow;
typedef Int (*GameWinInputFunc)(GameWindow *, UnsignedInt, Int, Int);
Int LeftHUDInput(GameWindow *, UnsignedInt, Int, Int) { return 0; }
Int OtherInput(GameWindow *, UnsignedInt, Int, Int) { return 1; }
enum { WIN_STATUS_SEE_THRU = 0x40 };
class GameWindow
{
public:
	GameWinInputFunc m_input;
	UnsignedInt m_status;
	GameWindow *m_parent;
	GameWinInputFunc winGetInputFunc() { return m_input; }
	UnsignedInt winGetStatus() { return m_status; }
	GameWindow *winGetParent() { return m_parent; }
};
GameWindow g_wins[3];
GameWindow g_drawWin;
GameWindow *g_firstWin;
struct WindowManagerStub
{
	GameWindow *getWindowUnderCursor(Int x, Int)
	{
		if (x >= 1000)
			return dw(x - 1000, 4) ? &g_drawWin : NULL;
		return g_firstWin;
	}
};
WindowManagerStub g_wm;
WindowManagerStub *TheWindowManager = &g_wm;
struct RadarStub
{
	GameWindow *m_radarWindow;
	Bool isRadarWindow(GameWindow *window) { return (m_radarWindow == window) && (m_radarWindow != NULL); }
	Bool isRadarForced() { return g_radarForced != 0; }
	Bool isRadarHidden() { return g_radarHidden != 0; }
};
RadarStub g_radar;
RadarStub *TheRadar = &g_radar;

/* ---- the rest of the game ---- */
struct GameTextStub
{
	UnicodeString fetch(const char *label)
	{
		if (strcmp(label, "OBJECT:Prop") == 0) return widen("PROP");
		if (strcmp(label, "TOOLTIP:SupplyWarehouse") == 0) return widen(" ($%d)");
		return widen(label);
	}
	UnicodeString fetch(const AsciiString &label) { return fetch(label.str()); }
};
GameTextStub g_text;
GameTextStub *TheGameText = &g_text;
struct RecorderStub
{
	Bool isMultiplayer() { return g_in[G + 1] != 0; }
	RecorderModeType getMode() { return g_playback ? RECORDERMODETYPE_PLAYBACK : RECORDERMODETYPE_RECORD; }
};
RecorderStub g_recorder;
RecorderStub *TheRecorder = &g_recorder;
struct PartitionStub
{
	void worldToCell(Real x, Real, Int *cx, Int *cy) { *cx = (Int)x; *cy = 0; }
	CellShroudStatus getShroudStatusForPlayer(Int, Int x, Int) { return dw(x, 9) ? CELLSHROUD_CLEAR : CELLSHROUD_FOGGED; }
};
PartitionStub g_partition;
PartitionStub *ThePartitionManager = &g_partition;
struct GlobalDataStub { Bool m_doubleClickAttackMove; Int m_baseValuePerSupplyBox; TimeOfDay m_timeOfDay; };
GlobalDataStub g_global;
GlobalDataStub *TheGlobalData = &g_global;
struct LookAtStub { Bool hasMouseMovedRecently() { return g_moved != 0; } };
LookAtStub g_lookAt;
LookAtStub *TheLookAtTranslator = &g_lookAt;
struct ViewStub
{
	void worldToScreen(const Coord3D *c, ICoord2D *s) { s->x = 1000 + (Int)c->x; s->y = 0; }
	void screenToTerrain(const ICoord2D *, Coord3D *p) { p->zero(); }
};
ViewStub g_view;
ViewStub *TheTacticalView = &g_view;
struct ThingFactoryStub { Drawable *newDrawable(const ThingTemplate *, Int) { return &g_icon; } };
ThingFactoryStub g_thingFactory;
ThingFactoryStub *TheThingFactory = &g_thingFactory;
struct CommandTranslator { enum CommandEvaluateType { DO_COMMAND, DO_HINT, EVALUATE_ONLY }; };
Int g_evalResult;
struct GameClientStub
{
	Drawable *findDrawableByID(DrawableID id) { return id >= 1 && id <= gi(G) ? &g_drawables[id] : NULL; }
	GameMessage::Type evaluateContextCommand(Drawable *, const Coord3D *, CommandTranslator::CommandEvaluateType) { return (GameMessage::Type)g_evalResult; }
};
GameClientStub g_gameClient;
GameClientStub *TheGameClient = &g_gameClient;
struct GameLogicStub { Object *findObjectByID(ObjectID id) { return id >= 1 && id <= gi(G) && dw(id, 0) ? &g_objects[id] : NULL; } };
GameLogicStub g_gameLogic;
GameLogicStub *TheGameLogic = &g_gameLogic;

struct MouseIO { ICoord2D pos; };
class Mouse
{
public:
#include "ch_mouse_enums.inc"
	MouseIO m_io;
	MouseCursor m_currentCursor;
	const MouseIO *getMouseStatus() { return &m_io; }
	void setCursor(MouseCursor c) { rec(300, c); m_currentCursor = c; }
	MouseCursor getMouseCursor() { return m_currentCursor; }
	Int getCursorIndex(const AsciiString &name);
	void setCursorTooltip(UnicodeString tooltip, Int = -1, const RGBColor *color = NULL) { rec(301, hashOf(tooltip), color ? color->v : 0, color != NULL); }
	void resetTooltipDelay() { rec(302); }
	void capture() { rec(303); }
	void releaseCapture() { rec(304); }
};
#include "ch_cursor_index.inc"
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;

static const Real placementOpacity = 0.45f;

class InGameUI
{
public:
	enum MouseMode { MOUSEMODE_DEFAULT = 0, MOUSEMODE_BUILD_PLACE, MOUSEMODE_GUI_COMMAND, MOUSEMODE_MAX };
	void setMouseCursor(Mouse::MouseCursor c);
	void createMouseoverHint(const GameMessage *msg);
	void createCommandHint(const GameMessage *msg);
	void setGUICommand(const CommandButton *command);
	void placeBuildAvailable(const ThingTemplate *build, Drawable *buildDrawable);
	void triggerDoubleClickAttackMoveGuardHint(void);
	void setRadiusCursor(RadiusCursorType t, const SpecialPowerTemplate *sp, WeaponSlotType slot) { rec(305, t, sp ? sp->id : -1, slot); }
	void setRadiusCursorNone() { setRadiusCursor(RADIUSCURSOR_NONE, NULL, PRIMARY_WEAPON); }
	Int getSelectCount() { return (Int)m_selected.size(); }
	const DrawableList *getAllSelectedDrawables() { return &m_selected; }
	const CommandButton *getGUICommand() const { return m_pendingGUICommand; }
	Bool isScrolling() { return m_isScrolling; }
	void setPreventLeftClickDeselectionInAlternateMouseModeForOneClick(Bool set) { m_prevent = set; }
	void setPlacementStart(const ICoord2D *start) { m_placeAnchorInProgress = start != NULL; }
	void destroyPlacementIcons() { m_placeIcon[0] = NULL; }
	Bool m_isScrolling, m_isSelecting;
	DrawableID m_mousedOverDrawableID;
	MouseMode m_mouseMode;
	Int m_mouseModeCursor;
	const CommandButton *m_pendingGUICommand;
	Int m_duringDoubleClickAttackMoveGuardHintTimer;
	Coord3D m_duringDoubleClickAttackMoveGuardHintStashedPosition;
	const ThingTemplate *m_pendingPlaceType;
	ObjectID m_pendingPlaceSourceObjectID;
	Drawable *m_placeIcon[1];
	Bool m_prevent, m_placeAnchorInProgress;
	DrawableList m_selected;
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;
/* SelectionXlat.h's declaration (line 84) */
Bool CanSelectDrawable( const Drawable *draw, Bool dragSelecting );

#include "ch_set_mouse_cursor.inc"
#include "ch_guard_hint.inc"
#include "ch_hints.inc"
#include "ch_set_gui_command.inc"
#include "ch_place_build.inc"
#include "ch_can_select.inc"

GameMessageDisposition selectionMouseover(const GameMessage *msg)
{
	GameMessageDisposition disp = KEEP_MESSAGE;
	switch (msg->getType())
	{
#include "ch_selection_mouseover.inc"
		default:
			break;
	}
	return disp;
}

void state()
{
	rec(200, g_ui.m_mouseMode, g_ui.m_mouseModeCursor, g_ui.m_mousedOverDrawableID, g_ui.m_duringDoubleClickAttackMoveGuardHintTimer,
		g_ui.m_pendingPlaceType ? g_ui.m_pendingPlaceType->idx : -1, g_ui.m_pendingPlaceSourceObjectID, g_ui.m_prevent);
	rec(201, g_ui.m_placeAnchorInProgress, g_ui.m_pendingGUICommand ? g_ui.m_pendingGUICommand->b : -1);
}

}  // namespace ch

using namespace ch;

extern "C" int ref_cursor_hint_seq(const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	g_in = in;
	Int n = in[G];
	g_dcam = in[G + 3]; g_radarForced = in[G + 4]; g_radarHidden = in[G + 5]; g_playback = in[G + 6]; g_moved = in[G + 7];
	g_global.m_doubleClickAttackMove = g_dcam != 0;
	g_global.m_baseValuePerSupplyBox = in[G + 2];
	g_global.m_timeOfDay = in[G + 8] ? TIME_OF_DAY_NIGHT : TIME_OF_DAY_AFTERNOON;
	for (Int p = 0; p < 4; ++p) { g_players[p].idx = p; g_players[p].team.owner = p; }
	for (Int t = 0; t < 4; ++t) g_templates[t].idx = t;
	for (Int s = 0; s < 64; ++s) g_powers[s].id = s;
	for (Int k = 1; k <= MAXD; ++k) {
		g_drawables[k].k = k; g_drawables[k].m_pos.set((Real)k, 0, 0);
		Object &o = g_objects[k];
		o.k = k; o.m_pos.set((Real)k, 0, 0);
		o.m_contain.k = o.m_stealth.k = o.m_mob.k = o.m_warehouse.k = k;
	}
	for (Int b = 0; b < 4; ++b) {
		g_buttons[b].b = b;
		g_buttons[b].m_cursorName = AsciiString(kNames[in[B + 16 * b + 2]]);
		g_buttons[b].m_invalidCursorName = AsciiString(kNames[in[B + 16 * b + 3]]);
	}
	/* the windows under the mouse */
	struct Wins {
		static void set(const int *w, Int count)
		{
			g_radar.m_radarWindow = NULL;
			g_firstWin = count > 0 ? &g_wins[0] : NULL;
			for (Int i = 0; i < count; ++i) {
				Int f = w[i];
				g_wins[i].m_input = (f & 1) ? LeftHUDInput : OtherInput;
				g_wins[i].m_status = (f & 2) ? WIN_STATUS_SEE_THRU : 0;
				g_wins[i].m_parent = i + 1 < count ? &g_wins[i + 1] : NULL;
				if ((f & 4) && !g_radar.m_radarWindow) g_radar.m_radarWindow = &g_wins[i];
			}
		}
	};
	{
		int w[3];
		Int count = in[W];
		for (Int i = 0; i < count; ++i) w[i] = in[W + 1 + 3 * i] | (in[W + 2 + 3 * i] << 1) | (in[W + 3 + 3 * i] << 2);
		Wins::set(w, count);
	}
	g_drawWin.m_input = OtherInput; g_drawWin.m_status = 0; g_drawWin.m_parent = NULL;
	g_mouse.m_io.pos.x = g_mouse.m_io.pos.y = 0;
	g_mouse.m_currentCursor = Mouse::ARROW;
	/* InGameUI's constructor's members */
	g_ui.m_isScrolling = g_ui.m_isSelecting = FALSE;
	g_ui.m_mousedOverDrawableID = INVALID_DRAWABLE_ID;
	g_ui.m_mouseMode = InGameUI::MOUSEMODE_DEFAULT;
	g_ui.m_mouseModeCursor = Mouse::ARROW;
	g_ui.m_pendingGUICommand = NULL;
	g_ui.m_duringDoubleClickAttackMoveGuardHintTimer = 0;
	g_ui.m_pendingPlaceType = NULL;
	g_ui.m_pendingPlaceSourceObjectID = INVALID_ID;
	g_ui.m_placeIcon[0] = NULL;
	g_ui.m_prevent = g_ui.m_placeAnchorInProgress = FALSE;
	g_ui.m_selected.clear();
	state();
	for (int s = SETUP; s + 8 <= nin; s += 8) {
		const int *w = in + s;
		switch (w[0]) {
		case 0: { GameMessage m(GameMessage::MSG_MOUSEOVER_DRAWABLE_HINT); m.m_arg.drawableID = w[1]; TheInGameUI->createMouseoverHint(&m); break; }
		case 1: { GameMessage m(GameMessage::MSG_MOUSEOVER_LOCATION_HINT); TheInGameUI->createMouseoverHint(&m); break; }
		case 2: { GameMessage m((GameMessage::Type)w[1]); TheInGameUI->createCommandHint(&m); break; }
		case 3: TheInGameUI->setGUICommand(w[1] >= 0 ? &g_buttons[w[1]] : NULL); break;
		case 4: TheInGameUI->placeBuildAvailable(w[1] >= 0 ? &g_templates[w[1]] : NULL, TheGameClient->findDrawableByID(w[2])); break;
		case 5: g_ui.m_isScrolling = w[1] != 0; g_ui.m_isSelecting = w[2] != 0; break;
		case 6: {
			g_ui.m_selected.clear();
			for (Int i = 0; i < w[1]; ++i) g_ui.m_selected.push_back(&g_drawables[w[2 + i]]);
			break;
		}
		case 7: TheInGameUI->triggerDoubleClickAttackMoveGuardHint(); break;
		case 8:
			g_dcam = w[1]; g_global.m_doubleClickAttackMove = g_dcam != 0;
			g_playback = w[2]; g_moved = w[3]; g_radarForced = w[4]; g_radarHidden = w[5];
			break;
		case 9: Wins::set(w + 2, w[1]); break;
		case 10: {
			GameMessage m(GameMessage::MSG_MOUSEOVER_DRAWABLE_HINT);
			m.m_arg.drawableID = w[1];
			g_evalResult = w[2];
			rec(306, selectionMouseover(&m));
			break;
		}
		case 11: TheInGameUI->setMouseCursor((Mouse::MouseCursor)w[1]); break;
		case 12: g_ui.m_placeAnchorInProgress = w[1] != 0; break;
		}
		state();
	}
	(void)n;
	int cnt = (int)g_out.size();
	if (cnt > cap)
		cnt = cap;
	memcpy(out, &g_out[0], cnt * sizeof(int));
	return cnt;
}
