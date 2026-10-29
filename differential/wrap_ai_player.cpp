/* Differential-test wrapper for AIPlayer.cpp (Phase 1 layer 10, p1w13-sonnet, batch A28; upstream commit 0a05454d).
**
** build.rs cuts verbatim into OUT_DIR/ai_player_gen:
**   ai_player_dlink.inc  GameCommon.h's DLINK_ITERATOR, MAKE_DLINK_HEAD and MAKE_DLINK (the intrusive lists of the queues)
**   ai_player_h.inc      AIPlayer.h from `enum { INVALID_SKILLSET_SELECTION` to its last `};` (WorkOrder, TeamInQueue, AIPlayer)
**   ai_player_cpp.inc    AIPlayer.cpp from `#define SUPPLY_CENTER_CLOSE_DIST` to the end
** and they are compiled here, in the namespace ap, over a world of this file's own: everything AIPlayer.cpp reaches outside
** itself (Player, PlayerList, ThingTemplate and ThingFactory, Object and its AI, supply truck, dozer, production, body, dock,
** rebuild hole, exit and command set; Team, TeamPrototype, TeamFactory; ScriptEngine, BuildAssistant, TerrainLogic, TerrainVisual,
** the partition's closest-object query with its filters, the pathfinder, UpgradeCenter, ScienceStore, ControlBar, GameLogic,
** GlobalData, NameKeyGenerator, Dict, AsciiString, Xfer) is a stand-in answering from the scene (ApScene, the same layout as
** src/differential.rs's) by the rules tests/differential_ai_player.rs's host follows, rule for rule. Every call that changes
** something, and every question whose order matters (isLocationLegalToBuild, the random values), is logged (AP_LOG_*).
** BuildListInfo is SidesList.h's members and inline functions (the ones AIPlayer.cpp calls), copied.
**
** ref_ai_player_run(scene, ops, nOps, out, cap, &outLen, log, logCap, &logLen): a new AIPlayer for player 0, then each op
** (ApOp); after each op its answer words and a dump of the AIPlayer and of the world. */
#include "Lib/BaseType.h"
#include <math.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdarg.h>
#include <list>
#include <vector>
#include <string>
#include <algorithm>

namespace ap {

#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_CRASH(m)
#define DEBUG_LOG(m)
#define CRCGEN_LOG(m)
#define BOGUSPTR(p) (false)

enum {
	AP_OBJECTS = 48, AP_TEMPLATES = 10, AP_PROTOS = 6, AP_TEAMS = 64, AP_PLAYERS = 3, AP_BUILD = 10, AP_SIDES = 2, AP_UPGRADES = 4,
};
typedef Int ObjectID;
#define INVALID_ID 0
typedef UnsignedInt TeamID;
#define TEAM_ID_INVALID 0
typedef Int PlayerIndex;
typedef Int ScienceType;
#define SCIENCE_INVALID ((ScienceType)-1)
typedef Int NameKeyType;
typedef UnsignedByte XferVersion;
enum XferMode { XFER_INVALID = 0, XFER_SAVE, XFER_LOAD, XFER_CRC };
enum SaveCode { SC_INVALID_DATA = 6 };
enum GameDifficulty { DIFFICULTY_EASY = 0, DIFFICULTY_NORMAL, DIFFICULTY_HARD };
enum Relationship { ENEMIES = 0, NEUTRAL, ALLIES };
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI, CMD_FROM_DOZER };
enum GuardMode { GUARDMODE_NORMAL = 0 };
enum BodyDamageType { BODY_PRISTINE = 0, BODY_DAMAGED, BODY_REALLYDAMAGED, BODY_RUBBLE };
enum DozerTask { DOZER_TASK_INVALID = -1, DOZER_TASK_BUILD = 0, DOZER_TASK_REPAIR, DOZER_TASK_FORTIFY };
enum UpgradeType { UPGRADE_TYPE_PLAYER = 0, UPGRADE_TYPE_OBJECT };
enum SpecialPowerType { SPECIAL_INVALID = 0, SPECIAL_SNEAK_ATTACK = 7 };
enum LegalBuildCode { LBC_OK = 0, LBC_RESTRICTED_TERRAIN };
enum AIDebugOptions { AI_DEBUG_NONE = 0, AI_DEBUG_PATHS, AI_DEBUG_TERRAIN };
enum { ALLOW_SAME_PLAYER = 0x01, ALLOW_ALLIES = 0x02, ALLOW_ENEMIES = 0x04, ALLOW_NEUTRAL = 0x08 };
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D, FROM_BOUNDINGSPHERE_2D, FROM_BOUNDINGSPHERE_3D };
enum { MAX_COMMANDS_PER_SET = 18 };
#define LOGICFRAMES_PER_SECOND 30
#define PATHFIND_CELL_SIZE_F 10.0f
/* the kinds the AI player asks about (a bit each in a template's kinds) */
enum KindOfType {
	KINDOF_STRUCTURE = 0, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_REBUILD_HOLE, KINDOF_SUPPLY_SOURCE, KINDOF_CASH_GENERATOR,
	KINDOF_COMMANDCENTER, KINDOF_FS_SUPERWEAPON, KINDOF_FS_BASE_DEFENSE, KINDOF_TECH_BASE_DEFENSE, KINDOF_VEHICLE, KINDOF_INFANTRY,
	KINDOF_AIRCRAFT, KINDOF_CONSERVATIVE_BUILDING, KINDOF_BRIDGE,
};
enum ObjectStatusTypes { OBJECT_STATUS_UNDER_CONSTRUCTION = 0, OBJECT_STATUS_SOLD, OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_STEALTHED,
	OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED };
struct KindOfMaskType { unsigned m; };
#define MAKE_KINDOF_MASK(k) (KindOfMaskType{ 1u << (k) })
const KindOfMaskType KINDOFMASK_NONE = { 0 };
struct ObjectStatusMaskType { unsigned m; Bool test(ObjectStatusTypes s) const { return (m >> s) & 1; } };
#define MAKE_OBJECT_STATUS_MASK(a) (ObjectStatusMaskType{ 1u << (a) })
#define MAKE_OBJECT_STATUS_MASK2(a, b) (ObjectStatusMaskType{ (1u << (a)) | (1u << (b)) })

/* ---- the scene (src/differential.rs ApScene, field for field) */
struct ApObj
{
	Int id, tmpl, player, team, flags, status;	/* id 0: no object; player -1: none; status bits ObjectStatusTypes */
	Real pos[3];
	Real radius;
	Int builder, spawner, reconstructed, dock, boxes, prodCount, damageState, damageStamp, cmdButtons;	/* cmdButtons -1: no set */
	Real rally[3];
};
enum {
	AP_F_AI = 1, AP_F_IDLE = 2, AP_F_SUPPLY_AI = 4, AP_F_FERRYING = 8, AP_F_FORCED = 16, AP_F_DOZER_AI = 32, AP_F_TASK = 64,
	AP_F_TASK_BUILD = 128, AP_F_PRODUCTION = 256, AP_F_BODY = 512, AP_F_LAST_DAMAGE = 1024, AP_F_NO_EFFECT = 2048,
	AP_F_CENTER_DOCK = 4096, AP_F_WAREHOUSE = 8192, AP_F_HOLE = 16384, AP_F_EXIT = 32768, AP_F_RALLY = 65536, AP_F_ABOVE = 131072,
	AP_F_DEAD = 262144, AP_F_INSIGNIFICANT = 524288,
	/* the flags a world op may toggle (the others say which interfaces and modules an object has) */
	AP_MUTABLE_FLAGS = AP_F_IDLE | AP_F_FERRYING | AP_F_FORCED | AP_F_TASK | AP_F_TASK_BUILD | AP_F_LAST_DAMAGE | AP_F_NO_EFFECT | AP_F_RALLY
		| AP_F_ABOVE | AP_F_DEAD | AP_F_INSIGNIFICANT,
};
struct ApTmpl { Int kinds, cost, equiv, newFlags; Real radius, angle; };	/* newFlags: a new object's flags (buildObjectNow) */
struct ApProto
{
	Int player, maxInstances, priority, idleFrames, flags, numUnits, condition;
	Int unitTmpl[4], unitMin[4], unitMax[4];	/* unitTmpl -1: a name no template has */
	Real home[3];
};
enum { AP_P_REINFORCE = 1, AP_P_HOME = 2, AP_P_ACTIONS = 4, AP_P_SINGLETON = 8, AP_P_CONDITION = 16 };
struct ApTeam { Int id, proto, active, idle; };	/* id 0: no team */
struct ApPlayer { Int money, flags, side, attackedFrame, sciencePoints, defaultTeam; };
enum { AP_PL_UNITS = 1, AP_PL_BASE = 2 };
struct ApBuild
{
	Int tmpl, objectId, timestamp, numRebuilds, flags, desired, current, health, name, script;	/* tmpl -1: empty, -2: unknown */
	Real loc[3], angle, rally[2];
};
enum { AP_B_INITIALLY = 1, AP_B_SUPPLY = 2, AP_B_UNDER_CONSTRUCTION = 4, AP_B_UNSELLABLE = 8, AP_B_PRIORITY = 16 };
struct ApSide { Int side, easy, normal, hard; Int numSkills[5]; Int skills[5][4]; };
struct ApScene
{
	ApObj o[AP_OBJECTS]; ApTmpl t[AP_TEMPLATES]; ApProto pr[AP_PROTOS]; ApTeam tm[AP_TEAMS]; ApPlayer pl[AP_PLAYERS];
	ApBuild b[AP_BUILD]; ApSide sides[AP_SIDES];
	Int numBuild, frame, seed, debugAI, boxValue, difficulty, skirmish, enemy, nextObjectId, nextTeamId, legalPct;
	Int rel[AP_PLAYERS][AP_PLAYERS];
	Real extent[4];
	/* the AI data */
	Int rebuildDelaySeconds, resourcesPoor, resourcesWealthy;
	Real structureSeconds, structuresPoorMod, structuresWealthyMod, teamResourcesToBuild, maxRecruitDistance, safeRadius,
		teamSeconds, teamPoorMod, teamWealthyMod;
};
struct ApOp { Int kind, a, b, c; Real x, y, z, w; };

