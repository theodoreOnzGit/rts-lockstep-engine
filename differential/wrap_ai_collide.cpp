/* Differential-test wrapper for AIUpdate.cpp's collisions (Phase 1 layer 10, p1w13-sonnet, batch A22; upstream commit 0a05454d):
** AIUpdateInterface::hasHigherPathPriority, calculateMaxBlockedSpeed, blockedBy, needToRotate and processCollision, cut verbatim by
** build.rs into OUT_DIR/ai_gen/ai_collide_a22.inc and compiled here over a scene of two parties (0: the AI asked, 1: the other
** object and its AI), each an Object and an AIUpdateInterface of this file: the members the five read and write are the class's
** own (named as AIUpdate.h names them); every query the five make of the objects, the AIs, the path, the partition manager, the
** pathfinder and the logic answers from the scene (ColScene, the same layout as src/differential.rs's ColScene); the commands
** (aiMoveAwayFromUnit, aiMoveToPosition), the pathfinder's adjustToPossibleDestination and the path's computePointOnPath are
** logged. The angle getRelativeAngle2D answers is the party's angle to its path's point when given that point, else its angle to
** the other object.
**
** ref_ai_collide(op, scene, out, log, logCap, &logLen): op 0 processCollision, 1 blockedBy, 2 needToRotate, 3 hasHigherPathPriority,
** 4 calculateMaxBlockedSpeed (each asked of party 0 about party 1). out: the answer (a Bool as 0/1, the speed's bits), then each
** party's m_isBlocked, m_curMaxBlockedSpeed (bits), m_blockedFrames, m_isBlockedAndStuck. Returns the word count. */
#include "Lib/BaseType.h"
#include <math.h>
#include <vector>

#define CRCDEBUG_LOG(x)
#define DEBUG_LOG(x)

typedef Int ObjectID;
typedef UnsignedInt StateID;
enum KindOfType { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_DOZER };
enum ObjectStatusTypes { OBJECT_STATUS_IS_USING_ABILITY };
enum CrushSquishTestType { TEST_CRUSH_ONLY, TEST_SQUISH_ONLY, TEST_CRUSH_OR_SQUISH };
enum CommandSourceType { CMD_FROM_PLAYER = 0, CMD_FROM_SCRIPT, CMD_FROM_AI };
enum FormationID { NO_FORMATION_ID = 0 };
enum LocoGoalType { NONE = 0, POSITION_ON_PATH, POSITION_EXPLICIT, ANGLE };
enum { AI_PANIC = 19, AI_BUSY = 41 };
#define LOGICFRAMES_PER_SECOND 30
#define PATHFIND_CELL_SIZE_F 10.0f

struct ColParty
{
	Int id;
	Real pos[3], dir[3];
	Int kinds;		/* bit 0 infantry, 1 vehicle, 2 dozer */
	Int usingAbility;
	Int formation;
	Int hasPhysics;
	Real vel[3];
	Int crush[3];		/* canCrushOrSquish(the other party, test) by test */
	Real angleToOther, angleToPath;
	Int hasAi;
	UnsignedInt ignoreUntil;
	Int canPathThrough, isBlocked;
	Real curMaxBlockedSpeed;
	Int blockedFrames, isBlockedAndStuck, locoGoalType;
	Real goalPos[3];
	Int goalCell[2];
	Int moving, ground, waiting, movingAway, busy, idle, dead, stateId;
	Int hasLoco, backwards;
	Real wander;
	Int hasPath;
	Real pathPos[3];
	Real adjust[2];		/* adjustToPossibleDestination's offset */
};
struct ColScene { ColParty p[2]; UnsignedInt frame; };

namespace {
const ColScene *g_scene;
std::vector<unsigned> *g_log;
unsigned bitsOf(Real r) { unsigned u; memcpy(&u, &r, 4); return u; }
void L(std::initializer_list<unsigned> w) { g_log->insert(g_log->end(), w); }
}

