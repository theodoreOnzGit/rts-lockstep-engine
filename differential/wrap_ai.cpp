/* Differential-test wrapper for the AI (p1w13-sonnet, layer 10; upstream commit 0a05454d), batch A2: AIUpdateInterface's members
** that do not need its state machine, driving the REAL locomotors along REAL paths.
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp     (#included, UNCHANGED)
**   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h            (#included, UNCHANGED)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp        (batch P1's cut, pathfind_gen/path_p1.inc: the paths; and
**                                                                          ai_gen/ai_pathfinder_a2.inc: chooseBestLocomotorForPosition)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (verbatim members, ai_gen/ai_update_a2.inc)
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AIUpdate.h        (the data members, ai_gen/ai_update_members.inc)
** on the stand-ins of differential/shim_ai (ai_world.h; PreRTS.h renames them and every class above Ai*).
**
** AIUpdateInterface itself cannot be declared from its header (which needs the whole AI): this file declares it with the header's
** data members verbatim (ai_update_members.inc), the inlines the members under test call (copied from the header, each marked),
** and the members under test. Its module data is a stand-in with AIUpdateModuleData's members and its real
** findLocomotorTemplateVector. The world (ai_world.h) answers from the scene the test installs (AiEnv) and logs every call that
** crosses the cut; the pathfinder is the real class over a map of real cells (batch P1's hash), whose calls not compiled here are
** stand-ins: validMovementTerrain (the Locomotor build's), validMovementPosition (the hash), updateLayer and setDebugPath.
**
** The Locomotor blocks are read by the real INI reader into this build's TheLocomotorStore: the block table of INI.cpp names the
** Locomotor build's handler, so ref_ai_a2_load runs INI::load's loop itself, sending each `Locomotor` block to this build's
** (renamed) handler.
**
** Batch A3 adds the victim, mood, morale and state-query members (ai_update_a3.inc), over more stand-ins: the scene's other objects
** (ids 1..31; which exist, which have an AI), the controlling player and its upgrades, the team, the container, stealth, the last
** damage, TheAI's data, vision range and closest enemy, the current weapon, three behavior modules' hordes, and (until batch A4)
** the state machine's answers.
**
** Batch A4 replaces the stand-in state machine with the REAL one: AIStateMachine.h read unchanged, StateMachine.cpp compiled
** unchanged beside this file (State and StateMachine renamed by PreRTS.h), AIStates.cpp's AIStateMachine members, idle, dead and
** wait states (ai_states_a4.inc: verbatim but for the constructor, where each state class not yet translated is an
** AIPlaceholderState of its id, logged and answering from the hash) and AIUpdate.cpp's makeStateMachine, onObjectCreated and update
** (ai_update_a4.inc). New stand-ins: the pathfinder's goal position, snap, goal update and queue; the turrets' updates; the idle
** state's commands (aiMoveToObject, aiAttackObject: logged); the repulsor; the vision range; the dead state's model conditions and
** script notice; a Squad and a Waypoint that are never reached.
**
** Entry points: ref_ai_a2_reset, ref_ai_a4_xfer_path, ref_ai_a2_load, ref_ai_a2_new, ref_ai_a2_op, ref_ai_a2_dump (see each). */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <initializer_list>
#define private public
#define protected public
#include "PreRTS.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/XferCRC.h"
#include "Common/NameKeyGenerator.h"
#include "ai_world.h"
/* AIPathfind.h names PathfindCell in PathfindCellInfo before declaring it (MSVC took its friend declaration as one) */
class PathfindCell;
/* (Locomotor.cpp includes the real AIPathfind.h, after defining the surface names its headers declare) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp"
/* CRCDebug.h's logging macros, as the release build has them (empty); the perf timers (PerfTimer.h, off) */
#ifndef CRCDEBUG_LOG
#define CRCDEBUG_LOG(x)
#endif
#ifndef DUMPCOORD3D
#define DUMPCOORD3D(x)
#endif
#define DECLARE_PERF_TIMER(x)
#define USE_PERF_TIMER(x)
/* batch P1's line tests (the paths' calls of them are edited to these by build.rs) */
Bool p1IsLinePassable( Pathfinder *pf, const Object *obj, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindLayerEnum layer,
	const Coord3D& startWorld, const Coord3D& endWorld, Bool blocked, Bool allowPinched );
Bool p1IsGroundPathPassable( Pathfinder *pf, Bool isCrusher, const Coord3D& startWorld, PathfindLayerEnum startLayer, const Coord3D& endWorld,
	Int pathDiameter );
#include "path_p1.inc"
#include "ai_pathfinder_a2.inc"
/* batch A4: the real AIStateMachine.h (its State and StateMachine are StateMachine.cpp's, compiled beside this file) */
#include "GameLogic/AIStateMachine.h"

/* ---- the AI ------------------------------------------------------------------------------------------------------------------ */

class Thing;
class ModuleData;
class PolygonTrigger;
class AttackPriorityInfo;
class Waypoint;
class TurretAIData;
/* TurretAI (batch T1, its own build): the AI makes one per turret its data has; here an empty object, whose presence the mood
** matrix asks */
class TurretAI
{
public:
	TurretAI(Object *owner, const TurretAIData *data, WhichTurretType tur) : m_which((Int)tur) {}
	/* batch A4: the AI's update runs it (batch T1's, its own build): logged, its sleep from the hash */
	UpdateSleepTime updateTurretAI();
	Int m_which;
};

/* batch A4: AIStateMachine's neighbours not yet translated. Squad (Squad.cpp): no machine of these batches has one (the squad
** setters are not compiled; xfer writes `hasSquad` false, and a stream saying true is never loaded). Waypoint: no goal waypoint is
** set (xfer writes its name empty, and loads none). */
class Squad : public MemoryPoolObject, public Snapshot
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(Squad, "AiSquad")
public:
	Squad() {}
protected:
	virtual void crc( Xfer *xfer ) { abort(); }
	virtual void xfer( Xfer *xfer ) { abort(); }
	virtual void loadPostProcess() { abort(); }
};
EMPTY_DTOR(Squad)
class Waypoint
{
public:
	AsciiString getName() const { abort(); }
};

/* batch A4: the AI states not yet translated. Each is this class with its id (build.rs edits AIStateMachine's constructor): its
** isIdle/isAttack/isBusy are its class's static answers (AIAttackState's TRUE for the four attack states; FALSE for the others,
** whose answers are their inner machines' or the base's); its onEnter, update and onExit are logged, and onEnter and update
** answer from the hash (the scene's stateRate); its snapshot is the version only. The Rust side is AiState::Untranslated with the
** host's callbacks. */
class AIPlaceholderState : public State
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(AIPlaceholderState, "AIPlaceholderState")
public:
	AIPlaceholderState( StateMachine *machine, StateID id ) : State( machine, "AIPlaceholderState" ), m_id(id) { }
	virtual StateReturnType onEnter();
	virtual void onExit( StateExitType status );
	virtual StateReturnType update();
	virtual Bool isAttack() const
	{
		return m_id == AI_ATTACK_POSITION || m_id == AI_ATTACK_OBJECT || m_id == AI_FORCE_ATTACK_OBJECT || m_id == AI_ATTACK_AND_FOLLOW_OBJECT;
	}
protected:
	virtual void crc( Xfer *xfer ) {}
	virtual void xfer( Xfer *xfer ) { XferVersion cv = 1; XferVersion v = cv; xfer->xferVersion( &v, cv ); }
	virtual void loadPostProcess() {}
	StateID m_id;
};
EMPTY_DTOR(AIPlaceholderState)

/* AIUpdateModuleData's members (AIUpdate.h), and its real findLocomotorTemplateVector (ai_update_a2.inc) */
class AIUpdateModuleData
{
public:
	LocomotorTemplateMap	m_locomotorTemplates;
	const TurretAIData*		m_turretData[MAX_TURRETS];
	UnsignedInt						m_moodAttackCheckRate;
	Bool        m_forbidPlayerCommands;
	Bool        m_turretsLinked;
	UnsignedInt						m_autoAcquireEnemiesWhenIdle;
	const LocomotorTemplateVector* findLocomotorTemplateVector(LocomotorSetType t) const;
};

class AIUpdateInterface : public UpdateModule
{
public:
	AIUpdateInterface( Thing *thing, const ModuleData* moduleData );
	/* MAKE_STANDARD_MODULE_DATA_MACRO_ABC's getAIUpdateModuleData: the driver's data */
	const AIUpdateModuleData* getAIUpdateModuleData() const;

