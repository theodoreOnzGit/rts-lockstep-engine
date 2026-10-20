/* The stand-ins of the world that Team.cpp reaches, part 1: the declarations that Team.h needs before the real Team
** classes (p1w9-sonnet). Included through the stand-in GameLogic/Object.h, with every class below renamed by the
** #define lines of differential/wrap_team.cpp (TmTest*), so that no symbol of this archive collides with another
** fixture's. The out-of-line members, which need the real Team classes, are in team_standins.h.
**
** Method: the classes answer from the program's configuration (members set by the driver from the program words) and
** every call that has an effect writes a line to g_lines, formatted exactly as the Rust host (tests/differential_team.rs)
** writes it. Calls that only answer a question write nothing. Reals are written as their bit patterns. */
#ifndef TEAM_STANDINS_DECL_H
#define TEAM_STANDINS_DECL_H

#include <vector>
#include <string>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "Common/NameKeyGenerator.h"
#include "Common/Errors.h"

/* Common/GameState.h's SaveCode, verbatim from upstream (GameState.h lines 129-138): Team.cpp throws SC_INVALID_DATA */
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

/* ---- the trace ---------------------------------------------------------------------------------------------- */
static std::vector<std::string>* g_lines = NULL;
static unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static void Rec(const char* fmt, ...)
{
	char buf[2048];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof buf, fmt, ap);
	va_end(ap);
	if (g_lines) g_lines->push_back(buf);
	static const bool trace = getenv("TEAM_TRACE") != NULL;
	if (trace) fprintf(stderr, "%s\n", buf);
}
#define FB(x) fbits(x)

class Team; class TeamPrototype; class Object; class Player; class ThingTemplate; class PolygonTrigger;

/* ---- the enums of the headers the real Team.cpp includes that are not the real ones here ------------------------ */
enum AttitudeType { AI_SLEEP = -2, AI_PASSIVE = -1, AI_NORMAL = 0, AI_ALERT = 1, AI_AGGRESSIVE = 2, AI_INVALID = 3 };
typedef UnsignedInt LocomotorSurfaceTypeMask;
enum LocomotorSurfaceType
{
	LOCOMOTORSURFACE_GROUND = (1 << 0),
	LOCOMOTORSURFACE_WATER = (1 << 1),
	LOCOMOTORSURFACE_CLIFF = (1 << 2),
	LOCOMOTORSURFACE_AIR = (1 << 3),
	LOCOMOTORSURFACE_RUBBLE = (1 << 4)
};
enum DistanceCalculationType { FROM_CENTER_2D = 0, FROM_CENTER_3D = 1, FROM_BOUNDINGSPHERE_2D = 2, FROM_BOUNDINGSPHERE_3D = 3 };

/* ---- the dictionary (Dict is another unit, verified by differential_dict.rs: this stand-in has its getters' meaning --
** a value of another type, or none, reads as "does not exist" and gives 0, false or the empty string) ------------- */
class Dict
{
public:
	struct Val { int type; unsigned bits; AsciiString s; };
	std::map<unsigned, Val> m;
	void set(NameKeyType k, int type, unsigned bits, const AsciiString& s) { Val v; v.type = type; v.bits = bits; v.s = s; m[(unsigned)k] = v; }
	void setBool(NameKeyType k, Bool v) { set(k, 0, v ? 1 : 0, AsciiString()); }
	void setInt(NameKeyType k, Int v) { set(k, 1, (unsigned)v, AsciiString()); }
	void setReal(NameKeyType k, Real v) { unsigned b; memcpy(&b, &v, 4); set(k, 2, b, AsciiString()); }
	void setAsciiString(NameKeyType k, const AsciiString& v) { set(k, 3, 0, v); }
	const Val* find(NameKeyType k, int type) const { std::map<unsigned, Val>::const_iterator it = m.find((unsigned)k); return (it != m.end() && it->second.type == type) ? &it->second : NULL; }
	Bool getBool(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 0); if (exists) *exists = v != NULL; return v ? v->bits != 0 : false; }
	Int getInt(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 1); if (exists) *exists = v != NULL; return v ? (Int)v->bits : 0; }
	Real getReal(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 2); if (exists) *exists = v != NULL; Real r = 0; if (v) memcpy(&r, &v->bits, 4); return r; }
	AsciiString getAsciiString(NameKeyType k, Bool* exists = NULL) const { const Val* v = find(k, 3); if (exists) *exists = v != NULL; return v ? v->s : AsciiString(); }
};