/* ---- the scene's state as the run changes it */
ApScene W;
unsigned g_seed, g_rand;
std::vector<unsigned> *g_log;
unsigned bitsOf(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
void L(std::initializer_list<unsigned> w) { g_log->insert(g_log->end(), w); }
enum {
	AP_LOG_RANDOM = 1, AP_LOG_DEBUG, AP_LOG_CACHE, AP_LOG_SCRIPT, AP_LOG_ACTION, AP_LOG_CLEAR_FLAGS, AP_LOG_CAN_BUILD, AP_LOG_PURCHASE,
	AP_LOG_ADD_BUILD, AP_LOG_ADD_PRIORITY, AP_LOG_SET_POS, AP_LOG_SET_TEAM, AP_LOG_CLEAR_STATUS, AP_LOG_MAP_PROPS, AP_LOG_UPGRADE_MODULES,
	AP_LOG_AI, AP_LOG_FORCE_WANTING, AP_LOG_QUEUE_UNIT, AP_LOG_QUEUE_UPGRADE, AP_LOG_TEAM_ACTIVE, AP_LOG_TEAM_TRANSFER, AP_LOG_TEAM_DELETE,
	AP_LOG_TEAM_GUARD, AP_LOG_TEAM_CREATE, AP_LOG_BUILD_NOW, AP_LOG_LEGAL, AP_LOG_BIBS, AP_LOG_ADJUST,
};
/* AP_LOG_AI's commands */
enum { AP_C_DOCK = 1, AP_C_IDLE, AP_C_MOVE, AP_C_RESUME, AP_C_REPAIR, AP_C_EXIT_PATH, AP_C_JOIN };
}	/* namespace ap */

/* the answer both sides compute for the questions with no natural state */
extern "C" unsigned apAnswer(unsigned seed, unsigned kind, unsigned a, unsigned b)
{
	unsigned x = seed ^ (kind * 0x85ebca6bu);
	x ^= a; x *= 0x9E3779B1u; x ^= x >> 15;
	x ^= b; x *= 0x9E3779B1u; x ^= x >> 15;
	return x;
}

namespace ap {
Bool ans(unsigned kind, unsigned a, unsigned b) { return (apAnswer(g_seed, kind, a, b) & 3) != 0; }

/* ---- AsciiString, Dict, the name keys */
class AsciiString
{
public:
	std::string s;
	static AsciiString TheEmptyString;
	AsciiString() {}
	AsciiString(const char *c) : s(c ? c : "") {}
	const char *str() const { return s.c_str(); }
	Bool isEmpty() const { return s.empty(); }
	void concat(const char *c) { s += c; }
	void concat(const AsciiString &a) { s += a.s; }
	void concat(char c) { s += c; }
	void format(const char *f, ...) { char buf[1024]; va_list v; va_start(v, f); vsnprintf(buf, sizeof buf, f, v); va_end(v); s = buf; }
	Bool operator==(const AsciiString &o) const { return s == o.s; }
	Bool operator!=(const AsciiString &o) const { return s != o.s; }
};
AsciiString AsciiString::TheEmptyString;
AsciiString nameOf(const char *prefix, Int i) { char b[32]; snprintf(b, sizeof b, "%s%d", prefix, i); return AsciiString(b); }
/* the build list entries' names and scripts, the sides */
AsciiString buildingName(Int i) { return i < 0 ? AsciiString() : nameOf("Bldg", i); }
AsciiString scriptName(Int i) { return i < 0 ? AsciiString() : nameOf("Cond", i); }
AsciiString sideName(Int i) { return nameOf("Side", i); }
enum { TheKey_objectName = 1, TheKey_objectScriptAttachment, TheKey_objectInitialHealth, TheKey_objectUnsellable };
class Dict
{
public:
	AsciiString name, script; Int health; Bool unsellable;
	Dict() : health(0), unsellable(false) {}
	void setAsciiString(NameKeyType k, const AsciiString &v) { if (k == TheKey_objectName) name = v; else script = v; }
	void setInt(NameKeyType k, Int v) { health = v; }
	void setBool(NameKeyType k, Bool v) { unsellable = v; }
};
class NameKeyGenerator { public: AsciiString keyToName(NameKeyType k) { return nameOf("Player", k); } NameKeyType nameToKey(const char *n) { return n[0] == 'S' && n[6] == 'C' ? 1 : 2; } };
NameKeyGenerator g_nameKeys;
NameKeyGenerator *TheNameKeyGenerator = &g_nameKeys;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class Object; class Team; class TeamPrototype; class Player; class ThingTemplate; class UpgradeTemplate; class AIGroup;
class Script; class ScriptAction; class SpecialPowerTemplate; class Waypoint; class LocomotorSet;
void logStr(unsigned kind, const AsciiString &s) { std::vector<unsigned> w = { kind, (unsigned)s.s.size() }; for (char c : s.s) w.push_back((unsigned char)c); g_log->insert(g_log->end(), w.begin(), w.end()); }
}	/* namespace ap */

namespace ap {
/* a truth with a percentage, from the hash */
Bool chance(unsigned kind, unsigned a, unsigned b, unsigned pct) { return apAnswer(g_seed, kind, a, b) % 100u < pct; }
Int GameLogicRandomValue(Int lo, Int hi)
{
	g_rand = g_rand * 1664525u + 1013904223u;
	Int r = lo + (Int)((g_rand >> 8) % (unsigned)(hi - lo + 1));
	L({ AP_LOG_RANDOM, (unsigned)lo, (unsigned)hi, (unsigned)r });
	return r;
}

/* ---- the memory pool, the snapshot, the xfer */
/* the pool's memory is zeroed (WorkOrder() leaves m_required unset) */
class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { delete this; }
	static void *operator new(size_t n) { void *p = calloc(1, n); if (!p) abort(); return p; }
	static void operator delete(void *p) { free(p); }
};
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(T, n) public: virtual ~T();
#define newInstance(T) new T
class Xfer;
class Snapshot { public: virtual ~Snapshot() {} virtual void crc(Xfer *x) = 0; virtual void xfer(Xfer *x) = 0; virtual void loadPostProcess() = 0; };
class Xfer
{
public:
	XferMode m_mode; std::vector<unsigned char> m_bytes; size_t m_pos;
	Xfer(XferMode m) : m_mode(m), m_pos(0) {}
	XferMode getXferMode() const { return m_mode; }
	void raw(void *p, size_t n)
	{
		if (m_mode == XFER_SAVE) { const unsigned char *c = (const unsigned char *)p; m_bytes.insert(m_bytes.end(), c, c + n); return; }
		if (m_pos + n > m_bytes.size()) throw SC_INVALID_DATA;
		memcpy(p, &m_bytes[m_pos], n); m_pos += n;
	}
	void xferVersion(XferVersion *v, XferVersion cur) { raw(v, 1); if (*v > cur) throw SC_INVALID_DATA; }
	void xferUnsignedShort(UnsignedShort *v) { raw(v, 2); }
	void xferUser(void *p, Int n) { raw(p, (size_t)n); }
	void xferBool(Bool *v) { unsigned char b = *v ? 1 : 0; raw(&b, 1); *v = b != 0; }
	void xferInt(Int *v) { raw(v, 4); }
	void xferObjectID(ObjectID *v) { raw(v, 4); }
	void xferReal(Real *v) { raw(v, 4); }
	void xferCoord3D(Coord3D *c) { raw(&c->x, 4); raw(&c->y, 4); raw(&c->z, 4); }
	/* XferSave's and XferLoad's: a byte of length, then the bytes */
	void xferAsciiString(AsciiString *s)
	{
		if (m_mode == XFER_SAVE) {
			if (s->s.size() > 255) throw SC_INVALID_DATA;
			unsigned char n = (unsigned char)s->s.size(); raw(&n, 1); if (n) raw((void *)s->s.data(), n); return;
		}
		unsigned char n; raw(&n, 1); std::string t(n, '\0'); if (n) raw(&t[0], n); s->s = t;
	}
	void xferUnsignedByte(UnsignedByte *v) { raw(v, 1); }
	void xferSnapshot(Snapshot *s) { s->xfer(this); }
};
class XferCRC : public Xfer { public: XferCRC() : Xfer(XFER_CRC) {} unsigned getCRC() { return 0; } };

/* ---- templates */
class GeometryInfo { public: Real m_r; Real getBoundingCircleRadius() const { return m_r; } };
class ThingTemplate
{
public:
	Int m_i; GeometryInfo m_geom;
	AsciiString getName() const { return nameOf("Tmpl", m_i); }
	Bool isKindOf(KindOfType k) const { return (W.t[m_i].kinds >> k) & 1; }
	Int calcCostToBuild(const Player *p) const { return W.t[m_i].cost; }
	Bool isEquivalentTo(const ThingTemplate *o) const { return o && W.t[o->m_i].equiv == W.t[m_i].equiv; }
	const GeometryInfo &getTemplateGeometryInfo() const { return m_geom; }
	Real getPlacementViewAngle() const { return W.t[m_i].angle; }
	const ThingTemplate *friend_getNextTemplate() const;
};
ThingTemplate g_tmpls[AP_TEMPLATES];
const ThingTemplate *ThingTemplate::friend_getNextTemplate() const { return m_i + 1 < AP_TEMPLATES ? &g_tmpls[m_i + 1] : NULL; }
Int indexOf(const AsciiString &name, const char *prefix, Int n)
{
	size_t l = strlen(prefix);
	if (name.s.compare(0, l, prefix) != 0 || name.s.size() == l) return -1;
	Int v = 0;
	for (size_t i = l; i < name.s.size(); ++i) { if (name.s[i] < '0' || name.s[i] > '9') return -1; v = v * 10 + (name.s[i] - '0'); if (v >= n) return -1; }
	return v;
}
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &n) { Int i = indexOf(n, "Tmpl", AP_TEMPLATES); return i < 0 ? NULL : &g_tmpls[i]; }
	const ThingTemplate *firstTemplate() { return &g_tmpls[0]; }
};
ThingFactory g_factory; ThingFactory *TheThingFactory = &g_factory;
class UpgradeTemplate
{
public:
	Int m_i;
	AsciiString getUpgradeName() const { return nameOf("Up", m_i); }
	UpgradeType getUpgradeType() const { return (m_i & 1) ? UPGRADE_TYPE_OBJECT : UPGRADE_TYPE_PLAYER; }
};
UpgradeTemplate g_upgrades[AP_UPGRADES];
class ScienceStore { public: AsciiString getInternalNameForScience(ScienceType s) { return nameOf("Sci", s); } };
ScienceStore g_sciences; ScienceStore *TheScienceStore = &g_sciences;
class SpecialPowerTemplate { public: SpecialPowerType m_type; SpecialPowerType getSpecialPowerType() const { return m_type; } };

