/* Differential-test wrapper for GameLogic's frame (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp, cut verbatim by build.rs into
**   OUT_DIR/glframe_gen/game_logic_frame_cut.inc: processCommandList to deselectObject; the perf timers and update(); inCRCGen
**   and getCRC; isGamePaused to getObjectCount; and the buildable and control-bar overrides.
** They are compiled inside namespace glf, on stand-ins for everything they call: a GameLogic holding the members they
** use, a small world (objects 1-6, players 0-15, the AI's groups), the subsystems (display, client, script engine,
** tactical view, terrain, recorder, game info, stats collector, AI, build assistant, partition manager, stores,
** victory conditions, network, in-game UI, mouse, audio, load screen), a command list of stand-in messages (the two
** message types the code names take their values from the real MessageStream.h), and logicMessageDispatcher (batch
** D1's, verified there) as a stand-in that caches MSG_LOGIC_CRC's CRC by player and asks for validation on the local
** player's. The CRCs are the Xfer build's real XferCRC and XferDeepCRC (a file); the objects, the partition manager,
** the player list, the AI and the game state are snapshots whose crc xfers a value.
**
** Entry point (extern "C", tests/differential_game_logic_frame.rs):
**   ref_glframe_run(cfg, ops, n, out, cap)  runs n operations of six words and writes the log
**   ref_glsave_run(cfg, ops, n, out, cap)   batch L2: GameLogic::xfer's operations (tests/differential_game_logic_save.rs)
** Batch L2 adds the cut from findTOCEntryByName to loadPostProcess (the table of contents, prepareLogicForObjectLoad,
** xfer), with real XferSave/XferLoad (a file) and stand-ins for objects made by a factory (templates Tank, Bridge,
** WallWalker, Tower), bridges, the pathfinder, the campaign manager, the cave system, polygon triggers, the build
** assistant's sell list and the control bar's buttons. The override tables are the stand-in GameLogic's ordered maps
** (upstream's hash_maps save in STLport's order: see logic/world/game_logic_xfer.rs). */
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
#include <unistd.h>
#include <stdint.h>
#include <string>
#include <vector>
#include <map>
#include <list>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/Snapshot.h"
#include "Common/Xfer.h"
#include "Common/XferCRC.h"
#include "Common/XferDeepCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/LatchRestore.h"
#include "Common/MessageStream.h"

#undef GameMessage
#undef GameMessageArgument
#undef GameMessageList
#undef GameMessageTranslator
#undef MessageStream
#undef CommandList
#undef TheMessageStream
#undef TheCommandList
#undef buildRegion

#ifndef CRCGEN_LOG
#define CRCGEN_LOG(x)
#endif
#ifndef DECLARE_PERF_TIMER
#define DECLARE_PERF_TIMER(x)
#endif
#ifndef USE_PERF_TIMER
#define USE_PERF_TIMER(x)
#endif

namespace glf {

std::string g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...)
{
	static char buf[1 << 16];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	g_log += buf;
	g_log += '\n';
}

/* ---- configuration (cfg words and op 1) ---- */
unsigned g_flags;          /* bit 0 view time frozen, 1 camera movement finished, 2 script frozen (debug), 3 script frozen
                           ** (script), 4 a movie playing, 5 a recorder, 6 it is multiplayer, 7 it plays back, 8 a game info,
                           ** 9 a stats collector, 10 a network, 11 it saw a CRC mismatch, 12 a load screen, 13 UI input
                           ** enabled, 14 the mouse visible */
Int g_crcInterval, g_replayInterval;
unsigned g_connected;      /* the network's connected slots */
Int g_numPlayers;
unsigned g_players;        /* which of the 16 players exist */
Int g_localPlayer;
unsigned g_seedCrc;
unsigned g_time;
unsigned g_objDisabled, g_objMass, g_objDrawable;
unsigned g_nextGroup;

/* ---- the constants the code names ---- */
enum { MAX_SLOTS = 8, MAX_PLAYER_COUNT = 16 };
enum { CRC_CACHED, CRC_RECALC };
enum { GAME_SINGLE_PLAYER, GAME_LAN, GAME_SKIRMISH, GAME_REPLAY, GAME_SHELL, GAME_INTERNET, GAME_NONE };
enum RecorderModeType { RECORDERMODETYPE_RECORD, RECORDERMODETYPE_PLAYBACK, RECORDERMODETYPE_NONE };
enum SnapshotType { SNAPSHOT_SAVELOAD, SNAPSHOT_DEEPCRC_LOGICONLY, SNAPSHOT_DEEPCRC };
/* AudioAffect.h lines 35-44 */
enum AudioAffect
{
	AudioAffect_Music		= 0x01,
	AudioAffect_Sound		= 0x02,
	AudioAffect_Sound3D	= 0x04,
	AudioAffect_Speech	= 0x08,
	AudioAffect_All			= (AudioAffect_Music | AudioAffect_Sound | AudioAffect_Sound3D | AudioAffect_Speech),
	AudioAffect_SystemSetting = 0x10,
};
typedef Int BuildableStatus;
class CommandButton;
typedef const CommandButton *ConstCommandButtonPtr;
enum UpdateSleepTime { UPDATE_SLEEP_NONE = 1 };
Int REPLAY_CRC_INTERVAL;

void setFPMode() { logf("fpmode"); }
UnsignedInt timeGetTime() { unsigned t = g_time; g_time += 13; logf("time %d", (int)t); return t; }
UnsignedInt GetGameLogicRandomSeedCRC() { return g_seedCrc; }