/* ---- the thing templates ---------------------------------------------------------------------------------------- */
class ThingTemplate
{
public:
	int m_idx;
	KindOfMaskType m_kinds;
	bool m_buildFacility;
	int m_equiv;
	AsciiString m_name;
	std::vector<AsciiString> m_variations;
	Bool isEquivalentTo(const ThingTemplate* tt) const { if (!tt) return false; if (this == tt) return true; return m_equiv == tt->m_equiv; }
	Bool isKindOf(KindOfType t) const { return m_kinds.test(t); }
	Bool isKindOfMulti(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) const { return m_kinds.testSetAndClear(mustBeSet, mustBeClear); }
	Bool isBuildFacility() const { return m_buildFacility; }
	const AsciiString& getName() const { return m_name; }
	const std::vector<AsciiString>& getBuildVariations() const { return m_variations; }
};

class ThingFactory
{
public:
	std::vector<ThingTemplate*> m_templates;
	ThingTemplate* findTemplate(const AsciiString& name, Bool check = TRUE) const
	{
		for (size_t i = 0; i < m_templates.size(); ++i) if (m_templates[i]->m_name == name) return m_templates[i];
		return NULL;
	}
};

/* ---- the AI, the contain module, the groups, the triggers ------------------------------------------------------- */
class LocomotorSet
{
public:
	LocomotorSurfaceTypeMask m_valid;
	LocomotorSurfaceTypeMask getValidSurfaces() const { return m_valid; }
};
class AIUpdateInterface
{
public:
	Object* m_owner; bool m_idle, m_recruitable; LocomotorSet m_loco;
	Bool isIdle() const { return m_idle; }
	Bool isRecruitable() const { return m_recruitable; }
	const LocomotorSet& getLocomotorSet() const { return m_loco; }
};
class ContainModuleInterface
{
public:
	Object* m_owner; int m_count;
	Int getContainCount() const { return m_count; }
	void removeAllContained(Bool exposeStealthUnits = FALSE);
};
class AIGroup
{
public:
	int m_id;
	void add(Object* obj);
};
class PolygonTrigger { public: int m_id; };

/* ---- the terrain ------------------------------------------------------------------------------------------------- */
class Waypoint
{
public:
	UnsignedInt m_id; AsciiString m_name; Coord3D m_loc; Waypoint* m_next;
	Waypoint* getNext() const { return m_next; }
	const AsciiString& getName() const { return m_name; }
	const Coord3D* getLocation() const { return &m_loc; }
	UnsignedInt getID() const { return m_id; }
};
class TerrainLogic
{
public:
	std::vector<Waypoint*> m_ways;
	Waypoint* getFirstWaypoint() const { return m_ways.empty() ? NULL : m_ways[0]; }
	Waypoint* getWaypointByID(UnsignedInt id) const
	{
		for (size_t i = 0; i < m_ways.size(); ++i) if (m_ways[i]->m_id == id) return m_ways[i];
		return NULL;
	}
};

