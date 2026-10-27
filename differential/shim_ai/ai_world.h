/* Stand-ins for everything the AI build reaches outside the upstream text it compiles (AI build only; differential tests,
** p1w13-sonnet batch A2; build.rs, differential/wrap_ai.cpp). Every file of this directory under GameLogic/ includes this one, so a
** `#include "GameLogic/Object.h"` in the unchanged upstream text finds these classes. The names are renamed by macro in PreRTS.h;
** here they are written with their upstream names.
**
** A copy of differential/shim_locomotor/locomotor_world.h (the object a locomotor drives, its physics behaviour and body, the
** terrain, the partition manager, the global data and the game logic, answering from the scene AiEnv and logging every call), with
** what the paths (batch P1's cut of AIPathfind.cpp) and the AI's members (AIUpdate.cpp) add:
**  - Object: its id, crusher level, more kinds and disabled types, effectively-dead, clearStatus, the physics' allow-to-fall;
**  - TerrainLogic::getLayerForDestination (the paths), answered from the hash of batch P1;
**  - PartitionManager::getDistanceSquared (FROM_CENTER_3D);
**  - the AI's own neighbours: its sleepy update module (UpdateModule: the wake frame), its group (AIGroup), its state machine
**    (AIStateMachine: the goal position), declared here and defined by the wrapper.
** The pathfinder is the REAL class of AIPathfind.h (declared there); the wrapper defines the members the paths and the AI call. */
#ifndef SHIM_AI_WORLD_H
#define SHIM_AI_WORLD_H

#include "Lib/BaseType.h"
#include "Common/GameType.h"
#include "Common/GameCommon.h"
#include "matrix3d.h"
#include "vector3.h"
#include "locomotor_gen_defs.h"
class LocomotorTemplate;
/* AIUpdate.h's, AI.h's, UpdateModule.h's and PartitionManager.h's enums (ai_gen, verbatim) */
#include "ai_defs.h"

class Locomotor;

/* The scene of one call. The Rust test has the same struct (tests/differential_ai_update_a2.rs, `Env`): same field order, 4-byte
** fields only. The first block is the Locomotor build's LocoEnv, field for field. */
struct AiEnv
{
	UnsignedInt frame;
	Real gravity;
	Int penalty;			/* TheGlobalData->m_movementPenaltyDamageState */
	Int bodyState;			/* the body module's damage state */
	Int hasPhysics, stunned, motive;
	Real vel[3];
	Real velMag, fwd2d, fwd3d, mass;
	Int layer;
	Int kindMask;			/* bit 0 KINDOF_PROJECTILE, bit 1 KINDOF_DOZER, bit 2 KINDOF_IMMOBILE, bit 3 KINDOF_PRODUCED_AT_HELIPAD;
					** (batch A3) bit 4 KINDOF_INFANTRY, bit 5 KINDOF_IGNORED_IN_GUI, bit 6 KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT */
	Real deckHeight, majorRadius, boundingCircleRadius;
	Int disabledHeld;
	/* terrain: ground(x, y) = ga*x + gb*y + gc; water over every point with x < wx at height wz;
	** a layer above the ground is layerOffset higher; ground-or-structure is structOffset higher */
	Real ga, gb, gc, wx, wz, layerOffset, structOffset;
	Int highestLayer;
	/* the pathfinder's validMovementTerrain: a point is invalid inside the box [x0, x1] x [y0, y1] when enabled */
	Int invEnabled;
	Real invBox[4];
	/* ---- batch A2 */
	Int objectId;
	Int crusherLevel;
	Int disabledUnmanned;
	Int allowToFall;
	Int hasGroup;
	/* the hash of batch P1 (the paths' line tests, cells and layers; the pathfinder's validMovementPosition) and its rates */
	UnsignedInt seed, lineRate, groundRate, cliffRate, pinchRate, validPosRate;
	/* the state machine's goal position */
	Real goal[3];
	/* ---- batch A3 */
	Int effectivelyDead;
	Int playerType;			/* the controlling player's type; -1: none */
	Int teamAttackCommon;		/* the team's template's m_attackCommonTarget */
	Int teamTarget;			/* the team's target object (0: none) */
	UnsignedInt ableRate;		/* getAbleToAttackSpecificObject: possible at this rate (the hash), else after moving at it, else not */
	Int stealth;			/* -1: no stealth module; else isGrantedBySpecialPower */
	Int containerId;		/* the object containing this one (0: none) */
	Int containerAllowsFire;
	Real containerRadius;		/* the container's bounding circle radius */
	Int lastDamageType, lastDamageSource;
	Int forceIdleFrames, attackUsesLOS, ignoreInsignificant;	/* TheAI->getAiData()'s */
	Real visionRange;		/* TheAI->getAdjustedVisionRangeForObject's answer */
	Int closestEnemy;		/* TheAI->findClosestEnemy's answer (0: none) */
	Int weaponAnti;			/* the current weapon's anti mask; -1: no current weapon */
	Int upgrades;			/* the controlling player's completed upgrades: bit 0 nationalism, bit 1 fanaticism */
	Int horde;			/* three behavior modules: bits 0-2 with a horde interface, 3-5 in a horde, 6-8 allowed nationalism */
	UnsignedInt existMask;		/* the objects 1..31 that exist (TheGameLogic->findObjectByID) */
	UnsignedInt aiMask;		/* of those, the ones with an AI */
	Int hasMachine;			/* the AI's state machine exists */
	Int machineState, machineIdle, machineAttack, machineBusy;	/* its current state's id and isIdle, isAttack, isBusy */
	Int tempState;			/* its temporary state's id (INVALID_STATE_ID: none) */
	Int goalObject;			/* its goal object (0: none) */
	Int turrets;			/* at construction: the data's turrets (bit 0 main, bit 1 alt) */
};

