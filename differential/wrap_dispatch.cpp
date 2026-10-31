/* Differential-test wrapper for GameLogic::logicMessageDispatcher (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp, cut verbatim by build.rs into
**   OUT_DIR/dispatch_gen: dispatch_statics.inc (lines 96-224: the build plan's statics, doMoveTo, doSetRallyPoint,
**   getSingleObjectFromSelection) and dispatch_body.inc (line 352 to the end: logicMessageDispatcher), with one edit:
**   MSG_SELF_DESTRUCT's loop variable, read after its loop (MSVC's for-scope), declared before it.
** Both are compiled inside namespace gld, on stand-ins for everything the function calls. The stand-ins hold a small
** world (objects 1-12, up to six players, the AI's groups and the players' selections) configured by the test, answer
** the function's questions from it, and log every call that acts, in the format the test's Rust host logs. The messages
** are the real GameMessage of the message stream build (wrap_message_stream.cpp), under its names.
**
** Entry point (extern "C", tests/differential_dispatch.rs):
**   ref_dispatch_run(world, nworld, msgs, nmsgs, out, cap)  dispatches the messages (each: type, player, argument count,
**   then per argument its type and four words) and writes the log */
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
#include <string>
#include <vector>
#include <map>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#define protected public
#define private public
#include "Common/MessageStream.h"
#undef protected
#undef private

