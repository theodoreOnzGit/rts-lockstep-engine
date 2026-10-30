/* The world of ScriptConditions.cpp, the ScriptConditions fixture only (differential tests, p1w13-sonnet, layer 11 batch C1;
** differential/wrap_conditions.cpp). Every header the file includes is this one (the stand-ins of differential/shim_conditions); the
** real GameLogic/ScriptConditions.h is the file's own. Everything is compiled inside namespace scw (the wrapper opens it), so that no
** symbol collides with another fixture's.
**
** The world answers from a configuration of words (`g_cfg`, laid out in tests/differential_script_conditions_c1.rs, whose host answers
** from the same words with the same functions): players, team prototypes and teams, sixteen objects, trigger areas, and the answers of
** the stores, the partition and the script engine, most of them a hash of a seed and the question. Every call with an effect, and
** every script-engine completion query, writes a line (scLog). */
#ifndef SC_WORLD_H
#define SC_WORLD_H
#include <string>
#include <vector>
#include <list>
#include <cstdio>
#include <cstring>
#include <cstdarg>

typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
typedef float Real;
typedef char Char;
typedef Int ObjectID;
typedef UnsignedInt PlayerMaskType;
typedef Int NameKeyType;
#ifndef TRUE
#define TRUE true
#define FALSE false
#endif
#ifndef NULL
#define NULL 0
#endif
#define NEW new
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(c, x)
#define DEBUG_CRASH(x)
#define INVALID_ID 0
#define LOGICFRAMES_PER_SECOND 30

void scLog(const char *fmt, ...);

/* the configuration (see the test) */
enum { SC_CFG = 160, SC_PLAYERS = 8, SC_PROTOS = 8, SC_TEAMS = 16, SC_OBJS = 16 };
extern unsigned g_cfg[SC_CFG];
inline unsigned scHash(unsigned seed, unsigned a, unsigned b)
{
	unsigned x = seed ^ (a * 0x9E3779B1u) ^ (b * 0x85EBCA77u);
	x ^= x >> 15;
	x *= 0x2C1B3C6Du;
	x ^= x >> 12;
	x *= 0x297A2D39u;
	x ^= x >> 15;
	return x;
}
inline unsigned scStrHash(const char *s)
{
	unsigned h = 2166136261u;
	for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; }
	return h;
}
/* the number after `prefix` in `s` ("unit12" -> 12), or -1 */
inline int scNum(const char *s, const char *prefix)
{
	size_t n = strlen(prefix);
	if (strncmp(s, prefix, n) != 0 || !s[n]) return -1;
	int v = 0;
	for (const char *p = s + n; *p; ++p) {
		if (*p < '0' || *p > '9' || v > 1000) return -1;
		v = v * 10 + (*p - '0');
	}
	return v;
}
#define SC_SEED_A g_cfg[7]
#define SC_SEED_B g_cfg[8]
#define SC_SEED_C g_cfg[9]

class AsciiString
{
public:
	AsciiString() {}
	AsciiString(const char *s) : m_s(s) {}
	const char *str() const { return m_s.c_str(); }
	Bool isEmpty() const { return m_s.empty(); }
	Bool operator==(const AsciiString &o) const { return m_s == o.m_s; }
	Bool operator!=(const AsciiString &o) const { return m_s != o.m_s; }
	Bool operator==(const char *o) const { return m_s == o; }
	Bool operator!=(const char *o) const { return m_s != o; }
	std::string m_s;
};
inline AsciiString scName(const char *prefix, int n) { char b[32]; snprintf(b, sizeof(b), "%s%d", prefix, n); return AsciiString(b); }

class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &) { return 77; }
};
extern NameKeyGenerator *TheNameKeyGenerator;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init(void) = 0;
	virtual void reset(void) = 0;
	virtual void update(void) = 0;
};

/* the memory pool: plain new and delete (deleteInstance on NULL does nothing, as the pool's) */
class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { if (this) delete this; }
};
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(cls, name) protected: virtual ~cls(); public:
#define newInstance(cls) new cls
class MemoryPoolObjectHolder
{
public:
	MemoryPoolObjectHolder(MemoryPoolObject *o) : m_o(o) {}
	~MemoryPoolObjectHolder() { m_o->deleteInstance(); }
	MemoryPoolObject *m_o;
};

struct Coord3D { Real x, y, z; };
struct ICoord3D { Int x, y, z; };

/* KindOf.h (the values the file names), the mask a bitset of 128 */
enum KindOfType { KINDOF_STRUCTURE = 7, KINDOF_PROJECTILE = 22, KINDOF_MP_COUNT_FOR_VICTORY = 33, KINDOF_CRATE = 44, KINDOF_TECH_BUILDING = 65,
	KINDOF_INERT = 84 };
class KindOfMaskType
{
public:
	KindOfMaskType() { memset(m_bits, 0, sizeof(m_bits)); }
	void set(Int i, Int value = 1) { if (value) m_bits[i / 32] |= 1u << (i % 32); else m_bits[i / 32] &= ~(1u << (i % 32)); }
	Bool test(Int i) const { return (m_bits[i / 32] >> (i % 32) & 1) != 0; }
	unsigned m_bits[4];
};
extern KindOfMaskType KINDOFMASK_NONE;
inline KindOfMaskType MAKE_KINDOF_MASK(Int k) { KindOfMaskType m; m.set(k); return m; }

/* ObjectStatusTypes.h (the values the file names), the mask a bitset of 64 */
enum ObjectStatusTypes { OBJECT_STATUS_UNDER_CONSTRUCTION = 3, OBJECT_STATUS_STEALTHED = 16, OBJECT_STATUS_DETECTED = 17,
	OBJECT_STATUS_DISGUISED = 43 };