/* ---- the script engine -------------------------------------------------------------------------------------------- */
class ScriptAction { public: int m_serial; };
class Script
{
public:
	int m_serial; AsciiString m_name; UnsignedInt m_frameToEvaluate; Int m_delay; bool m_easy, m_normal, m_hard, m_oneShot;
	unsigned m_pattern; unsigned m_calls; ScriptAction m_action;
	Script* duplicate() const;
	void deleteInstance() { Rec("script.delete %d", m_serial); }
	UnsignedInt getFrameToEvaluate() const { return m_frameToEvaluate; }
	void setFrameToEvaluate(UnsignedInt f) { Rec("script.frame %d %u", m_serial, (unsigned)f); m_frameToEvaluate = f; }
	Int getDelayEvalSeconds() const { return m_delay; }
	Bool isEasy() const { return m_easy; }
	Bool isNormal() const { return m_normal; }
	Bool isHard() const { return m_hard; }
	Bool isOneShot() const { return m_oneShot; }
	AsciiString getName() const { return m_name; }
	ScriptAction* getAction() const { return const_cast<ScriptAction*>(&m_action); }
};
class ScriptEngine
{
public:
	std::vector<Script*> m_scripts;
	int m_nextSerial;
	const Script* findScriptByName(const AsciiString& name) const
	{
		for (size_t i = 0; i < m_scripts.size(); ++i) if (m_scripts[i]->m_name == name) return m_scripts[i];
		return NULL;
	}
	void friend_executeAction(ScriptAction* action, Team* team = NULL);
	Bool evaluateConditions(Script* script, Team* team = NULL, Player* player = NULL);
	void runScript(const AsciiString& name, Team* team);
	void notifyOfTeamDestruction(Team* team);
	void AppendDebugMessage(const AsciiString& msg, Bool forcePause = TRUE) { Rec("dbg %s %d", msg.str(), forcePause ? 1 : 0); }
};

/* ---- players ----------------------------------------------------------------------------------------------------- */
class PlayerTemplate
{
public:
	AsciiString m_beacon;
	AsciiString getBeaconTemplate() const { return m_beacon; }
};
class Player
{
public:
	int m_index; PlayerType m_type; GameDifficulty m_diff; Team* m_defaultTeam; PlayerTemplate m_tmpl; unsigned m_relSalt; std::vector<TeamPrototype*> m_protos;
	void addTeamToList(TeamPrototype* proto);
	void removeTeamFromList(TeamPrototype* proto);
	void preTeamDestroy(const Team* team);
	Team* getDefaultTeam() const { return m_defaultTeam; }
	Relationship getRelationship(const Team* that) const;
	PlayerType getPlayerType() const { return m_type; }
	GameDifficulty getPlayerDifficulty() const { return m_diff; }
	Int getPlayerIndex() const { return m_index; }
	const PlayerTemplate* getPlayerTemplate() const { return &m_tmpl; }
};
class PlayerList
{
public:
	Player* m_players[4]; int m_count; std::map<unsigned, int> m_byKey;
	Int getPlayerCount() const { return m_count; }
	Player* getNthPlayer(Int i) const { return (i >= 0 && i < m_count) ? m_players[i] : NULL; }
	Player* getNeutralPlayer() const { return m_players[0]; }
	Player* findPlayerWithNameKey(NameKeyType key) const
	{
		std::map<unsigned, int>::const_iterator it = m_byKey.find((unsigned)key);
		return it == m_byKey.end() ? NULL : m_players[it->second];
	}
	void teamAboutToBeDeleted(Team* team);
};

/* ---- the partition manager and the filters Team::updateState builds ----------------------------------------------- */
class PartitionFilter { public: virtual ~PartitionFilter() {} };
class PartitionFilterRelationship : public PartitionFilter
{
public:
	enum { ALLOW_SAME_PLAYER = 1, ALLOW_ALLIES = 2, ALLOW_ENEMIES = 4, ALLOW_NEUTRAL = 8 };
	PartitionFilterRelationship(const Object* obj, unsigned flags) {}
};
class PartitionFilterAlive : public PartitionFilter { public: PartitionFilterAlive() {} };
class PartitionFilterSameMapStatus : public PartitionFilter { public: PartitionFilterSameMapStatus(const Object* obj) {} };
class PartitionManager
{
public:
	Object* getClosestObject(const Object* obj, Real maxDist, DistanceCalculationType dc, PartitionFilter** filters);
};

