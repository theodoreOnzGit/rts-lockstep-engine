/* Differential-test wrapper for the object-lifetime part of GameLogic (upstream commit
** 0a05454d, GeneralsMD/Code/GameEngine): the sleepy-update heap, registerObject,
** allocateObjectID, the id lookup table, findObjectByID, processDestroyList,
** destroyAllObjectsImmediate, loadPostProcess, friend_awakenUpdateModule and the module
** loop of update(), plus Object::isInList/prependToList/removeFromList.
**
** The upstream text is NOT edited and not copied by hand: build.rs cuts each piece verbatim
** out of GameLogic.cpp, GameLogic.h and Object.cpp into OUT_DIR/world_gen/*.inc (first and
** last marker asserted to occur once), and this file #includes the pieces between its
** stand-ins. GameLogic.cpp itself cannot be compiled (it needs the whole engine), so the
** classes the pieces touch are minimal stand-ins with the members and signatures of
** GameLogic.h, Object.h and UpdateModule.h; the friend_ accessors of UpdateModule are
** copies of UpdateModule.h lines 190-230 (they are one-liners). What is NOT upstream text
** and is hand-written here, each marked "STAND-IN":
**   - destroyObject: the real one runs destroy modules, the AI, the pathfinder and the
**     control bar. The stand-in does the two steps the world depends on: set the destroyed
**     status, append to m_objectsToDestroy. (The Rust version's order of the hook calls is
**     pinned by a Rust unit test.)
**   - the frame: runSleepyLoop(now) is the verbatim block of update(); logicFrame() then
**     calls the verbatim processDestroyList and increments m_frame unless m_startNewGame,
**     the two statements of update() (lines 3786, 3824-3826) around the pieces not
**     compiled here (script engine, AI, partition manager, the disabled-status pass, whose
**     hooks are no-ops).
**   - the scripted update(): the module's behaviour, driven by the program.
**
** All engine class names are renamed by macro, as in the other wrappers, so that no symbol
** of this archive collides with another fixture's stand-ins. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <vector>
#include <list>

#define Object WorldTestObject
#define GameLogic WorldTestGameLogic
#define TheGameLogic WorldTestTheGameLogic
#define UpdateModule WorldTestUpdateModule
#define BehaviorModule WorldTestBehaviorModule
#define DisabledMaskType WorldTestDisabledMaskType

typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
#define TRUE true
#define FALSE false
#ifndef NULL
#define NULL 0
#endif

/* GameType.h */
enum ObjectID { INVALID_ID = 0, FORCE_OBJECTID_TO_LONG_SIZE = 0x7ffffff };
/* UpdateModule.h */
enum UpdateSleepTime { UPDATE_SLEEP_INVALID = 0, UPDATE_SLEEP_NONE = 1, UPDATE_SLEEP_FOREVER = 0x3fffffff };
enum SleepyUpdatePhase { PHASE_INITIAL = 0, PHASE_PHYSICS = 1, PHASE_NORMAL = 2, PHASE_FINAL = 3 };

#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define DEBUG_CRASH(m) ((void)0)
#define USE_PERF_TIMER(x)
static int g_crash = 0; /* 1: "illegal index", 2: "index mismatch" (RELEASE_CRASH in friend_awakenUpdateModule) */
#define RELEASE_CRASH(m) (g_crash = (strstr(m, "illegal") ? 1 : 2))

/* GameLogic.cpp: enum { OBJ_HASH_SIZE = 8192 }; */
enum { OBJ_HASH_SIZE = 8192 };

class Object;
class UpdateModule;
class GameLogic;
typedef UpdateModule* UpdateModulePtr;
typedef std::vector<Object*> ObjectPtrVector;
typedef std::list<Object*> ObjectPointerList;
typedef ObjectPointerList::iterator ObjectPointerListIterator;
extern GameLogic* TheGameLogic;

/* output words */
static std::vector<unsigned>* g_out = 0;
static void emit(unsigned v) { if (g_out) g_out->push_back(v); }

/* stand-in: the disabled flags; UpdateModule::getDisabledTypesToProcess is DISABLEDMASK_NONE */
struct DisabledMaskType {
	bool m_any;
	DisabledMaskType() : m_any(false) {}
	bool any() const { return m_any; }
	bool anyIntersectionWith(const DisabledMaskType&) const { return false; /* the other mask is NONE */ }
};

/* the programs */
struct Action { int kind, a, b, c; };
struct Step { unsigned sleep; std::vector<Action> actions; };
struct Script { std::vector<Step> steps; };
struct ModSpec { int phase; unsigned initial; int script; };
static std::vector<Script> g_scripts;
static std::vector<std::vector<ModSpec> > g_groups;

