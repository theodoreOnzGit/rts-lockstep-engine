/* Differential-test wrapper for four host-driven drafts (#215, upstream commit 0a05454d), cut verbatim by build.rs (drafts_guilogic3) into:
**   hosts_battle.inc  BattlePlanUpdate.cpp, BattlePlanUpdate::createVisionObject (to setStatus);   src/drafts/sweep_gui_logic/battle_plan_update.rs
**   hosts_chinook.inc ChinookAIUpdate.cpp, private___TellPortableStructureToAttackWithMe (to privateForceAttackObject);   chinook_ai_update.rs
**   hosts_group.inc   AIGroup.cpp, groupDoCommandButtonUsingWaypoints (to groupDoCommandButtonAtObject);   ai_group.rs
**   hosts_states.inc  AIStates.cpp, setGoalSquad and setGoalAIGroup (to getGoalSquad);   ai_states.rs
**   hosts_squad.inc   Squad.cpp, Squad::squadFromAIGroup (to aiGroupFromSquad), the function setGoalAIGroup calls.
** Built on the REAL AsciiString (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cuts), each logging what the cut asks of it, in order (the test's host logs the same events):
**   TheThingFactory->findTemplate(name) (a template for every name but `none`) and newObject(tt, team) (an object, or NULL as the test sets);
**   ThePlayerList->getNeutralPlayer() (NULL or a player) and Player::getDefaultTeam(); Object (getID, getPosition, setPosition,
**   setShroudClearingRange, getGeometryInfo().getBoundingSphereRadius(), isKindOf, isDisabledByType, getAIUpdateInterface, getContain);
**   ContainModuleInterface::friend_getRider(); AIUpdateInterface::aiAttackObject; Object::doCommandButtonUsingWaypoints; AIGroup::getAllIDs;
**   Squad (m_objectIDs, m_objectsCached, copied member by member as the implicit operator= does; the real one is a MemoryPoolObject and a Snapshot,
**   whose operator= copy nothing of the test's concern); `newInstance(T)` is `new T`; DEBUG_ASSERTCRASH is a no-op (release).
**
** Entry points (extern "C", tests/differential_draft_hosts.rs): ref_bp_run, ref_ck_run, ref_ag_run, ref_as_run; the programs are in the test. */
#include <string.h>
#include <vector>
#include <list>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theHostsAllocator;

#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define NULL 0
#define newInstance(T) new T
typedef int ObjectID;
enum { INVALID_ID = 0 };
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI, CMD_FROM_DOZER, CMD_DEFAULT_SWITCH_WEAPON };
enum ObjectKindOf { KINDOF_PORTABLE_STRUCTURE = 1 };
enum DisabledType { DISABLED_HACKED = 0, DISABLED_EMP, DISABLED_SUBDUED, DISABLED_PARALYZED };

static std::vector<int> theLog;
struct CommandButton { int dummy; };
struct Waypoint { int dummy; };
static int bitsOf(Real f) { union { Real f; int i; } u; u.f = f; return u.i; }
static Real realOf(int i) { union { Real f; int i; } u; u.i = i; return u.f; }

/* ---- BattlePlanUpdate ---- */
struct ThingTemplate { int id; };
struct Team { int id; };
class GeometryInfoStandIn { public: Real getBoundingSphereRadius() const { theLog.push_back(7); return r; } Real r; };
class Object
{
public:
	ObjectID getID() const { return m_id; }
	const Coord3D *getPosition() const { theLog.push_back(5); return &m_pos; }
	void setPosition(const Coord3D *p) { theLog.push_back(6); theLog.push_back(bitsOf(p->x)); theLog.push_back(bitsOf(p->y)); theLog.push_back(bitsOf(p->z)); }
	void setShroudClearingRange(Real r) { theLog.push_back(8); theLog.push_back(bitsOf(r)); }
	GeometryInfoStandIn &getGeometryInfo() { return m_geo; }
	Bool isKindOf(ObjectKindOf k) const { theLog.push_back(11); theLog.push_back(m_id); return m_portable; }
	Bool isDisabledByType(DisabledType t) const { theLog.push_back(12); theLog.push_back((int)t); theLog.push_back(m_id); return m_disabled[t]; }
	class AIUpdateInterface *getAIUpdateInterface() const { return m_ai; }
	class ContainModuleInterface *getContain() const { return m_contain; }
	void doCommandButtonUsingWaypoints(const CommandButton *button, const Waypoint *way, CommandSourceType src)
	{
		theLog.push_back(20); theLog.push_back(m_id); theLog.push_back(button->dummy); theLog.push_back(way ? way->dummy : -1); theLog.push_back((int)src);
	}
	ObjectID m_id;
	Coord3D m_pos;
	GeometryInfoStandIn m_geo;
	Bool m_portable;
	Bool m_disabled[4];
	class AIUpdateInterface *m_ai;
	class ContainModuleInterface *m_contain;
};
class AIUpdateInterface
{
public:
	void aiAttackObject(Object *victim, Int maxShotsToFire, CommandSourceType cmdSource)
	{
		theLog.push_back(13); theLog.push_back(m_riderId); theLog.push_back(victim->getID()); theLog.push_back(maxShotsToFire); theLog.push_back((int)cmdSource);
	}
	int m_riderId;
};
class ContainModuleInterface { public: const Object *friend_getRider() const { return m_rider; } Object *m_rider; };

