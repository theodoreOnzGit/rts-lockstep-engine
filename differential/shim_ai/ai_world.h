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
**  - the AI's own neighbours: its sleepy update module (UpdateModule: the wake frame) and its group (AIGroup), declared here and
**    defined by the wrapper (batch A4: its state machine is the real AIStateMachine, on StateMachine.cpp).
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
class Waypoint;
class Weapon;
class Player;
class AsciiString;

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
	/* batch A4: the pathfinder's goalPosition (TRUE with pfGoal when pfHasGoal) */
	Real pfGoal[3];
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
	Int hasMachine;			/* at construction: onObjectCreated (the AI's state machine made) */
	/* ---- batch A4 */
	Int pfHasGoal;			/* the pathfinder's goalPosition answers TRUE (with pfGoal) */
	Int disabledMask;		/* bit t: isDisabledByType(t), for DISABLED_PARALYZED, _EMP, _SUBDUED, _HACKED */
	Int repulsor;			/* TheAI->findClosestRepulsor's answer (0: none) */
	Real objVisionRange;		/* the object's getVisionRange */
	UnsignedInt turretRate;		/* a turret's update: UPDATE_SLEEP_NONE at this rate (the hash), else a sleep */
	UnsignedInt stateRate;		/* a placeholder state's onEnter/update: STATE_CONTINUE at this rate (the hash), else an end or a sleep */
	/* ---- batch A5 (the commands) */
	Int mobile;			/* isMobile */
	Real partitionCellSize;		/* TheGlobalData->m_partitionCellSize */
	Real extentHi[2];		/* TheTerrainLogic->getExtent: (0, 0) to (extentHi[0], extentHi[1]) */
	UnsignedInt teamMask;		/* the team's members (objects 1..31), in increasing id order */
	UnsignedInt actionRate;		/* the action manager's answers: TRUE at this rate (the hash) */
	UnsignedInt otherAiPath;	/* another object's AI has a path */
	UnsignedInt moveAwayRate;	/* getMoveAwayFromPath: a path at this rate (the hash) */
	UnsignedInt pathExistsRate;	/* clientSafeQuickDoesPathExist: TRUE at this rate (the hash) */
	Real continueRange;		/* the current weapon's continue-attack range */
	Int contactWeapon;		/* the current weapon is a contact weapon */
	Int closestAttack;		/* getClosestObject's answer (0: none) */
	UnsignedInt findPosRate;	/* findPositionAround: found at this rate (the hash) */
	Int hasContain;			/* the object has a contain module */
	Int modules;			/* bit 0 DeliverPayloadAIUpdate, bit 1 ProneUpdate, bit 2 HackInternetAIUpdate */
	Int hasCommandSet;		/* TheControlBar finds the object's command set */
	Int commandSlots[18];		/* the set's slots: a button's index (0..3), -1 empty */
	Int buttonTypes[4];		/* the buttons' GUI command types */
	/* ---- batch A6 (the paths) */
	Real pfExtentHi[2];		/* TheTerrainLogic->getMaximumPathfindExtent: (0, 0) to these */
	UnsignedInt serviceRate;	/* the pathfinder's searches (and getAircraftPath): a path at this rate (the hash) */
	UnsignedInt pfAnswerRate;	/* adjustToPossibleDestination, adjustDestination, the UI's quick path check, the attack view: TRUE at
					** this rate (the hash) */
	UnsignedInt rangeRate;		/* the weapon's range tests: TRUE at this rate (the hash) */
	Real repulsedDistance;		/* TheAI->getAiData()->m_repulsedDistance */
	UnsignedInt waypointLinks;	/* bit i: waypoint i links to waypoint i+1 */
	/* ---- batch A7 (the move states) */
	Int sounds;			/* the template's move sounds that are not empty: bit 0 start, 1 start damaged, 2 loop, 3 loop damaged */
	Int formationId;		/* getFormationID */
	Real groupSpeed;		/* the group's getSpeed */
	Real maxHeightAbovePosition;	/* every object's geometry's getMaxHeightAbovePosition */
	/* ---- batch A9 (the waypoint states) */
	UnsignedInt waypointLinks2;	/* bit i: waypoint i (with a first link) has a second, to waypoint (i+5)%8 */
	Real wallHeight;		/* the pathfinder's m_wallHeight */
	Real groupCenter[3];		/* the AI's group's getCenter */
	Int teamWaypoint;		/* at construction: the team's current waypoint (-1 none) */
	Int skirmish;			/* the controlling player's isSkirmishAIPlayer */
	Real teamCenter[3];		/* the team as a group: getCenter is the object's position moved by this */
	Int teamCount;			/* the team as a group: getCount */
	Real skirmishFudge;		/* TheAI->getAiData()->m_skirmishGroupFudgeValue */
	/* ---- batch A10a (the attack machine) */
	Int weaponStatus;		/* the weapons' getStatus (WeaponStatus) */
	Int leechRange;			/* the weapons' hasLeechRange */
	Int maxShotsInit;		/* at construction: the current weapon's max shot count (setMaxShotCount sets it) */
	Real aimDelta;			/* the weapons' getAimDelta */
	Int curSlot;			/* getCurrentWeapon(&slot)'s slot */
	Int slotMask;			/* getWeaponInWeaponSlot: bit per slot */
	Int ableToAttack;		/* isAbleToAttack */
	Int outOfAmmo;			/* isOutOfAmmo */
	Int curWeaponLocked;		/* isCurWeaponLocked */
	Int hasAnyWeapon;		/* hasAnyWeapon */
	Int crushesInfantry;		/* TheAI->getAiData()->m_aiCrushesInfantry */
	UnsignedInt objectRate;		/* the objects' answers (airborne, destroyed, crush, enclosing, fire point, weapon choice, reload):
					** TRUE at this rate (the hash) */
	Int garrisonable;		/* the contain's isGarrisonable */
	Int containCount;		/* the contain's getContainCount */
	Real relAngle;			/* getRelativeAngle2D */
	Real turretTurnRate[2];		/* the turrets' getTurnRate */
	Int turretCurWeapon;		/* bit i: turret i holds the owner's current weapon (isOwnersCurWeaponOnTurret) */
	Int teamEpoch;			/* the other objects' teams: one of two, by the hash of their id and this */
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
	LOG_ADD_TARGETER = 60, LOG_ABLE_TO_ATTACK, LOG_VISION_RANGE, LOG_CLOSEST_ENEMY, LOG_SET_BONUS, LOG_CLEAR_BONUS,
	LOG_PF_GOAL_POSITION = 70, LOG_PF_SNAP, LOG_PF_UPDATE_GOAL, LOG_PF_QUEUE, LOG_TURRET_UPDATE, LOG_AI_COMMAND, LOG_REPULSOR,
	LOG_CLEAR_SET_MODEL, LOG_SCRIPT_NOTIFY, LOG_STATE_ENTER, LOG_STATE_UPDATE, LOG_STATE_EXIT,
	LOG_EDGE_POINT = 82, LOG_ACTION, LOG_MOVE_AWAY_PATH, LOG_MOVE_ALLIES, LOG_PATH_EXISTS, LOG_MAX_SHOTS, LOG_CLOSEST_OBJECT,
	LOG_FIND_POSITION, LOG_CONTAIN, LOG_MODULE,
	LOG_SERVICE = 92, LOG_PF_ANSWER, LOG_WEAPON_RANGE,
	LOG_AUDIO_ADD = 95, LOG_AUDIO_REMOVE,
	LOG_TEAM_ACTIVE = 97, LOG_DESTROY_OBJECT,
	LOG_TEAM_WAYPOINT = 99, LOG_GROUP_CENTER, LOG_TEAM_GROUP, LOG_SKIRMISH,
	LOG_ATTACK = 103	/* batch A10a: { kind, words.. } (see wrap_ai.cpp) */
};

