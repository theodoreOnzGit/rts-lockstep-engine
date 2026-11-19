/* Differential-test wrapper for the POWTruckAIUpdate / PrisonDockUpdate draft (#215, upstream commit 0a05454d),
** src/drafts/logic_common/pow_truck_ai_update.rs:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/POWTruckAIUpdate.cpp from the constructor
**   `POWTruckAIUpdate::POWTruckAIUpdate( Thing *thing` to `void POWTruckAIUpdate::crc` (constructor, destructor, onDelete, aiDoCommand, update,
**   setTask, setAIMode, privatePickUpPrisoner, privateReturnPrisoners, updateWaiting, updateFindTarget, updateCollectingTarget,
**   updateReturnPrisoners, validateTarget, doReturnPrisoners, doReturnToPrison, findBestPrison, findBestTarget, unloadPrisonersToPrison,
**   loadPrisoner and the two static callbacks), and
**   .../DockUpdate/PrisonDockUpdate.cpp from `Bool PrisonDockUpdate::action` to `void PrisonDockUpdate::crc`, each cut verbatim by
**   build.rs (pow_truck.inc, prison_dock.inc). Upstream compiles both under `#ifdef ALLOW_SURRENDER`, which the shipped game does not
**   define; the cuts are compiled as if it were.
**
** Stand-ins (outside the cuts; in an anonymous namespace): the class declaration of POWTruckAIUpdate (POWTruckAIUpdate.h, members public;
** MAKE_STANDARD_MODULE_MACRO's getPOWTruckAIUpdateModuleData returns the test's data); AIUpdateInterface, Object, ContainModuleInterface,
** Player, Money, ScoreKeeper, ThingTemplate, GeometryInfo, Locomotor, ActionManager (canPickUpPrisoner: the scripted answer, FALSE for a
** NULL object, as ActionManager.cpp line 1186), PartitionManager (getDistanceSquared FROM_CENTER_2D: dx*dx + dy*dy of the positions),
** the pathfinder (quickDoesPathExist, scripted per target), GameLogic (frame, findObjectByID, the object list), GameText and InGameUI.
** Every call that changes something, or whose answer the logic acts on, is logged as words. A position's z carries the object's id, which
** stands for the pointer when the AI is asked about a position.
**
** Entry point (extern "C", tests/differential_draft_pow_truck.rs): see ref_pow_run's comment. */
#include "Lib/BaseType.h"
#include <string.h>
#include <vector>
#include <algorithm>

namespace {

#define DEBUG_ASSERTCRASH(c, m)
#define DEBUG_CRASH(m)
#define NULL 0

static std::vector<unsigned> theLog;
static unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static float floatOf(unsigned u) { float f; memcpy(&f, &u, 4); return f; }
static void put(unsigned a) { theLog.push_back(a); }
static void put(unsigned a, unsigned b) { theLog.push_back(a); theLog.push_back(b); }
static void put(unsigned a, unsigned b, unsigned c) { theLog.push_back(a); theLog.push_back(b); theLog.push_back(c); }

typedef unsigned int ObjectID;
const ObjectID INVALID_ID = 0;
typedef int UpdateSleepTime;
const UpdateSleepTime UPDATE_SLEEP_NONE = 0;
typedef int Color;
enum CommandSourceType { CMD_FROM_PLAYER, CMD_FROM_AI };
enum AICommandType { AICMD_PICK_UP_PRISONER, AICMD_RETURN_PRISONERS, AICMD_OTHER };
enum { LOGICFRAMES_PER_SECOND = 30 };
const Real HUGE_DIST = 1000000.0f; /* PartitionManager.h line 77 */
enum DistanceCalculationType { FROM_CENTER_2D };
enum KindOfType { KINDOF_COLLECTS_PRISON_BOUNTY };

struct Object;
struct Player;
struct Thing { Object *obj; };
struct POWTruckAIUpdateModuleData { UnsignedInt m_boredTimeInFrames; Real m_hangAroundPrisonDistance; };
struct ModuleData { POWTruckAIUpdateModuleData d; };

struct Locomotor { void setUltraAccurate(Bool) { put(1); } };
struct ThingTemplate { Int cost; Int calcCostToBuild(Player *) const { return cost; } };
struct ScoreKeeper { void addMoneyEarned(UnsignedInt v) { put(21, v); } };
struct Money { void deposit(UnsignedInt v) { put(20, v); } };
struct Player { Money money; ScoreKeeper sk; Money *getMoney() { return &money; } ScoreKeeper *getScoreKeeper() { return &sk; } };
struct UnicodeString { void format(const char *, ...) {} };
struct GameTextClass { const char *fetch(const char *) { return "%d"; } };
GameTextClass theGameTextObj;
GameTextClass *TheGameText = &theGameTextObj;
struct InGameUIClass { void addFloatingText(const UnicodeString &, const Coord3D *pos, Color) { put(22); put(bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z)); } };
InGameUIClass theInGameUIObj;
InGameUIClass *TheInGameUI = &theInGameUIObj;
struct GlobalDataClass { Real m_prisonBountyMultiplier; Color m_prisonBountyTextColor; };
GlobalDataClass theGlobalDataObj;
GlobalDataClass *TheGlobalData = &theGlobalDataObj;
struct GeometryInfo { Real height; Real getMaxHeightAbovePosition() const { return height; } };