class ObjectStatusMaskType
{
public:
	ObjectStatusMaskType() : m_bits(0) {}
	Bool test(Int i) const { return (m_bits >> i & 1) != 0; }
	Bool testForAny(const ObjectStatusMaskType &o) const { return (m_bits & o.m_bits) != 0; }
	unsigned long long m_bits;
};
inline ObjectStatusMaskType MAKE_OBJECT_STATUS_MASK(Int a) { ObjectStatusMaskType m; m.m_bits = 1ull << a; return m; }
inline ObjectStatusMaskType MAKE_OBJECT_STATUS_MASK2(Int a, Int b) { ObjectStatusMaskType m; m.m_bits = (1ull << a) | (1ull << b); return m; }
/* an object's status word: bit i is status i, bit 31 is OBJECT_STATUS_DISGUISED */
inline ObjectStatusMaskType scStatus(unsigned w)
{
	ObjectStatusMaskType m;
	m.m_bits = (unsigned long long)(w & 0x7fffffffu);
	if (w >> 31) m.m_bits |= 1ull << OBJECT_STATUS_DISGUISED;
	return m;
}

enum DisabledType { DISABLED_HELD = 3, DISABLED_UNMANNED = 5 };
enum ObjectShroudStatus { OBJECTSHROUD_INVALID = 0, OBJECTSHROUD_CLEAR, OBJECTSHROUD_PARTIAL_CLEAR, OBJECTSHROUD_FOGGED, OBJECTSHROUD_SHROUDED,
	OBJECTSHROUD_INVALID_BUT_PREVIOUS_VALID };
enum AIStateType { AI_EXIT = 37 };
typedef Int ScienceType;
enum { SCIENCE_INVALID = -1 };
typedef Int SpecialPowerType;
enum Relationship { ENEMIES = 0, NEUTRAL, ALLIES };

/* Scripts.h: THIS_TEAM, THIS_PLAYER_ENEMY, Parameter and Condition */
#define THIS_TEAM "<This Team>"
#define THIS_PLAYER_ENEMY "<This Player's Enemy>"
class Parameter
{
public:
	enum ParameterType { SIDE = 11 };
	enum { LESS_THAN = 0, LESS_EQUAL, EQUAL, GREATER_EQUAL, GREATER, NOT_EQUAL };
	enum { REL_ENEMY = ENEMIES, REL_NEUTRAL = NEUTRAL, REL_FRIEND = ALLIES };
	Int getParameterType() const { return m_type; }
	Int getInt() const { return m_int; }
	Real getReal() const { return m_real; }
	AsciiString getString() const { return m_string; }
	void friend_setInt(Int i) { m_int = i; }
	ObjectStatusMaskType getStatus() const { return m_status; }
	Int m_type;
	Int m_int;
	Real m_real;
	AsciiString m_string;
	ObjectStatusMaskType m_status;
};
enum { MAX_PARMS = 12 };
class Condition
{
public:
	enum ConditionType
	{
		CONDITION_FALSE, COUNTER, FLAG, CONDITION_TRUE, TIMER_EXPIRED, PLAYER_ALL_DESTROYED, PLAYER_ALL_BUILDFACILITIES_DESTROYED,
		TEAM_INSIDE_AREA_PARTIALLY, TEAM_DESTROYED, CAMERA_MOVEMENT_FINISHED, TEAM_HAS_UNITS, TEAM_STATE_IS, TEAM_STATE_IS_NOT,
		NAMED_INSIDE_AREA, NAMED_OUTSIDE_AREA, NAMED_DESTROYED, NAMED_NOT_DESTROYED, TEAM_INSIDE_AREA_ENTIRELY, TEAM_OUTSIDE_AREA_ENTIRELY,
		NAMED_ATTACKED_BY_OBJECTTYPE, TEAM_ATTACKED_BY_OBJECTTYPE, NAMED_ATTACKED_BY_PLAYER, TEAM_ATTACKED_BY_PLAYER, BUILT_BY_PLAYER,
		NAMED_CREATED, TEAM_CREATED, PLAYER_HAS_CREDITS, NAMED_DISCOVERED, TEAM_DISCOVERED, MISSION_ATTEMPTS, NAMED_OWNED_BY_PLAYER,
		TEAM_OWNED_BY_PLAYER, PLAYER_HAS_N_OR_FEWER_BUILDINGS, PLAYER_HAS_POWER, NAMED_REACHED_WAYPOINTS_END, TEAM_REACHED_WAYPOINTS_END,
		NAMED_SELECTED, NAMED_ENTERED_AREA, NAMED_EXITED_AREA, TEAM_ENTERED_AREA_ENTIRELY, TEAM_ENTERED_AREA_PARTIALLY,
		TEAM_EXITED_AREA_ENTIRELY, TEAM_EXITED_AREA_PARTIALLY, MULTIPLAYER_ALLIED_VICTORY, MULTIPLAYER_ALLIED_DEFEAT,
		MULTIPLAYER_PLAYER_DEFEAT, PLAYER_HAS_NO_POWER, HAS_FINISHED_VIDEO, HAS_FINISHED_SPEECH, HAS_FINISHED_AUDIO,
		BUILDING_ENTERED_BY_PLAYER, ENEMY_SIGHTED, UNIT_HEALTH, BRIDGE_REPAIRED, BRIDGE_BROKEN, NAMED_DYING, NAMED_TOTALLY_DEAD,
		PLAYER_HAS_OBJECT_COMPARISON, OBSOLETE_SCRIPT_1, OBSOLETE_SCRIPT_2, PLAYER_TRIGGERED_SPECIAL_POWER, PLAYER_COMPLETED_SPECIAL_POWER,
		PLAYER_MIDWAY_SPECIAL_POWER, PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED, PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED,
		PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED, DEFUNCT_PLAYER_SELECTED_GENERAL, DEFUNCT_PLAYER_SELECTED_GENERAL_FROM_NAMED,
		PLAYER_BUILT_UPGRADE, PLAYER_BUILT_UPGRADE_FROM_NAMED, PLAYER_DESTROYED_N_BUILDINGS_PLAYER, UNIT_COMPLETED_SEQUENTIAL_EXECUTION,
		TEAM_COMPLETED_SEQUENTIAL_EXECUTION, PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA, PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA,
		UNIT_EMPTIED, TYPE_SIGHTED, NAMED_BUILDING_IS_EMPTY, PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS, UNIT_HAS_OBJECT_STATUS,
		TEAM_ALL_HAS_OBJECT_STATUS, TEAM_SOME_HAVE_OBJECT_STATUS, PLAYER_POWER_COMPARE_PERCENT, PLAYER_EXCESS_POWER_COMPARE_VALUE,
		SKIRMISH_SPECIAL_POWER_READY, SKIRMISH_VALUE_IN_AREA, SKIRMISH_PLAYER_FACTION, SKIRMISH_SUPPLIES_VALUE_WITHIN_DISTANCE,
		SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE, SKIRMISH_COMMAND_BUTTON_READY_ALL, SKIRMISH_COMMAND_BUTTON_READY_PARTIAL,
		SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS, SKIRMISH_PLAYER_HAS_PREREQUISITE_TO_BUILD, SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED,
		SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS, SKIRMISH_NAMED_AREA_EXIST, SKIRMISH_PLAYER_HAS_UNITS_IN_AREA,
		SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER, SKIRMISH_PLAYER_IS_OUTSIDE_AREA, SKIRMISH_PLAYER_HAS_DISCOVERED_PLAYER,
		PLAYER_ACQUIRED_SCIENCE, PLAYER_HAS_SCIENCEPURCHASEPOINTS, PLAYER_CAN_PURCHASE_SCIENCE, MUSIC_TRACK_HAS_COMPLETED,
		PLAYER_LOST_OBJECT_TYPE, SUPPLY_SOURCE_SAFE, SUPPLY_SOURCE_ATTACKED, START_POSITION_IS, NAMED_HAS_FREE_CONTAINER_SLOTS,
		NUM_ITEMS
	};
	enum ConditionType getConditionType(void) { return m_conditionType; }
	Int getNumParameters(void) { return m_numParms; }
	Parameter *getParameter(Int ndx) { if (ndx >= 0 && ndx < m_numParms) return m_parms[ndx]; return NULL; }
	Int getCustomData(void) { return m_customData; }
	void setCustomData(Int val) { m_customData = val; }
	UnsignedInt getCustomFrame(void) { return m_customFrame; }
	void setCustomFrame(UnsignedInt frame) { m_customFrame = frame; }
	enum ConditionType m_conditionType;
	Int m_numParms;
	Parameter *m_parms[MAX_PARMS];
	Int m_customData;
	UnsignedInt m_customFrame;
};