/* batch A4: ModelConditionFlags (ModelState.h's BitFlags<MODELCONDITION_COUNT>), as far as AIDeadState uses it */
struct ModelConditionFlags
{
	UnsignedInt64 m_bits[4];
	ModelConditionFlags() { m_bits[0] = m_bits[1] = m_bits[2] = m_bits[3] = 0; }
	void set(ModelConditionFlagType t) { m_bits[t >> 6] |= 1ULL << (t & 63); }
	Bool test(Int t) const { return ((m_bits[t >> 6] >> (t & 63)) & 1) != 0; }
};
inline ModelConditionFlags makeModelConditionMask(ModelConditionFlagType t) { ModelConditionFlags f; f.set(t); return f; }
#define MAKE_MODELCONDITION_MASK(k) makeModelConditionMask(k)

/* batch A4: TheScriptEngine, as far as AIDeadState uses it (logged) */
class ScriptEngine
{
public:
	void notifyOfObjectCreationOrDestruction();
};
extern ScriptEngine *TheScriptEngine;

class GlobalData
{
public:
	Real m_gravity;
	Int m_movementPenaltyDamageState;
	/* batch A5 */
	Real m_partitionCellSize;
	/* batch A6 (always AI_DEBUG_NONE here) */
	AIDebugOptions m_debugAI;
};
extern GlobalData *TheGlobalData;