extern AiEnv g_aiEnv;
extern std::vector<unsigned> *g_aiLog;
extern int g_aiQuiet;

/* log record codes (the Rust test's `L_*` are the same numbers): the Locomotor build's, then the AI's */
enum
{
	LOG_TRANSFORM = 1, LOG_SET_STATUS, LOG_SET_MODEL_COND, LOG_CLEAR_MODEL_COND, LOG_APPLY_FORCE,
	LOG_SCRUB, LOG_SET_TURNING, LOG_SET_EXTRA_FRICTION, LOG_SET_AIRBORNE_FRICTION, LOG_SET_STICK,
	LOG_GROUND_HEIGHT = 20, LOG_LAYER_HEIGHT, LOG_UNDERWATER, LOG_HIGHEST_LAYER, LOG_STRUCT_HEIGHT,
	LOG_VALID_TERRAIN,
	LOG_SET_WAKE = 40, LOG_GROUP_SPEED, LOG_UPDATE_LAYER, LOG_VALID_POSITION, LOG_DISTANCE_SQUARED, LOG_LINE_PASSABLE,
	LOG_GROUND_PASSABLE, LOG_LAYER_FOR, LOG_EFFECTIVELY_DEAD,
	LOG_ADD_TARGETER = 60, LOG_ABLE_TO_ATTACK, LOG_VISION_RANGE, LOG_CLOSEST_ENEMY, LOG_SET_BONUS, LOG_CLEAR_BONUS
};

class GlobalData
{
public:
	Real m_gravity;
	Int m_movementPenaltyDamageState;
};
extern GlobalData *TheGlobalData;

class Object;
class GameLogic
{
public:
	UnsignedInt getFrame() const;
	/* batch A3: the scene's objects */
	Object *findObjectByID(ObjectID id) const;
};
extern GameLogic *TheGameLogic;

class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y, Coord3D *normal = NULL) const;
	Real getLayerHeight(Real x, Real y, PathfindLayerEnum layer, Coord3D *normal = NULL, Bool clip = TRUE) const;
	Bool isUnderwater(Real x, Real y, Real *waterZ = NULL, Real *terrainZ = NULL);
	PathfindLayerEnum getHighestLayerForDestination(const Coord3D *pos, Bool onlyHealthyBridges = FALSE);
	/* Thing::setPosition and setOrientation call it for KINDOF_STICK_TO_TERRAIN_SLOPE things */
	Int alignOnTerrain(Real angle, const Coord3D &pos, Bool stickToGround, Matrix3D &mtx);
	/* batch A2: the paths (batch P1's hash) */
	PathfindLayerEnum getLayerForDestination(const Coord3D *pos);
};
extern TerrainLogic *TheTerrainLogic;

class Object;

class PartitionManager
{
public:
	Real getGroundOrStructureHeight(Real posx, Real posy);
	/* batch A2 */
	Real getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc, Coord3D *vec = NULL);
};
extern PartitionManager *ThePartitionManager;