	/* AIUpdate.h's inlines that the members below call, verbatim */
	Bool isAiInDeadState( void ) const { return m_isAiDead; }				///< return true if we are dead
	const Locomotor *getCurLocomotor() const {return m_curLocomotor;}
	Locomotor *getCurLocomotor() { return m_curLocomotor; }
	Bool isWaitingForPath(void) const {return m_waitingForPath;}
	Path* getPath( void ) { return m_path; }				///< return the agent's current path
	const Path* getPath( void ) const { return m_path; }				///< return the agent's current path
	inline Real getPathExtraDistance() const { return m_pathExtraDistance; }
	inline AIStateMachine* getStateMachine() { return m_stateMachine; }
	/* batch A4 (AIUpdate.h): `inline StateID getCurrentStateID() const { return getStateMachine()->getCurrentStateID(); }` */
	inline StateID getCurrentStateID() const { return getStateMachine()->getCurrentStateID(); }
	inline const AIStateMachine* getStateMachine() const { return m_stateMachine; }
	inline const Coord3D* getGoalPosition() const { return getStateMachine()->getGoalPosition(); }	///< return the id of the current state of the machine
	void setFinalPosition(const Coord3D *pos) { m_finalPosition = *pos; m_doFinalPosition = false;}
	/* batch A3 */
	const AttackPriorityInfo *getAttackInfo(void) {return m_attackInfo;}
	UnsignedInt getNextMoodCheckTime() const { return m_nextMoodCheckTime; }
	const LocomotorSet& getLocomotorSet(void) const {return m_locomotorSet;}
	/* `virtual void addTargeter(ObjectID id, Bool add) { return; }`: what another object's AI does with it is its class's (logged) */
	void addTargeter(ObjectID id, Bool add);
	void notifyCrate(ObjectID id) { m_crateCreated = id; }

	/* batch A2's members (ai_update_a2.inc) */
	Real getCurLocomotorSpeed() const;
	void setLocomotorUpgrade(Bool set);
	Bool chooseLocomotorSet(LocomotorSetType wst);
	Bool chooseLocomotorSetExplicit(LocomotorSetType wst);
	void chooseGoodLocomotorFromCurrentSet( void );
	void setQueueForPathTime(Int frames);
	void wakeUpNow();
	void friend_notifyStateMachineChanged();
	Bool queueWaypoint( const Coord3D *pos );
	void executeWaypointQueue( void );
	void clearWaypointQueue( void );
	void markAsDead();
	void destroyPath( void );
	void friend_startingMove(void);
	void friend_endingMove();
	void friend_setPath(Path *path);
	Bool isValidLocomotorPosition(const Coord3D* pos) const;
	UpdateSleepTime doLocomotor( void );
	void setLocomotorGoalPositionOnPath();
	void setLocomotorGoalPositionExplicit(const Coord3D& newPos);
	void setLocomotorGoalOrientation(Real angle);
	void setLocomotorGoalNone();
	Bool isDoingGroundMovement(void) const;
	Bool isAircraftThatAdjustsDestination(void) const;
	Bool getTreatAsAircraftForLocoDistToGoal() const;
	Real getLocomotorDistanceToGoal();
	void setAttitude( AttitudeType tude );
	void ignoreObstacleID( ObjectID id );
	void setLastCommandSource( CommandSourceType source );
	Bool hasLocomotorForSurface(LocomotorSurfaceType surfaceType);

	/* batch A3's members (ai_update_a3.inc) */
	Object* checkForCrateToPickup();
	void setCurrentVictim( const Object *victim );
	Object *getCurrentVictim( void ) const;
	const Coord3D *getCurrentVictimPos( void ) const;
	AttitudeType getAttitude( void ) const;
	AIStateType getAIStateType() const;
	void ignoreObstacle( const Object *obj );
	ObjectID getIgnoredObstacleID( void ) const;
	Object* getEnterTarget();
	UnsignedInt getMoodMatrixValue( void ) const;
	UnsignedInt getMoodMatrixActionAdjustment( MoodMatrixAction action ) const;
	void wakeUpAndAttemptToTarget( void );
	void resetNextMoodCheckTime();
	void setNextMoodCheckTime( UnsignedInt frame );
	Bool canAutoAcquireWhileStealthed() const;
	Object* getNextMoodTarget( Bool calledByAI, Bool calledDuringIdle );
	void evaluateMoraleBonus( void );
	Bool isIdle() const;
	Bool isAttacking() const;
	Bool isBusy() const;
	Bool isClearingMines() const;
	Bool isMovingAwayFrom(Object *obj) const;
	Bool isMoving() const;
	AIGroup *getGroup(void);

	/* batch A4's members (ai_update_a4.inc) */
	AIStateMachine* makeStateMachine();
	void onObjectCreated();
	UpdateSleepTime update( void );
	/* AICommandInterface's (AI.h) commands the idle state gives: aiDoCommand, not yet translated (logged: the command, its source,
	** its object, its int value) */
	void aiMoveToObject( Object *obj, CommandSourceType cmdSource );
	void aiAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );

#include "ai_update_members.inc"
};

#include "ai_update_a2.inc"
#include "ai_update_a3.inc"
#include "ai_update_a4.inc"
#include "ai_states_a4.inc"

#undef private
#undef protected

/* ---- the scene, its log, the world ------------------------------------------------------------------------------------------- */

AiEnv g_aiEnv;
std::vector<unsigned> *g_aiLog = 0;
int g_aiQuiet = 0;

namespace {

unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
float floatOf(unsigned u) { float f; memcpy(&f, &u, 4); return f; }

std::vector<unsigned> g_log;

/* one log record: code, number of words, the words */
void L(int code, std::initializer_list<unsigned> w)
{
	if (g_aiQuiet) return;
	g_aiLog->push_back((unsigned)code);
	g_aiLog->push_back((unsigned)w.size());
	for (unsigned x : w) g_aiLog->push_back(x);
}

Real groundAt(Real x, Real y) { return g_aiEnv.ga * x + g_aiEnv.gb * y + g_aiEnv.gc; }

/* batch P1's hash: the seed, a tag, the words (tests/differential_ai_update.rs has the same function) */
unsigned p1Hash(unsigned tag, std::initializer_list<unsigned> words)
{
	unsigned x = g_aiEnv.seed ^ (tag * 0x85ebca6bu);
	for (unsigned w : words) {
		x ^= w;
		x *= 0x9E3779B1u;
		x ^= x >> 15;
	}
	return x;
}
bool p1Pick(unsigned h, unsigned rate) { return (h >> 8) % 100u < rate; }

Coord3D g_velocity;
Object g_object;
GlobalData g_globalData;
GameLogic g_gameLogic;
TerrainLogic g_terrain;
PartitionManager g_partition;
AI g_ai;
AIGroup g_group;
/* batch A3: the other objects of the scene (ids 1..31), the AIs they may have, the objects' neighbours */
Object g_others[32];
alignas(16) unsigned char g_otherAiMem[sizeof(AIUpdateInterface)];
AiData g_aiData;
Player g_player;
TeamTemplateInfo g_teamInfo;
TeamPrototype g_teamPrototype;
Team g_team;
ContainModuleInterface g_contain;
StealthUpdate g_stealth;
Weapon g_weapon;
DamageInfo g_lastDamage;
UpgradeTemplate g_nationalism, g_fanaticism, g_otherUpgrade;
UpgradeCenter g_upgradeCenter;
HordeUpdateInterface g_horde[3];
BehaviorModule g_modules[3];
BehaviorModule *g_moduleList[4];
ScriptEngine g_scriptEngine;
UnsignedInt g_nextCallFrame = 0;		/* the AI module's sleepy update: its next call frame */
alignas(16) unsigned char g_pathfinderMem[sizeof(Pathfinder)];

/* the map: W x H cells from (0, 0) (Pathfinder::m_map, m_extent); each cell's type and pinch from the hash of its coordinates */
enum { MAP_W = 24, MAP_H = 16 };
alignas(16) unsigned char g_cellMem[MAP_W * MAP_H * sizeof(PathfindCell)];
PathfindCell *g_columns[MAP_W];

AIUpdateModuleData *g_data = 0;
AIUpdateInterface *g_ai_update = 0;
/* batch A4: the file the machine's xfer goes through, and the last save */
std::string g_xferPath;
std::vector<unsigned char> g_saved;

}  // namespace

GlobalData *TheGlobalData = &g_globalData;
GameLogic *TheGameLogic = &g_gameLogic;
TerrainLogic *TheTerrainLogic = &g_terrain;
PartitionManager *ThePartitionManager = &g_partition;
AI *TheAI = &g_ai;
UpgradeCenter *TheUpgradeCenter = &g_upgradeCenter;
ScriptEngine *TheScriptEngine = &g_scriptEngine;

Pathfinder *AI::pathfinder() { return (Pathfinder *)g_pathfinderMem; }

UnsignedInt GameLogic::getFrame() const { return g_aiEnv.frame; }