struct BattlePlanUpdateModuleData { AsciiString m_visionObjectName; };
class Player { public: Team *getDefaultTeam() { return &m_team; } Team m_team; };
static bool havePlayer;
static Player thePlayerStandIn;
struct PlayerListStandIn { Player *getNeutralPlayer() { return havePlayer ? &thePlayerStandIn : NULL; } };
static PlayerListStandIn thePlayerListObj;
static PlayerListStandIn *ThePlayerList = &thePlayerListObj;
static Object theVisionObject;
static bool haveNewObject;
struct ThingFactoryStandIn
{
	const ThingTemplate *findTemplate(AsciiString name)
	{
		theLog.push_back(1); theLog.push_back((int)strlen(name.str())); for (const char *c = name.str(); *c; ++c) theLog.push_back((unsigned char)*c);
		static ThingTemplate tt; tt.id = (int)strlen(name.str()) + 1;
		return strcmp(name.str(), "none") == 0 ? NULL : &tt;
	}
	Object *newObject(const ThingTemplate *tt, Team *team)
	{
		theLog.push_back(4); theLog.push_back(tt->id); theLog.push_back(team->id);
		return haveNewObject ? &theVisionObject : NULL;
	}
};
static ThingFactoryStandIn theThingFactoryObj;
static ThingFactoryStandIn *TheThingFactory = &theThingFactoryObj;
static Object theOwner;
class BattlePlanUpdate
{
public:
	void createVisionObject();
	const BattlePlanUpdateModuleData *getBattlePlanUpdateModuleData() const { return &m_data; }
	Object *getObject() const { return &theOwner; }
	BattlePlanUpdateModuleData m_data;
	ObjectID m_visionObjectID;
};
#include "hosts_battle.inc"

extern "C" int ref_bp_run(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theHostsAllocator;
	theLog.clear();
	int at = 0;
	BattlePlanUpdate b;
	b.m_visionObjectID = in[at++];
	int nl = in[at++];
	char name[64];
	for (int i = 0; i < nl; ++i) name[i] = (char)in[at++];
	name[nl] = 0;
	b.m_data.m_visionObjectName = AsciiString(name);
	havePlayer = in[at++] != 0; thePlayerStandIn.m_team.id = in[at++];
	haveNewObject = in[at++] != 0; theVisionObject.m_id = in[at++];
	theOwner.m_pos.x = realOf(in[at++]); theOwner.m_pos.y = realOf(in[at++]); theOwner.m_pos.z = realOf(in[at++]);
	theOwner.m_geo.r = realOf(in[at++]);
	b.createVisionObject();
	std::vector<int> o = theLog;
	o.push_back(-1); o.push_back(b.m_visionObjectID);
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}