class AIUpdateInterface;
struct ContainModuleInterface
{
	Object *owner;
	int count, max;
	std::vector<Object *> list;
	Int getContainCount() const { return count; }
	Int getContainMax() const { return max; }
	void addToContain(Object *o);
	void removeFromContain(Object *o);
	void iterateContained(void (*func)(Object *, void *), void *userData, Bool reverse)
	{
		std::vector<Object *> snapshot = list;
		for (size_t i = 0; i < snapshot.size(); ++i) func(snapshot[i], userData);
	}
};

struct Object
{
	ObjectID id;
	Coord3D pos;
	ObjectID producer;
	Player *player;
	bool collects;
	GeometryInfo geom;
	ContainModuleInterface *contain;
	AIUpdateInterface *ai;
	ThingTemplate tmpl;
	Object *containedBy;
	Object *next;
	ObjectID getID() const { return id; }
	AIUpdateInterface *getAIUpdateInterface() const { return ai; }
	ContainModuleInterface *getContain() const { return contain; }
	Player *getControllingPlayer() const { return player; }
	const Coord3D *getPosition() const { return &pos; }
	ObjectID getProducerID() const { return producer; }
	Object *getContainedBy() const { return containedBy; }
	Bool isKindOf(KindOfType) const { return collects; }
	const ThingTemplate *getTemplate() const { return &tmpl; }
	const GeometryInfo &getGeometryInfo() const { return geom; }
	Object *getNextObject() const { return next; }
};

void ContainModuleInterface::addToContain(Object *o) { put(15, owner->id, o->id); list.push_back(o); ++count; o->containedBy = owner; }
void ContainModuleInterface::removeFromContain(Object *o)
{
	put(16, owner->id, o->id);
	std::vector<Object *>::iterator it = std::find(list.begin(), list.end(), o);
	if (it != list.end()) list.erase(it);
	--count;
	o->containedBy = 0;
}

struct AICommandParms { AICommandType m_cmd; CommandSourceType m_cmdSource; Object *m_obj; };

class POWTruckAIUpdateInterface;
struct GameLogicClass
{
	UnsignedInt frame;
	std::vector<Object *> objs;
	UnsignedInt getFrame() const { return frame; }
	Object *findObjectByID(ObjectID id) const { return (id >= 1 && id <= objs.size()) ? objs[id - 1] : 0; }
	Object *getFirstObject() const { return objs.empty() ? 0 : objs[0]; }
};
GameLogicClass theGameLogicObj;
GameLogicClass *TheGameLogic = &theGameLogicObj;

/* scripted answers, per object index */
struct Script { bool canPick, quickAvail, quickExists, hasAI; };
std::vector<Script> script;
Script &scriptOf(const Object *o) { return script[o->id - 1]; }