class BehaviorModule {
public:
	UpdateModule* m_update;
	BehaviorModule() : m_update(0) {}
	UpdateModule* getUpdate() { return m_update; }
};

class UpdateModule : public BehaviorModule {
public:
	UnsignedInt m_nextCallFrameAndPhase;
	Int m_indexInLogic;
	SleepyUpdatePhase m_phase;
	Object* m_object;
	int m_idx, m_script;
	unsigned m_calls;
	UpdateModule(Object* o, int idx, SleepyUpdatePhase ph, int script)
		: m_nextCallFrameAndPhase(0), m_indexInLogic(-1), m_phase(ph), m_object(o), m_idx(idx), m_script(script), m_calls(0) { m_update = this; }
	SleepyUpdatePhase getUpdatePhase() const { return m_phase; }
	DisabledMaskType getDisabledTypesToProcess() const { return DisabledMaskType(); }
	/* UpdateModule.h 190-230 */
	UnsignedInt friend_getPriority() const { return m_nextCallFrameAndPhase; }
	UnsignedInt friend_getNextCallFrame() const { return m_nextCallFrameAndPhase >> 2; }
	SleepyUpdatePhase friend_getNextCallPhase() const { return (SleepyUpdatePhase)(m_nextCallFrameAndPhase & 3); }
	void friend_setNextCallFrame(UnsignedInt frame)
	{
		if (frame > UPDATE_SLEEP_FOREVER)
			frame = UPDATE_SLEEP_FOREVER;
		m_nextCallFrameAndPhase = (frame << 2) | getUpdatePhase();
	}
	Int friend_getIndexInLogic() const { return m_indexInLogic; }
	void friend_setIndexInLogic(Int i) { m_indexInLogic = i; }
	const Object* friend_getObject() const { return m_object; }
	UpdateSleepTime update();
};

class Object {
public:
	ObjectID m_id;
	Object* m_next;
	Object* m_prev;
	bool m_destroyed;
	DisabledMaskType m_disabled;
	std::vector<UpdateModule*> m_mods;
	std::vector<BehaviorModule*> m_behaviors; /* NULL-terminated, as Object::m_behaviors */
	Object(ObjectID id) : m_id(id), m_next(0), m_prev(0), m_destroyed(false) {}
	ObjectID getID() const { return m_id; }
	bool isDestroyed() const { return m_destroyed; }
	DisabledMaskType getDisabledFlags() const { return m_disabled; }
	Object* getNextObject() { return m_next; }
	BehaviorModule** getBehaviorModules() { return &m_behaviors[0]; }
	Bool isInList(Object** pListHead) const;
	void prependToList(Object** pListHead);
	void removeFromList(Object** pListHead);
	void friend_deleteInstance();
};

class GameLogic {
public:
	UnsignedInt m_frame;
	Object* m_objList;
	ObjectPtrVector m_objVector;
	std::vector<UpdateModulePtr> m_sleepyUpdates;
	UpdateModulePtr m_curUpdateModule;
	ObjectID m_nextObjID;
	ObjectPointerList m_objectsToDestroy;
	bool m_startNewGame;

	GameLogic() : m_frame(0), m_objList(0), m_curUpdateModule(0), m_nextObjID((ObjectID)1), m_startNewGame(false)
	{
		m_objVector.resize(OBJ_HASH_SIZE, NULL); /* reset(): m_objVector.resize(OBJ_HASH_SIZE, NULL) */
	}

	UnsignedInt getFrame(void) { return m_frame; } /* GameLogic.h: inline UnsignedInt GameLogic::getFrame( void ) { return m_frame; } */
	Object* findObjectByID(ObjectID id);
	Object* getFirstObject(void);
	ObjectID allocateObjectID(void);
	void addObjectToLookupTable(Object* obj);
	void removeObjectFromLookupTable(Object* obj);
	void registerObject(Object* obj);
	void processDestroyList(void);
	void destroyAllObjectsImmediate();
	void loadPostProcess(void);
	void friend_awakenUpdateModule(Object* obj, UpdateModulePtr u, UnsignedInt whenToWakeUp);
	void validateSleepyUpdate() const;
	void eraseSleepyUpdate(Int i);
	Int rebalanceParentSleepyUpdate(Int i);
	Int rebalanceChildSleepyUpdate(Int i);
	void rebalanceSleepyUpdate(Int i);
	void remakeSleepyUpdate();
	void pushSleepyUpdate(UpdateModulePtr u);
	UpdateModulePtr peekSleepyUpdate() const;
	void popSleepyUpdate();
	void runSleepyLoop(UnsignedInt now);
	void destroyObject(Object* obj); /* STAND-IN */
	void logicFrame();               /* STAND-IN */
};

GameLogic* TheGameLogic = 0;