/* ---- players */
class Money { public: Int m_p; UnsignedInt countMoney() const { return (UnsignedInt)W.pl[m_p].money; } };
class BuildListInfo;
class Player
{
public:
	Int m_i; Money m_money; BuildListInfo *m_pBuildList;
	typedef std::list<TeamPrototype *> PlayerTeamList;
	PlayerTeamList m_teams;
	BuildListInfo *getBuildList() { return m_pBuildList; }
	Money *getMoney() { return &m_money; }
	const PlayerTeamList *getPlayerTeams() const { return &m_teams; }
	Bool getCanBuildUnits() { return (W.pl[m_i].flags & AP_PL_UNITS) != 0; }
	void setCanBuildUnits(Bool b) { L({ AP_LOG_CAN_BUILD, (unsigned)m_i, b ? 1u : 0u }); if (b) W.pl[m_i].flags |= AP_PL_UNITS; else W.pl[m_i].flags &= ~AP_PL_UNITS; }
	Bool getCanBuildBase() { return (W.pl[m_i].flags & AP_PL_BASE) != 0; }
	AsciiString getSide() { return sideName(W.pl[m_i].side); }
	UnsignedInt getAttackedFrame() { return (UnsignedInt)W.pl[m_i].attackedFrame; }
	Int getSciencePurchasePoints() const { return W.pl[m_i].sciencePoints; }
	Bool isCapableOfPurchasingScience(ScienceType s) const { return chance(85, (unsigned)m_i, (unsigned)s, 60); }
	Bool attemptToPurchaseScience(ScienceType s) { Bool ok = chance(86, (unsigned)m_i, (unsigned)s, 50); L({ AP_LOG_PURCHASE, (unsigned)m_i, (unsigned)s, ok ? 1u : 0u }); return ok; }
	NameKeyType getPlayerNameKey() const { return m_i; }
	PlayerIndex getPlayerIndex() const { return m_i; }
	Bool hasUpgradeInProduction(const UpgradeTemplate *u) { return chance(83, (unsigned)m_i, (unsigned)u->m_i, 25); }
	Bool hasUpgradeComplete(const UpgradeTemplate *u) { return chance(84, (unsigned)m_i, (unsigned)u->m_i, 25); }
	Team *getDefaultTeam();
	Relationship getRelationship(const Team *t) const;
	void addToBuildList(Object *obj);
	void addToPriorityBuildList(AsciiString templateName, Coord3D *pos, Real angle);
};
Player g_players[AP_PLAYERS];
class PlayerList { public: Player *getNthPlayer(Int i) { return (i >= 0 && i < AP_PLAYERS) ? &g_players[i] : NULL; } };
PlayerList g_playerList; PlayerList *ThePlayerList = &g_playerList;

/* ---- BuildListInfo: SidesList.h's members and inline functions (as AIPlayer.cpp calls them) */
class BuildListInfo
{
public:
	enum { UNLIMITED_REBUILDS = 0xFFFFFFFF };
	enum { MAX_RESOURCE_GATHERERS = 10 };
	AsciiString m_buildingName, m_templateName, m_script;
	Coord3D m_location; Coord2D m_rallyPointOffset; Real m_angle;
	Bool m_isInitiallyBuilt; UnsignedInt m_numRebuilds; BuildListInfo *m_nextBuildList;
	Int m_health; Bool m_unsellable, m_underConstruction, m_isSupplyBuilding, m_priorityBuild;
	ObjectID m_objectID; UnsignedInt m_objectTimestamp; Int m_desiredGatherers, m_currentGatherers;
	BuildListInfo() : m_angle(0), m_isInitiallyBuilt(false), m_numRebuilds(0), m_nextBuildList(NULL), m_health(100), m_unsellable(false),
		m_underConstruction(false), m_isSupplyBuilding(false), m_priorityBuild(false), m_objectID(INVALID_ID), m_objectTimestamp(0),
		m_desiredGatherers(0), m_currentGatherers(0) { m_location.x = m_location.y = m_location.z = 0; m_rallyPointOffset.x = m_rallyPointOffset.y = 0; }
	void setNumRebuilds(UnsignedInt n) { m_numRebuilds = n; }
	void setNextBuildList(BuildListInfo *p) { m_nextBuildList = p; }
	void setLocation(Coord3D loc) { m_location = loc; }
	void setAngle(Real a) { m_angle = a; }
	BuildListInfo *getNext(void) const { return m_nextBuildList; }
	AsciiString getBuildingName(void) const { return m_buildingName; }
	AsciiString getTemplateName(void) const { return m_templateName; }
	void setTemplateName(AsciiString name) { m_templateName = name; }
	Int getNumRebuilds(void) { return m_numRebuilds; }
	void decrementNumRebuilds(void) { if (m_numRebuilds > 0 && m_numRebuilds != UNLIMITED_REBUILDS) m_numRebuilds--; }
	void incrementNumRebuilds(void) { if (m_numRebuilds != UNLIMITED_REBUILDS) m_numRebuilds++; }
	const Coord3D *getLocation(void) const { return &m_location; }
	const Coord2D *getRallyOffset(void) const { return &m_rallyPointOffset; }
	Real getAngle(void) const { return m_angle; }
	Bool isInitiallyBuilt(void) { return m_isInitiallyBuilt; }
	AsciiString getScript(void) { return m_script; }
	Int getHealth(void) { return m_health; }
	Bool getUnsellable(void) { return m_unsellable; }
	void setObjectID(ObjectID id) { m_objectID = id; }
	ObjectID getObjectID(void) const { return m_objectID; }
	void setObjectTimestamp(UnsignedInt t) { m_objectTimestamp = t; }
	UnsignedInt getObjectTimestamp(void) const { return m_objectTimestamp; }
	Bool isBuildable(void) { if (getNumRebuilds() > 0 || getNumRebuilds() == BuildListInfo::UNLIMITED_REBUILDS) return true; return false; }
	void setUnderConstruction(Bool c) { m_underConstruction = c; }
	void markPriorityBuild(void) { m_priorityBuild = true; }
	Bool isSupplyBuilding(void) { return m_isSupplyBuilding; }
	void setSupplyBuilding(Bool b) { m_isSupplyBuilding = b; }
	Int getDesiredGatherers(void) { return m_desiredGatherers; }
	void setDesiredGatherers(Int n) { m_desiredGatherers = n; }
	Int getCurrentGatherers(void) { return m_currentGatherers; }
	void setCurrentGatherers(Int n) { m_currentGatherers = n; }
};