struct ActionManagerClass
{
	Bool canPickUpPrisoner(const Object *obj, const Object *prisoner, CommandSourceType)
	{
		if (obj == NULL || prisoner == NULL) { put(6, 0, 0); return FALSE; }
		Bool r = scriptOf(prisoner).canPick;
		put(6, prisoner->id, r);
		return r;
	}
};
ActionManagerClass theActionManagerObj;
ActionManagerClass *TheActionManager = &theActionManagerObj;

struct PartitionManagerClass
{
	Real getDistanceSquared(const Object *a, const Object *b, DistanceCalculationType)
	{
		Real dx = a->pos.x - b->pos.x;
		Real dy = a->pos.y - b->pos.y;
		return dx * dx + dy * dy;
	}
};
PartitionManagerClass thePartitionManagerObj;
PartitionManagerClass *ThePartitionManager = &thePartitionManagerObj;

struct PathfinderClass
{
	Bool quickDoesPathExist(int, const Coord3D *, const Coord3D *to)
	{
		Bool r = script[(int)to->z - 1].quickExists;
		put(19, (unsigned)to->z, r);
		return r;
	}
};
struct AIClass { PathfinderClass pf; PathfinderClass *pathfinder() { return &pf; } };
AIClass theAIObj;
AIClass *TheAI = &theAIObj;

bool g_idle = false, g_allowed = true;

class AIUpdateInterface
{
public:
	Object *m_owner;
	Locomotor m_loco;
	AIUpdateInterface(Thing *thing, const ModuleData *) : m_owner(thing->obj) {}
	virtual ~AIUpdateInterface() {}
	Object *getObject() const { return m_owner; }
	Locomotor *getCurLocomotor() { return &m_loco; }
	virtual UpdateSleepTime update() { put(2); return UPDATE_SLEEP_NONE; }
	virtual void aiDoCommand(const AICommandParms *) { put(5); }
	Bool isAllowedToRespondToAiCommands(const AICommandParms *) { put(3, g_allowed); return g_allowed; }
	void aiIdle(CommandSourceType) { put(4); }
	void ignoreObstacle(const Object *o) { put(7, o->id); }
	void aiMoveToObject(const Object *o, CommandSourceType) { put(8, o->id); }
	void aiDock(Object *o, CommandSourceType s) { put(9, o->id, s); }
	void aiPickUpPrisoner(Object *o, CommandSourceType) { put(11, o->id); }
	void aiReturnPrisoners(Object *o, CommandSourceType) { put(12, o->id); }
	Bool isIdle() const { put(10, g_idle); return g_idle; }
	void setSurrendered(const Object *, Bool) { put(17, m_owner->id); }
	Bool isQuickPathAvailable(const Coord3D *pos) { Bool r = script[(int)pos->z - 1].quickAvail; put(18, (unsigned)pos->z, r); return r; }
	CommandSourceType getLastCommandSource() const { return CMD_FROM_AI; }
	int getLocomotorSet() const { return 0; }
	virtual POWTruckAIUpdateInterface *getPOWTruckAIUpdateInterface() { return 0; }
};

enum POWTruckTask
{
	POW_TRUCK_TASK_WAITING = 0,
	POW_TRUCK_TASK_FIND_TARGET = 1,
	POW_TRUCK_TASK_COLLECTING_TARGET = 2,
	POW_TRUCK_TASK_RETURNING_PRISONERS = 3
};

class POWTruckAIUpdateInterface
{
public:
	virtual void setTask(POWTruckTask task, Object *taskObject = NULL) = 0;
	virtual POWTruckTask getCurrentTask(void) = 0;
	virtual void loadPrisoner(Object *prisoner) = 0;
	virtual void unloadPrisonersToPrison(Object *prison) = 0;
};