class Object;
class GameLogic
{
public:
	UnsignedInt getFrame() const;
	/* batch A3: the scene's objects */
	Object *findObjectByID(ObjectID id) const;
	/* batch A8: the move-and-delete state's end */
	void destroyObject(Object *obj);
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
	/* batch A4: AIStateMachine::xfer's (never reached: no goal waypoint is set) */
	Waypoint *getWaypointByName(AsciiString name);
	/* batch A9 */
	Waypoint *getWaypointByID(UnsignedInt id);
	/* batch A5: the commands' map edges */
	void getExtent(Region3D *extent) const;
	/* batch A6 */
	void getMaximumPathfindExtent(Region3D *extent) const;
	void getBridgeAttackPoints(const Object *bridge, TBridgeAttackInfo *info);
	Coord3D findClosestEdgePoint(const Coord3D *closestTo) const;
};
extern TerrainLogic *TheTerrainLogic;

class Object;

/* batch A5: privateAttackPosition's partition filters (their arguments kept for the stand-in search) and search options */
class PartitionFilter
{
public:
	virtual ~PartitionFilter() {}
};
class PartitionFilterPossibleToAttack : public PartitionFilter
{
public:
	PartitionFilterPossibleToAttack(AbleToAttackType t, const Object *obj, CommandSourceType commandSource)
		: m_obj(obj), m_commandSource(commandSource), m_attackType(t) {}
	const Object *m_obj;
	CommandSourceType m_commandSource;
	AbleToAttackType m_attackType;
};
class PartitionFilterSameMapStatus : public PartitionFilter
{
public:
	PartitionFilterSameMapStatus(const Object *obj) : m_obj(obj) {}
	const Object *m_obj;
};
/* batch A10a: the fire state's continue-attack search */
class PartitionFilterSamePlayer : public PartitionFilter
{
public:
	PartitionFilterSamePlayer(const Player *p) : m_player(p) {}
	const Player *m_player;
};
enum FindPositionFlags { FPF_NONE = 0 };
const Real RANDOM_START_ANGLE = -99999.9f;
/* PartitionManager.h, verbatim */
struct FindPositionOptions
{
	FindPositionOptions( void )
	{
		flags									= FPF_NONE;
		minRadius							= 0.0f;
		maxRadius							= 0.0f;
		startAngle						= RANDOM_START_ANGLE;
		maxZDelta							= 1e10f;	// ie, any z delta.
		ignoreObject					= NULL;
		sourceToPathToDest    = NULL;
		relationshipObject		= NULL;
	};
	FindPositionFlags flags;
	Real minRadius;
	Real maxRadius;
	Real startAngle;
	Real maxZDelta;
	const Object *ignoreObject;
	const Object *sourceToPathToDest;
	const Object *relationshipObject;
};

class PartitionManager
{
public:
	Real getGroundOrStructureHeight(Real posx, Real posy);
	/* batch A2 */
	Real getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc, Coord3D *vec = NULL);
	/* batch A5 */
	Object *getClosestObject(const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters = NULL,
		Real *closestDist = NULL, Coord3D *closestDistVec = NULL);
	Bool findPositionAround(const Coord3D *center, const FindPositionOptions *options, Coord3D *result);
	/* batch A10a (logged; the scene's angle) */
	Real getRelativeAngle2D( const Object *obj, const Object *otherObj );
	Real getRelativeAngle2D( const Object *obj, const Coord3D *pos );
};
extern PartitionManager *ThePartitionManager;

class Pathfinder;
class AIGroup;
class AttackPriorityInfo;
class PartitionFilter;
/* batch A3: TheAI->getAiData()'s fields the AI reads */
struct AiData
{
	UnsignedInt m_forceIdleFramesCount;
	Bool m_attackUsesLineOfSight;
	Bool m_attackIgnoreInsignificantBuildings;
	/* batch A6 */
	Real m_repulsedDistance;
	/* batch A9 */
	Real m_skirmishGroupFudgeValue;
	/* batch A10a */
	Bool m_aiCrushesInfantry;
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
	/* batch A4 */
	Object *findClosestRepulsor( const Object *me, Real range);
	/* batch A9: the team as a group (the scene's one group, never freed: the stand-in's) */
	AIGroup *createGroup( void );
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
	/* batch A7 */
	Real getMaxHeightAbovePosition() const;
};