/* ---- messages ---- */
class GameMessage
{
public:
	enum Type { MSG_CLEAR_GAME_DATA = ::MsTestGameMessage::MSG_CLEAR_GAME_DATA, MSG_LOGIC_CRC = ::MsTestGameMessage::MSG_LOGIC_CRC };
	Int m_type, m_player; UnsignedInt m_crc; Int m_index; GameMessage *m_next;
	GameMessage *next() { return m_next; }
	Int getType() const { return m_type; }
	void appendIntegerArgument(Int v) { logf("arg int %d", (int)v); }
	void appendBooleanArgument(Bool b) { logf("arg bool %d", (int)b); }
};
GameMessage g_msgs[64];
class CommandList
{
public:
	std::vector<GameMessage *> m_list;
	GameMessage *getFirstMessage() { return m_list.empty() ? NULL : m_list[0]; }
	Bool containsMessageOfType(Int t) { for (size_t i = 0; i < m_list.size(); ++i) if (m_list[i]->m_type == t) return TRUE; return FALSE; }
	void reset() { m_list.clear(); logf("commandlist reset"); }
	void push(GameMessage *m) { if (!m_list.empty()) m_list.back()->m_next = m; m->m_next = NULL; m_list.push_back(m); }
};
CommandList g_cmd;
CommandList *TheCommandList = &g_cmd;
class MessageStream
{
public:
	GameMessage m_scratch;
	GameMessage *appendMessage(GameMessage::Type t) { logf("append %d", (int)t); return &m_scratch; }
};
MessageStream g_stream;
MessageStream *TheMessageStream = &g_stream;

/* ---- objects, players, groups ---- */
class Drawable { public: Int m_id; };
class DisabledMaskType { public: Bool any() const { return FALSE; } Bool anyIntersectionWith(const DisabledMaskType &) const { return FALSE; } };
Drawable g_drawables[7];
class ThingTemplate;
/* batch L2's kinds (stand-in values) */
enum { KINDOF_BRIDGE = 0, KINDOF_WALK_ON_TOP_OF_WALL = 1 };
class Object : public ::Snapshot
{
public:
	Int m_id; Object *m_next;
	/* batch L2 */
	const ThingTemplate *m_template; unsigned m_kind; Int m_value; Bool m_destroyed; Coord3D m_pos;
	const ThingTemplate *getTemplate() const { return m_template; }
	Bool isKindOf(Int k) const { return (m_kind >> k) & 1; }
	const Coord3D *getPosition() const { return &m_pos; }
	Object *getNextObject() { return m_next; }
	Bool isDisabled() const { return (g_objDisabled >> m_id) & 1; }
	DisabledMaskType getDisabledFlags() const { return DisabledMaskType(); }
	void checkDisabledStatus() { logf("checkdisabled %d", (int)m_id); }
	Bool isMassSelectable() const { return (g_objMass >> m_id) & 1; }
	Drawable *getDrawable() { return ((g_objDrawable >> m_id) & 1) ? &g_drawables[m_id] : NULL; }
	virtual void crc(Xfer *x) { UnsignedInt v = (UnsignedInt)m_id * 2654435761u; x->xferUnsignedInt(&v); }
	virtual void xfer(Xfer *x) { x->xferInt(&m_value); logf("object %d xfer %d", (int)m_id, (int)m_value); }
	virtual void loadPostProcess() {}
};
Object g_objects[7];
class ValueSnapshot : public ::Snapshot
{
public:
	UnsignedInt m_value; const char *m_name;
	virtual void crc(Xfer *x) { UnsignedInt v = m_value; x->xferUnsignedInt(&v); }
	virtual void xfer(Xfer *x) { x->xferUnsignedInt(&m_value); logf("%s xfer %u", m_name, m_value); }
	virtual void loadPostProcess() {}
};
class AIGroup
{
public:
	UnsignedInt m_id;
	void add(Object *o) { logf("group %u add %d", m_id, (int)o->m_id); }
	Bool remove(Object *o) { Bool deleted = (m_id + o->m_id) % 3 == 0; logf("group %u remove %d -> %d", m_id, (int)o->m_id, (int)deleted); return deleted; }
};
AIGroup g_groups[256];
class Team {};
Team g_neutralTeam;
Bool g_hasNeutral;
class Player
{
public:
	Int m_index;
	Team *getDefaultTeam() { return &g_neutralTeam; }
	UnsignedShort getPlayerMask() const { return (UnsignedShort)(1 << m_index); }
	void setCurrentlySelectedAIGroup(AIGroup *g) { if (g) logf("player %d select group %u", (int)m_index, g->m_id); else logf("player %d select none", (int)m_index); }
	void addAIGroupToCurrentSelection(AIGroup *g) { logf("player %d add group %u", (int)m_index, g->m_id); }
	void getCurrentSelectionAsAIGroup(AIGroup *g) { logf("player %d selection into group %u", (int)m_index, g->m_id); }
};
Player g_pl[MAX_PLAYER_COUNT];
class PlayerList : public ValueSnapshot
{
public:
	Player *getNthPlayer(Int i) { return (i >= 0 && i < MAX_PLAYER_COUNT && ((g_players >> i) & 1)) ? &g_pl[i] : NULL; }
	Player *getNeutralPlayer() { return g_hasNeutral ? &g_pl[0] : NULL; }
	/* PlayerList.cpp lines 374-393 */
	Player *getEachPlayerFromMask( PlayerMaskType& maskToAdjust )
	{
		Player *player = NULL;
		Int i;
		for( i = 0; i < MAX_PLAYER_COUNT; i++ )
		{
			player = getNthPlayer( i );
			if ( player && BitTest(player->getPlayerMask(), maskToAdjust ))
			{
				maskToAdjust &= (~player->getPlayerMask());
				return player;
			}
		}
		maskToAdjust = 0;
		return NULL;
	}
};
PlayerList g_playerList;
PlayerList *ThePlayerList = &g_playerList;
class Pathfinder;
class AI : public ValueSnapshot
{
public:
	void UPDATE() { logf("ai update"); }
	AIGroup *createGroup() { AIGroup *g = &g_groups[g_nextGroup % 256]; g->m_id = g_nextGroup++; logf("create group %u", g->m_id); return g; }
	void destroyGroup(AIGroup *g) { logf("destroy group %u", g->m_id); }
	Pathfinder *pathfinder();
};
AI g_ai;
AI *TheAI = &g_ai;
class PartitionManager : public ValueSnapshot { public: void UPDATE() { logf("partition update"); } };
PartitionManager g_pm;
PartitionManager *ThePartitionManager = &g_pm;
class GameState
{
public:
	void friend_xferSaveDataForCRC(Xfer *x, SnapshotType which) { UnsignedInt v = 0xC0FFEE00u + (UnsignedInt)which; x->xferUnsignedInt(&v); logf("gamestate crc %d", (int)which); }
};
GameState g_gs;
GameState *TheGameState = &g_gs;

