/* Differential-test wrapper for the command translator's meta-key cases and MetaEvent's key map (GitHub #125, upstream
** commit 0a05454d), cut verbatim by build.rs into OUT_DIR/command_meta_gen:
**   command_gate_open.inc  CommandXlat.cpp, CommandTranslator::translateGameMessage from its first line to the debug
**                          build's locals after the input gate (not closed: the wrapper adds its switch);
**   system_message.inc     CommandXlat.cpp, isSystemMessage (to the end of the file);
**   view_hero.inc          CommandXlat.cpp, `struct CommandCenterLocator` to issueMoveToLocationCommand (viewCommandCenter,
**                          iNeedAHero and their callbacks);
**   meta_select_cases.inc  CommandXlat.cpp, translateGameMessage's cases from MSG_META_SELECT_MATCHING_UNITS to the
**                          commented-out chat case (the selection walks, the hero, the views, select all, scatter, stop,
**                          formation, deploy, follow);
**   meta_beacon_cases.inc  the cases MSG_META_PLACE_BEACON and MSG_META_REMOVE_BEACON (to MSG_META_OPTIONS);
**   meta_mode_cases.inc    MSG_META_TOGGLE_ATTACKMOVE to MSG_META_TOGGLE_FAST_FORWARD_REPLAY (attack move, the camera
**                          flags, tracking);
**   meta_held_cases.inc    MSG_META_BEGIN_FORCEMOVE to MSG_META_TAKE_SCREENSHOT (the held modes, the cheer);
**   group_cases.inc        the selected-group cases (MSG_CREATE_SELECTED_GROUP ... MSG_DESTROY_SELECTED_GROUP);
**   kind_of_selection.inc  InGameUI.cpp, `struct KindOfSelectionData` to showReplayControls (kindOfUnitSelection,
**                          similarUnitSelection);
**   select_by_type.inc     InGameUI.cpp, selectAllUnitsByTypeAcrossRegion to addFloatingText (selectAllUnitsByType*,
**                          selectMatching*, selectUnitsMatchingCurrentSelection, buildRegion);
**   command_options.inc    ControlBar.h, `enum CommandOption` to `#ifdef DEFINE_GUI_COMMMAND_NAMES` (GUICommandType);
**   meta_header.inc        MetaEvent.h, `enum MappableKeyCategories` to `class MetaMapRec` (the key enums and names);
**   meta_names.inc         MetaEvent.cpp, GameMessageMetaTypeNames;
**   meta_translate.inc     MetaEvent.cpp, MetaEventTranslator::translateGameMessage;
**   meta_map.inc           MetaEvent.cpp, MetaMap::findGameMessageMetaType and getMetaMapRec;
**   meta_ctor.inc          (#131) MetaEvent.cpp, MetaEventTranslator::MetaEventTranslator (to the destructor).
** They are compiled inside namespace cmx on stand-ins for the classes and singletons they name; the messages are the real
** GameMessage of the message stream build (wrap_message_stream.cpp), under its names.
**
** Entry point (extern "C", tests/differential_command_meta.rs): ref_command_meta_case(kind, in, nin, out, cap).
**   kind 0: one meta message through translateGameMessage; each message it appends then goes through translateGameMessage
**     too (for m_teamExists). in: see META_* below (the header) and DRAW_* (8 drawables of 12 words), then 4 template costs.
**     out: disposition; -1; per appended message (type, count, per argument (data type, value; a location's 3 float
**     bits)); -2; per lookAt the 3 float bits; -3; the selection after, front first; -4; per message() (label code, count);
**     -5; per addAudioEvent (tag, object); -6; per setGUICommand the command's type (-2 for NULL); -7; attack move, force
**     attack, force move, waypoint, prefer selection, camera tracking, displayed max warning, select count; -8;
**     m_teamExists after.
**   kind 1: one key message through MetaEventTranslator::translateGameMessage. in: type, key, state, client frame, shell
**     active, replay, m_lastModState, m_lastKeyDown, m_TiVOFastMode, record count, then per record (meta, key,
**     transition, modifiers, usable in) made with getMetaMapRec. out: disposition; -1; the appended metas; -2;
**     m_lastModState, m_lastKeyDown, m_TiVOFastMode, messages; -3; the records' metas in list order.
**   kind 2: in: i. out: GameMessageMetaTypeNames[i]'s value and name bytes (none past the end).
**   kind 3 (#131): a sequence of raw mouse messages through one MetaEventTranslator (constructed by its constructor in
**     zeroed storage: the constructor leaves m_mouseDownPosition unset, which the port starts at 0). in: TheMouse's
**     m_dragTolerance, then steps of 4 words: type, x, y, the modifier keys (argument 1). out: per step the
**     disposition, then per message inserted (type, the pixel region's lo.x, lo.y, hi.x, hi.y, the integer argument),
**     -1, the three m_nextUpShouldCreateDoubleClick, the three m_mouseDownPosition (x, y), -2. */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheMessageStream MsTestTheMessageStream
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <list>
#include <set>
#include <vector>
#include <new>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/MessageStream.h"
#include "GameClient/KeyDefs.h"