class POWTruckAIUpdate : public AIUpdateInterface, public POWTruckAIUpdateInterface
{
public:
	POWTruckAIUpdate(Thing *thing, const ModuleData *moduleData);
	virtual ~POWTruckAIUpdate();
	virtual void onDelete(void);
	virtual UpdateSleepTime update(void);
	virtual POWTruckAIUpdateInterface *getPOWTruckAIUpdateInterface(void) { return this; }
	virtual POWTruckTask getCurrentTask(void) { return m_currentTask; }
	virtual void loadPrisoner(Object *prisoner);
	virtual void unloadPrisonersToPrison(Object *prison);
	virtual void aiDoCommand(const AICommandParms *parms);
	virtual void setTask(POWTruckTask task, Object *taskObject = NULL);
	enum POWTruckAIMode { AUTOMATIC = 0, MANUAL = 1 };
	virtual void setAIMode(POWTruckAIMode mode);
	virtual void updateWaiting(void);
	virtual void updateFindTarget(void);
	virtual void updateCollectingTarget(void);
	virtual void updateReturnPrisoners(void);
	virtual Bool validateTarget(const Object *target);
	virtual void doReturnPrisoners(void);
	virtual void doReturnToPrison(Object *prison);
	virtual Object *findBestPrison(void);
	virtual Object *findBestTarget(void);
	virtual void privatePickUpPrisoner(Object *prisoner, CommandSourceType cmdSource);
	virtual void privateReturnPrisoners(Object *prison, CommandSourceType cmdSource);
	POWTruckAIMode m_aiMode;
	POWTruckTask m_currentTask;
	ObjectID m_targetID;
	ObjectID m_prisonID;
	UnsignedInt m_enteredWaitingFrame;
	UnsignedInt m_lastFindFrame;
	POWTruckAIUpdateModuleData m_data;
	const POWTruckAIUpdateModuleData *getPOWTruckAIUpdateModuleData() const { return &m_data; }
};

#include "pow_truck.inc"

struct DockUpdate { Object *m_obj; Object *getObject() const { return m_obj; } };
class PrisonDockUpdate : public DockUpdate
{
public:
	virtual Bool action(Object *docker, Object *drone);
};
#include "prison_dock.inc"

struct Scenario
{
	std::vector<Object> objs;
	std::vector<ContainModuleInterface> contains;
	std::vector<AIUpdateInterface *> ais;
	Player player;
	POWTruckAIUpdate *truck;
};

void putState(const POWTruckAIUpdate *t, const std::vector<Object> &objs)
{
	put(t->m_aiMode, t->m_currentTask, t->m_targetID);
	put(t->m_prisonID, t->m_enteredWaitingFrame, t->m_lastFindFrame);
	for (size_t i = 0; i < objs.size(); ++i)
	{
		const Object &o = objs[i];
		put(o.contain ? 1 + o.contain->count : 0, o.containedBy ? o.containedBy->id : 0, o.contain ? (unsigned)o.contain->list.size() : 0);
	}
}

} // namespace