class Object;
class Team;
class Player;
class ThingTemplate;
class PolygonTrigger;

/* --- templates: "tmplK" for K below cfg[11] & 255 --- */
class ThingTemplate
{
public:
	AsciiString getName() const { return m_name; }
	Int friend_getBuildCost() const { return m_index * 100 + 50; }
	AsciiString m_name;
	Int m_index;
};
extern ThingTemplate g_templates[8];
inline int scNumTemplates() { int n = (int)(g_cfg[11] & 255); return n > 8 ? 8 : n; }
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name)
	{
		int k = scNum(name.str(), "tmpl");
		return k >= 0 && k < scNumTemplates() ? &g_templates[k] : NULL;
	}
};
extern ThingFactory *TheThingFactory;

/* ObjectTypes.h/.cpp: the list (its logic as ObjectTypes.cpp's, verified in tests/differential_object_types.rs) */
class ObjectTypes : public MemoryPoolObject
{
public:
	void addObjectType(const AsciiString &objectType) { if (isInSet(objectType)) return; m_objectTypes.push_back(objectType); }
	Bool isInSet(const AsciiString &objectType) const
	{
		for (size_t i = 0; i < m_objectTypes.size(); ++i) if (m_objectTypes[i] == objectType) return TRUE;
		return FALSE;
	}
	Bool isInSet(const ThingTemplate *objectType) const { if (!objectType) return FALSE; return isInSet(objectType->getName()); }
	Int prepForPlayerCounting(std::vector<const ThingTemplate *> &templates, std::vector<Int> &counts)
	{
		for (size_t i = 0; i < m_objectTypes.size(); ++i) {
			const ThingTemplate *t = TheThingFactory->findTemplate(m_objectTypes[i]);
			if (t) templates.push_back(t);
		}
		Int retVal = (Int)templates.size();
		counts.resize(retVal);
		return retVal;
	}
	Bool canBuildAny(Player *player);
	AsciiString m_listName;
	std::vector<AsciiString> m_objectTypes;
};

/* --- trigger areas: "trigK", K < 8, when bit K of cfg[6] --- */
class PolygonTrigger
{
public:
	Bool pointInTrigger(ICoord3D &p) const { return (scHash(SC_SEED_B, (unsigned)(m_index * 1000 + p.x), (unsigned)(p.y * 31 + p.z)) & 1) != 0; }
	void getCenterPoint(Coord3D *c) const { c->x = m_index * 10.0f; c->y = m_index * 5.0f; c->z = 0.0f; }
	Real getRadius() const { return m_index * 2.0f + 1.0f; }
	Int m_index;
};
extern PolygonTrigger g_triggers[8];