namespace gld {

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

/* ---- the constants the function names ---- */
typedef UnsignedInt ObjectID;
enum { INVALID_ID = 0 };
typedef std::vector<ObjectID> VecObjectID;
typedef Int NameKeyType;
typedef Int WeaponSlotType;
typedef Int GuardMode;
typedef Int ScienceType;
typedef Int SpecialPowerType;
typedef Int ProductionID;
typedef Int GameDifficulty;
enum { PRIMARY_WEAPON = 0 };
enum { LOCKED_TEMPORARILY = 1, LOCKED_PERMANENTLY = 2 };
enum CommandSourceType { CMD_FROM_PLAYER = 0 };
enum { NO_MAX_SHOTS_LIMIT = 0x7fffffff };
enum { SCIENCE_INVALID = -1 };
enum { DIFFICULTY_NORMAL = 1 };
enum { ALLIES = 2 };
enum { PLAYER_HUMAN = 1 };
enum RecorderModeType { RECORDERMODETYPE_RECORD, RECORDERMODETYPE_PLAYBACK, RECORDERMODETYPE_NONE };
enum ObjectStatusTypes { OBJECT_STATUS_UNDER_CONSTRUCTION = 1, OBJECT_STATUS_RECONSTRUCTING = 2 };
enum { RADAR_EVENT_INFORMATION = 1 };
enum { EVA_BeaconDetected = 1 };
#define MAX_SLOTS 8
#define CRCGEN_LOG(x)
#define NAMEKEY(s) 0

/* ---- the world ---- */
struct ObjW { Bool exists, hasAI, local, drawSel, hasExit, hasProd, underCons, recons, hasDraw; Int owner; Int beaconOf; };
ObjW g_obj[64];
struct PlW { Int defeated, beaconT, count, isLocal, observerFlag; };
PlW g_pl[6];
Int g_rel[6][6];
Int g_playerCount, g_localPlayer, g_maxBeacons, g_fps;
Bool g_inGame, g_stats, g_inGameUI, g_recorder, g_playback, g_camFollows, g_camFinished, g_mouseMoved, g_network;
Int g_netConnected, g_netSlot[8];
UnsignedInt g_cfgBits;
std::map<Int, std::vector<ObjectID> > g_groups;
std::vector<ObjectID> g_selection[6];
Int g_nextGroup, g_nextObject;
Bool w(Int bit) { return (g_cfgBits >> (bit & 31)) & 1; }

void logCoord(const char *what, const Coord3D *p)
{
	unsigned x, y, z;
	memcpy(&x, &p->x, 4); memcpy(&y, &p->y, 4); memcpy(&z, &p->z, 4);
	logf("%s %08x %08x %08x", what, x, y, z);
}

class Player;
class Object;
class AIGroup;
class Team { public: Int m_player; };
Team g_teams[6];

class Money { public: Int m_player; void deposit(UnsignedInt amount) { logf("deposit %d %u", m_player, amount); } };
Money g_money[6];
class PlayerTemplate { public: Int m_player; AsciiString getBeaconTemplate() const { char b[16]; snprintf(b, sizeof b, "B%d", g_pl[m_player].beaconT); return AsciiString(b); } };
PlayerTemplate g_ptemplates[6];
class ThingTemplate
{
public:
	Int m_id;
	Bool isEquivalentTo(const ThingTemplate *t) const { return t && t->m_id == m_id; }
	UnsignedInt calcCostToBuild(Player *p) const;
	UnicodeString getDisplayName() const { return UnicodeString(); }
	AsciiString getName() const { return AsciiString(); }
};
ThingTemplate g_things[64];

class Player
{
public:
	Int m_index;
	Int getPlayerIndex() const { return m_index; }
	void getCurrentSelectionAsAIGroup(AIGroup *g);
	void setLogicalRetaliationModeEnabled(Bool b) { logf("retaliation %d %d", m_index, (int)b); }
	Money *getMoney() { return &g_money[m_index]; }
	PlayerTemplate *getPlayerTemplate() { return &g_ptemplates[m_index]; }
	void countObjectsByThingTemplate(Int, const ThingTemplate **t, Bool, Int *count) { (void)t; *count = g_pl[m_index].count; }
	Team *getDefaultTeam() { return &g_teams[m_index]; }
	Int getRelationship(const Team *t) const { return t ? g_rel[m_index][t->m_player] : 0; }
	UnicodeString getPlayerDisplayName() const { WideChar c[2] = { (WideChar)('A' + m_index), 0 }; return UnicodeString(c); }
	Bool isPlayerObserver() const { return g_pl[m_index].observerFlag; }
	void setCurrentlySelectedAIGroup(AIGroup *) { logf("clearselection %d", m_index); }
	void transferAssetsFromThat(Player *p) { logf("transfer %d from %d", m_index, p->m_index); }
	void killPlayer() { logf("killplayer %d", m_index); }
	void processCreateTeamGameMessage(Int n, GameMessage *) { logf("team 0 %d %d", m_index, n); }
	void processSelectTeamGameMessage(Int n, GameMessage *) { logf("team 1 %d %d", m_index, n); }
	void processAddTeamGameMessage(Int n, GameMessage *) { logf("team 2 %d %d", m_index, n); }
	Int getPlayerType() const { return PLAYER_HUMAN; }
	Bool isLocalPlayer() const { return m_index == g_localPlayer; }
	void attemptToPurchaseScience(ScienceType s) { logf("science %d %d", m_index, s); }
	UnsignedInt getPlayerMask() const { return 1u << m_index; }
};
Player g_players[6];
UnsignedInt ThingTemplate::calcCostToBuild(Player *p) const { return (UnsignedInt)(m_id * 100 + p->m_index); }

class PlayerList
{
public:
	Player *getNthPlayer(Int i) { return (i >= 0 && i < g_playerCount) ? &g_players[i] : NULL; }
	Int getPlayerCount() { return g_playerCount; }
	Player *getLocalPlayer() { return (g_localPlayer >= 0 && g_localPlayer < g_playerCount) ? &g_players[g_localPlayer] : NULL; }
};
PlayerList g_playerList;
PlayerList *ThePlayerList = &g_playerList;

class AIUpdateInterface
{
public:
	ObjectID m_id;
	void executeWaypointQueue() { logf("waypointqueue %u", m_id); }
	void aiExit(Object *container, CommandSourceType);
	void queueWaypoint(const Coord3D *) {}
	void clearWaypointQueue() {}
	void aiMoveToPosition(const Coord3D *, CommandSourceType) {}
};
AIUpdateInterface g_ais[13];
class ExitInterface { public: ObjectID m_id; void setRallyPoint(const Coord3D *p) { char b[32]; snprintf(b, sizeof b, "rally %u", m_id); logCoord(b, p); } };
ExitInterface g_exits[13];
class ProductionUpdateInterface
{
public:
	ObjectID m_id;
	void cancelUpgrade(const class UpgradeTemplate *u);
	void queueCreateUnit(const ThingTemplate *t, ProductionID id) { logf("queueunit %u %d %d", m_id, t->m_id, id); }
	void cancelUnitCreate(ProductionID id) { logf("cancelunit %u %d", m_id, id); }
};
ProductionUpdateInterface g_prods[13];
class Drawable
{
public:
	ObjectID m_id;
	Bool isSelected() const { return g_obj[m_id].drawSel; }
	void clearCaptionText() { logf("caption %u", m_id); }
	void setCaptionText(const UnicodeString &s)
	{
		std::string t;
		char b[8];
		for (int i = 0; i < s.getLength(); ++i) { snprintf(b, sizeof b, " %04x", (unsigned)s.getCharAt(i)); t += b; }
		logf("caption %u%s", m_id, t.c_str());
	}
	class ClientUpdateModule **getClientUpdateModules();
};
Drawable g_draws[64];

class Object
{
public:
	ObjectID m_id;
	ObjectID getID() const { return m_id; }
	AIUpdateInterface *getAIUpdateInterface() { return g_obj[m_id].hasAI ? &g_ais[m_id] : NULL; }
	void leaveGroup() {}
	void releaseWeaponLock(Int lock) { logf("releaselock %u %d", m_id, lock); }
	Player *getControllingPlayer() const { Int o = g_obj[m_id].owner; return (o >= 0 && o < g_playerCount) ? &g_players[o] : NULL; }
	Bool isLocallyControlled() const { return g_obj[m_id].local; }
	Bool m_hasPos; Coord3D m_pos;
	const Coord3D *getPosition() const { static Coord3D c; if (m_hasPos) return &m_pos; c.x = (Real)m_id * 10.0f; c.y = (Real)m_id * 20.0f; c.z = 0.0f; return &c; }
	const ThingTemplate *getTemplate() const { return &g_things[g_obj[m_id].beaconOf >= 0 ? g_obj[m_id].beaconOf : 63]; }
	Drawable *getDrawable() const { return g_obj[m_id].hasDraw ? &g_draws[m_id] : NULL; }
	ExitInterface *getObjectExitInterface() { return g_obj[m_id].hasExit ? &g_exits[m_id] : NULL; }
	ProductionUpdateInterface *getProductionUpdateInterface() { return g_obj[m_id].hasProd ? &g_prods[m_id] : NULL; }
	Bool testStatus(ObjectStatusTypes s) const { return s == OBJECT_STATUS_UNDER_CONSTRUCTION ? g_obj[m_id].underCons : g_obj[m_id].recons; }
	void kill() { logf("kill %u", m_id); }
	void setPosition(const Coord3D *p) { char b[32]; snprintf(b, sizeof b, "setposition %u", m_id); logCoord(b, p); m_hasPos = TRUE; m_pos = *p; }
	void setProducer(const Object *) { logf("setproducer %u", m_id); }
};
Object g_objects[64];
void AIUpdateInterface::aiExit(Object *container, CommandSourceType) { logf("aiexit %u %u", m_id, container->m_id); }

class ClientUpdateModule { public: virtual ~ClientUpdateModule() {} NameKeyType getModuleNameKey() const { return NAMEKEY("BeaconClientUpdate"); } };
class BeaconClientUpdate : public ClientUpdateModule { public: ObjectID m_id; void hideBeacon() { logf("hidebeacon %u", m_id); } };
/* Upstream finds the beacon module by its name key (NAMEKEY("BeaconClientUpdate")); this drawable's one client module is
** the beacon's: getModuleNameKey answers the key the function asks for */
BeaconClientUpdate g_beaconModules[64];
ClientUpdateModule *g_moduleLists[64][2];
ClientUpdateModule **Drawable::getClientUpdateModules() { g_moduleLists[m_id][0] = &g_beaconModules[m_id]; g_moduleLists[m_id][1] = NULL; return g_moduleLists[m_id]; }

class UpgradeTemplate { public: Int m_key; };
UpgradeTemplate g_upgrades[1];
void ProductionUpdateInterface::cancelUpgrade(const UpgradeTemplate *u) { logf("cancelupgrade %u %d", m_id, u->m_key); }

class AIGroup
{
public:
	Int m_id;
	const VecObjectID &getAllIDs() const { return g_groups[m_id]; }
	Bool isEmpty() const { return g_groups[m_id].empty(); }
	void add(Object *o) { g_groups[m_id].push_back(o->m_id); logf("groupadd %d %u", m_id, o->m_id); }
	Bool removeAnyObjectsNotOwnedByPlayer(const Player *p)
	{
		std::vector<ObjectID> &v = g_groups[m_id];
		for (size_t i = 0; i < v.size();) {
			Int o = g_obj[v[i]].owner;
			if (!p || o != p->m_index) { logf("groupremove %d %u", m_id, v[i]); v.erase(v.begin() + i); if (v.empty()) { logf("group %d emptied", m_id); return TRUE; } }
			else ++i;
		}
		return FALSE;
	}
	Bool setWeaponLockForGroup(WeaponSlotType slot, Int lock) { Bool r = w(m_id + slot); logf("group %d weaponlock %d %d: %d", m_id, slot, lock, (int)r); return r; }
	void releaseWeaponLockForGroup(Int lock) { logf("group %d releaselock %d", m_id, lock); }
	void groupAttackPosition(const Coord3D *p, Int max, CommandSourceType) { char b[48]; snprintf(b, sizeof b, "group %d attackpos %d", m_id, max); if (p) logCoord(b, p); else logf("%s none", b); }
	void groupAttackObject(Object *o, Int max, CommandSourceType) { logf("group %d attackobj %u %d", m_id, o->m_id, max); }
	void groupForceAttackObject(Object *o, Int max, CommandSourceType) { logf("group %d forceattackobj %u %d", m_id, o->m_id, max); }
	void groupAttackMoveToPosition(const Coord3D *p, Int max, CommandSourceType) { char b[48]; snprintf(b, sizeof b, "group %d attackmove %d", m_id, max); logCoord(b, p); }
	void groupMoveToPosition(const Coord3D *p, Bool wp, CommandSourceType) { char b[48]; snprintf(b, sizeof b, "group %d move %d", m_id, (int)wp); logCoord(b, p); }
	void groupCombatDrop(Object *t, const Coord3D &p, CommandSourceType) { char b[48]; snprintf(b, sizeof b, "group %d combatdrop %u", m_id, t ? t->m_id : 0); logCoord(b, &p); }
	void setMineClearingDetail(Bool) { logf("group %d mineclearing", m_id); }
	void groupDoSpecialPower(UnsignedInt id, UnsignedInt opt) { logf("group %d sp %u %u", m_id, id, opt); }
	void groupDoSpecialPowerAtLocation(UnsignedInt id, const Coord3D *p, Real a, Object *o, UnsignedInt opt)
	{
		unsigned ab; memcpy(&ab, &a, 4);
		char b[96]; snprintf(b, sizeof b, "group %d sploc %u %08x %u %u", m_id, id, ab, o ? o->m_id : 0, opt); logCoord(b, p);
	}
	void groupDoSpecialPowerAtObject(UnsignedInt id, Object *t, UnsignedInt opt) { logf("group %d spobj %u %u %u", m_id, id, t->m_id, opt); }
	void groupOverrideSpecialPowerDestination(SpecialPowerType t, const Coord3D *p, CommandSourceType) { char b[48]; snprintf(b, sizeof b, "group %d override %d", m_id, t); logCoord(b, p); }
	void groupGuardPosition(const Coord3D *p, GuardMode m, CommandSourceType) { char b[48]; snprintf(b, sizeof b, "group %d guardpos %d", m_id, m); logCoord(b, p); }
	void groupGuardObject(Object *o, GuardMode m, CommandSourceType) { logf("group %d guardobj %u %d", m_id, o->m_id, m); }
	void groupIdle(CommandSourceType) { logf("group %d idle", m_id); }
	void groupScatter(CommandSourceType) { logf("group %d scatter", m_id); }
	void groupCreateFormation(CommandSourceType) { logf("group %d formation", m_id); }
	void groupCheer(CommandSourceType) { logf("group %d cheer", m_id); }
	void groupEnter(Object *o, CommandSourceType) { logf("group %d enter %u", m_id, o->m_id); }
	void groupEvacuate(CommandSourceType) { logf("group %d evacuate", m_id); }
	void groupExecuteRailedTransport(CommandSourceType) { logf("group %d railed", m_id); }
	void groupHackInternet(CommandSourceType) { logf("group %d hack", m_id); }
	void groupGetRepaired(Object *o, CommandSourceType) { logf("group %d getrepaired %u", m_id, o->m_id); }
	void groupDock(Object *o, CommandSourceType) { logf("group %d dock %u", m_id, o->m_id); }
	void groupGetHealed(Object *o, CommandSourceType) { logf("group %d gethealed %u", m_id, o->m_id); }
	void groupRepair(Object *o, CommandSourceType) { logf("group %d repair %u", m_id, o->m_id); }
	void groupResumeConstruction(Object *o, CommandSourceType) { logf("group %d resume %u", m_id, o->m_id); }
	Bool isIdle() const { Bool r = w(m_id + 7); logf("group %d isidle: %d", m_id, (int)r); return r; }
	void queueUpgrade(const UpgradeTemplate *u) { logf("group %d queueupgrade %d", m_id, u->m_key); }
	void groupSell(CommandSourceType) { logf("group %d sell", m_id); }
	void groupToggleOvercharge(CommandSourceType) { logf("group %d overcharge", m_id); }
	Int getID() const { return m_id; }
};
std::map<Int, AIGroup *> g_groupObjs;

void Player::getCurrentSelectionAsAIGroup(AIGroup *g)
{
	for (size_t i = 0; i < g_selection[m_index].size(); ++i) g_groups[g->m_id].push_back(g_selection[m_index][i]);
	logf("selection %d into %d", m_index, g->m_id);
}

class LocomotorTemplate;
class LocomotorSet { public: void addLocomotor(const LocomotorTemplate *) {} };
class LocomotorStore { public: const LocomotorTemplate *findLocomotorTemplate(NameKeyType) { return NULL; } };
LocomotorStore g_locoStore;
LocomotorStore *TheLocomotorStore = &g_locoStore;
class Pathfinder { public: Bool clientSafeQuickDoesPathExist(const LocomotorSet &, const Coord3D *from, const Coord3D *to) { (void)from; return ((int)to->x & 3) != 0; } };
Pathfinder g_pathfinder;
class AI
{
public:
	AIGroup *createGroup() { AIGroup *g = new AIGroup; g->m_id = g_nextGroup++; g_groups[g->m_id]; logf("creategroup %d", g->m_id); return g; }
	void destroyGroup(AIGroup *g) { logf("destroygroup %d", g->m_id); g_groups.erase(g->m_id); }
	Pathfinder *pathfinder() { return &g_pathfinder; }
};
AI g_ai;
AI *TheAI = &g_ai;

class StatsCollector { public: void collectMsgStats(const GameMessage *m) { logf("stats %d", (int)m->getType()); } };
StatsCollector g_stats_c;
StatsCollector *TheStatsCollector = NULL;
class GameTextInterface { public: AsciiString m_last; UnicodeString fetch(const char *label) { m_last = AsciiString(label); return UnicodeString(); } };
GameTextInterface g_text;
GameTextInterface *TheGameText = &g_text;
class InGameUI
{
public:
	void message(const UnicodeString &) { logf("ui %s", g_text.m_last.str()); }
	void clearPopupMessageData() { logf("clearpopup"); }
	void deselectAllDrawables() { logf("deselectall"); }
	void selectDrawable(Drawable *d) { logf("selectdrawable %u", d->m_id); }
};
InGameUI g_ui;
InGameUI *TheInGameUI = NULL;
class AudioEventRTS
{
public:
	AsciiString m_name; Bool m_hasPos; Coord3D m_pos; Int m_player; ObjectID m_obj;
	AudioEventRTS(AsciiString n) : m_name(n), m_hasPos(FALSE), m_player(-1), m_obj(0) { m_pos.x = m_pos.y = m_pos.z = 0; }
	AudioEventRTS(const char *n) : m_name(n), m_hasPos(FALSE), m_player(-1), m_obj(0) { m_pos.x = m_pos.y = m_pos.z = 0; }
	void setPosition(const Coord3D *p) { m_hasPos = TRUE; m_pos = *p; }
	void setPlayerIndex(Int i) { m_player = i; }
	void setObjectID(ObjectID id) { m_obj = id; }
};
class AudioManager
{
public:
	void addAudioEvent(AudioEventRTS *e)
	{
		char b[96];
		snprintf(b, sizeof b, "audio %s player %d obj %u", e->m_name.str(), e->m_player, e->m_obj);
		if (e->m_hasPos) logCoord(b, &e->m_pos); else logf("%s nopos", b);
	}
};
AudioManager g_audio;
AudioManager *TheAudio = &g_audio;
/* the observer's look-at player: world bits 28-30 (7: none) */
class ControlBar
{
public:
	void markUIDirty() { logf("markuidirty"); }
	Player *getObserverLookAtPlayer() { Int i = (Int)((g_cfgBits >> 28) & 7); return i < g_playerCount ? &g_players[i] : NULL; }
};
ControlBar g_controlBar;
ControlBar *TheControlBar = &g_controlBar;
class UpgradeCenter { public: const UpgradeTemplate *findUpgradeByKey(NameKeyType k) { if (!w(k)) return NULL; g_upgrades[0].m_key = k; return &g_upgrades[0]; } };
UpgradeCenter g_upgradeCenter;
UpgradeCenter *TheUpgradeCenter = &g_upgradeCenter;
class ThingFactory
{
public:
	const ThingTemplate *findByTemplateID(Int id) { return (id >= 0 && id < 32 && w(id + 3)) ? &g_things[id] : NULL; }
	const ThingTemplate *findTemplate(const AsciiString &name)
	{
		Int id = atoi(name.str() + 1);
		return (id >= 0 && id < 32) ? &g_things[id] : NULL;
	}
	Object *newObject(const ThingTemplate *t, Team *team)
	{
		Int id = g_nextObject++;
		g_objects[id].m_id = (ObjectID)id;
		g_objects[id].m_hasPos = FALSE;
		g_obj[id].hasDraw = TRUE;
		g_draws[id].m_id = (ObjectID)id;
		g_beaconModules[id].m_id = (ObjectID)id;
		logf("newobject %u template %d team %d", (unsigned)id, t->m_id, team->m_player);
		return &g_objects[id];
	}
};
ThingFactory g_thingFactory;
ThingFactory *TheThingFactory = &g_thingFactory;
class BuildAssistant
{
public:
	void buildObjectNow(Object *c, const ThingTemplate *t, const Coord3D *p, Real a, Player *owner)
	{
		unsigned ab; memcpy(&ab, &a, 4);
		char b[96]; snprintf(b, sizeof b, "build %u %d %08x %d", c->m_id, t->m_id, ab, owner ? owner->m_index : -1); logCoord(b, p);
	}
	void buildObjectLineNow(Object *c, const ThingTemplate *t, const Coord3D *p, const Coord3D *e, Real a, Player *owner)
	{
		unsigned ab; memcpy(&ab, &a, 4);
		char b[96]; snprintf(b, sizeof b, "buildline %u %d %08x %d", c->m_id, t->m_id, ab, owner ? owner->m_index : -1); logCoord(b, p); logCoord(" to", e);
	}
};
BuildAssistant g_build;
BuildAssistant *TheBuildAssistant = &g_build;
class TerrainLogic
{
public:
	void getExtent(Region3D *r) { r->lo.x = 0; r->lo.y = 0; r->lo.z = 0; r->hi.x = 1000; r->hi.y = 800; r->hi.z = 100; }
	Coord3D findClosestEdgePoint(const Coord3D *p)
	{
		Coord3D c = *p;
		c.x = c.x < 0 ? 0 : (c.x > 1000 ? 1000 : c.x);
		c.y = c.y < 0 ? 0 : (c.y > 800 ? 800 : c.y);
		return c;
	}
};
TerrainLogic g_terrain;
TerrainLogic *TheTerrainLogic = &g_terrain;
class VictoryConditions { public: Bool hasSinglePlayerBeenDefeated(Player *p) { return g_pl[p->m_index].defeated; } };
VictoryConditions g_victory;
VictoryConditions *TheVictoryConditions = &g_victory;
class MultiplayerSettings { public: Int getMaxBeaconsPerPlayer() { return g_maxBeacons; } };
MultiplayerSettings g_mp;
MultiplayerSettings *TheMultiplayerSettings = &g_mp;
class Radar { public: void createEvent(const Coord3D *p, Int) { logCoord("radar", p); } };
Radar g_radar;
Radar *TheRadar = &g_radar;
class Eva { public: void setShouldPlay(Int) { logf("eva beacon"); } };
Eva g_eva;
Eva *TheEva = &g_eva;
class Recorder
{
public:
	RecorderModeType getMode() { return g_playback ? RECORDERMODETYPE_PLAYBACK : RECORDERMODETYPE_RECORD; }
	void handleCRCMessage(UnsignedInt crc, Int player, Bool fromPlayback) { logf("recordercrc %X %d %d", crc, player, (int)fromPlayback); }
};
Recorder g_recorderObj;
Recorder *TheRecorder = NULL;
class GlobalData { public: Bool m_useCameraInReplay; Int m_framesPerSecondLimit; Bool m_useFpsLimit; };
GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;
GlobalData *TheWritableGlobalData = &g_gd;
class GameEngine { public: void setFramesPerSecondLimit(Int f) { logf("fps %d", f); } };
GameEngine g_engine;
GameEngine *TheGameEngine = &g_engine;
class ViewLocation
{
public:
	Coord3D m_pos; Real m_angle, m_pitch, m_zoom;
	void init(Real x, Real y, Real z, Real a, Real p, Real zm) { m_pos.x = x; m_pos.y = y; m_pos.z = z; m_angle = a; m_pitch = p; m_zoom = zm; }
};
class View
{
public:
	Bool isCameraMovementFinished() { return g_camFinished; }
	void setLocation(const ViewLocation *l)
	{
		unsigned a, p, z; memcpy(&a, &l->m_angle, 4); memcpy(&p, &l->m_pitch, 4); memcpy(&z, &l->m_zoom, 4);
		char b[64]; snprintf(b, sizeof b, "camera %08x %08x %08x", a, p, z); logCoord(b, &l->m_pos);
	}
};
View g_view;
View *TheTacticalView = &g_view;
class LookAtTranslator { public: Bool hasMouseMovedRecently() { return g_mouseMoved; } void setCurrentPos(const ICoord2D &) {} };
LookAtTranslator g_lookAt;
LookAtTranslator *TheLookAtTranslator = &g_lookAt;
class Mouse
{
public:
	enum MouseCursor { NONE = 0 };
	Int m_cursor;
	void setCursor(MouseCursor c) { m_cursor = (Int)c; }
	void setPosition(Int x, Int y) { logf("mouse %d %d %d", m_cursor, x, y); }
};
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;
class NetworkInterface
{
public:
	UnicodeString getPlayerName(Int slot) { WideChar c[2] = { (WideChar)('A' + g_netSlot[slot]), 0 }; return UnicodeString(c); }
	Bool isPlayerConnected(Int slot) { return (g_netConnected >> slot) & 1; }
};
NetworkInterface g_net;
NetworkInterface *TheNetwork = NULL;

class GameLogic
{
public:
	Bool m_shouldValidateCRCs;
	UnsignedInt m_cachedCRCs[MAX_SLOTS];
	Bool isInGame() { return g_inGame; }
	Object *findObjectByID(ObjectID id) { return (id >= 1 && id <= 12 && g_obj[id].exists) ? &g_objects[id] : NULL; }
	void selectObject(Object *o, Bool createNew, UnsignedInt mask) { logf("select %u %d %X", o->m_id, (int)createNew, mask); }
	void deselectObject(Object *o, UnsignedInt mask) { logf("deselect %u %X", o->m_id, mask); }
	void destroyObject(Object *o) { logf("destroyobject %u", o->m_id); }
	void clearGameData(Bool showScoreScreen = TRUE) { (void)showScoreScreen; logf("cleargamedata"); }
	void prepareNewGame(Int mode, GameDifficulty diff, Int rank) { logf("preparenewgame %d %d %d", mode, diff, rank); }
	void startNewGame(Bool) { logf("startnewgame"); }
	void logicMessageDispatcher(GameMessage *msg, void *userData);
};
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;

/* the network's player name test: the slot's configured player is the player (getPlayerName(i) == getPlayerDisplayName()) */
inline Bool operator==(const UnicodeString &a, const UnicodeString &b) { return !(a != b); }

/* ---- the upstream code ---- */
#include "dispatch_statics.inc"
#include "dispatch_body.inc"

} // namespace gld