/* ---- subsystems ---- */
class Display { public: Bool isMoviePlaying() { return (g_flags >> 4) & 1; } };
Display g_display;
Display *TheDisplay = &g_display;
class GameClient { public: void setFrame(UnsignedInt f) { logf("client frame %u", f); } };
GameClient g_client;
GameClient *TheGameClient = &g_client;
class ScriptEngine
{
public:
	void UPDATE() { logf("script update"); }
	Bool isTimeFrozenDebug() { return (g_flags >> 2) & 1; }
	Bool isTimeFrozenScript() { return (g_flags >> 3) & 1; }
	void forceUnfreezeTime() { logf("unfreeze"); }
};
ScriptEngine g_se;
ScriptEngine *TheScriptEngine = &g_se;
class View { public: Bool isTimeFrozen() { return g_flags & 1; } Bool isCameraMovementFinished() { return (g_flags >> 1) & 1; } };
View g_view;
View *TheTacticalView = &g_view;
/* batch L2: bridges (an object's position's x names its bridge) */
enum { BRIDGE_MAX_TOWERS = 4 };
struct BridgeInfo { ObjectID bridgeObjectID; ObjectID towerObjectID[BRIDGE_MAX_TOWERS]; };
class Bridge { public: BridgeInfo m_info; const BridgeInfo *peekBridgeInfo() const { return &m_info; } };
Bridge g_bridges[4];
class TerrainLogic
{
public:
	void UPDATE() { logf("terrain update"); }
	Bridge *findBridgeAt(const Coord3D *p) { Int i = (Int)p->x; return (i >= 0 && i < 4) ? &g_bridges[i] : NULL; }
};
TerrainLogic g_tl;
TerrainLogic *TheTerrainLogic = &g_tl;
class RecorderClass
{
public:
	Bool isMultiplayer() { return (g_flags >> 6) & 1; }
	RecorderModeType getMode() { return ((g_flags >> 7) & 1) ? RECORDERMODETYPE_PLAYBACK : RECORDERMODETYPE_RECORD; }
	void UPDATE() { logf("recorder update"); }
};
RecorderClass g_rec;
RecorderClass *TheRecorder = NULL;
class GameInfo { public: Int getCRCInterval() { return g_crcInterval; } };
GameInfo g_gi;
GameInfo *TheGameInfo = NULL;
class StatsCollector { public: void update() { logf("stats update"); } };
StatsCollector g_stats;
StatsCollector *TheStatsCollector = NULL;
class Updater
{
public:
	const char *m_name; UnsignedInt m_value;
	void UPDATE() { logf("%s update", m_name); }
	void xferTheSellList(Xfer *x) { x->xferUnsignedInt(&m_value); logf("selllist xfer %u", m_value); }
};
Updater g_ba = { "buildassistant", 0 }, g_ws = { "weaponstore", 0 }, g_ls = { "locomotorstore", 0 }, g_vc = { "victory", 0 };
Updater *TheBuildAssistant = &g_ba, *TheWeaponStore = &g_ws, *TheLocomotorStore = &g_ls, *TheVictoryConditions = &g_vc;
class Network
{
public:
	Bool sawCRCMismatch() { return (g_flags >> 11) & 1; }
	void setSawCRCMismatch() { g_flags |= 1u << 11; logf("network mismatch"); }
	Bool isPlayerConnected(Int i) { return (g_connected >> i) & 1; }
	Int getNumPlayers() { return g_numPlayers; }
};
Network g_net;
Network *TheNetwork = NULL;
class InGameUI
{
public:
	Bool getInputEnabled() { return (g_flags >> 13) & 1; }
	void setInputEnabled(Bool b) { logf("ui input %d", (int)b); }
	void selectDrawable(Drawable *d) { logf("ui select %d", (int)d->m_id); }
	void deselectDrawable(Drawable *d) { logf("ui deselect %d", (int)d->m_id); }
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;
class Mouse
{
public:
	enum MouseCursor { ARROW = 1 };
	Bool getVisibility() { return (g_flags >> 14) & 1; }
	void setVisibility(Bool b) { logf("mouse visible %d", (int)b); }
	void setCursor(MouseCursor c) { logf("mouse cursor %d", (int)c); }
};
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;
class AudioManager
{
public:
	void pauseAudio(AudioAffect a) { logf("audio pause %d", (int)a); }
	void resumeAudio(AudioAffect a) { logf("audio resume %d", (int)a); }
};
AudioManager g_audio;
AudioManager *TheAudio = &g_audio;
class LoadScreen { public: void processProgress(Int p, Int pct) { logf("loadscreen %d %d", (int)p, (int)pct); } };
LoadScreen g_loadScreen;
class ThingTemplate { public: AsciiString m_name; const AsciiString &getName() const { return m_name; } };

/* ---- batch L2's stand-ins ---- */
/* GameState.h lines 129-140 */
enum SaveCode
{
	SC_INVALID = -1,
	SC_OK,
	SC_NO_FILE_AVAILABLE,
	SC_FILE_NOT_FOUND,
	SC_UNABLE_TO_OPEN_FILE,
	SC_INVALID_XFER,
	SC_UNKNOWN_BLOCK,
	SC_INVALID_DATA,
	SC_ERROR,
};
/* the factory's templates: Tank, Bridge (a bridge), WallWalker (walks on walls), Tower; others are unknown */
const char *g_tnames[6] = { "Tank", "Bridge", "WallWalker", "Tower", "Ghost", "tank" };
ThingTemplate g_tt[4];
Object g_pool[4096];
Int g_poolUsed, g_nextID;
Object *&objList();
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name) { for (int i = 0; i < 4; ++i) if (g_tt[i].m_name == name) return &g_tt[i]; return NULL; }
	Object *newObject(const ThingTemplate *tt, Team *)
	{
		if (g_poolUsed >= 4096) abort();   /* a program never makes this many */
		Object *o = &g_pool[g_poolUsed++];
		Int t = (Int)(tt - g_tt);
		o->m_id = g_nextID++; o->m_template = tt; o->m_kind = t == 1 ? 1u : t == 2 ? 2u : 0u; o->m_value = 0; o->m_destroyed = FALSE;
		o->m_pos.x = t == 1 ? 0 : -1; o->m_pos.y = 0; o->m_pos.z = 0;   /* a made bridge stands on bridge 0 */
		o->m_next = objList(); objList() = o;   /* registerObject prepends */
		logf("new object %d [%s]", (int)o->m_id, tt->getName().str());
		return o;
	}
};
ThingFactory g_tf;
ThingFactory *TheThingFactory = &g_tf;
class Pathfinder { public: void addWallPiece(Object *o) { logf("wall piece %d", (int)o->m_id); } void newMap() { logf("pathfinder newmap"); } };
Pathfinder g_pathfinder;
ValueSnapshot g_campaign, g_caves;
ValueSnapshot *TheCampaignManager = &g_campaign, *TheCaveSystem = &g_caves;
class PolygonTrigger : public ::Snapshot
{
public:
	Int m_id; UnsignedInt m_value; PolygonTrigger *m_next;
	static PolygonTrigger *s_first;
	PolygonTrigger *getNext() { return m_next; }
	Int getID() const { return m_id; }
	static PolygonTrigger *getFirstPolygonTrigger() { return s_first; }
	static PolygonTrigger *getPolygonTriggerByID(Int id) { for (PolygonTrigger *p = s_first; p; p = p->m_next) if (p->m_id == id) return p; return NULL; }
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *x) { x->xferUnsignedInt(&m_value); logf("poly %d xfer %u", (int)m_id, m_value); }
	virtual void loadPostProcess() {}
};
PolygonTrigger *PolygonTrigger::s_first = NULL;
PolygonTrigger g_polys[6];
class CommandButton { public: AsciiString m_name; const AsciiString &getName() const { return m_name; } };
CommandButton g_buttons[10];
class ControlBar
{
public:
	const CommandButton *findCommandButton(const AsciiString &name) { for (int i = 1; i <= 3; ++i) if (g_buttons[i].m_name == name) return &g_buttons[i]; return NULL; }
};
ControlBar g_controlBar;
ControlBar *TheControlBar = &g_controlBar;