/* --- the player list --- */
class Money { public: UnsignedInt countMoney() const { return m_money; } UnsignedInt m_money; };
class Energy
{
public:
	Bool hasSufficientPower() const { return (m_w0 >> 2 & 1) != 0; }
	Real getEnergySupplyRatio() const { return (Real)(m_w2 >> 24) / 100.0f; }
	Int getProduction() const { return (Int)(m_w3 & 0xffff); }
	Int getConsumption() const { return (Int)(m_w3 >> 16); }
	unsigned m_w0, m_w2, m_w3;
};
class TeamPrototype;
class Player
{
public:
	typedef std::list<TeamPrototype *> PlayerTeamList;
	unsigned w(int k) const { return g_cfg[16 + 4 * m_index + k]; }
	Int getPlayerIndex() const { return m_index; }
	PlayerMaskType getPlayerMask() const { return 1u << m_index; }
	Bool hasAnyObjects() const { return (w(0) & 1) != 0; }
	Bool hasAnyBuildFacility() const { return (w(0) >> 1 & 1) != 0; }
	Money *getMoney() { m_money.m_money = w(1); return &m_money; }
	Int countBuildings() const { return (Int)(w(2) & 255); }
	Int countObjects(KindOfMaskType must, KindOfMaskType mustNot)
	{
		/* (asked with the faction buildings' mask) */
		return must.test(KINDOF_MP_COUNT_FOR_VICTORY) && must.test(KINDOF_STRUCTURE) ? (Int)(w(2) >> 8 & 255) : -1;
	}
	Energy *getEnergy() { m_energy.m_w0 = w(0); m_energy.m_w2 = w(2); m_energy.m_w3 = w(3); return &m_energy; }
	const PlayerTeamList *getPlayerTeams() const { return &m_teams; }
	void countObjectsByThingTemplate(Int numTmplates, const ThingTemplate *const *things, Bool ignoreDead, Int *counts) const;
	Int getSciencePurchasePoints() const { return (Int)(w(0) >> 24); }
	Bool isCapableOfPurchasingScience(ScienceType science) const { return (scHash(SC_SEED_C, (unsigned)m_index, 1300u + (unsigned)science) & 1) != 0; }
	AsciiString getSide() const { return scName("side", (int)(w(0) >> 4 & 15)); }
	Int getSupplyBoxValue() const { return (Int)(w(0) >> 12 & 15); }
	Bool isSupplySourceSafe(Int minSupplies)
	{
		Bool r = minSupplies <= (Int)(w(2) >> 16 & 255);
		scLog("supplysafe %d %d %d", m_index, minSupplies, r ? 1 : 0);
		return r;
	}
	Bool isSupplySourceAttacked() { Bool r = (w(0) >> 3 & 1) != 0; scLog("supplyattacked %d %d", m_index, r ? 1 : 0); return r; }
	Int getMpStartIndex() const { return (Int)(w(0) >> 8 & 15); }
	Bool getAttackedBy(Int idx) const { return idx >= 0 && idx < 8 && (w(0) >> (16 + idx) & 1) != 0; }
	Bool canBuild(const ThingTemplate *t) const { return (scHash(SC_SEED_C, (unsigned)m_index, 1000u + (unsigned)t->m_index) & 1) != 0; }
	Int m_index;
	PlayerTeamList m_teams;
	Money m_money;
	Energy m_energy;
};
inline Bool ObjectTypes::canBuildAny(Player *player)
{
	for (size_t i = 0; i < m_objectTypes.size(); ++i) {
		const ThingTemplate *t = TheThingFactory->findTemplate(m_objectTypes[i]);
		if (t && player->canBuild(t)) return TRUE;
	}
	return FALSE;
}
inline int scNumPlayers() { int n = (int)(g_cfg[4] & 15); return n > SC_PLAYERS ? SC_PLAYERS : n; }
extern Player g_players[SC_PLAYERS];
class PlayerList
{
public:
	Player *getPlayerFromMask(PlayerMaskType mask)
	{
		for (int i = 0; i < scNumPlayers(); ++i) if (g_players[i].getPlayerMask() == mask) return &g_players[i];
		return NULL;
	}
	Player *getNeutralPlayer() { int n = (int)(g_cfg[4] >> 8 & 15); return n < scNumPlayers() ? &g_players[n] : NULL; }
};
extern PlayerList *ThePlayerList;

/* the list iterators of Team.h */
template <class T> class DLINK_ITERATOR
{
public:
	DLINK_ITERATOR(const std::vector<T *> *v) : m_v(v), m_i(0) {}
	Bool done() const { return m_i >= m_v->size(); }
	void advance() { ++m_i; }
	T *cur() const { return (*m_v)[m_i]; }
	const std::vector<T *> *m_v;
	size_t m_i;
};

/* --- teams: sixteen instances; eight prototypes "teamK" --- */
class Team
{
public:
	unsigned w() const { return g_cfg[64 + m_index]; }
	unsigned hs(unsigned trig, unsigned which) const { return scHash(SC_SEED_A, (unsigned)m_index * 8u + trig, which); }
	Bool hasAnyObjects() const { return (w() & 1) != 0; }
	Bool hasAnyUnits() const { return (w() >> 1 & 1) != 0; }
	Bool isCreated() const { return (w() >> 2 & 1) != 0; }
	Bool didEnterOrExit() const { return (w() >> 3 & 1) != 0; }
	AsciiString getState() const { return scName("state", (int)(w() >> 4 & 15)); }
	Player *getControllingPlayer() const { int p = (int)(w() >> 8 & 15); return p < scNumPlayers() ? &g_players[p] : NULL; }
	AsciiString getName() const { return m_name; }
	Bool someInsideSomeOutside(PolygonTrigger *t, UnsignedInt which) const { return (hs((unsigned)t->m_index, which) & 1) != 0; }
	Bool allInside(PolygonTrigger *t, UnsignedInt which) const { return (hs((unsigned)t->m_index, which) & 2) != 0; }
	Bool didAllEnter(PolygonTrigger *t, UnsignedInt which) const { return (hs((unsigned)t->m_index, which) & 4) != 0; }
	Bool didPartialEnter(PolygonTrigger *t, UnsignedInt which) const { return (hs((unsigned)t->m_index, which) & 8) != 0; }
	Bool didAllExit(PolygonTrigger *t, UnsignedInt which) const { return (hs((unsigned)t->m_index, which) & 16) != 0; }
	Bool didPartialExit(PolygonTrigger *t, UnsignedInt which) const { return (hs((unsigned)t->m_index, which) & 32) != 0; }
	DLINK_ITERATOR<Object> iterate_TeamMemberList() { return DLINK_ITERATOR<Object>(&m_members); }
	Int m_index;
	AsciiString m_name;
	std::vector<Object *> m_members;
};
class TeamPrototype
{
public:
	DLINK_ITERATOR<Team> iterate_TeamInstanceList() { return DLINK_ITERATOR<Team>(&m_instances); }
	std::vector<Team *> m_instances;
};
extern TeamPrototype g_protos[SC_PROTOS];
extern Team g_teams[SC_TEAMS];
class TeamFactory
{
public:
	TeamPrototype *findTeamPrototype(const AsciiString &name)
	{
		int k = scNum(name.str(), "team");
		return k >= 0 && k < SC_PROTOS ? &g_protos[k] : NULL;
	}
};
extern TeamFactory *TheTeamFactory;