/* ObjectStatusMaskType, as far as Locomotor.cpp and the AI use it */
struct ObjectStatusMaskType
{
	UnsignedInt64 m_bits;
	Bool test(Int bit) const { return ((m_bits >> bit) & 1) != 0; }
};
inline ObjectStatusMaskType makeObjectStatusMask(Int bit) { ObjectStatusMaskType m; m.m_bits = 1ULL << bit; return m; }
#define MAKE_OBJECT_STATUS_MASK(k) makeObjectStatusMask(k)
/* batch A10a */
inline ObjectStatusMaskType makeObjectStatusMask(Int a, Int b, Int c = -1, Int d = -1)
{
	ObjectStatusMaskType m; m.m_bits = (1ULL << a) | (1ULL << b);
	if (c >= 0) m.m_bits |= 1ULL << c;
	if (d >= 0) m.m_bits |= 1ULL << d;
	return m;
}
#define MAKE_OBJECT_STATUS_MASK2(a,b) makeObjectStatusMask(a, b)
#define MAKE_OBJECT_STATUS_MASK4(a,b,c,d) makeObjectStatusMask(a, b, c, d)

/* batch A3: the object's neighbours the AI reads, each answering from the scene */
/* (UpgradeTemplate and UpgradeCenter: the shared shim's Common/Upgrade.h, renamed by PreRTS.h; the wrapper defines findUpgrade) */
#include "Common/Upgrade.h"
class Player
{
public:
	PlayerType getPlayerType() const;
	Bool hasUpgradeComplete(const UpgradeTemplate *upgradeTemplate);
	/* batch A9 */
	Bool isSkirmishAIPlayer( void );
};
struct TeamTemplateInfo { Bool m_attackCommonTarget; };
class TeamPrototype { public: const TeamTemplateInfo *getTemplateInfo() const; };
class Team
{
public:
	const TeamPrototype *getPrototype();
	Object *getTeamTargetObject();
	/* batch A5: the scene's members (teamMask) */
	DLINK_ITERATOR<Object> iterate_TeamMemberList() const;
	/* batch A8: the move-and-evacuate state's end */
	void setActive();
	/* batch A10a (logged) */
	void setTeamTargetObject(const Object *target);
	/* batch A9: the team's current waypoint (logged when set), the team as a group */
	void setCurrentWaypoint(const Waypoint *way);
	const Waypoint *getCurrentWaypoint();
	void getTeamAsAIGroup(AIGroup *group);
};
typedef std::list<Object*> ContainedItemsList;
class ContainModuleInterface
{
public:
	Bool isPassengerAllowedToFire(ObjectID id = INVALID_ID) const;
	/* batch A10a (logged, from the hash; the scene's) */
	Bool isEnclosingContainerFor( const Object *obj ) const;
	Bool attemptBestFirePointPosition( Object *source, Weapon *weapon, Object *victim );
	Bool attemptBestFirePointPosition( Object *source, Weapon *weapon, const Coord3D *targetPos );
	Bool isGarrisonable() const;
	UnsignedInt getContainCount() const;
	/* batch A5 (logged) */
	const ContainedItemsList* getContainedItemsList() const;
	void removeAllContained( Bool exposeStealthUnits = FALSE );
	void markAllPassengersDetected();
	void orderAllPassengersToExit( CommandSourceType, Bool instantly );
};
class StealthUpdate { public: Bool isGrantedBySpecialPower(); };
class Weapon
{
public:
	Int getAntiMask() const;
	/* batch A5 */
	void setMaxShotCount(Int maxShots);
	Real getContinueAttackRange() const;
	Bool isContactWeapon() const;
	/* batch A6 (logged, from the hash) */
	Bool isWithinAttackRange(const Object *source, const Object *target) const;
	Bool isWithinAttackRange(const Object *source, const Coord3D* pos) const;
	Bool isGoalPosWithinAttackRange(const Object *source, const Coord3D* goalPos, const Object *target, const Coord3D* targetPos) const;
	Bool computeApproachTarget(const Object *source, const Object *target, const Coord3D *pos, Real angleOffset, Coord3D& approachTargetPos) const;
	/* batch A10a: the scene's; fireWeapon and setPreAttackFinishedFrame logged */
	WeaponStatus getStatus() const;
	Bool hasLeechRange() const;
	Int getMaxShotCount() const;
	Real getAimDelta() const;
	Bool fireWeapon(const Object *source, const Coord3D* pos);
	void setPreAttackFinishedFrame(UnsignedInt frame);
	Int m_slot;
};
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
	/* batch A5: the command buttons' "not implemented" branches name it */
	const AsciiString& getName() const;
	/* batch A7: the move sounds ("s0" .. "s3", or empty: the scene's) */
	const class AudioEventRTS *getSoundMoveStart() const;
	const class AudioEventRTS *getSoundMoveStartDamaged() const;
	const class AudioEventRTS *getSoundMoveLoop() const;
	const class AudioEventRTS *getSoundMoveLoopDamaged() const;
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
	const AIUpdateInterface *getAI() const;
	/* batch A4 */
	Real getVisionRange() const;
	/* batch A5 */
	Bool isMobile() const;
	Bool isSignificantlyAboveTerrain() const;
	class UpdateModule *findUpdateModule(UnsignedInt key) const;	/* (NameKeyType, not declared here) */
	const AsciiString& getCommandSetString() const;
	Bool isSelectable() const;
	/* the command buttons' "not implemented" branches name the object */
	const AsciiString& getName() const;
	const AiTemplateView *getTemplate() const { return &m_template; }
	/* batch A7 */
	FormationID getFormationID(void) const;
	/* batch A10a: the scene's and the hash's answers; the weapon calls logged */
	Bool isAirborneTarget() const;
	Bool isDestroyed() const;
	Bool canCrushOrSquish(Object *otherObj, CrushSquishTestType testType = TEST_CRUSH_OR_SQUISH) const;
	Relationship getRelationship(const Object *that) const;
	Bool isAbleToAttack() const;
	Bool isOutOfAmmo() const;
	Bool isCurWeaponLocked() const;
	Bool hasAnyWeapon() const;
	Weapon *getCurrentWeapon(WeaponSlotType* wslot);
	Weapon *getWeaponInWeaponSlot(WeaponSlotType wslot) const;
	Bool chooseBestWeaponForTarget(const Object* target, WeaponChoiceCriteria criteria, CommandSourceType cmdSource);
	void adjustModelConditionForWeaponStatus();
	void clearLeechRangeModeForAllWeapons();
	void setFiringConditionForCurrentWeapon() const;
	void preFireCurrentWeapon( const Object *victim );
	Bool fireCurrentWeapon(Object *target);
	Bool fireCurrentWeapon(const Coord3D* pos);
	void releaseWeaponLock(WeaponLockType lockType);
	void notifyFiringTrackerShotFired( const Weapon* weaponFired, ObjectID victimID );
	/* the team's member list (Team::iterate_TeamMemberList): the next member */
	Object *dlink_next_TeamMemberList() const;
	void clearAndSetModelConditionFlags( const ModelConditionFlags& clr, const ModelConditionFlags& set );
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
	/* batch A5: Squad.cpp's (not reached: no command makes a squad from a group) */
	const std::vector<ObjectID>& getAllIDs() const;
	void add(Object *obj);
	/* batch A7 */
	Real getSpeed( void );
	/* batch A9: the AI's group's centre, or the team's (m_team) and its count */
	void getCenter( Coord3D *center );
	Int getCount( void );
	Int m_team;
};