/* the sleepy heap: only update() reads it here, and the world's module loop (verified with the world) stands in */
struct SleepyStandIn { Bool empty() const { logf("sleepy"); return TRUE; } };
class UpdateModule
{
public:
	UnsignedInt friend_getNextCallFrame() { return 0; }
	void friend_setNextCallFrame(UnsignedInt) {}
	Object *friend_getObject() { return NULL; }
	DisabledMaskType getDisabledTypesToProcess() const { return DisabledMaskType(); }
	UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
};
typedef UpdateModule *UpdateModulePtr;

/* string-keyed maps for the overrides (upstream's hash_maps: only insertion and lookup are used here) */
struct AsciiLess { bool operator()(const AsciiString &a, const AsciiString &b) const { return strcmp(a.str(), b.str()) < 0; } };
template <class V> struct StrMap
{
	typedef typename std::map<AsciiString, V, AsciiLess>::const_iterator const_iterator;
	std::map<AsciiString, V, AsciiLess> m;
	V &operator[](const AsciiString &k) { return m[k]; }
	const_iterator find(const AsciiString &k) const { return m.find(k); }
	const_iterator begin() const { return m.begin(); }
	const_iterator end() const { return m.end(); }
	bool empty() const { return m.empty(); }
};

class GameLogic
{
public:
	typedef StrMap<BuildableStatus> BuildableMap;
	typedef StrMap<ConstCommandButtonPtr> ControlBarOverrideMap;
	Bool m_isInUpdate, m_startNewGame, m_shouldValidateCRCs, m_gamePaused, m_inputEnabledMemory, m_mouseVisibleMemory,
		m_forceGameStartByTimeOut;
	UnsignedInt m_frame, m_CRC;
	Int m_gameMode;
	std::map<Int, UnsignedInt> m_cachedCRCs;
	Bool m_progressComplete[MAX_SLOTS];
	Int m_progressCompleteTimeout[MAX_SLOTS];
	LoadScreen *m_loadScreen;
	Object *m_objList;
	SleepyStandIn m_sleepyUpdates;
	UpdateModulePtr m_curUpdateModule;
	BuildableMap m_thingTemplateBuildableOverrides;
	ControlBarOverrideMap m_controlBarOverrides;