/* --- objects: sixteen, id = index + 1, "unitK" --- */
struct DamageInfoInput { const ThingTemplate *m_sourceTemplate; ObjectID m_sourceID; PlayerMaskType m_sourcePlayerMask; };
struct DamageInfo { DamageInfoInput in; };
class BodyModuleInterface
{
public:
	Real getHealth() const { return (Real)(g_cfg[80 + 4 * m_obj + 1] >> 16 & 255); }
	Real getInitialHealth() const { return (Real)(g_cfg[80 + 4 * m_obj + 1] >> 24); }
	const DamageInfo *getLastDamageInfo() const;
	Int m_obj;
	DamageInfo m_info;
};
class ContainModuleInterface
{
public:
	unsigned w() const { return g_cfg[80 + 4 * m_obj + 3]; }
	PlayerMaskType getPlayerWhoEntered() const { int p = (int)(w() >> 9 & 15); return p == 15 ? 0u : 1u << p; }
	UnsignedInt getContainCount() const { return w() & 15; }
	UnsignedInt getContainMax() const { return w() >> 4 & 15; }
	Bool isGarrisonable() const { return (w() >> 8 & 1) != 0; }
	Int m_obj;
};
class Waypoint
{
public:
	AsciiString getPathLabel1() const { return scName("path", m_w); }
	AsciiString getPathLabel2() const { return scName("path", m_w + 1); }
	AsciiString getPathLabel3() const { return scName("path", m_w * 2 % 7); }
	Int m_w;
};
class AIUpdateInterface
{
public:
	const Waypoint *getCompletedWaypoint() const
	{
		int k = (int)(g_cfg[80 + 4 * m_obj + 3] >> 13 & 7);
		if (k == 7) return NULL;
		m_way.m_w = k;
		return &m_way;
	}
	AIStateType getCurrentStateID() const { scLog("aistate %d", m_obj); return AI_EXIT; }
	Int m_obj;
	mutable Waypoint m_way;
};
class SpecialPowerTemplate
{
public:
	SpecialPowerType getSpecialPowerType() const { return m_index + 1; }
	Int m_index;
};
class SpecialPowerModuleInterface
{
public:
	unsigned hs() const { return scHash(SC_SEED_C, (unsigned)m_obj, 400u + (unsigned)m_power); }
	Bool isReady() const { return (hs() & 2) != 0; }
	UnsignedInt getReadyFrame() const { return (hs() >> 8) % 2000u; }
	Int m_obj, m_power;
};
class UpdateModule { public: virtual ~UpdateModule() {} };
class SupplyWarehouseDockUpdate : public UpdateModule
{
public:
	Int getBoxesStored() const { return (Int)((scHash(SC_SEED_B, (unsigned)m_obj, 700u) >> 4) % 20u); }
	Int m_obj;
};
class Object
{
public:
	unsigned w(int k) const { return g_cfg[80 + 4 * m_index + k]; }
	ObjectID getID() const { return m_index + 1; }
	AsciiString getName() const { return scName("unit", m_index); }
	Bool isEffectivelyDead() const { return (w(0) >> 1 & 1) != 0; }
	const Coord3D *getPosition() const
	{
		m_pos.x = (Real)(w(3) >> 16 & 255) * 0.75f;
		m_pos.y = (Real)(w(3) >> 24) * 0.75f;
		m_pos.z = (Real)m_index * 0.25f;
		return &m_pos;
	}
	const ThingTemplate *getTemplate() const { int t = (int)(w(0) >> 12 & 7); return t < 7 ? &g_templates[t] : NULL; }
	Bool isKindOf(KindOfType k) const { return (w(1) >> ((unsigned)k & 15) & 1) != 0; }
	Bool isInside(const PolygonTrigger *t) const { return (scHash(SC_SEED_B, (unsigned)m_index, 200u + (unsigned)t->m_index) & 1) != 0; }
	Bool didEnter(const PolygonTrigger *t) const { return (scHash(SC_SEED_B, (unsigned)m_index, 200u + (unsigned)t->m_index) & 2) != 0; }
	Bool didExit(const PolygonTrigger *t) const { return (scHash(SC_SEED_B, (unsigned)m_index, 200u + (unsigned)t->m_index) & 4) != 0; }
	BodyModuleInterface *getBodyModule() const { if (!(w(0) >> 2 & 1)) return NULL; m_body.m_obj = m_index; return &m_body; }
	Player *getControllingPlayer() const { int p = (int)(w(0) >> 16 & 15); return p < scNumPlayers() ? &g_players[p] : NULL; }
	ContainModuleInterface *getContain() const { if (!(w(0) >> 3 & 1)) return NULL; m_contain.m_obj = m_index; return &m_contain; }
	ObjectStatusMaskType getStatusBits() const { return scStatus(w(2)); }
	Bool isDisabledByType(DisabledType t) const
	{
		if (t == DISABLED_HELD) return (w(0) >> 7 & 1) != 0;
		if (t == DISABLED_UNMANNED) return (w(0) >> 8 & 1) != 0;
		return FALSE;
	}
	Bool isDisabled() const { return (w(0) >> 9 & 1) != 0; }
	ObjectShroudStatus getShroudedStatus(Int idx) const { return (ObjectShroudStatus)(scHash(SC_SEED_B, (unsigned)m_index, 300u + (unsigned)idx) % 6u); }
	Real getVisionRange() const { return 100.0f; }
	AIUpdateInterface *getAIUpdateInterface() const { if (!(w(0) >> 4 & 1)) return NULL; m_ai.m_obj = m_index; return &m_ai; }
	Object *getContainedBy() const { return (w(0) >> 5 & 1) ? (Object *)this : NULL; }
	Bool isCaptured() const { return (w(0) >> 6 & 1) != 0; }
	SpecialPowerModuleInterface *getSpecialPowerModule(const SpecialPowerTemplate *p) const
	{
		if (!(scHash(SC_SEED_C, (unsigned)m_index, 400u + (unsigned)p->m_index) & 1)) return NULL;
		m_spm.m_obj = m_index;
		m_spm.m_power = p->m_index;
		return &m_spm;
	}
	Bool hasSpecialPower(SpecialPowerType t) const { return (scHash(SC_SEED_C, (unsigned)m_index, 600u + (unsigned)t) & 1) != 0; }
	UpdateModule *findUpdateModule(NameKeyType) const
	{
		if (!(scHash(SC_SEED_B, (unsigned)m_index, 700u) & 1)) return NULL;
		m_ware.m_obj = m_index;
		return &m_ware;
	}
	Int m_index;
	mutable Coord3D m_pos;
	mutable BodyModuleInterface m_body;
	mutable ContainModuleInterface m_contain;
	mutable AIUpdateInterface m_ai;
	mutable SpecialPowerModuleInterface m_spm;
	mutable SupplyWarehouseDockUpdate m_ware;
};
extern Object g_objects[SC_OBJS];
inline const DamageInfo *BodyModuleInterface::getLastDamageInfo() const
{
	unsigned w0 = g_cfg[80 + 4 * m_obj];
	if (!(w0 >> 10 & 1)) return NULL;
	DamageInfo *d = const_cast<DamageInfo *>(&m_info);
	int t = (int)(w0 >> 20 & 7);
	d->in.m_sourceTemplate = (w0 >> 11 & 1) && t < 7 ? &g_templates[t] : NULL;
	d->in.m_sourceID = (ObjectID)(w0 >> 23 & 31);
	int p = (int)(w0 >> 28);
	d->in.m_sourcePlayerMask = p == 15 ? 0u : 1u << p;
	return d;
}
inline void Player::countObjectsByThingTemplate(Int n, const ThingTemplate *const *things, Bool ignoreDead, Int *counts) const
{
	for (Int i = 0; i < n; ++i) {
		counts[i] = 0;
		for (int j = 0; j < SC_OBJS; ++j) {
			const Object &o = g_objects[j];
			if (o.getControllingPlayer() == this && o.getTemplate() == things[i] && !(ignoreDead && o.isEffectivelyDead())) ++counts[i];
		}
	}
}

