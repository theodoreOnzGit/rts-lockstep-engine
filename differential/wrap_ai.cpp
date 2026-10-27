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
** Entry points: ref_ai_a2_reset, ref_ai_a2_load, ref_ai_a2_new, ref_ai_a2_op, ref_ai_a2_dump (see each). */
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

/* ---- the AI ------------------------------------------------------------------------------------------------------------------ */

class Thing;
class ModuleData;
class PolygonTrigger;
class AttackPriorityInfo;
class Waypoint;
class TurretAIData;
/* TurretAI (batch T1, its own build): the AI makes one per turret its data has; the scenes of this build have none */
class TurretAI
{
public:
	TurretAI(Object *owner, const TurretAIData *data, WhichTurretType tur) { abort(); }
};

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
	inline const AIStateMachine* getStateMachine() const { return m_stateMachine; }
	inline const Coord3D* getGoalPosition() const { return getStateMachine()->getGoalPosition(); }	///< return the id of the current state of the machine
	void setFinalPosition(const Coord3D *pos) { m_finalPosition = *pos; m_doFinalPosition = false;}
	/* AIUpdate.cpp's getGroup (the AI's group): the scene's */
	AIGroup *getGroup(void);

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

#include "ai_update_members.inc"
};

#include "ai_update_a2.inc"

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

/* batch P1's hash: the seed, a tag, the words (tests/differential_ai_update_a2.rs has the same function) */
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
AIStateMachine g_machine;
Coord3D g_goal;
UnsignedInt g_nextCallFrame = 0;		/* the AI module's sleepy update: its next call frame */
alignas(16) unsigned char g_pathfinderMem[sizeof(Pathfinder)];

/* the map: W x H cells from (0, 0) (Pathfinder::m_map, m_extent); each cell's type and pinch from the hash of its coordinates */
enum { MAP_W = 24, MAP_H = 16 };
alignas(16) unsigned char g_cellMem[MAP_W * MAP_H * sizeof(PathfindCell)];
PathfindCell *g_columns[MAP_W];

AIUpdateModuleData *g_data = 0;
AIUpdateInterface *g_ai_update = 0;

}  // namespace

GlobalData *TheGlobalData = &g_globalData;
GameLogic *TheGameLogic = &g_gameLogic;
TerrainLogic *TheTerrainLogic = &g_terrain;
PartitionManager *ThePartitionManager = &g_partition;
AI *TheAI = &g_ai;

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
Real GeometryInfo::getBoundingCircleRadius() const { return g_aiEnv.boundingCircleRadius; }

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
	return FALSE;
}
void Object::setModelConditionState(ModelConditionFlagType a) { L(LOG_SET_MODEL_COND, { (unsigned)a }); }
void Object::clearModelConditionState(ModelConditionFlagType a) { L(LOG_CLEAR_MODEL_COND, { (unsigned)a }); }
ObjectID Object::getID() const { return (ObjectID)g_aiEnv.objectId; }
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
const Coord3D *AIStateMachine::getGoalPosition() const
{
	g_goal.x = g_aiEnv.goal[0];
	g_goal.y = g_aiEnv.goal[1];
	g_goal.z = g_aiEnv.goal[2];
	return &g_goal;
}
const AIUpdateModuleData* AIUpdateInterface::getAIUpdateModuleData() const { return g_data; }
AIGroup *AIUpdateInterface::getGroup(void) { return g_aiEnv.hasGroup ? &g_group : NULL; }

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

/* A new AI: its data's locomotor sets from `sets` (count, then per set: its type, its number of templates, their names' keys),
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
	for (int i = 0; i < MAX_TURRETS; ++i) g_data->m_turretData[i] = NULL;
	g_object = Object();
	++g_aiQuiet;
	Coord3D p = { pos[0], pos[1], pos[2] };
	g_object.setPosition(&p);
	g_object.setOrientation(angle);
	--g_aiQuiet;
	g_ai_update = new (ShimZeroFill{}) AIUpdateInterface(NULL, NULL);
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
	int n = (int)v.size();
	if (n <= cap && n > 0) memcpy(buf, &v[0], 4 * n);
	return n;
}

}  /* extern "C" */