	UnsignedInt getFrame() { return m_frame; }
	Int getGameMode() { return m_gameMode; }
	Bool isInMultiplayerGame() { return m_gameMode == GAME_LAN || m_gameMode == GAME_INTERNET; }
	Bool isInGameLogicUpdate() const { return m_isInUpdate; }
	void startNewGame(Bool b) { logf("startnewgame %d", (int)b); }
	UpdateModulePtr peekSleepyUpdate() const { return NULL; }
	void rebalanceSleepyUpdate(Int) {}
	void validateSleepyUpdate() const {}
	void processDestroyList()
	{
		logf("destroylist");
		for (Object **p = &m_objList; *p; ) {
			if ((*p)->m_destroyed) *p = (*p)->m_next; else p = &(*p)->m_next;
		}
	}
	/* batch L2 */
	struct ObjectTOCEntry { AsciiString name; UnsignedShort id; };
	typedef std::list< ObjectTOCEntry > ObjectTOCList;
	typedef ObjectTOCList::iterator ObjectTOCListIterator;
	ObjectTOCList m_objectTOC;
	Int m_rankLevelLimit, m_scriptHulkMaxLifetimeOverride, m_rankPointsToAddAtGameStart;
	Bool m_isScoringEnabled, m_showBehindBuildingMarkers, m_drawIconUI, m_showDynamicLOD;
	UnsignedShort m_superweaponRestriction;
	Object *findObjectByID(ObjectID id) { for (Object *o = m_objList; o; o = o->m_next) if ((ObjectID)o->m_id == id) return o; return NULL; }
	void destroyObject(Object *obj) { if (!obj || obj->m_destroyed) return; obj->m_destroyed = TRUE; logf("destroy %d", (int)obj->m_id); }
	void addTOCEntry( AsciiString name, UnsignedShort id );
	ObjectTOCEntry *findTOCEntryByName( AsciiString name );
	ObjectTOCEntry *findTOCEntryById( UnsignedShort id );
	void xferObjectTOC( Xfer *xfer );
	void prepareLogicForObjectLoad( void );
	void xfer( Xfer *xfer );
	Object *getFirstObject() { return m_objList; }
	void logicMessageDispatcher(GameMessage *msg, void *)
	{
		logf("dispatch %d player %d", (int)msg->m_type, (int)msg->m_player);
		if (msg->m_type == GameMessage::MSG_LOGIC_CRC) {
			if (msg->m_player == g_localPlayer) m_shouldValidateCRCs = TRUE;
			m_cachedCRCs[msg->m_player] = msg->m_crc;
		}
	}

	void processCommandList(CommandList *list);
	Bool isIntroMoviePlaying();
	void selectObject(Object *obj, Bool createNewSelection, PlayerMaskType playerMask, Bool affectClient = FALSE);
	void deselectObject(Object *obj, PlayerMaskType playerMask, Bool affectClient = FALSE);
	void update(void);
	UnsignedInt getCRC(Int mode = CRC_CACHED, AsciiString deepCRCFileName = AsciiString::TheEmptyString);
	Bool isGamePaused(void);
	void setGamePaused(Bool paused, Bool pauseMusic = TRUE);
	void processProgress(Int playerId, Int percentage);
	void processProgressComplete(Int playerId);
	Bool isProgressComplete(void);
	void lastHeardFrom(Int playerId);
	void testTimeOut(void);
	void timeOutGameStart(void);
	void initTimeOutValues(void);
	UnsignedInt getObjectCount(void);
	void setBuildableStatusOverride(const ThingTemplate *tt, BuildableStatus bs);
	Bool findBuildableStatusOverride(const ThingTemplate *tt, BuildableStatus &bs) const;
	void setControlBarOverride(const AsciiString &commandSetName, Int slot, ConstCommandButtonPtr commandButton);
	Bool findControlBarOverride(const AsciiString &commandSetName, Int slot, ConstCommandButtonPtr &commandButton) const;
	enum { PROGRESS_COMPLETE_TIMEOUT = 60000 };   /* GameLogic.h line 363 */
};
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
Object *&objList() { return g_logic.m_objList; }
Pathfinder *AI::pathfinder() { return &g_pathfinder; }

/* ---- the upstream code ---- */
#include "game_logic_frame_cut.inc"

/* ---- the test ---- */
const CommandButton *buttonOf(unsigned v) { return v == 0 ? NULL : &g_buttons[v]; }
unsigned buttonId(const CommandButton *b) { return b ? (unsigned)(b - g_buttons) : 0; }
const char *g_names[6] = { "Alpha", "beta", "", "Gamma_Set", "alpha", "x" };
ThingTemplate g_templates[6];
char g_deepFile[64];

void dumpState()
{
	std::string pc, pt, cc;
	char t[32];
	for (int i = 0; i < MAX_SLOTS; ++i) { snprintf(t, sizeof t, " %d", (int)g_logic.m_progressComplete[i]); pc += t; snprintf(t, sizeof t, " %d", (int)g_logic.m_progressCompleteTimeout[i]); pt += t; }
	for (std::map<Int, UnsignedInt>::iterator it = g_logic.m_cachedCRCs.begin(); it != g_logic.m_cachedCRCs.end(); ++it) { snprintf(t, sizeof t, " %d:%X", (int)it->first, it->second); cc += t; }
	logf("state frame %u crc %X new %d update %d paused %d input %d mouse %d force %d validate %d crcs%s complete%s timeout%s",
		g_logic.m_frame, g_logic.m_CRC, (int)g_logic.m_startNewGame, (int)g_logic.m_isInUpdate, (int)g_logic.m_gamePaused,
		(int)g_logic.m_inputEnabledMemory, (int)g_logic.m_mouseVisibleMemory, (int)g_logic.m_forceGameStartByTimeOut,
		(int)g_logic.m_shouldValidateCRCs, cc.c_str(), pc.c_str(), pt.c_str());
}