Real TerrainLogic::getGroundHeight(Real x, Real y, Coord3D *normal) const
{
	L(LOG_GROUND_HEIGHT, { bitsOf(x), bitsOf(y) });
	return groundAt(x, y);
}

Real TerrainLogic::getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal, Bool clip) const
{
	L(LOG_LAYER_HEIGHT, { bitsOf(x), bitsOf(y), (unsigned)layer, (unsigned)clip });
	Real h = groundAt(x, y);
	if (layer != LAYER_GROUND) h = h + g_aiEnv.layerOffset;
	if (!clip) h = h + 0.5f;
	return h;
}

Bool TerrainLogic::isUnderwater(Real x, Real y, Real *waterZ, Real *terrainZ)
{
	L(LOG_UNDERWATER, { bitsOf(x), bitsOf(y) });
	if (x < g_aiEnv.wx) {
		if (waterZ) *waterZ = g_aiEnv.wz;
		if (terrainZ) *terrainZ = groundAt(x, y);
		return TRUE;
	}
	if (terrainZ) *terrainZ = groundAt(x, y);
	return FALSE;
}

PathfindLayerEnum TerrainLogic::getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges)
{
	L(LOG_HIGHEST_LAYER, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	return (PathfindLayerEnum)g_aiEnv.highestLayer;
}

Int TerrainLogic::alignOnTerrain(Real angle, const Coord3D &pos, Bool stickToGround, Matrix3D &mtx)
{
	/* only for KINDOF_STICK_TO_TERRAIN_SLOPE things, which this scene has none of */
	abort();
}

Waypoint *TerrainLogic::getWaypointByName(AsciiString name) { abort(); }

/* the paths: batch P1's hash */
PathfindLayerEnum TerrainLogic::getLayerForDestination(const Coord3D *pos)
{
	unsigned h = p1Hash(1, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	PathfindLayerEnum layer = (PathfindLayerEnum)(1 + (h >> 8) % 3);
	L(LOG_LAYER_FOR, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (unsigned)layer });
	return layer;
}

Real PartitionManager::getGroundOrStructureHeight(Real posx, Real posy)
{
	L(LOG_STRUCT_HEIGHT, { bitsOf(posx), bitsOf(posy) });
	return groundAt(posx, posy) + g_aiEnv.structOffset;
}

/* PartitionManager::getDistanceSquared(obj, pos, FROM_CENTER_3D) (layer 8, translated): the 3-D distance from the object's centre */
Real PartitionManager::getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc, Coord3D *vec)
{
	if (dc != FROM_CENTER_3D) abort();
	const Coord3D *p = obj->getPosition();
	Real dx = pos->x - p->x, dy = pos->y - p->y, dz = pos->z - p->z;
	Real d = dx * dx + dy * dy + dz * dz;
	L(LOG_DISTANCE_SQUARED, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(d) });
	return d;
}