/* ---- verbatim upstream text (build.rs) ---- */
#include "object_list_a.inc"
#include "object_list_b.inc"
#include "find_object.inc"
#include "sleepy_heap.inc"
#include "process_destroy_list.inc"
#include "register_object.inc"
#include "destroy_all.inc"
#include "load_post_process.inc"
void GameLogic::runSleepyLoop(UnsignedInt now)
{
#include "sleepy_loop.inc"
}
/* ---- end of verbatim text ---- */

void GameLogic::destroyObject(Object* obj) /* STAND-IN, see the file comment */
{
	if (!obj || obj->isDestroyed())
		return;
	obj->m_destroyed = true;
	m_objectsToDestroy.push_back(obj);
}

void GameLogic::logicFrame() /* STAND-IN, see the file comment */
{
	runSleepyLoop(m_frame);
	processDestroyList();
	if (!m_startNewGame)
		m_frame++;
}

void Object::friend_deleteInstance()
{
	emit(3);
	emit((unsigned)m_id);
	for (size_t i = 0; i < m_mods.size(); ++i)
		delete m_mods[i];
	delete this;
}

static Object* buildObject(const std::vector<ModSpec>& specs)
{
	/* After loadPostProcess on an empty world m_nextObjID is INVALID_ID (0), and the next object
	** would be registered under id 0, which findObjectByID never finds (undefined in effect;
	** the Rust refuses the registration with BadId(0)). The id is allocated either way; the
	** object is not made. */
	ObjectID id = TheGameLogic->allocateObjectID();
	if (id == INVALID_ID)
		return NULL;
	Object* o = new Object(id);
	for (size_t i = 0; i < specs.size(); ++i) {
		UpdateModule* m = new UpdateModule(o, (int)i, (SleepyUpdatePhase)specs[i].phase, specs[i].script);
		if (specs[i].initial != 0)
			m->friend_setNextCallFrame(specs[i].initial);
		o->m_mods.push_back(m);
		o->m_behaviors.push_back(m);
	}
	o->m_behaviors.push_back(NULL);
	return o;
}

UpdateSleepTime UpdateModule::update()
{
	emit(1);
	emit(TheGameLogic->m_frame);
	emit((unsigned)m_object->m_id);
	emit((unsigned)m_idx);
	emit(friend_getPriority());
	if (m_script < 0 || m_script >= (int)g_scripts.size() || g_scripts[m_script].steps.empty())
		return UPDATE_SLEEP_NONE;
	const Script& s = g_scripts[m_script];
	const Step& st = s.steps[m_calls % s.steps.size()];
	m_calls++;
	for (size_t i = 0; i < st.actions.size(); ++i) {
		const Action& a = st.actions[i];
		if (a.kind == 0) {
			TheGameLogic->destroyObject(TheGameLogic->findObjectByID((ObjectID)a.a));
		} else if (a.kind == 1) {
			Object* o = TheGameLogic->findObjectByID((ObjectID)a.a);
			if (o && a.b >= 0 && a.b < (int)o->m_mods.size())
				TheGameLogic->friend_awakenUpdateModule(o, o->m_mods[a.b], TheGameLogic->m_frame + (UnsignedInt)a.c); /* setWakeFrame: now + delay */
		} else if (a.kind == 2) {
			if (a.a >= 0 && a.a < (int)g_groups.size()) {
				Object* made = buildObject(g_groups[a.a]);
				if (made)
					TheGameLogic->registerObject(made);
			}
		}
	}
	return (UpdateSleepTime)st.sleep;
}

static void snapshot(int opIndex)
{
	GameLogic* g = TheGameLogic;
	emit(2);
	emit((unsigned)opIndex);
	emit(g->m_frame);
	emit((unsigned)g->m_nextObjID);
	emit((unsigned)g->m_objVector.size());
	std::vector<unsigned> ids;
	for (Object* o = g->m_objList; o; o = o->m_next)
		ids.push_back((unsigned)o->m_id);
	emit((unsigned)ids.size());
	for (size_t i = 0; i < ids.size(); ++i) emit(ids[i]);
	emit((unsigned)g->m_objectsToDestroy.size());
	for (ObjectPointerListIterator it = g->m_objectsToDestroy.begin(); it != g->m_objectsToDestroy.end(); ++it) emit((unsigned)(*it)->m_id);
	emit((unsigned)g->m_sleepyUpdates.size());
	for (size_t i = 0; i < g->m_sleepyUpdates.size(); ++i) {
		UpdateModule* u = g->m_sleepyUpdates[i];
		emit((unsigned)u->m_object->m_id);
		emit((unsigned)u->m_idx);
		emit(u->friend_getPriority());
		emit((unsigned)u->friend_getIndexInLogic());
	}
	emit((unsigned)g_crash);
}