void applyFlags()
{
	TheRecorder = ((g_flags >> 5) & 1) ? &g_rec : NULL;
	TheGameInfo = ((g_flags >> 8) & 1) ? &g_gi : NULL;
	TheStatsCollector = ((g_flags >> 9) & 1) ? &g_stats : NULL;
	TheNetwork = ((g_flags >> 10) & 1) ? &g_net : NULL;
	g_logic.m_loadScreen = ((g_flags >> 12) & 1) ? &g_loadScreen : NULL;
}

void run_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4];
	switch (w[0]) {
	case 0: g_logic.update(); logf("updated"); break;
	case 1: g_flags = a; g_logic.m_gameMode = (Int)(b % 7); g_crcInterval = (Int)c; REPLAY_CRC_INTERVAL = (Int)d; applyFlags(); logf("config"); break;
	case 2: g_logic.m_startNewGame = (Bool)(a & 1); logf("pending %d", (int)(a & 1)); break;
	case 3: {
		if (g_cmd.m_list.size() >= 64) { logf("full"); break; }
		GameMessage *m = &g_msgs[g_cmd.m_list.size()];
		m->m_type = a % 3 == 0 ? GameMessage::MSG_LOGIC_CRC : a % 3 == 1 ? GameMessage::MSG_CLEAR_GAME_DATA : 1000 + (Int)(a % 50);
		m->m_player = (Int)(b % 10);
		m->m_crc = c;
		g_cmd.push(m);
		logf("push %d %d", (int)m->m_type, (int)m->m_player);
		break;
	}
	case 4: {
		AsciiString deep;
		if (b & 1) deep = AsciiString(g_deepFile);
		UnsignedInt crc = g_logic.getCRC((Int)(a % 2), deep);
		logf("crc %X", crc);
		if (b & 1) {
			FILE *f = fopen(g_deepFile, "rb");
			std::string h;
			int ch;
			char t[4];
			while (f && (ch = fgetc(f)) != EOF) { snprintf(t, sizeof t, "%02x", ch); h += t; }
			if (f) fclose(f);
			logf("deep %s", h.c_str());
		}
		break;
	}
	case 5: g_logic.selectObject(a % 7 == 0 ? NULL : &g_objects[a % 7], (Bool)(b & 1), (PlayerMaskType)c, (Bool)(d & 1)); logf("selected"); break;
	case 6: g_logic.deselectObject(a % 7 == 0 ? NULL : &g_objects[a % 7], (PlayerMaskType)c, (Bool)(d & 1)); logf("deselected"); break;
	case 7: g_logic.setGamePaused((Bool)(a & 1), (Bool)(b & 1)); logf("paused %d", (int)g_logic.isGamePaused()); break;
	case 8: g_logic.processProgress((Int)a - 2, (Int)(b % 101)); logf("progress"); break;
	case 9: g_logic.processProgressComplete((Int)a - 2); logf("complete"); break;
	case 10: logf("iscomplete %d", (int)g_logic.isProgressComplete()); break;
	case 11: g_logic.testTimeOut(); logf("tested %d", (int)g_logic.m_forceGameStartByTimeOut); break;
	case 12: g_logic.timeOutGameStart(); logf("forced"); break;
	case 13: g_logic.initTimeOutValues(); logf("inited"); break;
	case 14: logf("count %u", g_logic.getObjectCount()); break;
	case 15: {
		const ThingTemplate *tt = a % 7 == 6 ? NULL : &g_templates[a % 7 % 6];
		if (b & 1) { g_logic.setBuildableStatusOverride(tt, (BuildableStatus)(c % 5)); logf("setbuildable"); }
		else { BuildableStatus bs = (BuildableStatus)99; Bool found = g_logic.findBuildableStatusOverride(tt, bs); logf("findbuildable %d %d", (int)found, (int)bs); }
		break;
	}
	case 16: {
		AsciiString cs(g_names[a % 6]);
		Int slot = (Int)(c % 16) - 2;
		if (d == 48) slot = -48;
		if (b & 1) { g_logic.setControlBarOverride(cs, slot, buttonOf(b >> 1 & 3)); logf("setbar"); }
		else { ConstCommandButtonPtr p = buttonOf(9); Bool found = g_logic.findControlBarOverride(cs, slot, p); logf("findbar %d %u", (int)found, buttonId(p)); }
		break;
	}
	case 17: g_connected = a & 0xff; g_numPlayers = (Int)(b % 9); logf("network"); break;
	case 18: g_objDisabled = a & 0x7e; g_objMass = b & 0x7e; g_objDrawable = c & 0x7e; logf("objects"); break;
	case 19: dumpState(); break;
	default: logf("op?"); break;
	}
}


/* ==== batch L2: GameLogic::xfer ==== */
std::vector<unsigned char> g_saved;
char g_saveFile[64];

std::string hexOf(const std::vector<unsigned char> &b)
{
	std::string h;
	char t[4];
	for (size_t i = 0; i < b.size(); ++i) { snprintf(t, sizeof t, "%02x", b[i]); h += t; }
	return h;
}