/* ---- objects and their modules */
class LocomotorSet { public: mutable Int m_i; };	/* the mover's slot */
class SupplyTruckAIInterface
{
public:
	Int m_i;
	ObjectID getPreferredDockID() { return W.o[m_i].dock; }
	Bool isCurrentlyFerryingSupplies() { return (W.o[m_i].flags & AP_F_FERRYING) != 0; }
	Bool isForcedIntoWantingState() { return (W.o[m_i].flags & AP_F_FORCED) != 0; }
	void setForceWantingState(Bool b) { L({ AP_LOG_FORCE_WANTING, (unsigned)W.o[m_i].id, b ? 1u : 0u }); if (b) W.o[m_i].flags |= AP_F_FORCED; else W.o[m_i].flags &= ~AP_F_FORCED; }
};
class DozerAIInterface
{
public:
	Int m_i;
	Bool isAnyTaskPending() { return (W.o[m_i].flags & AP_F_TASK) != 0; }
	Bool isTaskPending(DozerTask t) { return t == DOZER_TASK_BUILD && (W.o[m_i].flags & AP_F_TASK_BUILD) != 0; }
};
class AIUpdateInterface
{
public:
	Int m_i; LocomotorSet m_set; SupplyTruckAIInterface m_truck; DozerAIInterface m_dozer;
	unsigned id() const { return (unsigned)W.o[m_i].id; }
	Bool isIdle() const { return (W.o[m_i].flags & AP_F_IDLE) != 0; }
	const Coord3D *getGoalPosition() const;
	const LocomotorSet &getLocomotorSet() const { m_set.m_i = m_i; return m_set; }
	SupplyTruckAIInterface *getSupplyTruckAIInterface() { m_truck.m_i = m_i; return (W.o[m_i].flags & AP_F_SUPPLY_AI) ? &m_truck : NULL; }
	DozerAIInterface *getDozerAIInterface() { m_dozer.m_i = m_i; return (W.o[m_i].flags & AP_F_DOZER_AI) ? &m_dozer : NULL; }
	void aiDock(Object *o, CommandSourceType s);
	void aiIdle(CommandSourceType s) { L({ AP_LOG_AI, id(), AP_C_IDLE, (unsigned)s }); }
	void aiMoveToPosition(const Coord3D *p, CommandSourceType s) { L({ AP_LOG_AI, id(), AP_C_MOVE, bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)s }); }
	void aiResumeConstruction(Object *o, CommandSourceType s);
	void aiRepair(Object *o, CommandSourceType s);
	void aiFollowExitProductionPath(const std::vector<Coord3D> *path, Object *ignore, CommandSourceType s)
	{
		std::vector<unsigned> w = { AP_LOG_AI, id(), AP_C_EXIT_PATH, (unsigned)path->size() };
		for (const Coord3D &c : *path) { w.push_back(bitsOf(c.x)); w.push_back(bitsOf(c.y)); w.push_back(bitsOf(c.z)); }
		w.push_back((unsigned)s);
		g_log->insert(g_log->end(), w.begin(), w.end());
	}
	void joinTeam() { L({ AP_LOG_AI, id(), AP_C_JOIN }); }
};
class ProductionUpdateInterface
{
public:
	Int m_i;
	Int getProductionCount() const { return W.o[m_i].prodCount; }
	UnsignedInt requestUniqueUnitID();
	Bool queueCreateUnit(const ThingTemplate *t, UnsignedInt id)
	{
		Bool ok = chance(80, (unsigned)W.o[m_i].id, (unsigned)t->m_i, 80);
		L({ AP_LOG_QUEUE_UNIT, (unsigned)W.o[m_i].id, (unsigned)t->m_i, id, ok ? 1u : 0u });
		if (ok) W.o[m_i].prodCount++;
		return ok;
	}
	Bool queueUpgrade(const UpgradeTemplate *u)
	{
		Bool ok = chance(81, (unsigned)W.o[m_i].id, (unsigned)u->m_i, 70);
		L({ AP_LOG_QUEUE_UPGRADE, (unsigned)W.o[m_i].id, (unsigned)u->m_i, ok ? 1u : 0u });
		return ok;
	}
};
UnsignedInt g_unitId;
UnsignedInt ProductionUpdateInterface::requestUniqueUnitID() { return ++g_unitId; }
class DamageInfoOutput { public: Bool m_noEffect; };
class DamageInfo { public: DamageInfoOutput out; };
class BodyModuleInterface
{
public:
	Int m_i; DamageInfo m_info;
	BodyDamageType getDamageState() const { return (BodyDamageType)W.o[m_i].damageState; }
	const DamageInfo *getLastDamageInfo() { m_info.out.m_noEffect = (W.o[m_i].flags & AP_F_NO_EFFECT) != 0; return (W.o[m_i].flags & AP_F_LAST_DAMAGE) ? &m_info : NULL; }
	UnsignedInt getLastDamageTimestamp() const { return (UnsignedInt)W.o[m_i].damageStamp; }
};
class SupplyWarehouseDockUpdate { public: Int m_i; Int getBoxesStored() { return W.o[m_i].boxes; } };
class RebuildHoleBehaviorInterface { public: Int m_i; ObjectID getSpawnerID() { return W.o[m_i].spawner; } ObjectID getReconstructedBuildingID() { return W.o[m_i].reconstructed; } };
class ExitInterface
{
public:
	Int m_i;
	Bool getNaturalRallyPoint(Coord3D &c) { if (!(W.o[m_i].flags & AP_F_RALLY)) return false; c.x = W.o[m_i].rally[0]; c.y = W.o[m_i].rally[1]; c.z = W.o[m_i].rally[2]; return true; }
	void setRallyPoint(const Coord3D *c) { abort(); /* never: `gotOffset;` sets nothing */ }
};
class Module {};
class Object
{
public:
	Int m_i;
	AIUpdateInterface m_ai; ProductionUpdateInterface m_pu; BodyModuleInterface m_body; SupplyWarehouseDockUpdate m_warehouse;
	RebuildHoleBehaviorInterface m_hole; ExitInterface m_exit; Module m_centerDock; GeometryInfo m_geom; Coord3D m_pos;
	ApObj &s() const { return W.o[m_i]; }
	ObjectID getID() const { return s().id; }
	const ThingTemplate *getTemplate() const { return &g_tmpls[s().tmpl]; }
	Bool isKindOf(KindOfType k) const { return (W.t[s().tmpl].kinds >> k) & 1; }
	const Coord3D *getPosition() const { Object *t = const_cast<Object *>(this); t->m_pos.x = s().pos[0]; t->m_pos.y = s().pos[1]; t->m_pos.z = s().pos[2]; return &m_pos; }
	void setPosition(const Coord3D *p) { L({ AP_LOG_SET_POS, (unsigned)s().id, bitsOf(p->x), bitsOf(p->y), bitsOf(p->z) }); s().pos[0] = p->x; s().pos[1] = p->y; s().pos[2] = p->z; }
	const GeometryInfo &getGeometryInfo() const { const_cast<Object *>(this)->m_geom.m_r = s().radius; return m_geom; }
	Player *getControllingPlayer() const { return s().player >= 0 ? &g_players[s().player] : NULL; }
	Team *getTeam() const;
	void setTeam(Team *t);
	ObjectStatusMaskType getStatusBits() const { return ObjectStatusMaskType{ (unsigned)s().status }; }
	Bool testStatus(ObjectStatusTypes st) const { return getStatusBits().test(st); }
	void clearStatus(ObjectStatusMaskType m) { L({ AP_LOG_CLEAR_STATUS, (unsigned)s().id, m.m }); s().status &= ~(Int)m.m; }
	ObjectID getBuilderID() const { return s().builder; }
	Bool isSignificantlyAboveTerrain() const { return (s().flags & AP_F_ABOVE) != 0; }
	void updateObjValuesFromMapProperties(Dict *d)
	{
		std::vector<unsigned> w = { AP_LOG_MAP_PROPS, (unsigned)s().id, (unsigned)d->health, d->unsellable ? 1u : 0u, (unsigned)d->name.s.size() };
		for (char c : d->name.s) w.push_back((unsigned char)c);
		w.push_back((unsigned)d->script.s.size());
		for (char c : d->script.s) w.push_back((unsigned char)c);
		g_log->insert(g_log->end(), w.begin(), w.end());
	}
	void updateUpgradeModules() { L({ AP_LOG_UPGRADE_MODULES, (unsigned)s().id }); }
	AIUpdateInterface *getAIUpdateInterface() const { Object *t = const_cast<Object *>(this); t->m_ai.m_i = m_i; return (s().flags & AP_F_AI) ? &t->m_ai : NULL; }
	AIUpdateInterface *getAI() const { return getAIUpdateInterface(); }
	Module *findUpdateModule(NameKeyType key)
	{
		if (key == 1) return (s().flags & AP_F_CENTER_DOCK) ? &m_centerDock : NULL;
		m_warehouse.m_i = m_i;
		return (s().flags & AP_F_WAREHOUSE) ? (Module *)&m_warehouse : NULL;
	}
	ProductionUpdateInterface *getProductionUpdateInterface() { m_pu.m_i = m_i; return (s().flags & AP_F_PRODUCTION) ? &m_pu : NULL; }
	BodyModuleInterface *getBodyModule() { m_body.m_i = m_i; return (s().flags & AP_F_BODY) ? &m_body : NULL; }
	ExitInterface *getObjectExitInterface() { m_exit.m_i = m_i; return (s().flags & AP_F_EXIT) ? &m_exit : NULL; }
	AsciiString getCommandSetString() const { return nameOf("Set", m_i); }
	Real getOrientation() const { return (Real)(s().id % 8) * 0.5f; }
	Bool isEffectivelyDead() const { return (s().flags & AP_F_DEAD) != 0; }
	Bool isOffMap() const { return !(W.extent[0] <= s().pos[0] && s().pos[0] <= W.extent[2] && W.extent[1] <= s().pos[1] && s().pos[1] <= W.extent[3]); }
	Bool isStructure() const { return isKindOf(KINDOF_STRUCTURE); }
	Bool isNonFactionStructure() const { return (s().flags & AP_F_INSIGNIFICANT) != 0; }
	Bool isKindOfMulti(KindOfMaskType must, KindOfMaskType clear) const { unsigned k = (unsigned)W.t[s().tmpl].kinds; return (k & must.m) == must.m && (k & clear.m) == 0; }
	Object *getNextObject() const;
	Object *dlink_next_TeamMemberList() const;
};
Object g_objs[AP_OBJECTS];
const Coord3D *AIUpdateInterface::getGoalPosition() const { return g_objs[m_i].getPosition(); }
void AIUpdateInterface::aiDock(Object *o, CommandSourceType s) { L({ AP_LOG_AI, id(), AP_C_DOCK, (unsigned)o->getID(), (unsigned)s }); }
void AIUpdateInterface::aiResumeConstruction(Object *o, CommandSourceType s) { L({ AP_LOG_AI, id(), AP_C_RESUME, (unsigned)o->getID(), (unsigned)s }); }
void AIUpdateInterface::aiRepair(Object *o, CommandSourceType s) { L({ AP_LOG_AI, id(), AP_C_REPAIR, (unsigned)o->getID(), (unsigned)s }); }
Object *Object::getNextObject() const { for (Int i = m_i + 1; i < AP_OBJECTS; ++i) if (W.o[i].id) return &g_objs[i]; return NULL; }
Object *objById(ObjectID id) { if (id == INVALID_ID) return NULL; for (Int i = 0; i < AP_OBJECTS; ++i) if (W.o[i].id == id) return &g_objs[i]; return NULL; }
class RebuildHoleBehavior
{
public:
	static RebuildHoleBehaviorInterface *getRebuildHoleBehaviorInterfaceFromObject(Object *o) { o->m_hole.m_i = o->m_i; return (o->s().flags & AP_F_HOLE) ? &o->m_hole : NULL; }
};
class SupplyCenterDockUpdate;
}	/* namespace ap */