/* the pathfinder's members not compiled here */
Bool Pathfinder::validMovementTerrain(PathfindLayerEnum layer, const Locomotor *locomotor, const Coord3D *pos)
{
	L(LOG_VALID_TERRAIN, { (unsigned)layer, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	if (g_aiEnv.invEnabled &&
		pos->x >= g_aiEnv.invBox[0] && pos->x <= g_aiEnv.invBox[1] &&
		pos->y >= g_aiEnv.invBox[2] && pos->y <= g_aiEnv.invBox[3])
		return FALSE;
	return TRUE;
}
Bool Pathfinder::validMovementPosition(Bool isCrusher, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindCell *toCell, PathfindCell *fromCell)
{
	unsigned cell = toCell ? (unsigned)(toCell - (PathfindCell *)g_cellMem) : 0xffffffffu;
	bool r = p1Pick(p1Hash(7, { (unsigned)isCrusher, (unsigned)acceptableSurfaces, cell }), g_aiEnv.validPosRate);
	L(LOG_VALID_POSITION, { (unsigned)isCrusher, (unsigned)acceptableSurfaces, cell, r ? 1u : 0u });
	return r;
}
void Pathfinder::updateLayer(Object *obj, PathfindLayerEnum layer) { L(LOG_UPDATE_LAYER, { (unsigned)layer }); }
void Pathfinder::setDebugPath(Path *debugpath) {}
/* a layer's cells (zone_p3.inc): the scenes' layers have none, so a layer's cell is the ground's (getCell) */
PathfindCell *PathfindLayer::getCell(Int x, Int y) { return NULL; }

Bool p1IsLinePassable( Pathfinder *pf, const Object *obj, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindLayerEnum layer,
	const Coord3D& startWorld, const Coord3D& endWorld, Bool blocked, Bool allowPinched )
{
	unsigned h = p1Hash(2, { (unsigned)obj->getID(), (unsigned)acceptableSurfaces, (unsigned)layer, bitsOf(startWorld.x), bitsOf(startWorld.y),
		bitsOf(endWorld.x), bitsOf(endWorld.y), (unsigned)blocked, (unsigned)allowPinched });
	bool r = p1Pick(h, g_aiEnv.lineRate);
	L(LOG_LINE_PASSABLE, { (unsigned)acceptableSurfaces, (unsigned)layer, bitsOf(startWorld.x), bitsOf(startWorld.y), bitsOf(endWorld.x),
		bitsOf(endWorld.y), (unsigned)blocked, (unsigned)allowPinched, r ? 1u : 0u });
	return r;
}

Bool p1IsGroundPathPassable( Pathfinder *pf, Bool isCrusher, const Coord3D& startWorld, PathfindLayerEnum startLayer, const Coord3D& endWorld,
	Int pathDiameter )
{
	unsigned h = p1Hash(3, { (unsigned)isCrusher, bitsOf(startWorld.x), bitsOf(startWorld.y), (unsigned)startLayer, bitsOf(endWorld.x),
		bitsOf(endWorld.y), (unsigned)pathDiameter });
	bool r = p1Pick(h, g_aiEnv.groundRate);
	L(LOG_GROUND_PASSABLE, { (unsigned)isCrusher, bitsOf(startWorld.x), bitsOf(startWorld.y), (unsigned)startLayer, bitsOf(endWorld.x),
		bitsOf(endWorld.y), (unsigned)pathDiameter, r ? 1u : 0u });
	return r;
}

BodyDamageType BodyModuleInterface::getDamageState() const { return (BodyDamageType)g_aiEnv.bodyState; }

Bool PhysicsBehavior::getIsStunned() const { return g_aiEnv.stunned != 0; }
void PhysicsBehavior::applyMotiveForce(const Coord3D *force) { L(LOG_APPLY_FORCE, { bitsOf(force->x), bitsOf(force->y), bitsOf(force->z) }); }
void PhysicsBehavior::scrubVelocity2D(Real desiredVelocity) { L(LOG_SCRUB, { bitsOf(desiredVelocity) }); }
void PhysicsBehavior::setTurning(PhysicsTurningType turning) { L(LOG_SET_TURNING, { (unsigned)(Int)turning }); }
const Coord3D *PhysicsBehavior::getVelocity() const
{
	g_velocity.x = g_aiEnv.vel[0];
	g_velocity.y = g_aiEnv.vel[1];
	g_velocity.z = g_aiEnv.vel[2];
	return &g_velocity;
}
Real PhysicsBehavior::getVelocityMagnitude() const { return g_aiEnv.velMag; }
Real PhysicsBehavior::getForwardSpeed2D() const { return g_aiEnv.fwd2d; }
Real PhysicsBehavior::getForwardSpeed3D() const { return g_aiEnv.fwd3d; }
Real PhysicsBehavior::getMass() const { return g_aiEnv.mass; }
Bool PhysicsBehavior::isMotive() const { return g_aiEnv.motive != 0; }
void PhysicsBehavior::setExtraFriction(Real friction) { L(LOG_SET_EXTRA_FRICTION, { bitsOf(friction) }); }
void PhysicsBehavior::setAllowAirborneFriction(Bool allow) { L(LOG_SET_AIRBORNE_FRICTION, { (unsigned)allow }); }
void PhysicsBehavior::setStickToGround(Bool stick) { L(LOG_SET_STICK, { (unsigned)stick }); }
Bool PhysicsBehavior::getAllowToFall() const { return g_aiEnv.allowToFall != 0; }

Real GeometryInfo::getMajorRadius() const { return g_aiEnv.majorRadius; }
Real GeometryInfo::getBoundingCircleRadius() const
{
	/* batch A3: another object's (the container's) radius is the scene's container radius */
	return this == &g_object.m_geometry ? g_aiEnv.boundingCircleRadius : g_aiEnv.containerRadius;
}

/* the owner's reaction to a transform change (Thing's pure virtual): logged with the NEW state */
void Object::reactToTransformChange(const Matrix3D *oldMtx, const Coord3D *oldPos, Real oldAngle)
{
	L(LOG_TRANSFORM, { bitsOf(m_transform[0][0]), bitsOf(m_transform[0][1]), bitsOf(m_transform[0][2]), bitsOf(m_transform[0][3]),
		bitsOf(m_transform[1][0]), bitsOf(m_transform[1][1]), bitsOf(m_transform[1][2]), bitsOf(m_transform[1][3]),
		bitsOf(m_transform[2][0]), bitsOf(m_transform[2][1]), bitsOf(m_transform[2][2]), bitsOf(m_transform[2][3]),
		bitsOf(m_cachedPos.x), bitsOf(m_cachedPos.y), bitsOf(m_cachedPos.z), bitsOf(m_cachedAngle) });
}

Object::Object()
{
	m_transform.Make_Identity();
	m_cachedPos.x = m_cachedPos.y = m_cachedPos.z = 0.0f;
	m_cachedAngle = 0.0f;
	m_cachedDirVector.x = m_cachedDirVector.y = m_cachedDirVector.z = 0.0f;
	m_cachedAltitudeAboveTerrain = 0.0f;
	m_cachedAltitudeAboveTerrainOrWater = 0.0f;
	m_cacheFlags = 0;
	m_status = 0;
	m_otherId = INVALID_ID;
}

PhysicsBehavior *Object::getPhysics() { return g_aiEnv.hasPhysics ? &m_physics : 0; }
const PhysicsBehavior *Object::getPhysics() const { return g_aiEnv.hasPhysics ? &m_physics : 0; }
BodyModuleInterface *Object::getBodyModule() { return &m_body; }
PathfindLayerEnum Object::getLayer() const { return (PathfindLayerEnum)g_aiEnv.layer; }
Bool Object::isKindOf(KindOfType t) const
{
	if (t == KINDOF_PROJECTILE) return (g_aiEnv.kindMask & 1) != 0;
	if (t == KINDOF_DOZER) return (g_aiEnv.kindMask & 2) != 0;
	if (t == KINDOF_IMMOBILE) return (g_aiEnv.kindMask & 4) != 0;
	if (t == KINDOF_PRODUCED_AT_HELIPAD) return (g_aiEnv.kindMask & 8) != 0;
	/* batch A3 */
	if (t == KINDOF_INFANTRY) return (g_aiEnv.kindMask & 16) != 0;
	if (t == KINDOF_IGNORED_IN_GUI) return (g_aiEnv.kindMask & 32) != 0;
	if (t == KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT) return (g_aiEnv.kindMask & 64) != 0;
	/* batch A4 */
	if (t == KINDOF_CAN_BE_REPULSED) return (g_aiEnv.kindMask & 128) != 0;
	return FALSE;
}
ObjectStatusMaskType Object::getStatusBits() const { ObjectStatusMaskType m; m.m_bits = m_status; return m; }
void Object::setStatus(ObjectStatusMaskType objectStatus, Bool set)
{
	int bit = 0;
	while (bit < 63 && !((objectStatus.m_bits >> bit) & 1)) ++bit;
	L(LOG_SET_STATUS, { (unsigned)bit, (unsigned)set });
	if (set) m_status |= objectStatus.m_bits; else m_status &= ~objectStatus.m_bits;
}
Real Object::getCarrierDeckHeight() const { return g_aiEnv.deckHeight; }
const GeometryInfo &Object::getGeometryInfo() const { return m_geometry; }
Bool Object::isDisabledByType(DisabledType type) const
{
	if (type == DISABLED_HELD) return g_aiEnv.disabledHeld != 0;
	if (type == DISABLED_UNMANNED) return g_aiEnv.disabledUnmanned != 0;
	/* batch A4 */
	if (type == DISABLED_PARALYZED || type == DISABLED_EMP || type == DISABLED_SUBDUED || type == DISABLED_HACKED)
		return ((g_aiEnv.disabledMask >> (Int)type) & 1) != 0;
	return FALSE;
}
void Object::setModelConditionState(ModelConditionFlagType a) { L(LOG_SET_MODEL_COND, { (unsigned)a }); }
void Object::clearModelConditionState(ModelConditionFlagType a) { L(LOG_CLEAR_MODEL_COND, { (unsigned)a }); }
ObjectID Object::getID() const { return m_otherId ? m_otherId : (ObjectID)g_aiEnv.objectId; }
UnsignedByte Object::getCrusherLevel() const { return (UnsignedByte)g_aiEnv.crusherLevel; }
void Object::setEffectivelyDead(Bool dead) { L(LOG_EFFECTIVELY_DEAD, { (unsigned)dead }); }

/* isAboveTerrain() of Thing.h: getHeightAboveTerrain() > 0; its ground-height query is the
** Thing machinery's, not the locomotor's, so it is not logged */
Bool Object::isAboveTerrain() const
{
	++g_aiQuiet;
	Bool r = getHeightAboveTerrain() > 0.0f;
	--g_aiQuiet;
	return r;
}

/* ---- Thing, verbatim (build.rs: `Thing::` is read as `Object::`) */
#include "thing_methods.inc"
#include "thing_heights.inc"

/* the AI's neighbours */
UpdateSleepTime UpdateModule::getWakeFrame() const
{
	UnsignedInt now = TheGameLogic->getFrame();
	return g_nextCallFrame > now ? UPDATE_SLEEP(g_nextCallFrame - now) : UPDATE_SLEEP_NONE;
}
void UpdateModule::setWakeFrame(Object *obj, UpdateSleepTime wakeDelay)
{
	L(LOG_SET_WAKE, { (unsigned)wakeDelay });
	g_nextCallFrame = TheGameLogic->getFrame() + (UnsignedInt)wakeDelay;
}
Object *UpdateModule::getObject() const { return &g_object; }
void AIGroup::recomputeGroupSpeed() { L(LOG_GROUP_SPEED, {}); }
const AIUpdateModuleData* AIUpdateInterface::getAIUpdateModuleData() const { return g_data; }

/* ---- batch A3's stand-ins */
namespace {
Object *findObject(ObjectID id)
{
	if ((unsigned)id >= 1 && (unsigned)id < 32 && ((g_aiEnv.existMask >> id) & 1)) {
		g_others[id].m_otherId = id;
		return &g_others[id];
	}
	return NULL;
}
}  // namespace
Object *GameLogic::findObjectByID(ObjectID id) const { return findObject(id); }
void AIUpdateInterface::addTargeter(ObjectID id, Bool add)
{
	/* (only another object's AI is asked) */
	L(LOG_ADD_TARGETER, { (unsigned)(this == g_ai_update ? 0 : 1), (unsigned)id, (unsigned)add });
}
const AiData *AI::getAiData()
{
	g_aiData.m_forceIdleFramesCount = (UnsignedInt)g_aiEnv.forceIdleFrames;
	g_aiData.m_attackUsesLineOfSight = g_aiEnv.attackUsesLOS != 0;
	g_aiData.m_attackIgnoreInsignificantBuildings = g_aiEnv.ignoreInsignificant != 0;
	return &g_aiData;
}
Real AI::getAdjustedVisionRangeForObject(const Object *object, Int factorsToConsider)
{
	L(LOG_VISION_RANGE, { (unsigned)object->getID(), (unsigned)factorsToConsider });
	return g_aiEnv.visionRange;
}
Object *AI::findClosestEnemy(const Object *me, Real range, UnsignedInt qualifiers, const AttackPriorityInfo *info, PartitionFilter *optionalFilter)
{
	L(LOG_CLOSEST_ENEMY, { (unsigned)me->getID(), bitsOf(range), qualifiers, info ? 1u : 0u });
	return findObject((ObjectID)g_aiEnv.closestEnemy);
}
const DamageInfo *BodyModuleInterface::getLastDamageInfo() const
{
	g_lastDamage.in.m_damageType = (DamageType)g_aiEnv.lastDamageType;
	g_lastDamage.in.m_sourceID = (ObjectID)g_aiEnv.lastDamageSource;
	return &g_lastDamage;
}
const UpgradeTemplate *UpgradeCenter::findUpgrade(const AsciiString &name)
{
	if (name == "Upgrade_Nationalism") return &g_nationalism;
	if (name == "Upgrade_Fanaticism") return &g_fanaticism;
	return &g_otherUpgrade;
}
PlayerType Player::getPlayerType() const { return (PlayerType)g_aiEnv.playerType; }
Bool Player::hasUpgradeComplete(const UpgradeTemplate *t)
{
	return (t == &g_nationalism && (g_aiEnv.upgrades & 1)) || (t == &g_fanaticism && (g_aiEnv.upgrades & 2));
}
const TeamTemplateInfo *TeamPrototype::getTemplateInfo() const { g_teamInfo.m_attackCommonTarget = g_aiEnv.teamAttackCommon != 0; return &g_teamInfo; }
const TeamPrototype *Team::getPrototype() { return &g_teamPrototype; }
Object *Team::getTeamTargetObject() { return findObject((ObjectID)g_aiEnv.teamTarget); }
Bool ContainModuleInterface::isPassengerAllowedToFire(ObjectID id) const { return g_aiEnv.containerAllowsFire != 0; }
Bool StealthUpdate::isGrantedBySpecialPower() { return g_aiEnv.stealth > 0; }
Int Weapon::getAntiMask() const { return g_aiEnv.weaponAnti; }
Bool HordeUpdateInterface::isInHorde() const { return (g_aiEnv.horde >> (3 + m_index)) & 1; }
Bool HordeUpdateInterface::isAllowedNationalism() const { return (g_aiEnv.horde >> (6 + m_index)) & 1; }
HordeUpdateInterface *BehaviorModule::getHordeUpdateInterface()
{
	if (!((g_aiEnv.horde >> m_index) & 1)) return NULL;
	g_horde[m_index].m_index = m_index;
	return &g_horde[m_index];
}
Bool Object::isEffectivelyDead() const { return g_aiEnv.effectivelyDead != 0; }
Player *Object::getControllingPlayer() const { return g_aiEnv.playerType >= 0 ? &g_player : NULL; }
Team *Object::getTeam() { return &g_team; }
CanAttackResult Object::getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType commandSource) const
{
	unsigned h = p1Hash(9, { (unsigned)t, (unsigned)target->getID(), (unsigned)commandSource });
	CanAttackResult r = p1Pick(h, g_aiEnv.ableRate) ? ATTACKRESULT_POSSIBLE
		: p1Pick(p1Hash(10, { h }), g_aiEnv.ableRate) ? ATTACKRESULT_POSSIBLE_AFTER_MOVING : ATTACKRESULT_NOT_POSSIBLE;
	L(LOG_ABLE_TO_ATTACK, { (unsigned)t, (unsigned)target->getID(), (unsigned)commandSource, (unsigned)r });
	return r;
}
Object *Object::getContainedBy() { return m_otherId ? NULL : findObject((ObjectID)g_aiEnv.containerId); }
ContainModuleInterface *Object::getContain() const { return &g_contain; }
StealthUpdate *Object::getStealth() const { return g_aiEnv.stealth >= 0 ? &g_stealth : NULL; }
Weapon *Object::getCurrentWeapon() { return g_aiEnv.weaponAnti >= 0 ? &g_weapon : NULL; }
const Weapon *Object::getCurrentWeapon() const { return g_aiEnv.weaponAnti >= 0 ? &g_weapon : NULL; }
BehaviorModule **Object::getBehaviorModules()
{
	for (int i = 0; i < 3; ++i) { g_modules[i].m_index = i; g_moduleList[i] = &g_modules[i]; }
	g_moduleList[3] = NULL;
	return g_moduleList;
}
CollideModuleInterface *BehaviorModule::getCollide() { abort(); }
Bool CollideModuleInterface::wouldLikeToCollideWith(const Object *other) const { abort(); }
void Object::setWeaponBonusCondition(WeaponBonusConditionType wst) { L(LOG_SET_BONUS, { (unsigned)wst }); }
void Object::clearWeaponBonusCondition(WeaponBonusConditionType wst) { L(LOG_CLEAR_BONUS, { (unsigned)wst }); }
AIGroup *Object::getGroup() { return g_aiEnv.hasGroup ? &g_group : NULL; }
AIUpdateInterface *Object::getAI()
{
	if (!m_otherId) return g_ai_update;
	return ((g_aiEnv.aiMask >> m_otherId) & 1) ? (AIUpdateInterface *)g_otherAiMem : NULL;
}

/* ---- batch A4's stand-ins */
Real Object::getVisionRange() const { return g_aiEnv.objVisionRange; }
void Object::clearAndSetModelConditionFlags(const ModelConditionFlags &clr, const ModelConditionFlags &set)
{
	/* the cleared conditions, then the set ones, each in increasing order */
	std::vector<unsigned> w;
	for (Int i = 0; i < MODELCONDITION_COUNT; ++i) if (clr.test(i)) w.push_back((unsigned)i);
	w.push_back(0xffffffffu);
	for (Int i = 0; i < MODELCONDITION_COUNT; ++i) if (set.test(i)) w.push_back((unsigned)i);
	if (g_aiQuiet) return;
	g_aiLog->push_back((unsigned)LOG_CLEAR_SET_MODEL);
	g_aiLog->push_back((unsigned)w.size());
	for (unsigned x : w) g_aiLog->push_back(x);
}
void ScriptEngine::notifyOfObjectCreationOrDestruction() { L(LOG_SCRIPT_NOTIFY, {}); }
Object *AI::findClosestRepulsor(const Object *me, Real range)
{
	L(LOG_REPULSOR, { (unsigned)me->getID(), bitsOf(range) });
	return findObject((ObjectID)g_aiEnv.repulsor);
}
Bool Pathfinder::goalPosition(Object *obj, Coord3D *pos)
{
	L(LOG_PF_GOAL_POSITION, { (unsigned)g_aiEnv.pfHasGoal });
	if (!g_aiEnv.pfHasGoal) return FALSE;
	pos->x = g_aiEnv.pfGoal[0];
	pos->y = g_aiEnv.pfGoal[1];
	pos->z = g_aiEnv.pfGoal[2];
	return TRUE;
}
/* the cell's centre (the stand-in's grid) */
void Pathfinder::snapPosition(Object *obj, Coord3D *pos)
{
	L(LOG_PF_SNAP, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	pos->x = floorf(pos->x / PATHFIND_CELL_SIZE_F) * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F / 2;
	pos->y = floorf(pos->y / PATHFIND_CELL_SIZE_F) * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F / 2;
}
void Pathfinder::updateGoal(Object *obj, const Coord3D *newGoalPos, PathfindLayerEnum layer)
{
	L(LOG_PF_UPDATE_GOAL, { bitsOf(newGoalPos->x), bitsOf(newGoalPos->y), bitsOf(newGoalPos->z), (unsigned)layer });
}
Bool Pathfinder::queueForPath(ObjectID id) { L(LOG_PF_QUEUE, { (unsigned)id }); return TRUE; }
UpdateSleepTime TurretAI::updateTurretAI()
{
	unsigned h = p1Hash(12, { TheGameLogic->getFrame(), (unsigned)m_which });
	UpdateSleepTime r = p1Pick(h, g_aiEnv.turretRate) ? UPDATE_SLEEP_NONE
		: ((h >> 16) & 1) ? UPDATE_SLEEP(1 + (h >> 20) % 60) : UPDATE_SLEEP_FOREVER;
	L(LOG_TURRET_UPDATE, { (unsigned)m_which, (unsigned)r });
	return r;
}
void AIUpdateInterface::aiMoveToObject(Object *obj, CommandSourceType cmdSource)
{
	L(LOG_AI_COMMAND, { (unsigned)AICMD_MOVE_TO_OBJECT, (unsigned)cmdSource, obj ? (unsigned)obj->getID() : 0u, 0u });
}
void AIUpdateInterface::aiAttackObject(Object *victim, Int maxShotsToFire, CommandSourceType cmdSource)
{
	L(LOG_AI_COMMAND, { (unsigned)AICMD_ATTACK_OBJECT, (unsigned)cmdSource, victim ? (unsigned)victim->getID() : 0u, (unsigned)maxShotsToFire });
}
namespace {
/* a placeholder state's answer: the hash of the frame, its id and the callback (0 onEnter, 1 update) */
StateReturnType placeholderAnswer(StateID id, unsigned which)
{
	unsigned h = p1Hash(11, { TheGameLogic->getFrame(), (unsigned)id, which });
	if (p1Pick(h, g_aiEnv.stateRate)) return STATE_CONTINUE;
	switch ((h >> 16) % 4u) {
	case 0: return STATE_SUCCESS;
	case 1: return STATE_FAILURE;
	case 2: return STATE_SLEEP(1 + (h >> 20) % 40);
	default: return STATE_SLEEP_FOREVER;
	}
}
}  // namespace
StateReturnType AIPlaceholderState::onEnter()
{
	StateReturnType r = placeholderAnswer(m_id, 0);
	L(LOG_STATE_ENTER, { (unsigned)m_id, (unsigned)(Int)r });
	return r;
}
StateReturnType AIPlaceholderState::update()
{
	StateReturnType r = placeholderAnswer(m_id, 1);
	L(LOG_STATE_UPDATE, { (unsigned)m_id, (unsigned)(Int)r });
	return r;
}
void AIPlaceholderState::onExit(StateExitType status) { L(LOG_STATE_EXIT, { (unsigned)m_id, (unsigned)status }); }

/* ---- the driver -------------------------------------------------------------------------------------------------------------- */

namespace {

/* the scene, and the global data the locomotors read from it */
void setScene(const AiEnv *env)
{
	g_aiEnv = *env;
	TheGlobalData->m_gravity = env->gravity;
	TheGlobalData->m_movementPenaltyDamageState = env->penalty;
}

void putU32(std::vector<unsigned> &v, unsigned x) { v.push_back(x); }
void putF(std::vector<unsigned> &v, Real r) { v.push_back(bitsOf(r)); }
void putPos(std::vector<unsigned> &v, const Coord3D &p) { putF(v, p.x); putF(v, p.y); putF(v, p.z); }

int nodeIndex(const Path *p, const PathNode *n)
{
	int i = 0;
	for (const PathNode *x = p->m_path; x; x = x->m_next, ++i) if (x == n) return i;
	return n ? 1000 : -1;
}

/* a path, as batch P1's driver writes it (putPath) */
void putPath(std::vector<unsigned> &v, const Path *p)
{
	unsigned count = 0;
	for (const PathNode *n = p->m_path; n; n = n->m_next) ++count;
	putU32(v, count);
	for (const PathNode *n = p->m_path; n; n = n->m_next) {
		putPos(v, n->m_pos);
		putU32(v, (unsigned)n->m_layer);
		putU32(v, n->m_canOptimize ? 1u : 0u);
		putU32(v, (unsigned)n->m_id);
		putU32(v, (unsigned)nodeIndex(p, n->m_nextOpti));
		putF(v, n->m_nextOptiDist2D);
		putF(v, n->m_nextOptiDirNorm2D.x); putF(v, n->m_nextOptiDirNorm2D.y);
		putU32(v, (unsigned)nodeIndex(p, n->m_prev));
	}
	putU32(v, (unsigned)nodeIndex(p, p->m_pathTail));
	putU32(v, p->m_isOptimized ? 1u : 0u);
	putU32(v, p->m_blockedByAlly ? 1u : 0u);
	putU32(v, p->m_cpopValid ? 1u : 0u);
	putU32(v, (unsigned)p->m_cpopCountdown);
	putPos(v, p->m_cpopIn);
	putF(v, p->m_cpopOut.distAlongPath);
	putPos(v, p->m_cpopOut.posOnPath);
	putU32(v, (unsigned)p->m_cpopOut.layer);
	putU32(v, (unsigned)nodeIndex(p, p->m_cpopRecentStart));
}

/* a locomotor: its template's key, then its data members in Locomotor::xfer's order (as ref_loco_state) */
void putLoco(std::vector<unsigned> &v, const Locomotor *l)
{
	putU32(v, (unsigned)NAMEKEY(l->getTemplateName()));
	putU32(v, (unsigned)l->m_donutTimer);
	putPos(v, l->m_maintainPos);
	putF(v, l->m_brakingFactor);
	putF(v, l->m_maxLift);
	putF(v, l->m_maxSpeed);
	putF(v, l->m_maxAccel);
	putF(v, l->m_maxBraking);
	putF(v, l->m_maxTurnRate);
	putF(v, l->m_closeEnoughDist);
	putU32(v, (unsigned)l->m_flags);
	putF(v, l->m_preferredHeight);
	putF(v, l->m_preferredHeightDamping);
	putF(v, l->m_angleOffset);
	putF(v, l->m_offsetIncrement);
}

/* the map's cells: a cliff at the cliff rate, else one of the other types; pinched at the pinch rate */
void makeMap()
{
	Pathfinder *pf = TheAI->pathfinder();
	memset(g_pathfinderMem, 0, sizeof g_pathfinderMem);
	pf->m_extent.lo.x = 0; pf->m_extent.lo.y = 0;
	pf->m_extent.hi.x = MAP_W - 1; pf->m_extent.hi.y = MAP_H - 1;
	memset(g_cellMem, 0, sizeof g_cellMem);
	PathfindCell *cells = (PathfindCell *)g_cellMem;
	static const PathfindCell::CellType others[6] = { PathfindCell::CELL_CLEAR, PathfindCell::CELL_CLEAR, PathfindCell::CELL_WATER,
		PathfindCell::CELL_RUBBLE, PathfindCell::CELL_OBSTACLE, PathfindCell::CELL_IMPASSABLE };
	for (int x = 0; x < MAP_W; ++x) {
		g_columns[x] = cells + x * MAP_H;
		for (int y = 0; y < MAP_H; ++y) {
			unsigned h = p1Hash(5, { (unsigned)x, (unsigned)y });
			g_columns[x][y].m_type = p1Pick(h, g_aiEnv.cliffRate) ? PathfindCell::CELL_CLIFF : others[(p1Hash(8, { h }) >> 8) % 6u];
			g_columns[x][y].m_pinched = p1Pick(p1Hash(6, { h }), g_aiEnv.pinchRate);
		}
	}
	pf->m_map = g_columns;
}

}  // namespace

extern "C" {

/* A new NameKeyGenerator (installed as TheNameKeyGenerator, reset: keys start at 1 again), an empty TheLocomotorStore, no AI. */
void ref_ai_a2_reset(void)
{
	static NameKeyGenerator *gen = 0;
	if (!gen) gen = new NameKeyGenerator;
	gen->reset();
	TheNameKeyGenerator = gen;
	g_ai_update = 0;	/* (leaked: its locomotors point into the store deleted below) */
	delete TheLocomotorStore;
	TheLocomotorStore = new LocomotorStore;
	static std::vector<unsigned> log;
	g_aiLog = &log;
	log.clear();
}

/* batch A4: the file the machine's xfer goes through (one per test thread) */
void ref_ai_a4_xfer_path(const char *path) { g_xferPath = path; g_saved.clear(); }

/* INI::load's loop over `filename` (load type `loadType`), each Locomotor block to this build's handler (see the header); 0, or 1
** if it threw */
int ref_ai_a2_load(const char *filename, int loadType)
{
	INI ini;
	try {
		ini.prepFile(AsciiString(filename), (INILoadType)loadType);
		try {
			while (ini.m_endOfFile == FALSE) {
				ini.readLine();
				const char *token = strtok(ini.m_buffer, ini.m_seps);
				if (token) {
					if (strcmp(token, "Locomotor") == 0) INI::aiParseLocomotorTemplateDefinition(&ini);
					else throw INI_UNKNOWN_TOKEN;
				}
			}
		} catch (...) {
			ini.unPrepFile();
			throw;
		}
		ini.unPrepFile();
	} catch (...) {
		return 1;
	}
	return 0;
}

/* A new AI: its data's locomotor sets from `sets` (count, then per set: its type, its number of templates, their names' keys,
** then (batch A3) the data's auto-acquire bits and mood check rate),
** the scene `env`, the object at the scene's start (identity, at `pos` facing `angle`), the AI made by its constructor
** (AIUpdateInterface(NULL, NULL)); the log of the construction is returned (length in words, may exceed cap). */
int ref_ai_a2_new(const unsigned *sets, const AiEnv *env, const float *pos, float angle, unsigned *log, int cap)
{
	setScene(env);
	g_aiLog->clear();
	makeMap();
	g_nextCallFrame = 0;
	g_data = new AIUpdateModuleData;
	unsigned k = 0, count = sets[k++];
	for (unsigned s = 0; s < count; ++s) {
		LocomotorSetType t = (LocomotorSetType)sets[k++];
		unsigned n = sets[k++];
		LocomotorTemplateVector &vec = g_data->m_locomotorTemplates[t];
		for (unsigned i = 0; i < n; ++i) {
			const LocomotorTemplate *lt = TheLocomotorStore->findLocomotorTemplate((NameKeyType)sets[k++]);
			vec.push_back(lt);
		}
	}
	/* the data's turrets (the scene's bits; TurretAI is an empty stand-in) */
	for (int i = 0; i < MAX_TURRETS; ++i) g_data->m_turretData[i] = ((g_aiEnv.turrets >> i) & 1) ? (const TurretAIData *)&g_aiEnv : NULL;
	g_data->m_autoAcquireEnemiesWhenIdle = sets[k++];
	g_data->m_moodAttackCheckRate = sets[k++];
	g_object = Object();
	++g_aiQuiet;
	Coord3D p = { pos[0], pos[1], pos[2] };
	g_object.setPosition(&p);
	g_object.setOrientation(angle);
	--g_aiQuiet;
	g_ai_update = new (ShimZeroFill{}) AIUpdateInterface(NULL, NULL);
	/* batch A4: the object's creation makes the AI's machine (onObjectCreated), unless the scene says not yet */
	if (g_aiEnv.hasMachine) g_ai_update->onObjectCreated();
	int n = (int)g_aiLog->size();
	if (n <= cap && n > 0) memcpy(log, &(*g_aiLog)[0], 4 * n);
	return n;
}

/* One operation on the AI, in the scene `env` (fa: floats, ia: ints): its results into `out` (length returned), its log into `log`
** (length in *logLen, may exceed cap). The operations are the Rust test's `Op` (the numbers below). */
int ref_ai_a2_op(int op, const float *fa, const int *ia, const AiEnv *env, unsigned *out, unsigned *log, int cap, int *logLen)
{
	setScene(env);
	g_aiLog->clear();
	AIUpdateInterface *ai = g_ai_update;
	std::vector<unsigned> r;
	Coord3D p = { fa[0], fa[1], fa[2] };
	switch (op) {
	case 0: r.push_back((unsigned)ai->doLocomotor()); break;
	case 1: r.push_back(ai->chooseLocomotorSet((LocomotorSetType)ia[0]) ? 1u : 0u); break;
	case 2: ai->setLocomotorUpgrade(ia[0] != 0); break;
	case 3: ai->setLocomotorGoalPositionOnPath(); break;
	case 4: ai->setLocomotorGoalPositionExplicit(p); break;
	case 5: ai->setLocomotorGoalOrientation(fa[0]); break;
	case 6: ai->setLocomotorGoalNone(); break;
	case 7: putF(r, ai->getLocomotorDistanceToGoal()); break;
	case 8: r.push_back(ai->isDoingGroundMovement() ? 1u : 0u); break;
	case 9: r.push_back(ai->isAircraftThatAdjustsDestination() ? 1u : 0u); break;
	case 10: r.push_back(ai->getTreatAsAircraftForLocoDistToGoal() ? 1u : 0u); break;
	case 11: r.push_back(ai->isValidLocomotorPosition(&p) ? 1u : 0u); break;
	case 12: ai->setQueueForPathTime(ia[0]); break;
	case 13: ai->wakeUpNow(); break;
	case 14: r.push_back(ai->queueWaypoint(&p) ? 1u : 0u); break;
	case 15: ai->executeWaypointQueue(); break;
	case 16: ai->clearWaypointQueue(); break;
	case 17: ai->destroyPath(); break;
	case 18: ai->friend_startingMove(); break;
	case 19: ai->friend_endingMove(); break;
	case 20: {
		/* a new path: ia[0] nodes at fa[3k..3k+2] on layer ia[1 + k] (appended), optimized (ia[0] + 1: 1 optimize, 2 ground) */
		Path *path = newInstance(Path);
		int n = ia[0];
		for (int i = 0; i < n; ++i) {
			Coord3D q = { fa[3 * i], fa[3 * i + 1], fa[3 * i + 2] };
			path->appendNode(&q, (PathfindLayerEnum)ia[1 + i]);
		}
		int how = ia[1 + n];
		if (how == 1) path->optimize(&g_object, (LocomotorSurfaceTypeMask)ia[2 + n], ia[3 + n] != 0);
		else if (how == 2) path->optimizeGroundPath(ia[2 + n] != 0, ia[3 + n]);
		ai->friend_setPath(path);
		break;
	}
	case 21: putF(r, ai->getCurLocomotorSpeed()); break;
	case 22: r.push_back(ai->hasLocomotorForSurface((LocomotorSurfaceType)ia[0]) ? 1u : 0u); break;
	case 23: ai->markAsDead(); break;
	case 24: ai->setAttitude((AttitudeType)ia[0]); break;
	case 25: ai->ignoreObstacleID((ObjectID)ia[0]); break;
	case 26: ai->setLastCommandSource((CommandSourceType)ia[0]); break;
	case 27:
		/* the state the AI's machine and collisions set (not yet translated): final position (and its flag), blocked, the
		** blocked speed, the desired speed, in-update, waiting for a path, the path's extra distance, the bump limit */
		ai->m_finalPosition = p;
		ai->m_doFinalPosition = ia[0] != 0;
		ai->m_isBlocked = ia[1] != 0;
		ai->m_curMaxBlockedSpeed = fa[3];
		ai->m_desiredSpeed = fa[4];
		ai->m_isInUpdate = ia[2] != 0;
		ai->m_waitingForPath = ia[3] != 0;
		ai->m_pathExtraDistance = fa[5];
		ai->m_bumpSpeedLimit = fa[6];
		break;
	case 28: ai->setFinalPosition(&p); break;
	case 29: ai->chooseGoodLocomotorFromCurrentSet(); break;
	case 30: ai->friend_notifyStateMachineChanged(); break;
	case 31: g_object.setPosition(&p); g_object.setOrientation(fa[3]); break;
	/* the sleep the AI's update returned (the update loop's, not yet translated): its next call, ia[0] frames from now */
	case 32: g_nextCallFrame = TheGameLogic->getFrame() + (UnsignedInt)ia[0]; break;
	/* batch A3 (an object argument is ia[0]'s object, NULL if the scene has none) */
	case 33: { Object *o = ai->checkForCrateToPickup(); r.push_back(o ? (unsigned)o->getID() : 0u); break; }
	case 34: ai->setCurrentVictim(findObject((ObjectID)ia[0])); break;
	case 35: { Object *o = ai->getCurrentVictim(); r.push_back(o ? (unsigned)o->getID() : 0u); break; }
	case 36: { const Coord3D *q = ai->getCurrentVictimPos(); r.push_back(q ? 1u : 0u); if (q) putPos(r, *q); break; }
	case 37: r.push_back((unsigned)ai->getAttitude()); break;
	case 38: r.push_back((unsigned)ai->getAIStateType()); break;
	case 39: ai->ignoreObstacle(findObject((ObjectID)ia[0])); break;
	case 40: r.push_back((unsigned)ai->getIgnoredObstacleID()); break;
	case 41: { Object *o = ai->getEnterTarget(); r.push_back(o ? (unsigned)o->getID() : 0u); break; }
	case 42: r.push_back(ai->getMoodMatrixValue()); break;
	case 43: r.push_back(ai->getMoodMatrixActionAdjustment((MoodMatrixAction)ia[0])); break;
	case 44: ai->wakeUpAndAttemptToTarget(); break;
	case 45: ai->resetNextMoodCheckTime(); break;
	case 46: ai->setNextMoodCheckTime((UnsignedInt)ia[0]); break;
	case 47: r.push_back(ai->canAutoAcquireWhileStealthed() ? 1u : 0u); break;
	case 48: { Object *o = ai->getNextMoodTarget(ia[0] != 0, ia[1] != 0); r.push_back(o ? (unsigned)o->getID() : 0u); break; }
	case 49: ai->evaluateMoraleBonus(); break;
	case 50: r.push_back(ai->isIdle() ? 1u : 0u); break;
	case 51: r.push_back(ai->isAttacking() ? 1u : 0u); break;
	case 52: r.push_back(ai->isBusy() ? 1u : 0u); break;
	case 53: r.push_back(ai->isClearingMines() ? 1u : 0u); break;
	case 54: { Object *o = findObject((ObjectID)ia[0]); if (o) r.push_back(ai->isMovingAwayFrom(o) ? 1u : 0u); break; }
	case 55: r.push_back(ai->isMoving() ? 1u : 0u); break;
	case 56: ai->notifyCrate((ObjectID)ia[0]); break;
	/* the object's status bit ia[0] set to ia[1] (the object's other modules', quietly) */
	case 57: if (ia[1]) g_object.m_status |= 1ULL << ia[0]; else g_object.m_status &= ~(1ULL << ia[0]); break;
	/* the ids privateMoveAwayFromUnit sets (not yet translated) */
	case 58: ai->m_moveOutOfWay1 = (ObjectID)ia[0]; ai->m_moveOutOfWay2 = (ObjectID)ia[1]; break;
	/* batch A4: the machine (AIStateMachine's overrides, StateMachine's members), the AI's update and creation */
	case 59: r.push_back((unsigned)(Int)ai->getStateMachine()->setState((StateID)ia[0])); break;
	case 60: r.push_back((unsigned)(Int)ai->getStateMachine()->setTemporaryState((StateID)ia[0], ia[1])); break;
	case 61: ai->getStateMachine()->clear(); break;
	case 62: r.push_back((unsigned)(Int)ai->getStateMachine()->resetToDefaultState()); break;
	case 63: ai->getStateMachine()->setGoalObject(findObject((ObjectID)ia[0])); break;
	case 64: ai->getStateMachine()->setGoalPosition(&p); break;
	case 65: r.push_back((unsigned)ai->update()); break;
	case 66: ai->onObjectCreated(); break;
	case 67: if (ia[0]) ai->getStateMachine()->lock("op"); else ai->getStateMachine()->unlock(); break;
	case 68: ai->getStateMachine()->addToGoalPath(&p); break;
	case 69: { const Coord3D *q = ai->getStateMachine()->getGoalPathPosition(ia[0]); r.push_back(q ? 1u : 0u); if (q) putPos(r, *q); break; }
	case 70: {
		/* the machine saved (xferSnapshot): the status, then the bytes */
		XferSave x;
		int st = 0;
		try { x.open(AsciiString(g_xferPath.c_str())); x.xferSnapshot(ai->getStateMachine()); } catch (...) { st = 1; }
		try { x.close(); } catch (...) {}
		g_saved.clear();
		if (FILE *f = fopen(g_xferPath.c_str(), "rb")) {
			unsigned char b[65536];
			size_t m = fread(b, 1, sizeof b, f);
			fclose(f);
			g_saved.assign(b, b + m);
		}
		r.push_back((unsigned)st);
		r.push_back((unsigned)g_saved.size());
		for (unsigned char c : g_saved) r.push_back(c);
		break;
	}
	case 71: {
		/* the last save loaded into the machine (xferSnapshot, then loadPostProcess): the status */
		if (FILE *f = fopen(g_xferPath.c_str(), "wb")) { if (!g_saved.empty()) fwrite(&g_saved[0], 1, g_saved.size(), f); fclose(f); }
		XferLoad x;
		int st = 0;
		try { x.open(AsciiString(g_xferPath.c_str())); x.xferSnapshot(ai->getStateMachine()); } catch (...) { st = 1; }
		try { x.close(); } catch (...) {}
		ai->getStateMachine()->loadPostProcess();
		r.push_back((unsigned)st);
		break;
	}
	}
	for (size_t i = 0; i < r.size(); ++i) out[i] = r[i];
	int n = (int)g_aiLog->size();
	*logLen = n;
	if (n <= cap && n > 0) memcpy(log, &(*g_aiLog)[0], 4 * n);
	return (int)r.size();
}

/* The AI and its object (the Rust test's `dump`): every data member of AIUpdateInterface that batch A2 sets, in the header's order;
** the locomotor set (its locomotors, the current one's index, the valid surfaces, downhill); the path; the object (transform,
** cached position and angle, status bits); the wake frame. Length returned (may exceed cap). */
int ref_ai_a2_dump(unsigned *buf, int cap)
{
	const AIUpdateInterface *ai = g_ai_update;
	std::vector<unsigned> v;
	putU32(v, ai->m_priorWaypointID);
	putU32(v, ai->m_currentWaypointID);
	putU32(v, ai->m_nextEnemyScanTime);
	putU32(v, (unsigned)ai->m_currentVictimID);
	putF(v, ai->m_desiredSpeed);
	putU32(v, (unsigned)ai->m_lastCommandSource);
	putU32(v, (unsigned)ai->m_guardMode);
	putU32(v, (unsigned)ai->m_guardTargetType[0]);
	putU32(v, (unsigned)ai->m_guardTargetType[1]);
	putPos(v, ai->m_locationToGuard);
	putU32(v, (unsigned)ai->m_objectToGuard);
	for (int i = 0; i < AIUpdateInterface::MAX_WAYPOINTS; ++i) putPos(v, ai->m_waypointQueue[i]);
	putU32(v, (unsigned)ai->m_waypointCount);
	putU32(v, (unsigned)ai->m_waypointIndex);
	putU32(v, (unsigned)ai->m_requestedVictimID);
	putPos(v, ai->m_requestedDestination);
	putPos(v, ai->m_requestedDestination2);
	putU32(v, ai->m_pathTimestamp);
	putU32(v, (unsigned)ai->m_ignoreObstacleID);
	putF(v, ai->m_pathExtraDistance);
	putU32(v, (unsigned)ai->m_pathfindGoalCell.x); putU32(v, (unsigned)ai->m_pathfindGoalCell.y);
	putU32(v, (unsigned)ai->m_pathfindCurCell.x); putU32(v, (unsigned)ai->m_pathfindCurCell.y);
	putU32(v, (unsigned)ai->m_blockedFrames);
	putF(v, ai->m_curMaxBlockedSpeed);
	putF(v, ai->m_bumpSpeedLimit);
	putU32(v, ai->m_ignoreCollisionsUntil);
	putU32(v, ai->m_queueForPathFrame);
	putPos(v, ai->m_finalPosition);
	putU32(v, (unsigned)ai->m_repulsor1);
	putU32(v, (unsigned)ai->m_repulsor2);
	putU32(v, (unsigned)ai->m_nextGoalPathIndex);
	putU32(v, (unsigned)ai->m_moveOutOfWay1);
	putU32(v, (unsigned)ai->m_moveOutOfWay2);
	/* the locomotors */
	const LocomotorSet &set = ai->m_locomotorSet;
	putU32(v, (unsigned)set.m_locomotors.size());
	int cur = -1;
	for (size_t i = 0; i < set.m_locomotors.size(); ++i) {
		putLoco(v, set.m_locomotors[i]);
		if (set.m_locomotors[i] == ai->m_curLocomotor) cur = (int)i;
	}
	putU32(v, (unsigned)cur);
	putU32(v, (unsigned)set.m_validLocomotorSurfaces);
	putU32(v, set.m_downhillOnly ? 1u : 0u);
	putU32(v, (unsigned)ai->m_curLocomotorSet);
	putU32(v, (unsigned)ai->m_locomotorGoalType);
	putPos(v, ai->m_locomotorGoalData);
	putU32(v, (unsigned)ai->m_turretSyncFlag);
	putU32(v, (unsigned)ai->m_attitude);
	putU32(v, ai->m_nextMoodCheckTime);
	putU32(v, (unsigned)ai->m_crateCreated);
	putU32(v, (unsigned)ai->m_tmpInt);
	const Bool flags[] = { ai->m_doFinalPosition, ai->m_waitingForPath, ai->m_isAttackPath, ai->m_isFinalGoal, ai->m_isApproachPath,
		ai->m_isSafePath, ai->m_movementComplete, ai->m_isMoving, ai->m_isBlocked, ai->m_isBlockedAndStuck, ai->m_upgradedLocomotors,
		ai->m_canPathThroughUnits, ai->m_randomlyOffsetMoodCheck, ai->m_isAiDead, ai->m_isRecruitable, ai->m_executingWaypointQueue,
		ai->m_retryPath, ai->m_allowedToChase, ai->m_isInUpdate, ai->m_fixLocoInPostProcess };
	for (Bool f : flags) putU32(v, f ? 1u : 0u);
	/* the path */
	putU32(v, ai->m_path ? 1u : 0u);
	if (ai->m_path) putPath(v, ai->m_path);
	/* the object */
	for (int i = 0; i < 3; ++i)
		for (int j = 0; j < 4; ++j) putF(v, g_object.m_transform[i][j]);
	putPos(v, g_object.m_cachedPos);
	putF(v, g_object.m_cachedAngle);
	putU32(v, (unsigned)(g_object.m_status & 0xffffffffu));
	putU32(v, (unsigned)(g_object.m_status >> 32));
	putU32(v, g_nextCallFrame);
	/* batch A4: the machine (StateMachine's members, AIStateMachine's, the idle state's) */
	const AIStateMachine *sm = ai->m_stateMachine;
	putU32(v, sm ? 1u : 0u);
	if (sm) {
		putU32(v, sm->m_currentState ? (unsigned)sm->m_currentState->getID() : (unsigned)INVALID_STATE_ID);
		putU32(v, sm->m_sleepTill);
		putU32(v, (unsigned)sm->m_defaultStateID);
		putU32(v, (unsigned)sm->m_goalObjectID);
		putPos(v, sm->m_goalPosition);
		putU32(v, sm->m_locked ? 1u : 0u);
		putU32(v, sm->m_defaultStateInited ? 1u : 0u);
		putU32(v, (unsigned)sm->getTemporaryState());
		putU32(v, sm->m_temporaryStateFramEnd);
		putU32(v, (unsigned)sm->m_goalPath.size());
		for (size_t i = 0; i < sm->m_goalPath.size(); ++i) putPos(v, sm->m_goalPath[i]);
		const AIIdleState *idle = (const AIIdleState *)sm->m_stateMap.find(AI_IDLE)->second;
		putU32(v, (unsigned)idle->m_initialSleepOffset);
		putU32(v, idle->m_shouldLookForTargets ? 1u : 0u);
		putU32(v, idle->m_inited ? 1u : 0u);
	}
	int n = (int)v.size();
	if (n <= cap && n > 0) memcpy(buf, &v[0], 4 * n);
	return n;
}

}  /* extern "C" */