extern "C" int ref_dispatch_run(const unsigned *world, int nworld, const unsigned *msgs, int nmsgs, char *out, int cap)
{
	using namespace gld;
	g_log.clear();
	(void)nworld;
	/* world: [0] flags, [1] bits, [2] player count, [3] local player, [4] max beacons, [5] fps, [6] network connected,
	** [7..18] the twelve objects (bits), [19..24] six players (bits), [25..30] the relationship rows (two bits each),
	** [31..36] the selections (bit i: object i+1), [37] the network slots (four bits each: player + 1) */
	UnsignedInt f = world[0];
	g_inGame = f & 1; g_stats = (f >> 1) & 1; g_inGameUI = (f >> 2) & 1; g_recorder = (f >> 3) & 1; g_playback = (f >> 4) & 1;
	g_camFollows = (f >> 5) & 1; g_camFinished = (f >> 6) & 1; g_mouseMoved = (f >> 7) & 1; g_network = (f >> 8) & 1;
	g_gd.m_useCameraInReplay = g_camFollows;
	g_cfgBits = world[1];
	g_playerCount = (Int)world[2]; g_localPlayer = (Int)world[3]; g_maxBeacons = (Int)world[4]; g_fps = (Int)world[5];
	g_gd.m_framesPerSecondLimit = g_fps;
	g_netConnected = (Int)world[6];
	for (int i = 1; i <= 12; ++i) {
		UnsignedInt b = world[6 + i];
		ObjW &o = g_obj[i];
		o.exists = b & 1; o.hasAI = (b >> 1) & 1; o.local = (b >> 2) & 1; o.drawSel = (b >> 3) & 1; o.hasExit = (b >> 4) & 1;
		o.hasProd = (b >> 5) & 1; o.underCons = (b >> 6) & 1; o.recons = (b >> 7) & 1; o.hasDraw = (b >> 8) & 1;
		o.owner = (Int)((b >> 9) & 7) - 1; o.beaconOf = (Int)((b >> 12) & 63) - 1;
		g_objects[i].m_id = i; g_objects[i].m_hasPos = FALSE; g_ais[i].m_id = i; g_exits[i].m_id = i; g_prods[i].m_id = i; g_draws[i].m_id = i; g_beaconModules[i].m_id = i;
	}
	for (int p = 0; p < 6; ++p) {
		UnsignedInt b = world[19 + p];
		g_pl[p].defeated = b & 1; g_pl[p].beaconT = (Int)((b >> 1) & 63) - 1; g_pl[p].count = (Int)((b >> 7) & 7);
		g_pl[p].observerFlag = (b >> 10) & 1;
		g_players[p].m_index = p; g_teams[p].m_player = p; g_money[p].m_player = p; g_ptemplates[p].m_player = p;
		for (int q = 0; q < 6; ++q) g_rel[p][q] = (Int)((world[25 + p] >> (2 * q)) & 3);
		g_selection[p].clear();
		for (int i = 0; i < 12; ++i) if ((world[31 + p] >> i) & 1) g_selection[p].push_back((gld::ObjectID)(i + 1));
	}
	for (int s = 0; s < 8; ++s) g_netSlot[s] = (Int)((world[37] >> (4 * s)) & 15) - 1;
	for (int t = 0; t < 64; ++t) g_things[t].m_id = t;
	g_groups.clear();
	g_nextGroup = 100;
	g_nextObject = 20;
	g_logic.m_shouldValidateCRCs = FALSE;
	memset(g_logic.m_cachedCRCs, 0, sizeof g_logic.m_cachedCRCs);
	theBuildPlan = false;
	thePlanSubjectCount = 0;
	TheStatsCollector = g_stats ? &g_stats_c : NULL;
	/* the game always has an in-game UI and a recorder: upstream dereferences both unchecked */
	TheInGameUI = &g_ui;
	TheRecorder = &g_recorderObj;
	TheNetwork = g_network ? &g_net : NULL;
	const unsigned *m = msgs;
	for (int i = 0; i < nmsgs; ++i) {
		GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)(Int)m[0]);
		msg->friend_setPlayerIndex((Int)m[1]);
		unsigned na = m[2];
		m += 3;
		for (unsigned k = 0; k < na; ++k) {
			GameMessageArgument *a = msg->allocArg();
			memcpy(&a->m_data, m + 1, 16);
			a->m_type = (GameMessageArgumentDataType)m[0];
			m += 5;
		}
		logf("msg %d", (int)msg->getType());
		g_logic.logicMessageDispatcher(msg, NULL);
		msg->deleteInstance();
	}
	std::string crcs;
	char b[16];
	for (int s = 0; s < MAX_SLOTS; ++s) { snprintf(b, sizeof b, " %X", g_logic.m_cachedCRCs[s]); crcs += b; }
	logf("end plan %d validate %d crcs%s", (int)theBuildPlan, (int)g_logic.m_shouldValidateCRCs, crcs.c_str());
	int len = (int)g_log.size();
	if (len <= cap) memcpy(out, g_log.data(), len);
	return len;
}