namespace ap {
#include "ai_player_dlink.inc"

/* ---- teams and their prototypes. A team is a slot of W.tm (its handle is its id); its members are the live objects with its
** id, in slot order; a prototype's instances are the teams with its index, in slot order. */
struct TCreateUnitsInfo { Int minUnits, maxUnits; AsciiString unitThingName; };
class TeamTemplateInfo
{
public:
	enum { MAX_UNIT_TYPES = 7 };
	TCreateUnitsInfo m_unitsInfo[MAX_UNIT_TYPES]; Int m_numUnitsInfo; Coord3D m_homeLocation; Bool m_hasHomeLocation;
	Int m_initialIdleFrames; Bool m_automaticallyReinforce, m_executeActions; AsciiString m_productionCondition;
	Int m_maxInstances; mutable Int m_productionPriority;
};
Team *teamById(TeamID id);
class TeamPrototype
{
public:
	Int m_i; mutable TeamTemplateInfo m_info;
	ApProto &s() const { return W.pr[m_i]; }
	AsciiString getName() const { return nameOf("Proto", m_i); }
	/* the scene's prototype, every call (unitTmpl -1: a name no template has) */
	const TeamTemplateInfo *getTemplateInfo() const
	{
		m_info.m_numUnitsInfo = s().numUnits;
		for (Int i = 0; i < 4; ++i) {
			m_info.m_unitsInfo[i].minUnits = s().unitMin[i];
			m_info.m_unitsInfo[i].maxUnits = s().unitMax[i];
			m_info.m_unitsInfo[i].unitThingName = s().unitTmpl[i] < 0 ? AsciiString("Nothing") : nameOf("Tmpl", s().unitTmpl[i]);
		}
		m_info.m_homeLocation.x = s().home[0]; m_info.m_homeLocation.y = s().home[1]; m_info.m_homeLocation.z = s().home[2];
		m_info.m_hasHomeLocation = (s().flags & AP_P_HOME) != 0;
		m_info.m_initialIdleFrames = s().idleFrames;
		m_info.m_automaticallyReinforce = (s().flags & AP_P_REINFORCE) != 0;
		m_info.m_executeActions = (s().flags & AP_P_ACTIONS) != 0;
		m_info.m_productionCondition = scriptName(s().condition);
		m_info.m_maxInstances = s().maxInstances;
		m_info.m_productionPriority = s().priority;
		return &m_info;
	}
	Bool getIsSingleton() const { return (s().flags & AP_P_SINGLETON) != 0; }
	Player *getControllingPlayer() const { return &g_players[s().player]; }
	Team *firstInstance() const;
	DLINK_ITERATOR<Team> iterate_TeamInstanceList() const;
	Int countTeamInstances() const { Int n = 0; for (Int k = 0; k < AP_TEAMS; ++k) if (W.tm[k].id && W.tm[k].proto == m_i) ++n; return n; }
	/* without the condition flag, true; else the hash of the prototype and the frame */
	Bool evaluateProductionCondition() { return !(s().flags & AP_P_CONDITION) || chance(87, (unsigned)m_i, (unsigned)W.frame, 50); }
};
TeamPrototype g_protos[AP_PROTOS];
class AIGroup;
class Team
{
public:
	Int m_k;
	ApTeam &s() const { return W.tm[m_k]; }
	TeamID getID() const { return (TeamID)s().id; }
	TeamPrototype *getPrototype() const { return &g_protos[s().proto]; }
	AsciiString getName() const { return getPrototype()->getName(); }
	Object *firstMember() const { for (Int i = 0; i < AP_OBJECTS; ++i) if (W.o[i].id && W.o[i].team == s().id) return &g_objs[i]; return NULL; }
	DLINK_ITERATOR<Object> iterate_TeamMemberList() const { return DLINK_ITERATOR<Object>(firstMember(), &Object::dlink_next_TeamMemberList); }
	Object *getFirstItemIn_TeamMemberList() const { return firstMember(); }
	Team *dlink_next_TeamInstanceList() const { for (Int k = m_k + 1; k < AP_TEAMS; ++k) if (W.tm[k].id && W.tm[k].proto == s().proto) return teamById(W.tm[k].id); return NULL; }
	Bool hasAnyUnits() const { for (Object *o = firstMember(); o; o = o->dlink_next_TeamMemberList()) if (!o->isKindOf(KINDOF_STRUCTURE)) return true; return false; }
	Bool hasAnyObjects() const { return firstMember() != NULL; }
	Bool isIdle() const { return s().idle != 0; }
	void setActive() { L({ AP_LOG_TEAM_ACTIVE, (unsigned)s().id }); s().active = 1; }
	void transferUnitsTo(Team *to)
	{
		Int toId = to ? to->s().id : 0;
		L({ AP_LOG_TEAM_TRANSFER, (unsigned)s().id, (unsigned)toId });
		Int id = s().id;
		for (Int i = 0; i < AP_OBJECTS; ++i) if (W.o[i].id && W.o[i].team == id) W.o[i].team = toId;
	}
	/* the slot freed; any member left has no team */
	void deleteInstance()
	{
		L({ AP_LOG_TEAM_DELETE, (unsigned)s().id });
		Int id = s().id;
		for (Int i = 0; i < AP_OBJECTS; ++i) if (W.o[i].id && W.o[i].team == id) W.o[i].team = 0;
		s().id = 0;
	}
	/* the closest live object with an AI and a team (not this one), of an equivalent template, of the prototype's player,
	** within the radius of the origin (2D, squared, the first of equals) */
	Object *tryToRecruit(const ThingTemplate *t, const Coord3D *origin, Real radius)
	{
		Object *best = NULL; Real bestD = 0;
		Int owner = getPrototype()->s().player;
		for (Int i = 0; i < AP_OBJECTS; ++i) {
			ApObj &o = W.o[i];
			if (!o.id || (o.flags & AP_F_DEAD) || !(o.flags & AP_F_AI) || o.team == s().id || !teamById((TeamID)o.team) || o.player != owner) continue;
			if (W.t[o.tmpl].equiv != W.t[t->m_i].equiv) continue;
			Real dx = o.pos[0] - origin->x, dy = o.pos[1] - origin->y;
			Real d = dx * dx + dy * dy;
			if (d > radius * radius) continue;
			if (!best || d < bestD) { best = &g_objs[i]; bestD = d; }
		}
		return best;
	}
	void countObjectsByThingTemplate(Int n, const ThingTemplate *const *things, Bool ignoreDead, Int *counts)
	{
		for (Object *o = firstMember(); o; o = o->dlink_next_TeamMemberList())
			for (Int i = 0; i < n; ++i) if (o->getTemplate()->isEquivalentTo(things[i])) counts[i]++;
	}
	const Coord3D *getEstimateTeamPosition() const { Object *o = firstMember(); return o ? o->getPosition() : NULL; }
	void getTeamAsAIGroup(AIGroup *g);
};
Team g_teams[AP_TEAMS];
Team *teamById(TeamID id) { if (id == TEAM_ID_INVALID) return NULL; for (Int k = 0; k < AP_TEAMS; ++k) if ((TeamID)W.tm[k].id == id) return &g_teams[k]; return NULL; }
Team *TeamPrototype::firstInstance() const { for (Int k = 0; k < AP_TEAMS; ++k) if (W.tm[k].id && W.tm[k].proto == m_i) return &g_teams[k]; return NULL; }
DLINK_ITERATOR<Team> TeamPrototype::iterate_TeamInstanceList() const { return DLINK_ITERATOR<Team>(firstInstance(), &Team::dlink_next_TeamInstanceList); }
Object *Object::dlink_next_TeamMemberList() const { if (!s().team) return NULL; for (Int i = m_i + 1; i < AP_OBJECTS; ++i) if (W.o[i].id && W.o[i].team == s().team) return &g_objs[i]; return NULL; }
Team *Object::getTeam() const { return teamById((TeamID)s().team); }
void Object::setTeam(Team *t) { Int id = t ? t->s().id : 0; L({ AP_LOG_SET_TEAM, (unsigned)s().id, (unsigned)id }); s().team = id; }
class AIGroup { public: TeamID m_team; void groupGuardPosition(const Coord3D *p, GuardMode m, CommandSourceType c) { L({ AP_LOG_TEAM_GUARD, m_team, bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)m, (unsigned)c }); } };
void Team::getTeamAsAIGroup(AIGroup *g) { g->m_team = getID(); }
class TeamFactory
{
public:
	Team *findTeam(const AsciiString &name) { Int p = indexOf(name, "Proto", AP_PROTOS); return p < 0 ? NULL : g_protos[p].firstInstance(); }
	Team *findTeamByID(TeamID id) { return teamById(id); }
	/* the first free slot, the next team id; idle by the hash of the id */
	Team *createInactiveTeam(const AsciiString &name)
	{
		Int p = indexOf(name, "Proto", AP_PROTOS);
		for (Int k = 0; k < AP_TEAMS; ++k) {
			if (W.tm[k].id) continue;
			W.tm[k].id = W.nextTeamId++; W.tm[k].proto = p; W.tm[k].active = 0; W.tm[k].idle = chance(94, (unsigned)W.tm[k].id, 0, 75);
			L({ AP_LOG_TEAM_CREATE, (unsigned)p, (unsigned)W.tm[k].id });
			return &g_teams[k];
		}
		abort();	/* the generator keeps a free slot */
	}
};
TeamFactory g_teamFactory; TeamFactory *TheTeamFactory = &g_teamFactory;

/* ---- the players' teams and relationships, their build lists */
Team *Player::getDefaultTeam() { return teamById((TeamID)W.pl[m_i].defaultTeam); }
Relationship Player::getRelationship(const Team *t) const { if (!t) return NEUTRAL; return (Relationship)W.rel[m_i][t->getPrototype()->s().player]; }
void Player::addToBuildList(Object *obj)
{
	L({ AP_LOG_ADD_BUILD, (unsigned)m_i, (unsigned)obj->getID() });
	BuildListInfo *n = new BuildListInfo;
	n->setObjectID(obj->getID()); n->setTemplateName(obj->getTemplate()->getName()); n->setLocation(*obj->getPosition());
	n->setAngle(obj->getOrientation()); n->setNumRebuilds(0); n->setNextBuildList(m_pBuildList);
	m_pBuildList = n;
}
void Player::addToPriorityBuildList(AsciiString templateName, Coord3D *pos, Real angle)
{
	L({ AP_LOG_ADD_PRIORITY, (unsigned)m_i, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(angle) });
	BuildListInfo *n = new BuildListInfo;
	n->setTemplateName(templateName); n->setLocation(*pos); n->setAngle(angle); n->markPriorityBuild(); n->setNumRebuilds(1);
	n->setNextBuildList(m_pBuildList);
	m_pBuildList = n;
}

/* ---- the script engine: a script "Cond<k>" exists and has an action by the hash of k */
class ScriptAction { public: Int m_k; };
class Script { public: Int m_k; ScriptAction m_action; Bool m_hasAction; const ScriptAction *getAction() const { return m_hasAction ? &m_action : NULL; } };
Script g_scripts[32];
class ScriptEngine
{
public:
	GameDifficulty getGlobalDifficulty() { return (GameDifficulty)W.difficulty; }
	void addObjectToCache(Object *o) { L({ AP_LOG_CACHE, (unsigned)o->getID() }); }
	void runObjectScript(const AsciiString &name, Object *o) { logStr(AP_LOG_SCRIPT, name); L({ (unsigned)o->getID() }); }
	void AppendDebugMessage(const AsciiString &m, Bool b) { logStr(AP_LOG_DEBUG, m); }
	const Script *findScriptByName(const AsciiString &name)
	{
		Int k = indexOf(name, "Cond", 32);
		if (k < 0 || !chance(88, (unsigned)k, 0, 70)) return NULL;
		g_scripts[k].m_k = k; g_scripts[k].m_action.m_k = k; g_scripts[k].m_hasAction = chance(89, (unsigned)k, 0, 60);
		return &g_scripts[k];
	}
	void friend_executeAction(const ScriptAction *a, Team *t) { L({ AP_LOG_ACTION, (unsigned)a->m_k, t ? (unsigned)t->getID() : 0u }); }
	void clearTeamFlags() { L({ AP_LOG_CLEAR_FLAGS }); }
	Player *getSkirmishEnemyPlayer() { return &g_players[W.enemy]; }
};
ScriptEngine g_scriptEngine; ScriptEngine *TheScriptEngine = &g_scriptEngine;