static GameMessage *mkMsg(GameMessage::Type t) { return newInstance(GameMessage)(t); }

namespace cmx {

enum {
	META_TYPE = 0, META_INPUT, META_NDRAW, META_LOCAL, META_MAXSEL, META_MAXWARN, META_RADAR, META_RADAR_X, META_RADAR_Y,
	META_RADAR_Z, META_MP, META_REPLAY, META_ACTIVE, META_NETMIN, META_GAMEINFO_MP, META_BEACONS, META_MAXBEACONS,
	META_BEACON_BUTTON, META_ATTACKMOVE, META_FORCEATTACK, META_FORCEMOVE, META_WAYPOINT, META_PREFER, META_TRACKING,
	META_TEAM, META_NSEL, META_SEL0, META_NOBJ = META_SEL0 + 8, META_OBJ0, META_HEADER = 48,
	DRAW_WORDS = 12, DRAW_HASOBJ = 0, DRAW_KINDS, DRAW_MOBILE, DRAW_LOCAL, DRAW_CONTAINER, DRAW_DEAD, DRAW_MASS, DRAW_OFFMAP,
	DRAW_SCREEN, DRAW_TEMPLATE, DRAW_CARBOMB,
	META_COSTS = META_HEADER + 8 * DRAW_WORDS,
};

std::vector<int> g_out, g_msgs, g_looks, g_uiMsgs, g_audio, g_gui;
static Int realBits(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }

/* ---- the stand-ins' text and kinds ---- */
enum { LABEL_NONE, LABEL_NOTHING_SELECTED, LABEL_ACROSS_SCREEN, LABEL_ACROSS_MAP, LABEL_MAX_SELECTION, LABEL_FF_ON, LABEL_FF_OFF, LABEL_OTHER };
class UnicodeString
{
public:
	Int m_label, m_count;
	UnicodeString() : m_label(LABEL_NONE), m_count(0) {}
	Int str() const { return m_label; }
	void format(Int label, ...) { va_list a; va_start(a, label); m_label = label; m_count = va_arg(a, Int); va_end(a); }
	void clear() { m_label = LABEL_NONE; }
};
class GameText
{
public:
	UnicodeString fetch(const char *s)
	{
		UnicodeString u;
		u.m_label = !strcmp(s, "GUI:NothingSelected") ? LABEL_NOTHING_SELECTED : !strcmp(s, "GUI:SelectedAcrossScreen") ? LABEL_ACROSS_SCREEN
			: !strcmp(s, "GUI:SelectedAcrossMap") ? LABEL_ACROSS_MAP : !strcmp(s, "GUI:MaxSelectionSize") ? LABEL_MAX_SELECTION
			: !strcmp(s, "GUI:FF_ON") ? LABEL_FF_ON : !strcmp(s, "GUI:FF_OFF") ? LABEL_FF_OFF : LABEL_OTHER;
		return u;
	}
};
GameText g_text;
GameText *TheGameText = &g_text;

enum KindOfType
{
	KINDOF_STRUCTURE = 7, KINDOF_AIRCRAFT = 10, KINDOF_DOZER = 12, KINDOF_HARVESTER = 13, KINDOF_COMMANDCENTER = 14,
	KINDOF_HERO = 85, KINDOF_IGNORES_SELECT_ALL = 86, KINDOF_NO_SELECT = 108,
};
/* the 8 kinds a case's bits name, in order */
static const KindOfType s_kinds[8] = { KINDOF_STRUCTURE, KINDOF_AIRCRAFT, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_COMMANDCENTER, KINDOF_HERO, KINDOF_IGNORES_SELECT_ALL, KINDOF_NO_SELECT };
class KindOfMaskType
{
public:
	UnsignedInt m_w[4];
	KindOfMaskType() { memset(m_w, 0, sizeof m_w); }
	void set(KindOfType k) { m_w[k / 32] |= 1u << (k % 32); }
	Bool test(KindOfType k) const { return (m_w[k / 32] >> (k % 32)) & 1; }
	Bool any(const KindOfMaskType &o) const { for (Int i = 0; i < 4; ++i) if (m_w[i] & o.m_w[i]) return TRUE; return FALSE; }
	Bool all(const KindOfMaskType &o) const { for (Int i = 0; i < 4; ++i) if ((m_w[i] & o.m_w[i]) != o.m_w[i]) return FALSE; return TRUE; }
};
enum ObjectStatusTypes { OBJECT_STATUS_IS_CARBOMB = 1 };

/* ---- audio ---- */
class AudioEventRTS
{
public:
	Int m_tag;
	ObjectID m_object;
	AudioEventRTS() : m_tag(0), m_object((ObjectID)0) {}
	void setObjectID(ObjectID id) { m_object = id; }
};
struct MiscAudio { AudioEventRTS m_allCheerSound; };
class GameAudio
{
public:
	MiscAudio m_misc;
	MiscAudio *getMiscAudio() { return &m_misc; }
	void addAudioEvent(AudioEventRTS *e) { g_audio.push_back(e->m_tag); g_audio.push_back((Int)e->m_object); }
};
GameAudio g_audioSys;
GameAudio *TheAudio = &g_audioSys;

/* ---- templates, objects, drawables ---- */
class Player;
class ThingTemplate
{
public:
	Int m_index, m_cost;
	AudioEventRTS m_voiceSelect;
	Bool isEquivalentTo(const ThingTemplate *t) const { return this == t; }
	Int calcCostToBuild(const Player *) const { return m_cost; }
	const AudioEventRTS *getVoiceSelect() const { return &m_voiceSelect; }
};
ThingTemplate g_templates[4];
class Drawable;
class Object
{
public:
	ObjectID m_id;
	Bool m_mobile, m_local, m_dead, m_mass, m_offMap, m_carBomb;
	Object *m_container;
	KindOfMaskType m_kinds;
	ThingTemplate *m_tmpl;
	Drawable *m_draw;
	Coord3D m_pos;
	ObjectID getID() const { return m_id; }
	Bool isMobile() const { return m_mobile; }
	Bool isLocallyControlled() const { return m_local; }
	Bool isContained() const { return m_container != NULL; }
	Object *getContainedBy() const { return m_container; }
	Bool isKindOf(KindOfType k) const { return m_kinds.test(k); }
	Bool isKindOfMulti(const KindOfMaskType &set, const KindOfMaskType &clear) const { return m_kinds.all(set) && !m_kinds.any(clear); }
	Bool isAnyKindOf(const KindOfMaskType &m) const { return m_kinds.any(m); }
	Drawable *getDrawable() const { return m_draw; }
	Bool isEffectivelyDead() const { return m_dead; }
	Bool isMassSelectable() const { return m_mass; }
	Bool isOffMap() const { return m_offMap; }
	Bool testStatus(ObjectStatusTypes) const { return m_carBomb; }
	const ThingTemplate *getTemplate() const { return m_tmpl; }
	const Coord3D *getPosition() const { return &m_pos; }
	Player *getControllingPlayer() const { return NULL; }
};
class Drawable
{
public:
	Int m_id;
	Object *m_obj;
	Drawable *m_next, *m_prev;
	Bool m_selected;
	Coord3D m_pos;
	ThingTemplate *m_tmpl;
	Object *getObject() const { return m_obj; }
	Drawable *getNextDrawable() const { return m_next; }
	Drawable *getPrevDrawable() const { return m_prev; }
	Bool isSelected() const { return m_selected; }
	const Coord3D *getPosition() const { return &m_pos; }
	const ThingTemplate *getTemplate() const { return m_tmpl; }
	/* the drawable's template's kinds: the object's in the stand-in */
	Bool isKindOf(KindOfType k) const { return m_obj->isKindOf(k); }
	Bool isAnyKindOf(const KindOfMaskType &m) const { return m_obj->isAnyKindOf(m); }
};
typedef std::list<Drawable *> DrawableList;
typedef DrawableList::iterator DrawableListIt;
typedef DrawableList::const_iterator DrawableListCIt;
Drawable g_draws[9];
Object g_objs[9];
Int g_ndraw;

class GameClient
{
public:
	Drawable *firstDrawable() { return g_ndraw > 0 ? &g_draws[1] : NULL; }
	Drawable *getDrawableList() { return firstDrawable(); }
};
GameClient g_client;
GameClient *TheGameClient = &g_client;

/* ---- the tactical view ---- */
std::vector<Drawable *> g_onScreen;
class View
{
public:
	void lookAt(const Coord3D *p) { g_looks.push_back(realBits(p->x)); g_looks.push_back(realBits(p->y)); g_looks.push_back(realBits(p->z)); }
	void getOrigin(Int *x, Int *y) { *x = 0; *y = 0; }
	Int getWidth() { return 800; }
	Int getHeight() { return 600; }
	Int iterateDrawablesInRegion(IRegion2D *, Bool (*fn)(Drawable *, void *), void *data)
	{
		Int n = 0;
		for (size_t i = 0; i < g_onScreen.size(); ++i) n += fn(g_onScreen[i], data) ? 1 : 0;
		return n;
	}
};
View g_view;
View *TheTacticalView = &g_view;

class Radar
{
public:
	Bool m_has;
	Coord3D m_last;
	Bool getLastEventLoc(Coord3D *c) { if (m_has) *c = m_last; return m_has; }
};
Radar g_radar;
Radar *TheRadar = &g_radar;

/* ---- the players and the game ---- */
std::vector<Object *> g_playerObjs;
class PlayerTemplate { public: const char *getBeaconTemplate() const { return "Beacon"; } };
PlayerTemplate g_ptmpl;
Int g_beaconCount;
Bool g_active;
class Player
{
public:
	void iterateObjects(void (*fn)(Object *, void *), void *data) { for (size_t i = 0; i < g_playerObjs.size(); ++i) fn(g_playerObjs[i], data); }
	Bool isPlayerActive() { return g_active; }
	const PlayerTemplate *getPlayerTemplate() { return &g_ptmpl; }
	void countObjectsByThingTemplate(Int n, const ThingTemplate *const *, Bool, Int *counts) { for (Int i = 0; i < n; ++i) counts[i] = g_beaconCount; }
};
Player g_local;
Bool g_hasLocal;
class PlayerList { public: Player *getLocalPlayer() { return g_hasLocal ? &g_local : NULL; } };
PlayerList g_players;
PlayerList *ThePlayerList = &g_players;
class ThingFactory { public: const ThingTemplate *findTemplate(const char *) { return &g_templates[0]; } };
ThingFactory g_factory;
ThingFactory *TheThingFactory = &g_factory;
Bool g_mp, g_replay, g_gameInfoMp;
class GameLogic
{
public:
	Bool isInMultiplayerGame() { return g_mp; }
	Bool isInReplayGame() { return g_replay; }
};
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
class GameInfo { public: Bool isMultiPlayer() { return g_gameInfoMp; } };
GameInfo g_ginfo;
GameInfo *TheGameInfo = &g_ginfo;
class MultiplayerSettings { public: Int m_max; Int getMaxBeaconsPerPlayer() { return m_max; } };
MultiplayerSettings g_mps;
MultiplayerSettings *TheMultiplayerSettings = &g_mps;
struct GlobalData { Int m_netMinPlayers; Int m_TiVOFastMode; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;
GlobalData *TheWritableGlobalData = &g_global;

#include "command_options.inc"
class CommandButton
{
public:
	GUICommandType m_type;
	GUICommandType getCommandType() const { return m_type; }
};
CommandButton g_beaconButton;
Bool g_hasBeaconButton;
class ControlBar { public: const CommandButton *findCommandButton(const char *) { return g_hasBeaconButton ? &g_beaconButton : NULL; } };
ControlBar g_bar;
ControlBar *TheControlBar = &g_bar;

/* the stream the cases append to: the real GameMessage, kept for the dump */
class StreamStandIn
{
public:
	std::vector<GameMessage *> m_msgs;
	GameMessage *appendMessage(GameMessage::Type t) { GameMessage *m = mkMsg(t); m_msgs.push_back(m); return m; }
	GameMessage *insertMessage(GameMessage::Type t, GameMessage *) { return appendMessage(t); }
};
StreamStandIn g_stream;
StreamStandIn *TheMessageStream = &g_stream;

class InGameUI
{
public:
	DrawableList m_selectedDrawables;
	Int m_selectCount, m_maxSelectCount;
	Bool m_displayedMaxWarning, m_inputEnabled, m_attackMove, m_forceAttack, m_forceMove, m_waypoint, m_prefer, m_tracking;
	Bool getInputEnabled() { return m_inputEnabled; }
	Int getSelectCount() { return m_selectCount; }
	Int getMaxSelectCount() { return m_maxSelectCount; }
	Bool getDisplayedMaxWarning() { return m_displayedMaxWarning; }
	void setDisplayedMaxWarning(Bool b) { m_displayedMaxWarning = b; }
	const DrawableList *getAllSelectedDrawables() const { return &m_selectedDrawables; }
	Drawable *getFirstSelectedDrawable() { return m_selectedDrawables.empty() ? NULL : m_selectedDrawables.front(); }
	void selectDrawable(Drawable *d) { if (!d->m_selected) { d->m_selected = TRUE; m_selectedDrawables.push_front(d); ++m_selectCount; } }
	void deselectDrawable(Drawable *d) { if (d->m_selected) { d->m_selected = FALSE; m_selectedDrawables.remove(d); --m_selectCount; } }
	void deselectAllDrawables(Bool postMsg = true)
	{
		DrawableList copy = m_selectedDrawables;
		for (DrawableListIt it = copy.begin(); it != copy.end(); ++it) deselectDrawable(*it);
		m_selectedDrawables.clear();
		if (postMsg) TheMessageStream->appendMessage(GameMessage::MSG_DESTROY_SELECTED_GROUP)->appendBooleanArgument(true);
	}
	void message(UnicodeString m, ...) { g_uiMsgs.push_back(m.m_label); g_uiMsgs.push_back(m.m_count); }
	void setGUICommand(const CommandButton *c) { g_gui.push_back(c ? (Int)c->getCommandType() : -2); }
	void toggleAttackMoveToMode() { m_attackMove = !m_attackMove; }
	void setForceMoveMode(Bool b) { m_forceMove = b; }
	void setForceAttackMode(Bool b) { m_forceAttack = b; }
	void setWaypointMode(Bool b) { m_waypoint = b; }
	void setPreferSelectionMode(Bool b) { m_prefer = b; }
	Bool isInForceMoveToMode() { return m_forceMove; }
	void setCameraTrackingDrawable(Bool b) { m_tracking = b; }
	void setCameraRotateLeft(Bool) {}
	void setCameraRotateRight(Bool) {}
	void setCameraZoomIn(Bool) {}
	void setCameraZoomOut(Bool) {}
	void resetCamera() {}
	/* InGameUI.cpp's, cut */
	Int selectAllUnitsByTypeAcrossRegion( IRegion2D *region, KindOfMaskType mustBeSet, KindOfMaskType mustBeClear );
	Int selectMatchingAcrossRegion( IRegion2D *region );
	Int selectAllUnitsByTypeAcrossScreen(KindOfMaskType mustBeSet, KindOfMaskType mustBeClear);
	Int selectMatchingAcrossScreen( void );
	Int selectAllUnitsByTypeAcrossMap(KindOfMaskType mustBeSet, KindOfMaskType mustBeClear);
	Int selectMatchingAcrossMap();
	Int selectAllUnitsByType(KindOfMaskType mustBeSet, KindOfMaskType mustBeClear);
	Int selectUnitsMatchingCurrentSelection();
	void buildRegion( const ICoord2D *anchor, const ICoord2D *dest, IRegion2D *region );
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

static void pickAndPlayUnitVoiceResponse(const DrawableList *, GameMessage::Type, void * = NULL) {}

class CommandTranslator : public GameMessageTranslator
{
public:
	Bool m_teamExists;
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};
static Bool isSystemMessage( const GameMessage *msg );

#include "kind_of_selection.inc"
#include "select_by_type.inc"
#include "view_hero.inc"
#include "command_gate_open.inc"
	switch (t)
	{
#include "meta_select_cases.inc"
#include "meta_beacon_cases.inc"
#include "meta_mode_cases.inc"
#include "meta_held_cases.inc"
#include "group_cases.inc"
		default:
			break;
	}
	return disp;
}
#include "system_message.inc"

/* ---- MetaEvent ---- */
struct LookupListRec { const char *name; Int value; };
#include "meta_header.inc"
class MetaMapRec
{
public:
	MetaMapRec *m_next;
	GameMessage::Type m_meta;
	MappableKeyType m_key;
	MappableKeyTransition m_transition;
	MappableKeyModState m_modState;
	CommandUsableInType m_usableIn;
	MappableKeyCategories m_category;
	UnicodeString m_description;
	UnicodeString m_displayName;
};
class MetaMap
{
public:
	MetaMapRec *m_metaMaps;
	MetaMapRec *getFirstMetaMapRec() const { return m_metaMaps; }
	GameMessage::Type findGameMessageMetaType(const char *name);
	MetaMapRec *getMetaMapRec(GameMessage::Type t);
};
MetaMap g_metaMap;
MetaMap *TheMetaMap = &g_metaMap;
class Shell { public: Bool m_active; Bool isShellActive() { return m_active; } };
Shell g_shell;
Shell *TheShell = &g_shell;
UnsignedInt g_clientFrame;
class GameClientFrame { public: UnsignedInt getFrame() { return g_clientFrame; } };
/* (#131: UnsignedInt, Mouse.h's type, which the release's `abs(...) < m_dragTolerance` compares unsigned) */
struct MouseStandIn { UnsignedInt m_dragTolerance; };
MouseStandIn g_mouse;
MouseStandIn *TheMouse = &g_mouse;
enum { NUM_MOUSE_BUTTONS = 3 };
enum { MK_NONE_STANDIN = 0 };
class MetaEventTranslator : public GameMessageTranslator
{
public:
	MappableKeyType m_lastKeyDown;
	Int m_lastModState;
	Bool m_nextUpShouldCreateDoubleClick[NUM_MOUSE_BUTTONS];
	ICoord2D m_mouseDownPosition[NUM_MOUSE_BUTTONS];
	MetaEventTranslator();
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};
#define stricmp strcasecmp
#define newInstance(T) new T
namespace metaclient { GameClientFrame g_frameClient; }
#define TheGameClient (&metaclient::g_frameClient)
#include "meta_names.inc"
#include "meta_translate.inc"
#include "meta_map.inc"
#include "meta_ctor.inc"
#undef TheGameClient
#undef newInstance

} // namespace cmx

using namespace cmx;

static void dumpMsgs()
{
	for (size_t i = 0; i < g_stream.m_msgs.size(); ++i) {
		GameMessage *m = g_stream.m_msgs[i];
		Int n = m->getArgumentCount();
		g_msgs.push_back((Int)m->getType());
		g_msgs.push_back(n);
		for (Int a = 0; a < n; ++a) {
			Int dt = (Int)m->getArgumentDataType(a);
			g_msgs.push_back(dt);
			if (dt == ARGUMENTDATATYPE_LOCATION) {
				const Coord3D &c = m->getArgument(a)->location;
				g_msgs.push_back(realBits(c.x)); g_msgs.push_back(realBits(c.y)); g_msgs.push_back(realBits(c.z));
			} else if (dt == ARGUMENTDATATYPE_BOOLEAN) {
				g_msgs.push_back(m->getArgument(a)->boolean ? 1 : 0);
			} else {
				g_msgs.push_back(m->getArgument(a)->integer);
			}
		}
	}
}

static void putSection(const std::vector<int> &v, Int mark)
{
	g_out.insert(g_out.end(), v.begin(), v.end());
	g_out.push_back(mark);
}

extern "C" int ref_command_meta_case(int kind, const int *in, int nin, int *out, int cap)
{
	(void)nin;
	g_out.clear();
	if (kind == 0) {
		g_msgs.clear(); g_looks.clear(); g_uiMsgs.clear(); g_audio.clear(); g_gui.clear();
		g_stream.m_msgs.clear();
		g_ui.m_inputEnabled = in[META_INPUT] != 0;
		g_ndraw = in[META_NDRAW];
		g_hasLocal = in[META_LOCAL] != 0;
		g_ui.m_maxSelectCount = in[META_MAXSEL];
		g_ui.m_displayedMaxWarning = in[META_MAXWARN] != 0;
		g_radar.m_has = in[META_RADAR] != 0;
		memcpy(&g_radar.m_last.x, &in[META_RADAR_X], 4); memcpy(&g_radar.m_last.y, &in[META_RADAR_Y], 4); memcpy(&g_radar.m_last.z, &in[META_RADAR_Z], 4);
		g_mp = in[META_MP] != 0;
		g_replay = in[META_REPLAY] != 0;
		g_active = in[META_ACTIVE] != 0;
		g_global.m_netMinPlayers = in[META_NETMIN];
		g_gameInfoMp = in[META_GAMEINFO_MP] != 0;
		g_beaconCount = in[META_BEACONS];
		g_mps.m_max = in[META_MAXBEACONS];
		g_hasBeaconButton = in[META_BEACON_BUTTON] != 0;
		g_beaconButton.m_type = GUICOMMANDMODE_PLACE_BEACON;
		g_ui.m_attackMove = in[META_ATTACKMOVE] != 0;
		g_ui.m_forceAttack = in[META_FORCEATTACK] != 0;
		g_ui.m_forceMove = in[META_FORCEMOVE] != 0;
		g_ui.m_waypoint = in[META_WAYPOINT] != 0;
		g_ui.m_prefer = in[META_PREFER] != 0;
		g_ui.m_tracking = in[META_TRACKING] != 0;
		g_audioSys.m_misc.m_allCheerSound.m_tag = -7;
		for (Int k = 0; k < 4; ++k) {
			g_templates[k].m_index = k;
			g_templates[k].m_cost = in[META_COSTS + k];
			g_templates[k].m_voiceSelect.m_tag = 100 + k;
			g_templates[k].m_voiceSelect.m_object = (ObjectID)0;
		}
		g_onScreen.clear();
		for (Int i = 1; i <= 8; ++i) {
			const int *d = &in[META_HEADER + (i - 1) * DRAW_WORDS];
			cmx::Drawable &dr = g_draws[i];
			cmx::Object &o = g_objs[i];
			dr.m_id = i;
			dr.m_selected = FALSE;
			dr.m_pos.x = (Real)i * 10.0f + 0.5f; dr.m_pos.y = (Real)i * 3.0f - 2.0f; dr.m_pos.z = (Real)i * 0.25f;
			dr.m_next = (i < g_ndraw) ? &g_draws[i + 1] : NULL;
			dr.m_prev = (i > 1) ? &g_draws[i - 1] : NULL;
			dr.m_obj = d[DRAW_HASOBJ] ? &o : NULL;
			o.m_id = (ObjectID)i;
			o.m_kinds = KindOfMaskType();
			for (Int b = 0; b < 8; ++b) if ((d[DRAW_KINDS] >> b) & 1) o.m_kinds.set(s_kinds[b]);
			o.m_mobile = d[DRAW_MOBILE] != 0;
			o.m_local = d[DRAW_LOCAL] != 0;
			o.m_container = d[DRAW_CONTAINER] ? &g_objs[d[DRAW_CONTAINER]] : NULL;
			o.m_dead = d[DRAW_DEAD] != 0;
			o.m_mass = d[DRAW_MASS] != 0;
			o.m_offMap = d[DRAW_OFFMAP] != 0;
			o.m_tmpl = &g_templates[d[DRAW_TEMPLATE]];
			o.m_carBomb = d[DRAW_CARBOMB] != 0;
			o.m_draw = &dr;
			o.m_pos.x = (Real)i * 7.0f + 1.25f; o.m_pos.y = (Real)i * -2.0f; o.m_pos.z = 0.5f;
			dr.m_tmpl = o.m_tmpl;
			if (i <= g_ndraw && d[DRAW_SCREEN]) g_onScreen.push_back(&dr);
		}
		g_ui.m_selectedDrawables.clear();
		g_ui.m_selectCount = 0;
		for (Int s = 0; s < in[META_NSEL]; ++s) g_ui.selectDrawable(&g_draws[in[META_SEL0 + s]]);
		g_playerObjs.clear();
		for (Int k = 0; k < in[META_NOBJ]; ++k) g_playerObjs.push_back(&g_objs[in[META_OBJ0 + k]]);

		CommandTranslator x;
		x.m_teamExists = in[META_TEAM] != 0;
		GameMessage *msg = mkMsg((GameMessage::Type)in[META_TYPE]);
		Int disp = (Int)x.translateGameMessage(msg);
		msg->deleteInstance();
		dumpMsgs();
		/* the messages appended pass the translator in their turn (m_teamExists) */
		std::vector<GameMessage *> appended = g_stream.m_msgs;
		g_stream.m_msgs.clear();
		for (size_t i = 0; i < appended.size(); ++i) {
			x.translateGameMessage(appended[i]);
			appended[i]->deleteInstance();
		}
		for (size_t i = 0; i < g_stream.m_msgs.size(); ++i) g_stream.m_msgs[i]->deleteInstance();
		g_stream.m_msgs.clear();
		g_out.push_back(disp);
		g_out.push_back(-1);
		putSection(g_msgs, -2);
		putSection(g_looks, -3);
		for (DrawableListIt it = g_ui.m_selectedDrawables.begin(); it != g_ui.m_selectedDrawables.end(); ++it) g_out.push_back((*it)->m_id);
		g_out.push_back(-4);
		putSection(g_uiMsgs, -5);
		putSection(g_audio, -6);
		putSection(g_gui, -7);
		Int w[8] = { (Int)g_ui.m_attackMove, (Int)g_ui.m_forceAttack, (Int)g_ui.m_forceMove, (Int)g_ui.m_waypoint, (Int)g_ui.m_prefer, (Int)g_ui.m_tracking,
			(Int)g_ui.m_displayedMaxWarning, g_ui.m_selectCount };
		g_out.insert(g_out.end(), w, w + 8);
		g_out.push_back(-8);
		g_out.push_back((Int)x.m_teamExists);
	} else if (kind == 1) {
		g_msgs.clear(); g_uiMsgs.clear();
		g_stream.m_msgs.clear();
		g_clientFrame = (UnsignedInt)in[3];
		g_shell.m_active = in[4] != 0;
		g_replay = in[5] != 0;
		g_global.m_TiVOFastMode = in[8];
		while (g_metaMap.m_metaMaps) { MetaMapRec *n = g_metaMap.m_metaMaps->m_next; delete g_metaMap.m_metaMaps; g_metaMap.m_metaMaps = n; }
		Int nrec = in[9];
		for (Int r = 0; r < nrec; ++r) {
			const int *w = &in[10 + r * 5];
			MetaMapRec *m = g_metaMap.getMetaMapRec((GameMessage::Type)w[0]);
			m->m_key = (MappableKeyType)w[1];
			m->m_transition = (MappableKeyTransition)w[2];
			m->m_modState = (MappableKeyModState)w[3];
			m->m_usableIn = (CommandUsableInType)w[4];
		}
		MetaEventTranslator x;
		x.m_lastModState = in[6];
		x.m_lastKeyDown = (MappableKeyType)in[7];
		GameMessage *msg = mkMsg((GameMessage::Type)in[0]);
		msg->appendIntegerArgument(in[1]);
		msg->appendIntegerArgument(in[2]);
		Int disp = (Int)x.translateGameMessage(msg);
		msg->deleteInstance();
		g_out.push_back(disp);
		g_out.push_back(-1);
		for (size_t i = 0; i < g_stream.m_msgs.size(); ++i) { g_out.push_back((Int)g_stream.m_msgs[i]->getType()); g_stream.m_msgs[i]->deleteInstance(); }
		g_stream.m_msgs.clear();
		g_out.push_back(-2);
		g_out.push_back(x.m_lastModState);
		g_out.push_back((Int)x.m_lastKeyDown);
		g_out.push_back(g_global.m_TiVOFastMode);
		g_out.push_back((Int)(g_uiMsgs.size() / 2));
		g_out.push_back(-3);
		for (MetaMapRec *m = g_metaMap.m_metaMaps; m; m = m->m_next) g_out.push_back((Int)m->m_meta);
	} else if (kind == 3) {
		g_stream.m_msgs.clear();
		while (g_metaMap.m_metaMaps) { MetaMapRec *n = g_metaMap.m_metaMaps->m_next; delete g_metaMap.m_metaMaps; g_metaMap.m_metaMaps = n; }
		g_mouse.m_dragTolerance = (UnsignedInt)in[0];
		static unsigned char storage[sizeof(MetaEventTranslator)];
		memset(storage, 0, sizeof storage);
		MetaEventTranslator *x = new (storage) MetaEventTranslator;
		for (Int p = 1; p + 3 < nin; p += 4) {
			GameMessage *msg = mkMsg((GameMessage::Type)in[p]);
			ICoord2D px; px.x = in[p + 1]; px.y = in[p + 2];
			msg->appendPixelArgument(px);
			msg->appendIntegerArgument(in[p + 3]);
			msg->appendIntegerArgument(0);
			Int disp = (Int)x->translateGameMessage(msg);
			msg->deleteInstance();
			g_out.push_back(disp);
			for (size_t i = 0; i < g_stream.m_msgs.size(); ++i) {
				GameMessage *m = g_stream.m_msgs[i];
				const IRegion2D &r = m->getArgument(0)->pixelRegion;
				Int w[6] = { (Int)m->getType(), r.lo.x, r.lo.y, r.hi.x, r.hi.y, m->getArgument(1)->integer };
				g_out.insert(g_out.end(), w, w + 6);
				m->deleteInstance();
			}
			g_stream.m_msgs.clear();
			g_out.push_back(-1);
			for (Int b = 0; b < 3; ++b) g_out.push_back((Int)x->m_nextUpShouldCreateDoubleClick[b]);
			for (Int b = 0; b < 3; ++b) { g_out.push_back(x->m_mouseDownPosition[b].x); g_out.push_back(x->m_mouseDownPosition[b].y); }
			g_out.push_back(-2);
		}
		x->~MetaEventTranslator();
	} else if (kind == 2) {
		Int n = 0;
		while (GameMessageMetaTypeNames[n].name) ++n;
		if (in[0] >= 0 && in[0] < n) {
			g_out.push_back(GameMessageMetaTypeNames[in[0]].value);
			for (const char *c = GameMessageMetaTypeNames[in[0]].name; *c; ++c) g_out.push_back(*c);
		}
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