/* --- the script engine --- */
class ScriptEngine
{
public:
	Player *getPlayerFromAsciiString(const AsciiString &name)
	{
		if (name == THIS_PLAYER_ENEMY) { int e = (int)(g_cfg[4] >> 4 & 15); return e < scNumPlayers() ? &g_players[e] : NULL; }
		int k = scNum(name.str(), "player");
		return k >= 0 && k < scNumPlayers() ? &g_players[k] : NULL;
	}
	Team *getTeamNamed(const AsciiString &name)
	{
		if (name == THIS_TEAM) { unsigned t = g_cfg[5] & 255; return t < SC_TEAMS ? &g_teams[t] : NULL; }
		int k = scNum(name.str(), "team");
		if (k < 0 || k >= SC_PROTOS) return NULL;
		unsigned t = g_cfg[48 + k] >> 20 & 255;
		return t < SC_TEAMS ? &g_teams[t] : NULL;
	}
	Object *getUnitNamed(const AsciiString &name)
	{
		int k = scNum(name.str(), "unit");
		return k >= 0 && k < SC_OBJS && (g_objects[k].w(0) & 1) ? &g_objects[k] : NULL;
	}
	Bool didUnitExist(const AsciiString &name) { int k = scNum(name.str(), "unit"); return k >= 0 && k < 32 && (g_cfg[10] >> k & 1); }
	PolygonTrigger *getQualifiedTriggerAreaByName(const AsciiString &name)
	{
		int k = scNum(name.str(), "trig");
		return k >= 0 && k < 8 && (g_cfg[6] >> k & 1) ? &g_triggers[k] : NULL;
	}
	UnsignedInt getFrameObjectCountChanged() { return g_cfg[2]; }
	ObjectTypes *getObjectTypes(const AsciiString &name)
	{
		int k = scNum(name.str(), "list");
		if (k < 0 || k >= (int)(g_cfg[12] & 7)) return NULL;
		m_list.m_listName = name;
		m_list.m_objectTypes.clear();
		for (int j = 0; j < 10; ++j) {
			if (scHash(SC_SEED_C, (unsigned)k, 100u + (unsigned)j) & 1) m_list.m_objectTypes.push_back(j < 9 ? scName("tmpl", j) : AsciiString("x0"));
		}
		return &m_list;
	}
	Bool done(const char *what, const AsciiString &name, Bool remove, unsigned salt)
	{
		Bool r = (scHash(SC_SEED_C, scStrHash(name.str()), salt) & 1) != 0;
		scLog("%s %s %d %d", what, name.str(), remove ? 1 : 0, r ? 1 : 0);
		return r;
	}
	Bool isVideoComplete(AsciiString name, Bool remove) { return done("video", name, remove, 1400); }
	Bool isSpeechComplete(AsciiString name, Bool remove) { return done("speech", name, remove, 1401); }
	Bool isAudioComplete(AsciiString name, Bool remove) { return done("audio", name, remove, 1402); }
	Bool power(const char *what, Int idx, const AsciiString &name, Bool remove, ObjectID src, unsigned salt)
	{
		Bool r = (scHash(SC_SEED_C, (unsigned)idx * 977u + scStrHash(name.str()), salt + (unsigned)src) & 1) != 0;
		scLog("%s %d %s %d %d %d", what, idx, name.str(), remove ? 1 : 0, src, r ? 1 : 0);
		return r;
	}
	Bool isSpecialPowerTriggered(Int i, AsciiString n, Bool r, ObjectID s) { return power("sptriggered", i, n, r, s, 1500); }
	Bool isSpecialPowerMidway(Int i, AsciiString n, Bool r, ObjectID s) { return power("spmidway", i, n, r, s, 1600); }
	Bool isSpecialPowerComplete(Int i, AsciiString n, Bool r, ObjectID s) { return power("spcomplete", i, n, r, s, 1700); }
	Bool isUpgradeComplete(Int i, AsciiString n, Bool r, ObjectID s) { return power("upgrade", i, n, r, s, 1800); }
	Bool isScienceAcquired(Int idx, ScienceType sci, Bool remove)
	{
		Bool r = (scHash(SC_SEED_C, (unsigned)idx, 1900u + (unsigned)sci) & 1) != 0;
		scLog("science %d %d %d %d", idx, sci, remove ? 1 : 0, r ? 1 : 0);
		return r;
	}
	Int getObjectCount(Int idx, const AsciiString &name)
	{
		for (size_t i = 0; i < m_counts.size(); ++i) if (m_counts[i].idx == idx && m_counts[i].name == name.m_s) return m_counts[i].count;
		return (Int)(scHash(SC_SEED_C, (unsigned)idx * 977u + scStrHash(name.str()), 2100u) % 4u);
	}
	void setObjectCount(Int idx, const AsciiString &name, Int count)
	{
		scLog("setobjectcount %d %s %d", idx, name.str(), count);
		for (size_t i = 0; i < m_counts.size(); ++i) if (m_counts[i].idx == idx && m_counts[i].name == name.m_s) { m_counts[i].count = count; return; }
		Count c; c.idx = idx; c.name = name.m_s; c.count = count;
		m_counts.push_back(c);
	}
	struct Count { Int idx; std::string name; Int count; };
	std::vector<Count> m_counts;
	ObjectTypes m_list;
};
extern ScriptEngine *TheScriptEngine;