void dumpSave()
{
	std::string o, toc, bo, cb;
	char t[96];
	for (Object *x = g_logic.m_objList; x; x = x->m_next) { snprintf(t, sizeof t, " %d:%s:%d:%u", (int)x->m_id, x->m_template->getName().str(), (int)x->m_value, x->m_kind); o += t; }
	for (GameLogic::ObjectTOCListIterator it = g_logic.m_objectTOC.begin(); it != g_logic.m_objectTOC.end(); ++it) { snprintf(t, sizeof t, " %s:%u", it->name.str(), (unsigned)it->id); toc += t; }
	for (GameLogic::BuildableMap::const_iterator it = g_logic.m_thingTemplateBuildableOverrides.begin(); it != g_logic.m_thingTemplateBuildableOverrides.end(); ++it) { snprintf(t, sizeof t, " %s:%d", it->first.str(), (int)it->second); bo += t; }
	for (GameLogic::ControlBarOverrideMap::const_iterator it = g_logic.m_controlBarOverrides.begin(); it != g_logic.m_controlBarOverrides.end(); ++it) { snprintf(t, sizeof t, " %s:%u", it->first.str(), buttonId(it->second)); cb += t; }
	logf("logic frame %u scoring %d markers %d icon %d lod %d hulk %d rankpts %d sw %u ranklimit %d campaign %u caves %u sell %u",
		g_logic.m_frame, (int)g_logic.m_isScoringEnabled, (int)g_logic.m_showBehindBuildingMarkers, (int)g_logic.m_drawIconUI,
		(int)g_logic.m_showDynamicLOD, (int)g_logic.m_scriptHulkMaxLifetimeOverride, (int)g_logic.m_rankPointsToAddAtGameStart,
		(unsigned)g_logic.m_superweaponRestriction, (int)g_logic.m_rankLevelLimit, g_campaign.m_value, g_caves.m_value, g_ba.m_value);
	logf("objects%s", o.c_str());
	logf("toc%s", toc.c_str());
	logf("buildable%s bar%s", bo.c_str(), cb.c_str());
}

Object *nthObject(unsigned n)
{
	Int count = 0;
	for (Object *x = g_logic.m_objList; x; x = x->m_next) ++count;
	if (count == 0) return NULL;
	Object *x = g_logic.m_objList;
	for (unsigned i = 0; i < n % count; ++i) x = x->m_next;
	return x;
}

void run_save_op(const unsigned *w)
{
	unsigned a = w[1], b = w[2], c = w[3], d = w[4], e = w[5];
	switch (w[0]) {
	case 0: {
		Object *o = TheThingFactory->newObject(&g_tt[a % 4], NULL);
		o->m_value = (Int)b;
		o->m_pos.x = (Real)((Int)(c % 6) - 1);
		logf("made %d", (int)o->m_id);
		break;
	}
	case 1: { Object *o = nthObject(a); if (o) o->m_value = (Int)b; logf("value"); break; }
	case 2: {
		FILE *f = fopen(g_saveFile, "wb"); if (f) fclose(f);
		XferSave x;
		x.open(AsciiString(g_saveFile));
		try { g_logic.xfer(&x); logf("save"); }
		catch (XferStatus st) { logf("save threw xfer %d", (int)st); }
		catch (SaveCode sc) { logf("save threw save %d", (int)sc); }
		x.close();
		g_saved.clear();
		f = fopen(g_saveFile, "rb");
		int ch;
		while (f && (ch = fgetc(f)) != EOF) g_saved.push_back((unsigned char)ch);
		if (f) fclose(f);
		logf("bytes %s", hexOf(g_saved).c_str());
		dumpSave();
		break;
	}
	case 3: {
		std::vector<unsigned char> bytes = g_saved;
		if ((b & 1) && !bytes.empty()) bytes[c % bytes.size()] = (unsigned char)d;
		if ((b & 2) && !bytes.empty()) bytes.resize(bytes.size() - 1 - (e % bytes.size()));
		FILE *f = fopen(g_saveFile, "wb");
		if (!bytes.empty()) fwrite(&bytes[0], 1, bytes.size(), f);
		fclose(f);
		XferLoad x;
		x.open(AsciiString(g_saveFile));
		try { g_logic.xfer(&x); logf("load"); }
		catch (XferStatus st) { logf("load threw xfer %d", (int)st); }
		catch (SaveCode sc) { logf("load threw save %d", (int)sc); }
		x.close();
		dumpSave();
		break;
	}
	case 4:
		g_logic.m_isScoringEnabled = (Bool)(a & 1); g_logic.m_showBehindBuildingMarkers = (Bool)(a >> 1 & 1);
		g_logic.m_drawIconUI = (Bool)(a >> 2 & 1); g_logic.m_showDynamicLOD = (Bool)(a >> 3 & 1);
		g_logic.m_scriptHulkMaxLifetimeOverride = (Int)b; g_logic.m_rankPointsToAddAtGameStart = (Int)c;
		g_logic.m_superweaponRestriction = (UnsignedShort)d; g_logic.m_rankLevelLimit = (Int)e;
		logf("members");
		break;
	case 5:
		if (a & 1) g_logic.setBuildableStatusOverride(&g_tt[b % 4], (BuildableStatus)(c % 5));
		else g_logic.setControlBarOverride(AsciiString(g_names[b % 6]), (Int)(c % 4), buttonOf(d % 4));
		logf("override");
		break;
	case 6: g_logic.m_thingTemplateBuildableOverrides.m.clear(); g_logic.m_controlBarOverrides.m.clear(); logf("cleared"); break;
	case 7: {
		g_hasNeutral = (Bool)(a & 1);
		Int n = (Int)(b % 5);
		PolygonTrigger::s_first = n ? &g_polys[0] : NULL;
		for (Int i = 0; i < n; ++i) { g_polys[i].m_id = (Int)((c >> (3 * i)) & 7); g_polys[i].m_next = i + 1 < n ? &g_polys[i + 1] : NULL; }
		logf("config");
		break;
	}
	case 8: dumpSave(); break;
	case 9: g_logic.m_frame = a; logf("frame"); break;
	case 10: { Object *o = nthObject(a); g_logic.destroyObject(o); g_logic.processDestroyList(); logf("destroyed"); break; }
	case 11: {
		BridgeInfo &bi = g_bridges[a % 4].m_info;
		bi.bridgeObjectID = (ObjectID)(b % 24);
		for (int i = 0; i < BRIDGE_MAX_TOWERS; ++i) bi.towerObjectID[i] = (ObjectID)((c >> (5 * i)) & 31);
		logf("bridge");
		break;
	}
	case 12: g_campaign.m_value = a; g_caves.m_value = b; g_ba.m_value = c; for (int i = 0; i < 6; ++i) g_polys[i].m_value = d + (unsigned)i; logf("snapshots"); break;
	default: logf("op?"); break;
	}
}

} // namespace glf