class Pathfinder;
class AttackPriorityInfo;
class PartitionFilter;
/* batch A3: TheAI->getAiData()'s fields the AI reads */
struct AiData
{
	UnsignedInt m_forceIdleFramesCount;
	Bool m_attackUsesLineOfSight;
	Bool m_attackIgnoreInsignificantBuildings;
};
class AI
{
public:
	Pathfinder *pathfinder();
	/* batch A3 */
	const AiData *getAiData();
	Real getAdjustedVisionRangeForObject(const Object *object, Int factorsToConsider);
#include "ai_qualifiers.inc"
	Object *findClosestEnemy( const Object *me, Real range, UnsignedInt qualifiers,
		const AttackPriorityInfo *info=NULL, PartitionFilter *optionalFilter=NULL);
};
extern AI *TheAI;

/* batch A3: the damage info's input as the AI reads it */
class DamageInfo
{
public:
	struct { DamageType m_damageType; ObjectID m_sourceID; } in;
};

class BodyModuleInterface
{
public:
	BodyDamageType getDamageState() const;
	/* batch A3 */
	const DamageInfo *getLastDamageInfo() const;
};

class PhysicsBehavior
{
public:
	Bool getIsStunned() const;
	void applyMotiveForce(const Coord3D *force);
	void scrubVelocity2D(Real desiredVelocity);
	void setTurning(PhysicsTurningType turning);
	const Coord3D *getVelocity() const;
	Real getVelocityMagnitude() const;
	Real getForwardSpeed2D() const;
	Real getForwardSpeed3D() const;
	Real getMass() const;
	Bool isMotive() const;
	void setExtraFriction(Real friction);
	void setAllowAirborneFriction(Bool allow);
	void setStickToGround(Bool stick);
	/* batch A2 */
	Bool getAllowToFall() const;
};

class GeometryInfo
{
public:
	Real getMajorRadius() const;
	Real getBoundingCircleRadius() const;
};

/* ObjectStatusMaskType, as far as Locomotor.cpp and the AI use it */
struct ObjectStatusMaskType
{
	UnsignedInt64 m_bits;
	Bool test(Int bit) const { return ((m_bits >> bit) & 1) != 0; }
};
inline ObjectStatusMaskType makeObjectStatusMask(Int bit) { ObjectStatusMaskType m; m.m_bits = 1ULL << bit; return m; }
#define MAKE_OBJECT_STATUS_MASK(k) makeObjectStatusMask(k)

/* batch A3: the object's neighbours the AI reads, each answering from the scene */
/* (UpgradeTemplate and UpgradeCenter: the shared shim's Common/Upgrade.h, renamed by PreRTS.h; the wrapper defines findUpgrade) */
#include "Common/Upgrade.h"
class Player
{
public:
	PlayerType getPlayerType() const;
	Bool hasUpgradeComplete(const UpgradeTemplate *upgradeTemplate);
};
struct TeamTemplateInfo { Bool m_attackCommonTarget; };
class TeamPrototype { public: const TeamTemplateInfo *getTemplateInfo() const; };
class Team
{
public:
	const TeamPrototype *getPrototype();
	Object *getTeamTargetObject();
};
class ContainModuleInterface { public: Bool isPassengerAllowedToFire(ObjectID id = INVALID_ID) const; };
class StealthUpdate { public: Bool isGrantedBySpecialPower(); };
class Weapon { public: Int getAntiMask() const; };
class HordeUpdateInterface
{
public:
	Bool isInHorde() const;
	Bool isAllowedNationalism() const;
	int m_index;
};
class CollideModuleInterface { public: Bool wouldLikeToCollideWith(const Object *other) const; };
class BehaviorModule
{
public:
	HordeUpdateInterface *getHordeUpdateInterface();
	/* (checkForCrateToPickup's: never reached, it looks the crate up after clearing its id) */
	CollideModuleInterface *getCollide();
	int m_index;
};
class AIGroup;
class AIUpdateInterface;

/* what Object's m_template stands in for: Thing.cpp asks it isKindOf(KINDOF_STICK_TO_TERRAIN_SLOPE) */
struct AiTemplateView
{
	const AiTemplateView *operator->() const { return this; }
	Bool isKindOf(KindOfType t) const { return FALSE; }
};

class Object
{
public:
	/* ---- Thing (upstream text: loco_gen/thing_methods.inc, thing_heights.inc) */
	void setPosition(const Coord3D *pos);
	void setPositionZ(Real z);
	void setOrientation(Real angle);
	inline const Coord3D *getPosition() const { return &m_cachedPos; }
	inline Real getOrientation() const { return m_cachedAngle; }
	const Coord3D *getUnitDirectionVector2D() const;
	void getUnitDirectionVector2D(Coord3D &dir) const;
	void getUnitDirectionVector3D(Coord3D &dir) const;
	Real getHeightAboveTerrain() const;
	Real getHeightAboveTerrainOrWater() const;
	void setTransformMatrix(const Matrix3D *mx);
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	Real calculateHeightAboveTerrain(void) const;
	void reactToTransformChange(const Matrix3D *oldMtx, const Coord3D *oldPos, Real oldAngle);
	/* isAboveTerrain, quiet: the ground-height query of the Thing machinery is not logged */
	Bool isAboveTerrain() const;