class TerrainLogic
{
public:
	Bool anyBridgesDamageStatesChanged() { return (g_cfg[0] >> 2 & 1) != 0; }
	Bool isBridgeBroken(Object *o) { return (scHash(SC_SEED_B, (unsigned)o->m_index, 2200u) & 1) != 0; }
	Bool isBridgeRepaired(Object *o) { return (scHash(SC_SEED_B, (unsigned)o->m_index, 2200u) & 2) != 0; }
};
extern TerrainLogic *TheTerrainLogic;

/* --- the partition: the filters say which search it is --- */
enum DistanceCalculationType { FROM_CENTER_2D };
enum IterOrderType { ITER_FASTEST };
class PartitionFilter
{
public:
	virtual ~PartitionFilter() {}
	virtual int kind() const { return 0; }
	virtual int flags() const { return 0; }
	virtual Player *player() const { return NULL; }
};
class PartitionFilterRelationship : public PartitionFilter
{
public:
	enum RelationshipAllowTypes { ALLOW_ALLIES = (1 << ALLIES), ALLOW_ENEMIES = (1 << ENEMIES), ALLOW_NEUTRAL = (1 << NEUTRAL) };
	PartitionFilterRelationship(const Object *, Int flags) : m_flags(flags) {}
	int kind() const { return 1; }
	int flags() const { return m_flags; }
	Int m_flags;
};
class PartitionFilterAlive : public PartitionFilter { public: int kind() const { return 2; } };
class PartitionFilterRejectByObjectStatus : public PartitionFilter
{
public:
	PartitionFilterRejectByObjectStatus(ObjectStatusMaskType, ObjectStatusMaskType) {}
};
class PartitionFilterSameMapStatus : public PartitionFilter { public: PartitionFilterSameMapStatus(const Object *) {} };
class PartitionFilterAcceptByKindOf : public PartitionFilter
{
public:
	PartitionFilterAcceptByKindOf(const KindOfMaskType &m, const KindOfMaskType &) : m_tech(m.test(KINDOF_TECH_BUILDING)) {}
	int kind() const { return m_tech ? 4 : 3; }
	Bool m_tech;
};
enum { ALLOW_ALLIES = PartitionFilterRelationship::ALLOW_ALLIES, ALLOW_ENEMIES = PartitionFilterRelationship::ALLOW_ENEMIES,
	ALLOW_NEUTRAL = PartitionFilterRelationship::ALLOW_NEUTRAL };