extern "C" int ref_glframe_run(const unsigned *cfg, const unsigned *ops, int n, char *out, int cap)
{
	using namespace glf;
	g_log.clear();
	g_flags = cfg[0];
	g_logic.m_gameMode = (Int)(cfg[1] % 7);
	g_crcInterval = (Int)cfg[2];
	REPLAY_CRC_INTERVAL = (Int)cfg[3];
	g_players = cfg[4] & 0xffff;
	g_localPlayer = (Int)(cfg[5] % 10);
	g_seedCrc = cfg[6];
	g_time = cfg[7];
	g_connected = 0xff;
	g_numPlayers = 2;
	g_objDisabled = g_objMass = g_objDrawable = 0;
	g_nextGroup = 100;
	applyFlags();
	g_logic.m_isInUpdate = FALSE;
	g_logic.m_startNewGame = FALSE;
	g_logic.m_shouldValidateCRCs = FALSE;
	g_logic.m_gamePaused = FALSE;
	g_logic.m_inputEnabledMemory = FALSE;
	g_logic.m_mouseVisibleMemory = FALSE;
	g_logic.m_forceGameStartByTimeOut = FALSE;
	g_logic.m_frame = cfg[8];
	g_logic.m_CRC = 0;
	g_logic.m_cachedCRCs.clear();
	for (int i = 0; i < glf::MAX_SLOTS; ++i) { g_logic.m_progressComplete[i] = FALSE; g_logic.m_progressCompleteTimeout[i] = 0; }
	g_logic.m_thingTemplateBuildableOverrides.m.clear();
	g_logic.m_controlBarOverrides.m.clear();
	g_cmd.m_list.clear();
	for (int i = 1; i <= 6; ++i) { g_objects[i].m_id = i; g_objects[i].m_next = i < 6 ? &g_objects[i + 1] : NULL; g_drawables[i].m_id = i; }
	g_logic.m_objList = (cfg[9] % 7 == 0) ? NULL : &g_objects[7 - cfg[9] % 7];
	for (int i = 0; i < glf::MAX_PLAYER_COUNT; ++i) g_pl[i].m_index = i;
	g_playerList.m_value = cfg[10];
	g_ai.m_value = cfg[11];
	g_pm.m_value = cfg[12];
	for (int i = 0; i < 6; ++i) g_templates[i].m_name = AsciiString(g_names[i]);
	snprintf(g_deepFile, sizeof g_deepFile, "/tmp/ref_glframe_%d.crc", (int)getpid());
	for (int i = 0; i < n; ++i)
		run_op(ops + 6 * i);
	unlink(g_deepFile);
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}

extern "C" int ref_glsave_run(const unsigned *cfg, const unsigned *ops, int n, char *out, int cap)
{
	using namespace glf;
	g_log.clear();
	for (int i = 0; i < 4; ++i) g_tt[i].m_name = AsciiString(g_tnames[i]);
	for (int i = 0; i < 6; ++i) g_templates[i].m_name = AsciiString(g_names[i]);
	g_buttons[1].m_name = AsciiString("Command_A"); g_buttons[2].m_name = AsciiString("Command_B"); g_buttons[3].m_name = AsciiString("Command_C");
	g_logic.m_objList = NULL;
	g_poolUsed = 0;
	g_nextID = 1;
	g_logic.m_frame = cfg[0];
	g_hasNeutral = TRUE;
	PolygonTrigger::s_first = NULL;
	for (int i = 0; i < 6; ++i) { g_polys[i].m_id = 0; g_polys[i].m_value = 0; g_polys[i].m_next = NULL; }
	g_campaign.m_name = "campaign"; g_caves.m_name = "caves";
	g_campaign.m_value = cfg[1]; g_caves.m_value = cfg[2]; g_ba.m_value = cfg[3];
	g_logic.m_objectTOC.clear();
	g_logic.m_isScoringEnabled = TRUE; g_logic.m_showBehindBuildingMarkers = FALSE; g_logic.m_drawIconUI = TRUE; g_logic.m_showDynamicLOD = TRUE;
	g_logic.m_scriptHulkMaxLifetimeOverride = -1; g_logic.m_rankPointsToAddAtGameStart = 0; g_logic.m_superweaponRestriction = 0; g_logic.m_rankLevelLimit = 1000;
	g_logic.m_thingTemplateBuildableOverrides.m.clear();
	g_logic.m_controlBarOverrides.m.clear();
	for (int i = 0; i < 4; ++i) { g_bridges[i].m_info.bridgeObjectID = (ObjectID)0; for (int k = 0; k < BRIDGE_MAX_TOWERS; ++k) g_bridges[i].m_info.towerObjectID[k] = (ObjectID)0; }
	g_saved.clear();
	snprintf(g_saveFile, sizeof g_saveFile, "/tmp/ref_glsave_%d.sav", (int)getpid());
	for (int i = 0; i < n; ++i)
		run_save_op(ops + 6 * i);
	unlink(g_saveFile);
	g_logic.m_objList = NULL;
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