	/* ---- the rest of Object, from the scene */
	PhysicsBehavior *getPhysics();
	const PhysicsBehavior *getPhysics() const;
	BodyModuleInterface *getBodyModule();
	PathfindLayerEnum getLayer() const;
	Bool isKindOf(KindOfType t) const;
	ObjectStatusMaskType getStatusBits() const;
	void setStatus(ObjectStatusMaskType objectStatus, Bool set = TRUE);
	Real getCarrierDeckHeight() const;
	const GeometryInfo &getGeometryInfo() const;
	Bool isDisabledByType(DisabledType type) const;
	void setModelConditionState(ModelConditionFlagType a);
	void clearModelConditionState(ModelConditionFlagType a);
	/* batch A2 */
	ObjectID getID() const;
	UnsignedByte getCrusherLevel() const;
	/* Object.h: `void clearStatus( ObjectStatusMaskType objectStatus ) { setStatus( objectStatus, false ); }` */
	void clearStatus(ObjectStatusMaskType objectStatus) { setStatus(objectStatus, false); }
	void setEffectivelyDead(Bool dead);
	/* batch A3 (Object.h: `Bool testStatus( ObjectStatusTypes bit ) const { return m_status.test( bit ); }`) */
	Bool testStatus(ObjectStatusTypes bit) const { return getStatusBits().test(bit); }
	Bool isEffectivelyDead() const;
	Player *getControllingPlayer() const;
	Team *getTeam();
	CanAttackResult getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType commandSource) const;
	Object *getContainedBy();
	ContainModuleInterface *getContain() const;
	StealthUpdate *getStealth() const;
	Weapon *getCurrentWeapon();
	const Weapon *getCurrentWeapon() const;
	BehaviorModule **getBehaviorModules();
	void setWeaponBonusCondition(WeaponBonusConditionType wst);
	void clearWeaponBonusCondition(WeaponBonusConditionType wst);
	AIGroup *getGroup();
	AIUpdateInterface *getAI();
	/* 0: the AI's own object; else the id of another object of the scene */
	ObjectID m_otherId;

	Object();

	AiTemplateView m_template;
	Matrix3D m_transform;
	enum
	{
		VALID_DIRVECTOR = 0x01,
		VALID_ALTITUDE_TERRAIN = 0x02,
		VALID_ALTITUDE_SEALEVEL = 0x04
	};
	mutable Coord3D m_cachedPos;
	mutable Real m_cachedAngle;
	mutable Coord3D m_cachedDirVector;
	mutable Real m_cachedAltitudeAboveTerrain;
	mutable Real m_cachedAltitudeAboveTerrainOrWater;
	mutable Int m_cacheFlags;
	UnsignedInt64 m_status;
	PhysicsBehavior m_physics;
	BodyModuleInterface m_body;
	GeometryInfo m_geometry;
};

/* ---- batch A2: the AI's neighbours */

/* UpdateModule (layer 8), as the AI's base: its object and the sleepy update's wake frame (a stand-in kept in the scene) */
class UpdateModule
{
public:
	UpdateModule(class Thing *thing, const class ModuleData *moduleData) {}
	UpdateSleepTime getWakeFrame() const;
	void setWakeFrame(Object *obj, UpdateSleepTime wakeDelay);
	Object *getObject() const;
};

/* AIGroup (AIGroup.cpp, not yet translated): the speed recomputation a locomotor change asks for (logged) */
class AIGroup
{
public:
	void recomputeGroupSpeed();
};

/* AIStateMachine (AIStates.cpp, not yet translated): the goal position, the current and temporary states, the goal object (the
** scene's) */
typedef UnsignedInt StateID;
enum { MACHINE_DONE_STATE_ID = 999998, INVALID_STATE_ID = 999999 };	/* StateMachine.h, verbatim */
class AIStateMachine
{
public:
	const Coord3D *getGoalPosition() const;
	/* batch A3 */
	StateID getCurrentStateID() const;
	Bool isInIdleState() const;
	Bool isInAttackState() const;
	Bool isInBusyState() const;
	StateID getTemporaryState(void) const;
	Object *getGoalObject();
};

#endif