extern "C" {

/* ref_world_run: a program on a fresh GameLogic. The program is a stream of ints:
**   nScripts, then per script: nSteps, then per step: sleep, nActions, then per action
**     4 ints (kind, a, b, c): 0 destroy(object a); 1 awaken(object a, module b, delay c: frame now + c);
**     2 spawn(group a)
**   nGroups, then per group: nMods, then per module: phase, initialCallFrame, script (-1: none)
**   nOps, then per op 4 ints (code, a, b, c):
**     0 spawn(group a)            1 run a logic frames       2 destroyObject(id a)
**     3 friend_awakenUpdateModule(object a, module b, frame c) if the module exists
**     4 processDestroyList        5 object a: disabled = b   6 loadPostProcess
**     7 destroyAllObjectsImmediate (then m_nextObjID = 1, as reset())
**     8 m_startNewGame = a        9 m_frame = a (a test knob: upstream sets it in setDefaults)
** Output words: per update call (1, frame, object, module, priority before); per deletion
** (3, id); after every op, and after every frame of a run op, a snapshot (2, op, frame, nextObjID,
** table size, n, list ids newest first.., n, destroy list.., n, (object, module, priority,
** indexInLogic) per heap entry.., crash flag). Returns the number of words, or -1 if they
** would not fit in cap. */
int ref_world_run(const int* prog, int nProg, unsigned* out, int cap)
{
	g_scripts.clear();
	g_groups.clear();
	g_crash = 0;
	int p = 0;
	#define NEXT() (p < nProg ? prog[p++] : 0)
	int nScripts = NEXT();
	for (int s = 0; s < nScripts; ++s) {
		Script sc;
		int nSteps = NEXT();
		for (int i = 0; i < nSteps; ++i) {
			Step st;
			st.sleep = (unsigned)NEXT();
			int nAct = NEXT();
			for (int k = 0; k < nAct; ++k) {
				Action a;
				a.kind = NEXT(); a.a = NEXT(); a.b = NEXT(); a.c = NEXT();
				st.actions.push_back(a);
			}
			sc.steps.push_back(st);
		}
		g_scripts.push_back(sc);
	}
	int nGroups = NEXT();
	for (int g = 0; g < nGroups; ++g) {
		std::vector<ModSpec> mods;
		int nMods = NEXT();
		for (int m = 0; m < nMods; ++m) {
			ModSpec ms;
			ms.phase = NEXT(); ms.initial = (unsigned)NEXT(); ms.script = NEXT();
			mods.push_back(ms);
		}
		g_groups.push_back(mods);
	}
	std::vector<unsigned> words;
	g_out = &words;
	GameLogic* logic = new GameLogic();
	TheGameLogic = logic;
	int nOps = NEXT();
	for (int i = 0; i < nOps; ++i) {
		int code = NEXT(), a = NEXT(), b = NEXT(), c = NEXT();
		switch (code) {
		case 0:
			if (a >= 0 && a < (int)g_groups.size()) {
				Object* made = buildObject(g_groups[a]);
				if (made)
					logic->registerObject(made);
			}
			break;
		case 1:
			for (int f = 0; f < a; ++f) {
				logic->logicFrame();
				snapshot(i);
			}
			continue; /* snapshots already written */
		case 2:
			logic->destroyObject(logic->findObjectByID((ObjectID)a));
			break;
		case 3: {
			Object* o = logic->findObjectByID((ObjectID)a);
			if (o && b >= 0 && b < (int)o->m_mods.size())
				logic->friend_awakenUpdateModule(o, o->m_mods[b], (UnsignedInt)c);
			break;
		}
		case 4: logic->processDestroyList(); break;
		case 5: {
			Object* o = logic->findObjectByID((ObjectID)a);
			if (o) o->m_disabled.m_any = (b != 0);
			break;
		}
		case 6: logic->loadPostProcess(); break;
		case 7:
			logic->destroyAllObjectsImmediate();
			logic->m_nextObjID = (ObjectID)1;
			break;
		case 8: logic->m_startNewGame = (a != 0); break;
		case 9: logic->m_frame = (UnsignedInt)a; break;
		default: break;
		}
		snapshot(i);
	}
	g_out = 0;
	/* free what is left (destroyed-but-unprocessed objects and live ones) */
	for (Object* o = logic->m_objList; o;) {
		Object* next = o->m_next;
		for (size_t k = 0; k < o->m_mods.size(); ++k) delete o->m_mods[k];
		delete o;
		o = next;
	}
	delete logic;
	TheGameLogic = 0;
	if ((int)words.size() > cap)
		return -1;
	for (size_t i = 0; i < words.size(); ++i) out[i] = words[i];
	return (int)words.size();
}

}