/* ---- ChinookAIUpdate ---- */
class ChinookAIUpdate
{
public:
	void private___TellPortableStructureToAttackWithMe( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
	Object *getObject() const { return &m_chinook; }
	mutable Object m_chinook;
};
#include "hosts_chinook.inc"

extern "C" int ref_ck_run(const int *in, int nin, int *out, int cap)
{
	theLog.clear();
	int at = 0;
	ChinookAIUpdate c;
	ContainModuleInterface contain; Object rider; AIUpdateInterface ai; Object victim;
	bool hasContain = in[at++] != 0, hasRider = in[at++] != 0;
	rider.m_id = in[at++]; rider.m_portable = in[at++] != 0;
	for (int i = 0; i < 4; ++i) rider.m_disabled[i] = in[at++] != 0;
	bool hasAI = in[at++] != 0;
	ai.m_riderId = rider.m_id; rider.m_ai = hasAI ? &ai : NULL; rider.m_contain = NULL;
	contain.m_rider = hasRider ? &rider : NULL;
	c.m_chinook.m_contain = hasContain ? &contain : NULL;
	victim.m_id = in[at++];
	int maxShots = in[at++]; int src = in[at++];
	c.private___TellPortableStructureToAttackWithMe(&victim, maxShots, (CommandSourceType)src);
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}

/* ---- AIGroup ---- */
class AIGroup
{
public:
	void groupDoCommandButtonUsingWaypoints( const CommandButton *commandButton, const Waypoint *way, CommandSourceType cmdSource );
	std::list<Object *> m_memberList;
	std::vector<ObjectID> getAllIDs() const { std::vector<ObjectID> v; for (std::list<Object *>::const_iterator i = m_memberList.begin(); i != m_memberList.end(); ++i) v.push_back((*i)->getID()); return v; }
};
#include "hosts_group.inc"

extern "C" int ref_ag_run(const int *in, int nin, int *out, int cap)
{
	theLog.clear();
	int at = 0;
	AIGroup g;
	static Object objs[32];
	static CommandButton buttons[8];
	static Waypoint ways[8];
	int n = in[at++];
	for (int i = 0; i < n; ++i) { objs[i].m_id = in[at++]; g.m_memberList.push_back(&objs[i]); }
	int bi = in[at++], wi = in[at++], src = in[at++];
	for (int i = 0; i < 8; ++i) { buttons[i].dummy = i; ways[i].dummy = i; }
	g.groupDoCommandButtonUsingWaypoints(&buttons[bi], wi < 0 ? (const Waypoint *)NULL : &ways[wi], (CommandSourceType)src);
	std::vector<int> o = theLog;
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}

/* ---- AIStateMachine goal squad ---- */
class Squad
{
public:
	void squadFromAIGroup(const AIGroup* fromAIGroup, Bool clearSquadFirst);
	std::vector<ObjectID> m_objectIDs;
	std::vector<Object *> m_objectsCached;
};
class AIStateMachine
{
public:
	AIStateMachine() : m_goalSquad(NULL) {}
	void setGoalSquad( const Squad *squad );
	void setGoalAIGroup( const AIGroup *group );
	Squad *m_goalSquad;
};
#include "hosts_squad.inc"
#include "hosts_states.inc"

extern "C" int ref_as_run(const int *in, int nin, int *out, int cap)
{
	static AIStateMachine m;
	m.m_goalSquad = NULL;
	std::vector<int> o;
	static Object objs[64];
	int at = 0;
	while (at < nin)
	{
		int op = in[at++];
		if (op == 0)
		{
			Squad s;
			int n = in[at++];
			for (int i = 0; i < n; ++i) s.m_objectIDs.push_back(in[at++]);
			int c = in[at++];
			for (int i = 0; i < c; ++i) s.m_objectsCached.push_back(&objs[in[at++]]);
			m.setGoalSquad(&s);
		}
		else if (op == 1)
		{
			bool has = in[at++] != 0;
			AIGroup g;
			int n = in[at++];
			for (int i = 0; i < n; ++i) { int id = in[at++]; objs[id].m_id = id; g.m_memberList.push_back(&objs[id]); }
			m.setGoalAIGroup(has ? &g : (const AIGroup *)NULL);
		}
		else
		{
			o.push_back(m.m_goalSquad != NULL);
			if (m.m_goalSquad)
			{
				o.push_back((int)m.m_goalSquad->m_objectIDs.size());
				for (size_t i = 0; i < m.m_goalSquad->m_objectIDs.size(); ++i) o.push_back(m.m_goalSquad->m_objectIDs[i]);
				o.push_back((int)m.m_goalSquad->m_objectsCached.size());
				for (size_t i = 0; i < m.m_goalSquad->m_objectsCached.size(); ++i) o.push_back((int)(m.m_goalSquad->m_objectsCached[i] - &objs[0]));
			}
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}