class PartitionFilterPlayerAffiliation : public PartitionFilter
{
public:
	PartitionFilterPlayerAffiliation(const Player *p, Int, Bool) : m_p((Player *)p) {}
	Player *player() const { return m_p; }
	Player *m_p;
};
class PartitionFilterOnMap : public PartitionFilter {};
class PartitionFilterPlayer : public PartitionFilter { public: PartitionFilterPlayer(const Player *, Bool) {} };
class SimpleObjectIterator : public MemoryPoolObject
{
public:
	Object *first() { m_i = 0; return next(); }
	Object *next() { return m_i < m_list.size() ? m_list[m_i++] : NULL; }
	std::vector<Object *> m_list;
	size_t m_i;
};
class PartitionManager
{
public:
	SimpleObjectIterator *iterateObjectsInRange(const Object *obj, Real, DistanceCalculationType, PartitionFilter **filters)
	{
		/* seen by obj: with a relationship filter first (its flags), else alive first */
		unsigned rel = filters[0]->kind() == 1 ? (unsigned)filters[0]->flags() : 9u;
		SimpleObjectIterator *it = new SimpleObjectIterator;
		for (int j = 0; j < SC_OBJS; ++j) {
			if (j == obj->m_index || !(g_objects[j].w(0) & 1)) continue;
			if (scHash(SC_SEED_B, (unsigned)(obj->m_index * 64 + j), 800u + rel) & 1) it->m_list.push_back(&g_objects[j]);
		}
		return it;
	}
	SimpleObjectIterator *iterateObjectsInRange(const Coord3D *, Real, DistanceCalculationType, PartitionFilter **filters, IterOrderType)
	{
		/* the player's (affiliated) structures near the supplies' area */
		Player *p = filters[1]->player();
		SimpleObjectIterator *it = new SimpleObjectIterator;
		for (int j = 0; j < SC_OBJS; ++j) {
			if (scHash(SC_SEED_B, (unsigned)j, 900u + (unsigned)p->m_index) & 1) it->m_list.push_back(&g_objects[j]);
		}
		return it;
	}
	Object *getClosestObject(const Coord3D *, Real, DistanceCalculationType, PartitionFilter **filters)
	{
		Player *p = filters[1]->player();
		return ((scHash(SC_SEED_B, (unsigned)p->m_index, 950u) ^ g_cfg[14] >> p->m_index) & 1) ? &g_objects[0] : NULL;
	}
};
extern PartitionManager *ThePartitionManager;

/* --- the stores and the control bar --- */
class SpecialPowerStore
{
public:
	const SpecialPowerTemplate *findSpecialPowerTemplate(AsciiString name)
	{
		int k = scNum(name.str(), "power");
		return k >= 0 && k < (int)(g_cfg[11] >> 8 & 7) ? &m_powers[k] : NULL;
	}
	Bool canUseSpecialPower(Object *o, const SpecialPowerTemplate *p)
	{
		return (scHash(SC_SEED_C, (unsigned)o->m_index, 500u + (unsigned)p->m_index) & 1) != 0;
	}
	SpecialPowerTemplate m_powers[8];
};
extern SpecialPowerStore *TheSpecialPowerStore;
class ScienceStore
{
public:
	ScienceType getScienceFromInternalName(const AsciiString &name) const
	{
		int k = scNum(name.str(), "sci");
		return k >= 0 && k < (int)(g_cfg[11] >> 16 & 255) ? k : SCIENCE_INVALID;
	}
};
extern ScienceStore *TheScienceStore;
class UpgradeTemplate {};
extern UpgradeTemplate g_upgrade;
class CommandButton
{
public:
	unsigned kind() const { return scHash(SC_SEED_C, (unsigned)m_index, 1100u) % 3u; }
	const SpecialPowerTemplate *getSpecialPowerTemplate() const
	{
		int n = (int)(g_cfg[11] >> 8 & 7);
		return kind() == 2 && n > 0 ? &TheSpecialPowerStore->m_powers[m_index % n] : NULL;
	}
	const UpgradeTemplate *getUpgradeTemplate() const { return kind() == 0 || (kind() == 2 && !getSpecialPowerTemplate()) ? &g_upgrade : NULL; }
	Bool isReady(const Object *o) const { return (scHash(SC_SEED_C, (unsigned)(m_index * 64 + o->m_index), 1200u) & 1) != 0; }
	Int m_index;
};
class ControlBar
{
public:
	const CommandButton *findCommandButton(const AsciiString &name)
	{
		int k = scNum(name.str(), "btn");
		return k >= 0 && k < (int)(g_cfg[11] >> 24 & 7) ? &m_buttons[k] : NULL;
	}
	CommandButton m_buttons[8];
};
extern ControlBar *TheControlBar;

/* --- the logic and the client --- */
class GameLogic
{
public:
	UnsignedInt getFrame() { return g_cfg[1]; }
	Object *findObjectByID(ObjectID id) { return id >= 1 && id <= SC_OBJS ? &g_objects[id - 1] : NULL; }
};
extern GameLogic *TheGameLogic;
class GameEngine { public: Bool isMultiplayerSession() { return (g_cfg[0] & 1) != 0; } };
extern GameEngine *TheGameEngine;
class Drawable
{
public:
	Object *getObject() { return m_obj; }
	Object *m_obj;
};
typedef std::list<Drawable *> DrawableList;
typedef DrawableList::const_iterator DrawableListCIt;
class InGameUI
{
public:
	UnsignedInt getFrameSelectionChanged() { return g_cfg[3]; }
	const DrawableList *getAllSelectedDrawables()
	{
		m_selected.clear();
		for (int j = 0; j < SC_OBJS; ++j) {
			m_draws[j].m_obj = &g_objects[j];
			if (g_cfg[13] >> j & 1) m_selected.push_back(&m_draws[j]);
		}
		m_none.m_obj = NULL;
		if (g_cfg[13] >> 31) m_selected.push_back(&m_none);
		return &m_selected;
	}
	Drawable m_draws[SC_OBJS];
	Drawable m_none;
	DrawableList m_selected;
};
extern InGameUI *TheInGameUI;
class View { public: Bool isCameraMovementFinished() { return (g_cfg[0] >> 1 & 1) != 0; } };
extern View *TheTacticalView;
class VictoryConditionsInterface
{
public:
	Bool isLocalAlliedVictory() { return (g_cfg[0] >> 3 & 1) != 0; }
	Bool isLocalAlliedDefeat() { return (g_cfg[0] >> 4 & 1) != 0; }
	Bool isLocalDefeat() { return (g_cfg[0] >> 5 & 1) != 0; }
};
extern VictoryConditionsInterface *TheVictoryConditions;
class GameAudio
{
public:
	Bool hasMusicTrackCompleted(const AsciiString &name, Int times)
	{
		return (scHash(SC_SEED_C, scStrHash(name.str()), 2000u + (unsigned)times) & 1) != 0;
	}
};
extern GameAudio *TheAudio;

#endif