class Object;
class AIUpdateInterface;
class PhysicsBehavior
{
public:
	Int m_party;
	Coord3D m_vel;
	const Coord3D *getVelocity() const { return &m_vel; }
};
class Locomotor
{
public:
	Int m_party;
	Bool isMovingBackwards() const { return g_scene->p[m_party].backwards != 0; }
	Real getWanderWidthFactor() const { return g_scene->p[m_party].wander; }
};
class LocomotorSet { public: Int m_party; };
struct ClosestPointOnPathInfo { Real distAlongPath; Coord3D posOnPath; Int layer; };
class Path
{
public:
	Int m_party;
	void computePointOnPath(const Object *obj, const LocomotorSet &set, const Coord3D &pos, ClosestPointOnPathInfo &out)
	{
		const ColParty &c = g_scene->p[m_party];
		L({ 3u, (unsigned)m_party });
		out.distAlongPath = 0;
		out.posOnPath.x = c.pathPos[0]; out.posOnPath.y = c.pathPos[1]; out.posOnPath.z = c.pathPos[2];
		out.layer = 0;
	}
};
class ColMachine
{
public:
	Int m_party;
	Coord3D m_goal;
	const Coord3D *getGoalPosition() const { return &m_goal; }
	StateID getCurrentStateID() const { return (StateID)g_scene->p[m_party].stateId; }
};
class Object
{
public:
	Int m_party;
	Coord3D m_pos, m_dir;
	PhysicsBehavior m_physics;
	ObjectID getID() const { return (ObjectID)g_scene->p[m_party].id; }
	const Coord3D *getPosition() const { return &m_pos; }
	const Coord3D *getUnitDirectionVector2D() const { return &m_dir; }
	Bool isKindOf(KindOfType k) const { return ((g_scene->p[m_party].kinds >> (Int)k) & 1) != 0; }
	Bool testStatus(ObjectStatusTypes s) const { return g_scene->p[m_party].usingAbility != 0; }
	FormationID getFormationID() const { return (FormationID)g_scene->p[m_party].formation; }
	PhysicsBehavior *getPhysics() const { return g_scene->p[m_party].hasPhysics ? const_cast<PhysicsBehavior *>(&m_physics) : NULL; }
	Bool canCrushOrSquish(Object *other, CrushSquishTestType t) const { return g_scene->p[m_party].crush[t] != 0; }
	AIUpdateInterface *getAI();
};
class PartitionManager
{
public:
	Real getRelativeAngle2D(const Object *obj, const Coord3D *pos)
	{
		const ColParty &c = g_scene->p[obj->m_party];
		return (pos->x == c.pathPos[0] && pos->y == c.pathPos[1] && pos->z == c.pathPos[2]) ? c.angleToPath : c.angleToOther;
	}
};
class GameLogic { public: UnsignedInt getFrame() const { return g_scene->frame; } };
class Pathfinder
{
public:
	void adjustToPossibleDestination(Object *obj, const LocomotorSet &set, Coord3D *pos)
	{
		const ColParty &c = g_scene->p[obj->m_party];
		L({ 2u, (unsigned)obj->m_party, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
		pos->x += c.adjust[0];
		pos->y += c.adjust[1];
	}
};
class AI { public: Pathfinder m_pf; Pathfinder *pathfinder() { return &m_pf; } };
namespace {
PartitionManager g_partition;
GameLogic g_logic;
AI g_ai;
}
PartitionManager *ThePartitionManager = &g_partition;
GameLogic *TheGameLogic = &g_logic;
AI *TheAI = &g_ai;

class AIUpdateInterface
{
public:
	Int m_party;
	Object *m_object;
	Locomotor m_loco;
	Path m_pathObj;
	ColMachine m_machine;
	ICoord2D m_pathfindGoalCell;
	/* AIUpdate.h's members, as these functions use them */
	UnsignedInt m_ignoreCollisionsUntil;
	Bool m_canPathThroughUnits;
	Bool m_isBlocked;
	Real m_curMaxBlockedSpeed;
	Int m_blockedFrames;
	Bool m_isBlockedAndStuck;
	LocoGoalType m_locomotorGoalType;
	LocomotorSet m_locomotorSet;

	Object *getObject() const { return m_object; }
	const ColParty &c() const { return g_scene->p[m_party]; }
	Bool isMoving() const { return c().moving != 0; }
	Bool isDoingGroundMovement() const { return c().ground != 0; }
	Bool isWaitingForPath() const { return c().waiting != 0; }
	Bool isMovingAwayFrom(Object *obj) const { return c().movingAway != 0; }
	Bool isBusy() const { return c().busy != 0; }
	Bool isIdle() const { return c().idle != 0; }
	Bool isAiInDeadState() const { return c().dead != 0; }
	StateID getCurrentStateID() const { return (StateID)c().stateId; }
	ColMachine *getStateMachine() { return &m_machine; }
	Locomotor *getCurLocomotor() { return c().hasLoco ? &m_loco : NULL; }
	Path *getPath() { return c().hasPath ? &m_pathObj : NULL; }
	const ICoord2D *getPathfindGoalCell() const { return &m_pathfindGoalCell; }
	Int getNumFramesBlocked() const { return m_blockedFrames; }
	const LocomotorSet &getLocomotorSet() const { return m_locomotorSet; }
	void aiMoveAwayFromUnit(Object *obj, CommandSourceType cmdSource) { L({ 0u, (unsigned)m_party, (unsigned)obj->getID(), (unsigned)cmdSource }); }
	void aiMoveToPosition(const Coord3D *pos, CommandSourceType cmdSource)
	{
		L({ 1u, (unsigned)m_party, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (unsigned)cmdSource });
	}

	Bool hasHigherPathPriority(AIUpdateInterface *otherAI) const;
	Real calculateMaxBlockedSpeed(Object *other) const;
	Bool blockedBy(Object *other);
	Bool needToRotate(void);
	Bool processCollision(PhysicsBehavior *physics, Object *other);
};

namespace {
Object g_obj[2];
AIUpdateInterface g_aiu[2];
}
AIUpdateInterface *Object::getAI() { return g_scene->p[m_party].hasAi ? &g_aiu[m_party] : NULL; }

#include "ai_collide_a22.inc"

extern "C" int ref_ai_collide(int op, const ColScene *scene, unsigned *out, unsigned *log, int logCap, int *logLen)
{
	std::vector<unsigned> lg;
	g_scene = scene;
	g_log = &lg;
	for (int i = 0; i < 2; ++i) {
		const ColParty &c = scene->p[i];
		Object &o = g_obj[i];
		o.m_party = i;
		o.m_pos.x = c.pos[0]; o.m_pos.y = c.pos[1]; o.m_pos.z = c.pos[2];
		o.m_dir.x = c.dir[0]; o.m_dir.y = c.dir[1]; o.m_dir.z = c.dir[2];
		o.m_physics.m_party = i;
		o.m_physics.m_vel.x = c.vel[0]; o.m_physics.m_vel.y = c.vel[1]; o.m_physics.m_vel.z = c.vel[2];
		AIUpdateInterface &a = g_aiu[i];
		a.m_party = i;
		a.m_object = &o;
		a.m_loco.m_party = i;
		a.m_pathObj.m_party = i;
		a.m_machine.m_party = i;
		a.m_machine.m_goal.x = c.goalPos[0]; a.m_machine.m_goal.y = c.goalPos[1]; a.m_machine.m_goal.z = c.goalPos[2];
		a.m_pathfindGoalCell.x = c.goalCell[0]; a.m_pathfindGoalCell.y = c.goalCell[1];
		a.m_ignoreCollisionsUntil = c.ignoreUntil;
		a.m_canPathThroughUnits = c.canPathThrough != 0;
		a.m_isBlocked = c.isBlocked != 0;
		a.m_curMaxBlockedSpeed = c.curMaxBlockedSpeed;
		a.m_blockedFrames = c.blockedFrames;
		a.m_isBlockedAndStuck = c.isBlockedAndStuck != 0;
		a.m_locomotorGoalType = (LocoGoalType)c.locoGoalType;
		a.m_locomotorSet.m_party = i;
	}
	std::vector<unsigned> r;
	AIUpdateInterface &me = g_aiu[0];
	switch (op) {
	case 0: r.push_back(me.processCollision(&g_obj[0].m_physics, &g_obj[1]) ? 1u : 0u); break;
	case 1: r.push_back(me.blockedBy(&g_obj[1]) ? 1u : 0u); break;
	case 2: r.push_back(me.needToRotate() ? 1u : 0u); break;
	case 3: r.push_back(me.hasHigherPathPriority(&g_aiu[1]) ? 1u : 0u); break;
	case 4: r.push_back(bitsOf(me.calculateMaxBlockedSpeed(&g_obj[1]))); break;
	}
	for (int i = 0; i < 2; ++i) {
		const AIUpdateInterface &a = g_aiu[i];
		r.push_back(a.m_isBlocked ? 1u : 0u);
		r.push_back(bitsOf(a.m_curMaxBlockedSpeed));
		r.push_back((unsigned)a.m_blockedFrames);
		r.push_back(a.m_isBlockedAndStuck ? 1u : 0u);
	}
	for (size_t i = 0; i < r.size(); ++i) out[i] = r[i];
	*logLen = (int)lg.size();
	for (int i = 0; i < (int)lg.size() && i < logCap; ++i) log[i] = lg[i];
	g_log = NULL;
	return (int)r.size();
}