/* ---- the game logic ----------------------------------------------------------------------------------------------- */
class GameLogic
{
public:
	UnsignedInt m_frame; std::vector<Object*> m_objs;
	UnsignedInt getFrame() const { return m_frame; }
	Object* getFirstObject() const { return m_objs.empty() ? NULL : m_objs[0]; }
	Object* findObjectByID(ObjectID id) const;
	void destroyObject(Object* obj);
};

/* ---- the sides list (Team::initFromSides reads the team dictionaries) -------------------------------------------- */
class TeamsInfo
{
public:
	Dict m_dict;
	Dict* getDict() { return &m_dict; }
};
class SidesList
{
public:
	std::vector<TeamsInfo*> m_teams;
	Int getNumTeams() const { return (Int)m_teams.size(); }
	TeamsInfo* getTeamInfo(Int i) { return m_teams[i]; }
};

extern PlayerList* ThePlayerList;
extern ScriptEngine* TheScriptEngine;
extern TerrainLogic* TheTerrainLogic;
extern GameLogic* TheGameLogic;
extern PartitionManager* ThePartitionManager;
extern ThingFactory* TheThingFactory;

/* ---- the object ---------------------------------------------------------------------------------------------------- */
class Object : public MemoryPoolObject
{
	MAKE_DLINK(Object, TeamMemberList)
public:
	ObjectID m_id; ThingTemplate* m_tpl; Team* m_team; Player* m_player; ObjectStatusMaskType m_status;
	bool m_dead, m_destroyed, m_contained, m_held, m_enemyNear;
	bool m_hasAI; AIUpdateInterface m_ai; bool m_hasContain; ContainModuleInterface m_contain;
	Coord3D m_pos; float m_vision;
	unsigned m_didEnter, m_didExit, m_inside;
	unsigned m_reactions; /* 1 kill, 2 destroyObject, 4 removeAllContained, 8 attemptDamage: the object moves to the neutral player's default team */
	Object* m_nextObj;

	ObjectID getID() const { return m_id; }
	const ThingTemplate* getTemplate() const { return m_tpl; }
	Bool isEffectivelyDead() const { return m_dead; }
	Bool isDestroyed() const { return m_destroyed; }
	Bool isKindOf(KindOfType t) const { return m_tpl->isKindOf(t); }
	Bool isKindOfMulti(KindOfMaskType mustBeSet, KindOfMaskType mustBeClear) const { return m_tpl->isKindOfMulti(mustBeSet, mustBeClear); }
	const ObjectStatusMaskType& getStatusBits() const { return m_status; }
	Bool testStatus(ObjectStatusTypes s) const { return m_status.test(s); }
	Bool isDisabledByType(DisabledType type) const { return m_held && type == DISABLED_HELD; }
	Object* getContainedBy() const { return m_contained ? const_cast<Object*>(this) : NULL; }
	Player* getControllingPlayer() const { return m_player; }
	Team* getTeam() const { return m_team; }
	void setTeam(Team* team);
	const Coord3D* getPosition() const { return &m_pos; }
	Real getVisionRange() const { return m_vision; }
	void healCompletely();
	void kill();
	void attemptDamage(DamageInfo* info);
	void handlePartitionCellMaintenance();
	ContainModuleInterface* getContain() const { return m_hasContain ? const_cast<ContainModuleInterface*>(&m_contain) : NULL; }
	AIUpdateInterface* getAIUpdateInterface() const { return m_hasAI ? const_cast<AIUpdateInterface*>(&m_ai) : NULL; }
	Bool didEnter(const PolygonTrigger* t) const { return (m_didEnter >> t->m_id) & 1; }
	Bool didExit(const PolygonTrigger* t) const { return (m_didExit >> t->m_id) & 1; }
	Bool isInside(const PolygonTrigger* t) const { return (m_inside >> t->m_id) & 1; }
	Object* getNextObject() const { return m_nextObj; }
};

#endif