/* ---- batch A5: what the commands hand over to (logged) */
class ActionManager
{
public:
	Bool canGetRepairedAt( const Object *obj, const Object *repairDest, CommandSourceType commandSource );
	Bool canGetHealedAt( const Object *obj, const Object *healDest, CommandSourceType commandSource );
	Bool canEnterObject( const Object *obj, const Object *objectToEnter, CommandSourceType commandSource, Int mode );
};
enum CanEnterType { CHECK_CAPACITY, DONT_CHECK_CAPACITY, COMBATDROP_INTO };	/* ActionManager.h, verbatim */
extern ActionManager *TheActionManager;
class DeliverPayloadAIUpdate { public: void deliverPayloadViaModuleData( const Coord3D *moveToPos ); };
class ProneUpdate { public: void goProne( const DamageInfo *damageInfo ); };
class HackInternetAIUpdate { public: void hackInternet(); };
class PolygonTrigger { public: void getCenterPoint(Coord3D* pOutCoord) const; Int m_index; };
class CommandButton { public: Int getCommandType() const; Int m_index; };
class CommandSet { public: const CommandButton* getCommandButton(Int i) const; };
class ControlBar { public: const CommandSet *findCommandSet( const AsciiString& name ); };
extern ControlBar *TheControlBar;

#endif