/* ---- the build assistant, the terrain, the pathfinder */
class BuildAssistant
{
public:
	enum { TERRAIN_RESTRICTIONS = 0x00000001, CLEAR_PATH = 0x00000002, NO_OBJECT_OVERLAP = 0x00000004, NO_ENEMY_OBJECT_OVERLAP = 0x00000008 };
	/* a new object in the first free slot (none free: NULL): the next id, the owner's default team, the template's new flags,
	** under construction when a builder builds it */
	Object *buildObjectNow(Object *builder, const ThingTemplate *t, const Coord3D *pos, Real angle, Player *owner)
	{
		Int slot = -1;
		for (Int i = 0; i < AP_OBJECTS; ++i) if (!W.o[i].id) { slot = i; break; }
		unsigned newId = slot < 0 ? 0u : (unsigned)W.nextObjectId;
		L({ AP_LOG_BUILD_NOW, builder ? (unsigned)builder->getID() : 0u, (unsigned)t->m_i, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(angle),
			(unsigned)owner->m_i, newId });
		if (slot < 0) return NULL;
		ApObj &o = W.o[slot];
		memset(&o, 0, sizeof o);
		o.id = W.nextObjectId++; o.tmpl = t->m_i; o.player = owner->m_i; o.team = W.pl[owner->m_i].defaultTeam;
		o.flags = W.t[t->m_i].newFlags; o.status = builder ? (1 << OBJECT_STATUS_UNDER_CONSTRUCTION) : 0;
		o.pos[0] = pos->x; o.pos[1] = pos->y; o.pos[2] = pos->z; o.radius = W.t[t->m_i].radius;
		o.builder = builder ? builder->getID() : INVALID_ID; o.cmdButtons = -1;
		return &g_objs[slot];
	}
	/* legal by the hash of the position, the template and the options, W.legalPct percent of the time */
	LegalBuildCode isLocationLegalToBuild(const Coord3D *pos, const ThingTemplate *t, Real angle, UnsignedInt options, Object *builder, Player *p)
	{
		Bool ok = apAnswer(g_seed, 90, bitsOf(pos->x) ^ (bitsOf(pos->y) * 31u), (unsigned)t->m_i * 16u + options) % 100u < (unsigned)W.legalPct;
		L({ AP_LOG_LEGAL, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (unsigned)t->m_i, bitsOf(angle), options, builder ? (unsigned)builder->getID() : 0u,
			p ? (unsigned)p->m_i : 0xFFFFFFFFu, ok ? 1u : 0u });
		return ok ? LBC_OK : LBC_RESTRICTED_TERRAIN;
	}
	Bool isPossibleToMakeUnit(Object *factory, const ThingTemplate *t) { return chance(91, (unsigned)factory->getID(), (unsigned)t->m_i, 75); }
};
BuildAssistant g_buildAssistant; BuildAssistant *TheBuildAssistant = &g_buildAssistant;
class TerrainVisual { public: void removeAllBibs() { L({ AP_LOG_BIBS }); } };
TerrainVisual g_terrainVisual; TerrainVisual *TheTerrainVisual = &g_terrainVisual;
class TerrainLogic
{
public:
	/* the cells of 50 by 50: (column + 2 row) & 7 */
	Real getGroundHeight(Real x, Real y) { return (Real)(((Int)floorf(x / 50.0f) + 2 * (Int)floorf(y / 50.0f)) & 7); }
	void getExtent(Region3D *r) { r->lo.x = W.extent[0]; r->lo.y = W.extent[1]; r->lo.z = 0; r->hi.x = W.extent[2]; r->hi.y = W.extent[3]; r->hi.z = 0; }
};
TerrainLogic g_terrainLogic; TerrainLogic *TheTerrainLogic = &g_terrainLogic;
class Pathfinder
{
public:
	Bool clientSafeQuickDoesPathExist(const LocomotorSet &set, const Coord3D *from, const Coord3D *to);
	Bool adjustToPossibleDestination(Object *o, const LocomotorSet &set, Coord3D *pos)
	{
		L({ AP_LOG_ADJUST, (unsigned)o->getID(), bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
		pos->x += 5.0f;
		return true;
	}
};
/* a path by the hash of the destination and the mover (the locomotor set's) */
Bool Pathfinder::clientSafeQuickDoesPathExist(const LocomotorSet &set, const Coord3D *from, const Coord3D *to)
{
	return chance(92, bitsOf(to->x) ^ bitsOf(to->y), (unsigned)W.o[set.m_i].id, 85);
}

/* ---- the AI: its data (from the scene, built at the start of a run), the pathfinder, a group */
enum { MAX_AI_UPGRADES = 20 };
class TSkillSet { public: Int m_numSkills; ScienceType m_skills[MAX_AI_UPGRADES]; };
class AISideInfo { public: AsciiString m_side; Int m_easy, m_normal, m_hard; TSkillSet m_skillSet1, m_skillSet2, m_skillSet3, m_skillSet4, m_skillSet5; AISideInfo *m_next; };
class TAiData
{
public:
	Real m_structureSeconds, m_teamSeconds; Int m_resourcesWealthy, m_resourcesPoor; Real m_structuresWealthyMod, m_teamWealthyMod,
		m_structuresPoorMod, m_teamPoorMod, m_teamResourcesToBuild, m_maxRecruitDistance, m_supplyCenterSafeRadius;
	Int m_rebuildDelaySeconds; AISideInfo *m_sideInfo;
};
class AI
{
public:
	TAiData m_data; AISideInfo m_sides[AP_SIDES]; Pathfinder m_pathfinder; AIGroup m_group;
	const TAiData *getAiData() { return &m_data; }
	Pathfinder *pathfinder() { return &m_pathfinder; }
	AIGroup *createGroup() { return &m_group; }
};
AI g_ai; AI *TheAI = &g_ai;

/* ---- the upgrades, the command sets: an object's set has the buttons of the bits of its cmdButtons (-1: none); button j has
** no name when j % 7 == 6, no upgrade when j % 5 == 4, else upgrade j % AP_UPGRADES */
class UpgradeCenter
{
public:
	const UpgradeTemplate *findUpgrade(const AsciiString &n) { Int i = indexOf(n, "Up", AP_UPGRADES); return i < 0 ? NULL : &g_upgrades[i]; }
	Bool canAffordUpgrade(Player *p, const UpgradeTemplate *u) { return chance(93, (unsigned)p->m_i, (unsigned)u->m_i, 70); }
};
UpgradeCenter g_upgradeCenter; UpgradeCenter *TheUpgradeCenter = &g_upgradeCenter;
class CommandButton
{
public:
	Int m_j;
	AsciiString getName() const { return m_j % 7 == 6 ? AsciiString() : nameOf("Btn", m_j); }
	const UpgradeTemplate *getUpgradeTemplate() const { return m_j % 5 == 4 ? NULL : &g_upgrades[m_j % AP_UPGRADES]; }
};
CommandButton g_buttons[MAX_COMMANDS_PER_SET];
class CommandSet { public: Int m_bits; const CommandButton *getCommandButton(Int j) const { return ((m_bits >> j) & 1) ? &g_buttons[j] : NULL; } };
CommandSet g_commandSet;
class ControlBar
{
public:
	const CommandSet *findCommandSet(const AsciiString &n)
	{
		Int i = indexOf(n, "Set", AP_OBJECTS);
		if (i < 0 || W.o[i].cmdButtons < 0) return NULL;
		g_commandSet.m_bits = W.o[i].cmdButtons;
		return &g_commandSet;
	}
};
ControlBar g_controlBar; ControlBar *TheControlBar = &g_controlBar;

/* ---- the game logic, the global data */
class GameLogic
{
public:
	UnsignedInt getFrame() { return (UnsignedInt)W.frame; }
	Object *findObjectByID(ObjectID id) { return objById(id); }
	Object *getFirstObject() { for (Int i = 0; i < AP_OBJECTS; ++i) if (W.o[i].id) return &g_objs[i]; return NULL; }
};
GameLogic g_gameLogic; GameLogic *TheGameLogic = &g_gameLogic;
class GlobalData { public: Int m_debugAI; Int m_baseValuePerSupplyBox; Bool m_debugSupplyCenterPlacement; };
GlobalData g_globalData; GlobalData *TheGlobalData = &g_globalData;

/* ---- the partition: the closest object passing every filter, by 2D centre distance (squared) within the radius plus its
** bounding radius, the first of equals in slot order */
class PartitionFilter { public: virtual ~PartitionFilter() {} virtual Bool allow(Object *o) = 0; };
class PartitionFilterAcceptByKindOf : public PartitionFilter
{
public:
	KindOfMaskType m_set, m_clear;
	PartitionFilterAcceptByKindOf(KindOfMaskType s, KindOfMaskType c) : m_set(s), m_clear(c) {}
	Bool allow(Object *o) { return o->isKindOfMulti(m_set, m_clear); }
};
class PartitionFilterRejectByKindOf : public PartitionFilter
{
public:
	KindOfMaskType m_set, m_clear;
	PartitionFilterRejectByKindOf(KindOfMaskType s, KindOfMaskType c) : m_set(s), m_clear(c) {}
	Bool allow(Object *o) { return !o->isKindOfMulti(m_set, m_clear); }
};
class PartitionFilterPlayer : public PartitionFilter
{
public:
	Player *m_player; Bool m_match;
	PartitionFilterPlayer(Player *p, Bool m) : m_player(p), m_match(m) {}
	Bool allow(Object *o) { return (m_player == o->getControllingPlayer()) == m_match; }
};
class PartitionFilterPlayerAffiliation : public PartitionFilter
{
public:
	Player *m_player; Int m_affiliation; Bool m_match;
	PartitionFilterPlayerAffiliation(Player *p, Int a, Bool m) : m_player(p), m_affiliation(a), m_match(m) {}
	/* PartitionManager.cpp's */
	Bool allow(Object *other)
	{
		Relationship rel = m_player->getRelationship(other->getTeam());
		switch (rel) {
			case ENEMIES: if (m_affiliation & ALLOW_ENEMIES) return m_match; break;
			case NEUTRAL: if (m_affiliation & ALLOW_NEUTRAL) return m_match; break;
			case ALLIES: if (m_affiliation & ALLOW_ALLIES) return m_match; break;
		}
		return !m_match;
	}
};
class PartitionFilterOnMap : public PartitionFilter { public: Bool allow(Object *o) { return !o->isOffMap(); } };
class PartitionFilterAlive : public PartitionFilter { public: Bool allow(Object *o) { return !o->isEffectivelyDead(); } };
class PartitionFilterRejectByObjectStatus : public PartitionFilter
{
public:
	ObjectStatusMaskType m_mustBeSet, m_mustBeClear;
	PartitionFilterRejectByObjectStatus(ObjectStatusMaskType s, ObjectStatusMaskType c) : m_mustBeSet(s), m_mustBeClear(c) {}
	Bool allow(Object *o) { unsigned st = o->getStatusBits().m; return !((st & m_mustBeSet.m) == m_mustBeSet.m && (st & m_mustBeClear.m) == 0); }
};
class PartitionFilterInsignificantBuildings : public PartitionFilter
{
public:
	Bool m_allowNonBuildings, m_allowInsignificant;
	PartitionFilterInsignificantBuildings(Bool allowNonBuildings, Bool allowInsignificant) : m_allowNonBuildings(allowNonBuildings), m_allowInsignificant(allowInsignificant) {}
	/* PartitionManager.cpp's, with no object containing anything (getContain() NULL) */
	Bool allow(Object *other)
	{
		if (other->isStructure()) return true;
		return m_allowNonBuildings;
	}
};
class PartitionManager
{
public:
	Object *getClosestObject(const Coord3D *pos, Real radius, DistanceCalculationType dc, PartitionFilter **filters)
	{
		Object *best = NULL; Real bestD = 0;
		for (Int i = 0; i < AP_OBJECTS; ++i) {
			if (!W.o[i].id) continue;
			Object *o = &g_objs[i];
			Bool ok = true;
			for (PartitionFilter **f = filters; *f && ok; ++f) ok = (*f)->allow(o);
			if (!ok) continue;
			Real dx = W.o[i].pos[0] - pos->x, dy = W.o[i].pos[1] - pos->y;
			Real d = dx * dx + dy * dy, r = radius + W.o[i].radius;
			if (d > r * r) continue;
			if (!best || d < bestD) { best = o; bestD = d; }
		}
		return best;
	}
};
PartitionManager g_partition; PartitionManager *ThePartitionManager = &g_partition;
}	/* namespace ap */

/* ---- AIPlayer.h and AIPlayer.cpp, verbatim */
namespace ap {
#define private public
#define protected public
#include "ai_player_h.inc"
#include "ai_player_cpp.inc"
#undef private
#undef protected

/* the AI player of the run: isSkirmishAI() as the scene says (AIPlayer's own is false) */
class ApAIPlayer : public AIPlayer
{
public:
	Bool m_skirmish;
	ApAIPlayer(Player *p, Bool skirmish) : AIPlayer(p), m_skirmish(skirmish) {}
	virtual Bool isSkirmishAI(void) { return m_skirmish; }
};

std::vector<unsigned> *g_out;
void O(unsigned w) { g_out->push_back(w); }
void OF(Real r) { O(bitsOf(r)); }
void OS(const AsciiString &s) { O((unsigned)s.s.size()); for (char c : s.s) O((unsigned char)c); }
void dumpTeam(TeamInQueue *t)
{
	O(t->m_priorityBuild); O(t->m_team ? t->m_team->getID() : 0u); O((unsigned)t->m_frameStarted); O(t->m_sentToStartLocation);
	O(t->m_stopQueueing); O(t->m_reinforcement); O((unsigned)t->m_reinforcementID);
	unsigned n = 0;
	for (WorkOrder *w = t->m_workOrders; w; w = w->m_next) ++n;
	O(n);
	for (WorkOrder *w = t->m_workOrders; w; w = w->m_next) {
		O(w->m_thing ? (unsigned)w->m_thing->m_i : 0xFFFFFFFFu); O((unsigned)w->m_factoryID); O((unsigned)w->m_numCompleted);
		O((unsigned)w->m_numRequired); O(w->m_required); O(w->m_isResourceGatherer);
	}
}
/* the AI player, then the world (objects, teams, players, the ids, the unit id, the random state, player 0's build list) */
void dump(AIPlayer *a)
{
	unsigned n = 0;
	for (TeamInQueue *t = a->getFirstItemIn_TeamBuildQueue(); t; t = t->dlink_next_TeamBuildQueue()) ++n;
	O(n);
	for (TeamInQueue *t = a->getFirstItemIn_TeamBuildQueue(); t; t = t->dlink_next_TeamBuildQueue()) dumpTeam(t);
	n = 0;
	for (TeamInQueue *t = a->getFirstItemIn_TeamReadyQueue(); t; t = t->dlink_next_TeamReadyQueue()) ++n;
	O(n);
	for (TeamInQueue *t = a->getFirstItemIn_TeamReadyQueue(); t; t = t->dlink_next_TeamReadyQueue()) dumpTeam(t);
	O(a->m_readyToBuildTeam); O(a->m_readyToBuildStructure); O((unsigned)a->m_teamTimer); O((unsigned)a->m_structureTimer);
	O((unsigned)a->m_teamSeconds); O((unsigned)a->m_buildDelay); O((unsigned)a->m_teamDelay); O((unsigned)a->m_frameLastBuildingBuilt);
	O((unsigned)a->m_difficulty); O((unsigned)a->m_skillsetSelector); OF(a->m_baseCenter.x); OF(a->m_baseCenter.y); OF(a->m_baseCenter.z);
	O(a->m_baseCenterSet); OF(a->m_baseRadius); O((unsigned)a->m_structuresToRepair[0]); O((unsigned)a->m_structuresToRepair[1]);
	O((unsigned)a->m_repairDozer); OF(a->m_repairDozerOrigin.x); OF(a->m_repairDozerOrigin.y); OF(a->m_repairDozerOrigin.z);
	O((unsigned)a->m_structuresInQueue); O(a->m_dozerQueuedForRepair); O(a->m_dozerIsRepairing); O((unsigned)a->m_bridgeTimer);
	O(a->m_supplySourceAttackCheckFrame); O((unsigned)a->m_attackedSupplyCenter); O((unsigned)a->m_curWarehouseID);
	const unsigned *w = (const unsigned *)W.o;
	for (size_t i = 0; i < sizeof W.o / 4; ++i) O(w[i]);
	w = (const unsigned *)W.tm;
	for (size_t i = 0; i < sizeof W.tm / 4; ++i) O(w[i]);
	w = (const unsigned *)W.pl;
	for (size_t i = 0; i < sizeof W.pl / 4; ++i) O(w[i]);
	O((unsigned)W.nextObjectId); O((unsigned)W.nextTeamId); O(g_unitId); O(g_rand);
	n = 0;
	for (BuildListInfo *b = g_players[0].m_pBuildList; b; b = b->getNext()) ++n;
	O(n);
	for (BuildListInfo *b = g_players[0].m_pBuildList; b; b = b->getNext()) {
		OS(b->m_buildingName); OS(b->m_templateName); OS(b->m_script); OF(b->m_location.x); OF(b->m_location.y); OF(b->m_location.z);
		OF(b->m_rallyPointOffset.x); OF(b->m_rallyPointOffset.y); OF(b->m_angle); O(b->m_isInitiallyBuilt); O(b->m_numRebuilds);
		O((unsigned)b->m_health); O(b->m_unsellable); O(b->m_underConstruction); O(b->m_isSupplyBuilding); O(b->m_priorityBuild);
		O((unsigned)b->m_objectID); O(b->m_objectTimestamp); O((unsigned)b->m_desiredGatherers); O((unsigned)b->m_currentGatherers);
	}
}
AsciiString tmplName(Int i) { return (i >= 0 && i < AP_TEMPLATES) ? nameOf("Tmpl", i) : AsciiString("Nothing"); }
const ThingTemplate *tmplOrNull(Int i) { return (i >= 0 && i < AP_TEMPLATES) ? &g_tmpls[i] : NULL; }
TeamPrototype *protoOrNull(Int i) { return (i >= 0 && i < AP_PROTOS) ? &g_protos[i] : NULL; }
BuildListInfo *entryAt(Int k) { if (k < 0) return NULL; BuildListInfo *b = g_players[0].m_pBuildList; while (b && k-- > 0) b = b->getNext(); return b; }
Coord3D posOf(const ApOp &op) { Coord3D c; c.x = op.x; c.y = op.y; c.z = op.z; return c; }

/* the world at the start of a run */
void setUp(const ApScene *scene)
{
	W = *scene;
	g_seed = (unsigned)W.seed; g_rand = (unsigned)W.seed ^ 0x5bd1e995u; g_unitId = 0;
	for (Int i = 0; i < AP_TEMPLATES; ++i) { g_tmpls[i].m_i = i; g_tmpls[i].m_geom.m_r = W.t[i].radius; }
	for (Int i = 0; i < AP_UPGRADES; ++i) g_upgrades[i].m_i = i;
	for (Int i = 0; i < AP_OBJECTS; ++i) g_objs[i].m_i = i;
	for (Int i = 0; i < AP_TEAMS; ++i) g_teams[i].m_k = i;
	for (Int i = 0; i < AP_PROTOS; ++i) g_protos[i].m_i = i;
	for (Int j = 0; j < MAX_COMMANDS_PER_SET; ++j) g_buttons[j].m_j = j;
	for (Int p = 0; p < AP_PLAYERS; ++p) {
		Player &pl = g_players[p];
		pl.m_i = p; pl.m_money.m_p = p; pl.m_teams.clear();
		for (Int i = 0; i < AP_PROTOS; ++i) if (W.pr[i].player == p) pl.m_teams.push_back(&g_protos[i]);
		while (pl.m_pBuildList) { BuildListInfo *n = pl.m_pBuildList->getNext(); delete pl.m_pBuildList; pl.m_pBuildList = n; }
	}
	/* player 0's build list, the scene's entries in order */
	BuildListInfo **tail = &g_players[0].m_pBuildList;
	for (Int i = 0; i < W.numBuild; ++i) {
		const ApBuild &e = W.b[i];
		BuildListInfo *b = new BuildListInfo;
		b->m_buildingName = buildingName(e.name); b->m_templateName = e.tmpl == -1 ? AsciiString() : tmplName(e.tmpl < 0 ? -1 : e.tmpl);
		b->m_script = scriptName(e.script);
		b->m_location.x = e.loc[0]; b->m_location.y = e.loc[1]; b->m_location.z = e.loc[2]; b->m_angle = e.angle;
		b->m_rallyPointOffset.x = e.rally[0]; b->m_rallyPointOffset.y = e.rally[1];
		b->m_isInitiallyBuilt = (e.flags & AP_B_INITIALLY) != 0; b->m_numRebuilds = (UnsignedInt)e.numRebuilds;
		b->m_health = e.health; b->m_unsellable = (e.flags & AP_B_UNSELLABLE) != 0; b->m_underConstruction = (e.flags & AP_B_UNDER_CONSTRUCTION) != 0;
		b->m_isSupplyBuilding = (e.flags & AP_B_SUPPLY) != 0; b->m_priorityBuild = (e.flags & AP_B_PRIORITY) != 0;
		b->m_objectID = e.objectId; b->m_objectTimestamp = (UnsignedInt)e.timestamp; b->m_desiredGatherers = e.desired; b->m_currentGatherers = e.current;
		*tail = b; tail = &b->m_nextBuildList;
	}
	/* the AI data */
	TAiData &d = g_ai.m_data;
	d.m_structureSeconds = W.structureSeconds; d.m_teamSeconds = W.teamSeconds; d.m_resourcesWealthy = W.resourcesWealthy;
	d.m_resourcesPoor = W.resourcesPoor; d.m_structuresWealthyMod = W.structuresWealthyMod; d.m_teamWealthyMod = W.teamWealthyMod;
	d.m_structuresPoorMod = W.structuresPoorMod; d.m_teamPoorMod = W.teamPoorMod; d.m_teamResourcesToBuild = W.teamResourcesToBuild;
	d.m_maxRecruitDistance = W.maxRecruitDistance; d.m_supplyCenterSafeRadius = W.safeRadius; d.m_rebuildDelaySeconds = W.rebuildDelaySeconds;
	d.m_sideInfo = NULL;
	for (Int s = AP_SIDES - 1; s >= 0; --s) {
		AISideInfo &si = g_ai.m_sides[s];
		const ApSide &as = W.sides[s];
		si.m_side = sideName(as.side); si.m_easy = as.easy; si.m_normal = as.normal; si.m_hard = as.hard;
		TSkillSet *sets[5] = { &si.m_skillSet1, &si.m_skillSet2, &si.m_skillSet3, &si.m_skillSet4, &si.m_skillSet5 };
		for (Int k = 0; k < 5; ++k) {
			memset(sets[k], 0, sizeof *sets[k]);
			sets[k]->m_numSkills = as.numSkills[k];
			for (Int j = 0; j < 4; ++j) sets[k]->m_skills[j] = as.skills[k][j];
		}
		si.m_next = d.m_sideInfo; d.m_sideInfo = &si;
	}
	g_globalData.m_debugAI = W.debugAI; g_globalData.m_baseValuePerSupplyBox = W.boxValue;
}

/* one op: its answer words */
void runOp(ApAIPlayer *&a, const ApOp &op)
{
	Coord3D pos = posOf(op);
	switch (op.kind) {
		case 0: a->update(); break;
		case 1: a->newMap(); break;
		case 2: { Object *u = objById(op.b); if (!u) { O(0xFFFFFFFFu); break; } a->onUnitProduced(objById(op.a), u); break; }
		case 3: { Object *b = objById(op.b); if (!b) { O(0xFFFFFFFFu); break; } a->onStructureProduced(objById(op.a), b); break; }
		case 4: a->buildSpecificAITeam(protoOrNull(op.a), op.b != 0); break;
		case 5: a->recruitSpecificAITeam(protoOrNull(op.a), op.x); break;
		case 6: a->buildAIBaseDefense(op.a != 0); break;
		case 7: a->buildAIBaseDefenseStructure(tmplName(op.a), op.b != 0); break;
		case 8: a->buildSpecificAIBuilding(tmplName(op.a)); break;
		case 9: a->repairStructure(op.a); break;
		case 10: a->selectSkillset(op.a); break;
		case 11: a->buildBySupplies(op.a, tmplName(op.b)); break;
		case 12: { Team *t = teamById((TeamID)op.b); if (!t) { O(0xFFFFFFFFu); break; } a->buildSpecificBuildingNearestTeam(tmplName(op.a), t); break; }
		case 13: a->buildUpgrade(op.a >= 0 ? nameOf("Up", op.a) : AsciiString("Nothing")); break;
		case 14: a->aiPreTeamDestroy(teamById((TeamID)op.a)); break;
		case 15: O(a->isSupplySourceSafe(op.a)); break;
		case 16: O(a->isSupplySourceAttacked()); break;
		case 17: O(a->isLocationSafe(&pos, tmplOrNull(op.a))); break;
		case 18: { Team *t = teamById((TeamID)op.a); if (!t) { O(0xFFFFFFFFu); break; } a->guardSupplyCenter(t, op.b); break; }
		case 19: { Bool ok = a->calcClosestConstructionZoneLocation(tmplOrNull(op.a), &pos); O(ok); OF(pos.x); OF(pos.y); OF(pos.z); break; }
		case 20: {
			SpecialPowerTemplate sp; sp.m_type = op.a ? SPECIAL_SNEAK_ATTACK : SPECIAL_INVALID;
			Coord3D r; r.x = r.y = r.z = 0;
			Bool ok = a->computeSuperweaponTarget(&sp, &r, op.b, op.x);
			O(ok); OF(r.x); OF(r.y); OF(r.z);
			break;
		}
		case 21: O((unsigned)AIPlayer::getPlayerSuperweaponValue(&pos, op.a, op.w, op.b != 0)); break;
		case 22: { Region2D r; AIPlayer::getPlayerStructureBounds(&r, op.a, op.b != 0); OF(r.lo.x); OF(r.lo.y); OF(r.hi.x); OF(r.hi.y); break; }
		case 23: W.frame += op.a; break;
		case 24: if (op.a >= 0 && op.a < AP_OBJECTS) W.o[op.a].id = 0; break;
		case 25: {
			/* a < 0: the AI player's repair dozer */
			Object *o = op.a < 0 ? objById(a->m_repairDozer) : NULL;
			Int k = op.a < 0 ? (o ? o->m_i : -1) : op.a;
			if (k >= 0 && k < AP_OBJECTS) { W.o[k].flags ^= op.b & AP_MUTABLE_FLAGS; W.o[k].status ^= op.c; }
			break;
		}
		case 26: if (op.a >= 0 && op.a < AP_PLAYERS) W.pl[op.a].money = op.b; break;
		case 27: {
			/* save, a new AI player for the same player, load */
			Xfer s(XFER_SAVE);
			a->xfer(&s);
			O((unsigned)s.m_bytes.size());
			for (unsigned char c : s.m_bytes) O(c);
			Player *p = a->m_player;
			delete a;
			a = new ApAIPlayer(p, W.skirmish != 0);
			Xfer l(XFER_LOAD);
			l.m_bytes = s.m_bytes;
			a->xfer(&l);
			a->loadPostProcess();
			break;
		}
		case 28: if (op.a >= 0 && op.a < AP_TEAMS) W.tm[op.a].idle ^= 1; break;
		case 29: O((unsigned)a->getAIDifficulty()); break;
		case 30: a->processBaseBuilding(); break;
		case 31: a->processTeamBuilding(); break;
		case 32: a->queueUnits(); break;
		case 33: a->checkReadyTeams(); break;
		case 34: a->checkQueuedTeams(); break;
		case 35: a->doUpgradesAndSkills(); break;
		case 36: a->queueSupplyTruck(); break;
		case 37: a->queueDozer(); break;
		case 38: a->updateBridgeRepair(); break;
		case 39: { Object *o = a->findDozer(&pos); O(o ? (unsigned)o->getID() : 0u); break; }
		case 40: { Object *o = a->findSupplyCenter(op.a); O(o ? (unsigned)o->getID() : 0u); break; }
		case 41: { Coord3D c; Real r; a->computeCenterAndRadiusOfBase(&c, &r); OF(c.x); OF(c.y); OF(c.z); OF(r); break; }
		case 42: O(a->dozerInQueue()); break;
		case 43: O(a->selectTeamToBuild()); break;
		case 44: O(a->selectTeamToReinforce(op.a)); break;
		case 45: O(a->isAGoodIdeaToBuildTeam(&g_protos[op.a])); break;
		case 46: { Bool money = false; Bool ok = a->isPossibleToBuildTeam(&g_protos[op.a], op.b != 0, money); O(ok); O(money); break; }
		case 47: { Object *o = a->findFactory(&g_tmpls[op.a], op.b != 0); O(o ? (unsigned)o->getID() : 0u); break; }
		case 48: a->clearTeamsInQueue(); break;
		case 49: case 50: {
			BuildListInfo *e = entryAt(op.b);
			if (!e) { O(0xFFFFFFFFu); break; }
			Object *o = op.kind == 49 ? a->buildStructureNow(&g_tmpls[op.a], e) : a->buildStructureWithDozer(&g_tmpls[op.a], e);
			O(o ? (unsigned)o->getID() : 0u);
			break;
		}
		case 51: { BuildListInfo *e = entryAt(op.b); Object *o = objById(op.a); if (!e || !o) { O(0xFFFFFFFFu); break; } a->checkForSupplyCenter(e, o); break; }
		case 52: a->doBaseBuilding(); break;
		case 53: a->doTeamBuilding(); break;
		case 54: {
			WorkOrder *w = newInstance(WorkOrder);
			w->m_thing = &g_tmpls[op.a]; w->m_required = op.c != 0;
			Bool ok = a->startTraining(w, op.b != 0, nameOf("Proto", 0));
			O(ok); O((unsigned)w->m_factoryID);
			w->deleteInstance();
			break;
		}
		case 55: if (op.a >= 0 && op.a < AP_PLAYERS) W.pl[op.a].flags ^= op.b; break;
		case 56: {
			/* a unit for the first work order in a factory (build queue order): a new object of its template at the factory,
			** player 0's, in its default team, with the template's new flags and op.b's AI flags; onUnitProduced */
			WorkOrder *w = NULL;
			for (TeamInQueue *t = a->getFirstItemIn_TeamBuildQueue(); t && !w; t = t->dlink_next_TeamBuildQueue())
				for (WorkOrder *o = t->m_workOrders; o; o = o->m_next)
					if (o->m_factoryID != INVALID_ID && o->m_numCompleted < o->m_numRequired) { w = o; break; }
			Object *f = w ? objById(w->m_factoryID) : NULL;
			Int slot = -1;
			for (Int i = 0; i < AP_OBJECTS; ++i) if (!W.o[i].id) { slot = i; break; }
			if (!f || slot < 0) { O(0xFFFFFFFFu); break; }
			ApObj &u = W.o[slot];
			memset(&u, 0, sizeof u);
			u.id = W.nextObjectId++; u.tmpl = w->m_thing->m_i; u.player = 0; u.team = W.pl[0].defaultTeam;
			u.flags = W.t[u.tmpl].newFlags | (op.b & (AP_F_AI | AP_F_SUPPLY_AI | AP_F_IDLE | AP_F_DOZER_AI));
			u.pos[0] = f->s().pos[0]; u.pos[1] = f->s().pos[1]; u.pos[2] = f->s().pos[2]; u.radius = W.t[u.tmpl].radius; u.cmdButtons = -1;
			O((unsigned)u.id);
			a->onUnitProduced(f, &g_objs[slot]);
			break;
		}
		case 58: {
			/* a capture: the object's player (a < 0: the factory of the first work order in a factory) */
			Int k = op.a;
			if (op.a < 0) {
				k = -1;
				for (TeamInQueue *t = a->getFirstItemIn_TeamBuildQueue(); t && k < 0; t = t->dlink_next_TeamBuildQueue())
					for (WorkOrder *o = t->m_workOrders; o; o = o->m_next)
						if (o->m_factoryID != INVALID_ID) { Object *f = objById(o->m_factoryID); k = f ? f->m_i : -2; break; }
			}
			if (k >= 0 && k < AP_OBJECTS) W.o[k].player = op.b;
			break;
		}
		case 57: {
			/* a < 0: the AI player's first structure to repair */
			Object *o = op.a < 0 && a->m_structuresInQueue > 0 ? objById(a->m_structuresToRepair[0]) : NULL;
			Int k = op.a < 0 ? (o ? o->m_i : -1) : op.a;
			if (k >= 0 && k < AP_OBJECTS) W.o[k].damageState = op.b;
			break;
		}
	}
}
}	/* namespace ap */

/* A run: the scene, a new AI player for player 0, then each op: its answer (length, words) and the dump (length, words). The log
** of the whole run. Returns 0, or 1 when a load threw. The out and log lengths may exceed their caps (nothing past a cap is
** written). */
extern "C" int ref_ai_player_run(const ap::ApScene *scene, const ap::ApOp *ops, int nOps, unsigned *out, int cap, int *outLen,
	unsigned *log, int logCap, int *logLen)
{
	using namespace ap;
	std::vector<unsigned> o, lg;
	g_out = &o; g_log = &lg;
	setUp(scene);
	int status = 0;
	ApAIPlayer *a = new ApAIPlayer(&g_players[0], W.skirmish != 0);
	try {
		for (int i = 0; i < nOps; ++i) {
			size_t at = o.size(); O(0);
			runOp(a, ops[i]);
			o[at] = (unsigned)(o.size() - at - 1);
			at = o.size(); O(0);
			dump(a);
			o[at] = (unsigned)(o.size() - at - 1);
		}
	} catch (...) {
		status = 1;
	}
	delete a;
	*outLen = (int)o.size(); *logLen = (int)lg.size();
	memcpy(out, o.data(), sizeof(unsigned) * std::min((size_t)cap, o.size()));
	memcpy(log, lg.data(), sizeof(unsigned) * std::min((size_t)logCap, lg.size()));
	return status;
}

/* sizeof(ApScene), for the Rust's layout check */
extern "C" int ref_ai_player_scene_size(void) { return (int)sizeof(ap::ApScene); }