extern "C" {

/* in: [n, frame, bored, hang bits, mult bits, allowed, idle, then per object (n of them, the first is the truck): canPick, quickAvail,
**      quickExists, x bits, y bits, producer, hasPlayer, collects, hasContain, count, max, hasAI, cost, height bits, inTruck].
** ops: [0 frame] [1 update] [2 cmd source obj] aiDoCommand [3 task obj] setTask [4 mode] setAIMode [5 obj] loadPrisoner
**      [6 obj] unloadPrisonersToPrison [7 docker prison] PrisonDockUpdate::action [8 idle] [9 allowed]  (obj 0 = NULL, else the id).
** out: per op [1000+op, the calls' words, the result if any, the state]; returns the count of words, or -1 beyond cap. */
int ref_pow_run(const unsigned *in, const int *ops, int nops, unsigned *out, int cap)
{
	theLog.clear();
	int n = (int)in[0];
	Scenario sc;
	sc.objs.resize(n);
	sc.contains.resize(n);
	script.assign(n, Script());
	theGameLogicObj.frame = in[1];
	theGameLogicObj.objs.clear();
	TheGlobalData->m_prisonBountyMultiplier = floatOf(in[4]);
	TheGlobalData->m_prisonBountyTextColor = 7;
	g_allowed = in[5] != 0;
	g_idle = in[6] != 0;
	ModuleData md;
	md.d.m_boredTimeInFrames = in[2];
	md.d.m_hangAroundPrisonDistance = floatOf(in[3]);
	const unsigned *f = in + 7;
	std::vector<int> inTruck(n);
	for (int i = 0; i < n; ++i, f += 15)
	{
		Object &o = sc.objs[i];
		o.id = i + 1;
		script[i].canPick = f[0] != 0; script[i].quickAvail = f[1] != 0; script[i].quickExists = f[2] != 0;
		o.pos.x = floatOf(f[3]); o.pos.y = floatOf(f[4]); o.pos.z = (Real)(i + 1);
		o.producer = f[5];
		o.player = f[6] ? &sc.player : 0;
		o.collects = f[7] != 0;
		bool hasContain = f[8] != 0 || i == 0;
		sc.contains[i].owner = &o; sc.contains[i].count = (int)f[9]; sc.contains[i].max = (int)f[10];
		o.contain = hasContain ? &sc.contains[i] : 0;
		script[i].hasAI = f[11] != 0 || i == 0;
		o.tmpl.cost = (Int)f[12];
		o.geom.height = floatOf(f[13]);
		inTruck[i] = (int)f[14];
		o.containedBy = 0;
		o.ai = 0;
		o.next = 0;
		theGameLogicObj.objs.push_back(&o);
	}
	for (int i = 0; i + 1 < n; ++i) sc.objs[i].next = &sc.objs[i + 1];
	Thing truckThing; truckThing.obj = &sc.objs[0];
	POWTruckAIUpdate truck(&truckThing, &md);
	truck.m_data = md.d; /* MAKE_STANDARD_MODULE_MACRO's getPOWTruckAIUpdateModuleData reads the module data this holds */
	sc.objs[0].ai = &truck;
	std::vector<Thing> things(n);
	std::vector<AIUpdateInterface *> others;
	for (int i = 1; i < n; ++i)
	{
		if (script[i].hasAI) { things[i].obj = &sc.objs[i]; AIUpdateInterface *a = new AIUpdateInterface(&things[i], &md); sc.objs[i].ai = a; others.push_back(a); }
	}
	for (int i = 1; i < n; ++i)
		if (inTruck[i]) { sc.objs[i].containedBy = &sc.objs[0]; sc.contains[0].list.push_back(&sc.objs[i]); }
	putState(&truck, sc.objs);
	PrisonDockUpdate dock;
	int p = 0;
	while (p < nops)
	{
		int op = ops[p++];
		put(1000 + op);
		switch (op)
		{
		case 0: theGameLogicObj.frame = (UnsignedInt)ops[p++]; break;
		case 1: truck.update(); break;
		case 2: {
			AICommandParms parms;
			parms.m_cmd = (AICommandType)ops[p]; parms.m_cmdSource = (CommandSourceType)ops[p + 1];
			parms.m_obj = ops[p + 2] ? theGameLogicObj.findObjectByID(ops[p + 2]) : 0;
			p += 3;
			truck.aiDoCommand(&parms);
			break; }
		case 3: { int task = ops[p], o = ops[p + 1]; p += 2; truck.setTask((POWTruckTask)task, o ? theGameLogicObj.findObjectByID(o) : 0); break; }
		case 4: truck.setAIMode((POWTruckAIUpdate::POWTruckAIMode)ops[p++]); break;
		case 5: { int o = ops[p++]; truck.loadPrisoner(o ? theGameLogicObj.findObjectByID(o) : 0); break; }
		case 6: { int o = ops[p++]; truck.unloadPrisonersToPrison(o ? theGameLogicObj.findObjectByID(o) : 0); break; }
		case 7: {
			int d = ops[p], pr = ops[p + 1]; p += 2;
			dock.m_obj = theGameLogicObj.findObjectByID(pr);
			put(dock.action(d ? theGameLogicObj.findObjectByID(d) : 0, 0));
			break; }
		case 8: g_idle = ops[p++] != 0; break;
		default: g_allowed = ops[p++] != 0; break;
		}
		putState(&truck, sc.objs);
	}
	for (size_t i = 0; i < others.size(); ++i) delete others[i];
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}

}
