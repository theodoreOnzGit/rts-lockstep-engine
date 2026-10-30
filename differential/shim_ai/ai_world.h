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
/* batch A38 (upstream's, standalone) */
#include "GameLogic/WeaponSetType.h"
/* batch A38 (ObjectIter.h's) */
enum IterOrderType { ITER_FASTEST, ITER_SORTED_NEAR_TO_FAR, ITER_SORTED_FAR_TO_NEAR, ITER_SORTED_CHEAP_TO_EXPENSIVE, ITER_SORTED_EXPENSIVE_TO_CHEAP };

class Locomotor;
class Waypoint;
class Weapon;
class Player;
/* batch A20: another object's AI (the scene's one memory) and its goal */
class Object;
class AIUpdateInterface;
Bool isOtherAi(const AIUpdateInterface *ai);
Object *otherAiGoalObject();
/* batch A11: JetAIUpdate (never an AI of this build: getJetAIUpdate is NULL) */
class JetAIUpdate { public: Bool isOutOfSpecialReloadAmmo() const; };
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
	Int modules;			/* bit 0 DeliverPayloadAIUpdate (batch A37: unused: the delivering AI is its own, subclass 9), bit 1 ProneUpdate, bit 2 HackInternetAIUpdate (batch A34: unused: the hacker is its own AI, subclass 6) */
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
	/* ---- batch A12 (the hunt and attack-area states) */
	Int unitsHunt;			/* the controlling player's getUnitsShouldHunt */
	/* ---- batch A13 (the exit states) */
	Int exitFlags;			/* the contain's getContainExitInterface (bit 0: one), the exit's isExitBusy (bit 1) */
	Int exitDoor;			/* the exit's reserveDoorForExit */
	/* ---- batch A14 (the enter state) */
	Real minorRadius;		/* the other objects' geometry's getMinorRadius */
	Int geomBox;			/* the other objects' geometry's getGeomType is GEOMETRY_BOX (else GEOMETRY_SPHERE) */
	/* ---- batch A15 (the rappel state) */
	UnsignedInt containedMask;	/* the contain's getContainedItemsList: the existing objects without an AI of this mask, in id order */
	/* ---- batch A16 (the attack-squad state) */
	Int difficulty;			/* the controlling player's getPlayerDifficulty */
	Int chooseNormal;		/* TheScriptEngine->getChooseVictimAlwaysUsesNormal */
	/* ---- batch A17 (the dock state) */
	UnsignedInt dockMask;		/* the other objects with a DockUpdateInterface (fixed for a case) */
	Int dockFlags;			/* bit 0 isDockOpen, 1 isAllowPassthroughType, 2 isRallyPointAfterDockType, 3 the dock's object has an
					** exit interface, 4 it has a rally point */
	Real dockPos[3];		/* the dock's positions: the approach (reserve/advance) here, the enter, dock and exit 0.1, 0.2, 0.3 east */
	Int dockIndex;			/* the approach index reserve/advance give */
	UnsignedInt droneMask;		/* the other objects whose producer is the owner */
	UnsignedInt dockRate;		/* the dock's answers (reserve, advance, clear to enter, clear to advance, action): TRUE at this rate */
	/* ---- batch A18 (the guard state) */
	Int clearableAttacker;		/* the body's getClearableLastAttacker (clearLastAttacker zeroes it) */
	Int guardFlags;			/* the template's isEnterGuard (bit 0), isHijackGuard (bit 1) */
	UnsignedInt objectsChangedFrame;	/* TheGameLogic->getFrameObjectsChangedTriggerAreas */
	UnsignedInt guardScanRate, guardReturnScanRate, guardChaseFrames;	/* TheAI->getAiData()'s */
	/* batch A20 */
	Int tunnelIds[4];		/* the tunnel network's tunnels after the owner's container (0: none) */
	Int tunnelNemesis;		/* its getCurNemesis */
	UnsignedInt tunnelStamp;	/* another object's last damage's time: this plus a hash of the object below 64 */
	UnsignedInt tunnelNoEffect;	/* another object's last damage's out.m_noEffect (bit by object id) */
	Int otherAiGoal;		/* another object's AI's getGoalObject */
	/* batch A23 */
	Int otherAiState;		/* another object's AI's machine's current state (set, not entered) */
	Real otherAiGoalPos[3];	/* and its goal position */
	/* batch A30: the AI's class (0 AIUpdateInterface, 1 WanderAIUpdate, 2 TransportAIUpdate; batch A31: 3 DeployStyleAIUpdate) */
	Int subclass;
	/* batch A31: a DeployStyleAIUpdate's data's bits (reset turret before packing, turrets function only when deployed, turrets
	** must center before packing, manual deploy animations) and the object's drawable (bit 4); its unpack and pack times */
	Int deployFlags;
	UnsignedInt deployTimes[2];
	/* batch A32: an AssaultTransportAIUpdate's data's members-get-healed-at life ratio */
	Real assaultRatio;
	/* batch A33: the railed transport's named waypoints ("railStartNN", "railEndNN": bit NN-1) and its docks (bit 0 the object's
	** dock, bit 1 a railed transport dock) */
	UnsignedInt railStartMask, railEndMask;
	Int railFlags;
	/* batch A34: a HackInternetAIUpdate's data (unpack, pack, cash delay, fast cash delay; regular, veteran, elite, heroic cash; xp;
	** variation factor), the hacker's world (bit 0 the player's money, 1 an experience tracker, 2 the hacker locally controlled,
	** 3 its container locally controlled) and its veterancy level */
	UnsignedInt hackTimes[4];
	UnsignedInt hackCash[4];
	UnsignedInt hackXp;
	Real hackVariation;
	Int hackFlags;
	Int hackLevel;
	/* batch A35: a SupplyTruckAIUpdate's data (max boxes; center and warehouse delays; scan distance; bit 0 of the flags: its
	** depleted voice named) and the truck's world (the resource manager's best center and warehouse, the player's closest cash
	** generator, command center and structure; the objects with a warehouse's or a center's dock module) */
	Int supplyMaxBoxes;
	UnsignedInt supplyDelays[2];
	Real supplyScan;
	Int supplyFlags;
	Int supplyBest[2];
	Int supplyClosest[3];
	UnsignedInt supplyDockMasks[2];
	/* batch A36: a MissileAIUpdate's data (flags: bit 0 try to follow the target, 1 use the weapon's speed, 2 detonate on no fuel,
	** 3 detonate calls kill, 4 an ignition FX, 5 a garrison-hit-kill FX; times: fuel lifetime, ignition delay; reals: initial
	** velocity, distance before turning, before diving, scatter when jammed, lock distance; the garrison-hit-kill count, its
	** required and forbidden kinds (bit k: kind k), the kill-self delay) and its world (the weapon templates' speeds, their
	** die-on-detonate bits, the collide rate, the countermeasure, the garrison-immune bit) */
	Int missileFlags;
	UnsignedInt missileTimes[2];
	Real missileReals[5];
	Int missileGarrison[3];
	Int missileKillSelfDelay;
	Real missileWeaponSpeeds[3];
	Int missileDieMask;
	UnsignedInt missileCollideRate;
	Int missileCountermeasure;
	Int missileGarrisonImmune;
	/* batch A37: a DeliverPayloadAIUpdate's module data (door and drop delays; the maximum distance, the drop offset and variance,
	** the decal radius; the maximum attempts and the put-in-container name (0 none, 1 "pt0", 2 "pt1", 3 "ptx": unknown)), the
	** delivery decals' template (its name (0 none, 1 "decal0"), style, throb time, colour, owner-only flag; its opacities), the data
	** op 109 delivers (reals: the distance, the pre-open distance, the drop offset and variance, the dive's start and end, the exit
	** pitch rate, the strafe length, the decal radius; ints: the maximum attempts, the drop delay, the visible bones, the strafing
	** slot (-1 none), the items per interval, the visible payload weapon (-1 none), the strafe FX (-1 none), the bone name (0 none,
	** 1 "bone"), the subobject name (0 none, 1 "sub"), the payload template (as the container's)), the flags (bit 0 the object's
	** drawable, 1 the payload a projectile, 2 the payload with an AI, 3 fire the weapon, 4 self-destruct, 5 inherit the velocity,
	** 6 parachute directly) and the world (the map's border, the payload's mass and centre-of-mass offset; the physics' turning;
	** the items with a minefield module, with a smart bomb module) */
	UnsignedInt deliverTimes[2];
	Real deliverReals[8];
	Int deliverInts[2];
	Int deliverDecal[5];
	Real deliverOpacity[2];
	Real deliverDataReals[13];
	Int deliverDataInts[10];
	Int deliverFlags;
	Real deliverWorld[3];
	Int deliverTurning;
	UnsignedInt deliverModuleMasks[2];
	/* batch A38: a DozerAIUpdate's data (repair percent per second, bored time, bored range) and its world (the AI data's bored
	** radius modifier, every object's bounding sphere radius; the templates' time and cost to build, the player's index, the
	** constructed object's id; the objects with a bridge tower interface, with a bridge interface, the repair candidates, the create
	** modules (bit i: module i); the towers' bridge, the bridges' towers) */
	Real dozerReals[5];
	Int dozerInts[4];
	UnsignedInt dozerMasks[4];
	Int dozerBridge;
	Int dozerTowers[4];
	/* batch A39: a WorkerAIUpdate's upgraded supply boost (its other data the dozer's and the supply truck's fields) */
	Int workerBoost;
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
	LOG_ATTACK = 103,	/* batch A10a: { kind, words.. } (see wrap_ai.cpp) */
	LOG_RAPPEL = 104,	/* batch A15: { kind, words.. } (see wrap_ai.cpp) */
	LOG_DOCK = 105,		/* batch A17: { kind, words.. } (see wrap_ai.cpp) */
	LOG_TUNNEL = 106,	/* batch A20: { kind, words.. } (see wrap_ai.cpp) */
	LOG_AI_SNAPSHOT = 107,	/* batch A21: { kind, words.. } (see wrap_ai.cpp) */
	LOG_PASSENGER = 108,	/* batch A30: a transport's passenger's command (see wrap_ai.cpp) */
	LOG_ANIM_FRAME = 109,	/* batch A31: the drawable's setAnimationFrame { frame } */
	LOG_CLEAR_MODEL_FLAGS = 110,	/* batch A31: clearModelConditionFlags { the conditions, in increasing order } */
	LOG_MEMBER_AI = 111,	/* batch A32: an assault member's AI { kind, the member, words.. } (see wrap_ai.cpp) */
	LOG_RAILED = 112,	/* batch A33: the railed transport's docks { 0 setDockOpen, open; 1 isLoadingOrUnloading, answer; 2 unloadAll } */
	LOG_SUPPLY = 114,	/* batch A35: the supply truck's world { 0 findBestSupplyCenter, answer; 1 findBestSupplyWarehouse, answer;
			** 2 findClosestByKindOf, kind, answer; 3 getDistanceSquared to an object (2-D), it, bits; 4 updateDrawableSupplyStatus,
			** max, boxes } */
	LOG_MISSILE = 115,	/* batch A36: the missile's world { 0 attemptDamage, type, death, source, amount; 1 setDrawableHidden, hide;
			** 2 positionProjectileForLaunch, launcher, slot, barrel; 3 shouldProjectileCollideWith, weapon, launcher, other, victim,
			** answer; 4 handleProjectileDetonation, weapon, bonus, damage; 5 createAttachedParticleSystemID, template, id;
			** 6 destroyParticleSystemByID, id; 7 getDistanceSquared, kind, it, bits; 8 calculateCountermeasureToDivertTo, victim,
			** answer } */
	LOG_DELIVER = 116,	/* batch A37: the delivery's world { kind, words.. } (see wrap_ai.cpp) */
	LOG_DOZER = 117,	/* batch A38: the dozer's world { kind, words.. } (see wrap_ai.cpp) */
	LOG_HACK = 113	/* batch A34: the hacker's world { 0 deposit, amount; 1 addMoneyEarned, amount; 2 addExperiencePoints, xp, canScale;
			** 3 isLocallyControlled, the object, answer; 4 the floating cash text, amount; 5 setAnimationLoopDuration, frames } */
};

/* batch A36: Weapon.h's `typedef UnsignedInt WeaponBonusConditionFlags;` */
typedef UnsignedInt WeaponBonusConditionFlags;
/* batch A35: KindOf.h's KindOfMaskType (BitFlags<KINDOF_COUNT>), as far as the supply truck's regrouping uses it */
struct KindOfMaskType
{
	unsigned long long m_bits[4];
	KindOfMaskType() { clear(); }
	void set(KindOfType t) { m_bits[t >> 6] |= 1ULL << (t & 63); }
	void clear() { m_bits[0] = m_bits[1] = m_bits[2] = m_bits[3] = 0; }
	Bool test(KindOfType t) const { return (m_bits[t >> 6] >> (t & 63)) & 1; }
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
/* batch A31 (ModelState.h's MAKE_MODELCONDITION_MASK2) */
inline ModelConditionFlags makeModelConditionMask(ModelConditionFlagType a, ModelConditionFlagType b) { ModelConditionFlags f; f.set(a); f.set(b); return f; }
#define MAKE_MODELCONDITION_MASK2(a,b) makeModelConditionMask(a, b)
/* batch A34 (ModelState.h's MAKE_MODELCONDITION_MASK3) */
inline ModelConditionFlags makeModelConditionMask(ModelConditionFlagType a, ModelConditionFlagType b, ModelConditionFlagType c) { ModelConditionFlags f; f.set(a); f.set(b); f.set(c); return f; }
#define MAKE_MODELCONDITION_MASK3(a,b,c) makeModelConditionMask(a, b, c)

/* batch A4: TheScriptEngine, as far as AIDeadState uses it (logged) */
class AttackPriorityInfo;
class ScriptEngine
{
public:
	void notifyOfObjectCreationOrDestruction();
	/* batch A16 */
	Bool getChooseVictimAlwaysUsesNormal() const;
	/* batch A21: by name ("ap<index>") */
	const AttackPriorityInfo *getAttackInfo(const AsciiString& name);
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
	/* batch A18 */
	UnsignedInt getFrameObjectsChangedTriggerAreas(void);
};
extern GameLogic *TheGameLogic;

class PolygonTrigger;
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
	/* batch A18: "poly<index>" */
	PolygonTrigger *getTriggerAreaByName(AsciiString name);
	/* batch A9 */
	Waypoint *getWaypointByID(UnsignedInt id);
	/* batch A5: the commands' map edges */
	void getExtent(Region3D *extent) const;
	/* batch A6 */
	void getMaximumPathfindExtent(Region3D *extent) const;
	void getBridgeAttackPoints(const Object *bridge, TBridgeAttackInfo *info);
	Coord3D findClosestEdgePoint(const Coord3D *closestTo) const;
	/* batch A37: the extent and the scene's border */
	void getExtentIncludingBorder(Region3D *extent) const;
	/* batch A38 (logged) */
	void flattenTerrain(Object *obj);
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
/* batch A18: the guard's filters (kept for the stand-in search: what they are) */
class PartitionFilterRelationship : public PartitionFilter
{
public:
	enum { ALLOW_ALLIES = (1<<ALLIES), ALLOW_ENEMIES = (1<<ENEMIES), ALLOW_NEUTRAL = (1<<NEUTRAL) };
	PartitionFilterRelationship(const Object *obj, Int flags) : m_obj(obj), m_flags(flags) { }
	const Object *m_obj;
	Int m_flags;
};
class PartitionFilterIsFlying : public PartitionFilter { public: PartitionFilterIsFlying() { } };
/* batch A19 */
class PartitionFilterRejectBuildings : public PartitionFilter { public: PartitionFilterRejectBuildings(const Object *o) : m_self(o) { } const Object *m_self; };
class PartitionFilterPossibleToEnter : public PartitionFilter
{
public:
	PartitionFilterPossibleToEnter(const Object *obj, CommandSourceType commandSource) : m_obj(obj), m_commandSource(commandSource) { }
	const Object *m_obj;
	CommandSourceType m_commandSource;
};
class PartitionFilterPossibleToHijack : public PartitionFilter
{
public:
	PartitionFilterPossibleToHijack(const Object *obj, CommandSourceType commandSource) : m_obj(obj), m_commandSource(commandSource) { }
	const Object *m_obj;
	CommandSourceType m_commandSource;
};
/* batch A16: the attack-squad state's (its squad kept for the stand-in search) */
class Squad;
class PartitionFilterAcceptOnSquad : public PartitionFilter
{
public:
	PartitionFilterAcceptOnSquad(const Squad *squad) : m_squad(squad) {}
	const Squad *m_squad;
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
	/* batch A37 (logged) */
	void registerObject(Object *obj);
	void unRegisterObject(Object *obj);
	/* batch A38: the bored dozer's search (logged; the scene's) */
	class ObjectIterator *iterateObjectsInRange(const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters = NULL,
		IterOrderType order = ITER_FASTEST);
	/* batch A2 */
	Real getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc, Coord3D *vec = NULL);
	/* batch A35: object to object (FROM_BOUNDINGSPHERE_2D only; logged) */
	Real getDistanceSquared(const Object *obj, const Object *otherObj, DistanceCalculationType dc, Coord3D *vec = NULL);	/* (batch A36: FROM_CENTER_2D and FROM_BOUNDINGSPHERE_3D too) */
	/* batch A5 */
	/* batch A16: the attack-squad state's (filters[0] a PartitionFilterAcceptOnSquad) */
	Object *getClosestObject(const Object *obj, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters = NULL,
		Real *closestDist = NULL, Coord3D *closestDistVec = NULL);
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
	/* batch A38 */
	Real m_aiDozerBoredRadiusModifier;
	/* batch A18 */
	UnsignedInt m_guardChaseUnitFrames;
	UnsignedInt m_guardEnemyScanRate;
	UnsignedInt m_guardEnemyReturnScanRate;
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
	struct { DamageType m_damageType; ObjectID m_sourceID;
		/* batch A36 (the missile's own death) */
		Int m_deathType; Real m_amount; } in;
	/* batch A20 */
	struct { Bool m_noEffect; } out;
};

class BodyModuleInterface
{
public:
	BodyDamageType getDamageState() const;
	/* batch A3 */
	const DamageInfo *getLastDamageInfo() const;
	/* batch A18 (logged) */
	ObjectID getClearableLastAttacker() const;
	void clearLastAttacker();
	/* batch A20 */
	UnsignedInt getLastDamageTimestamp() const;
	/* batch A38 (another object's: logged) */
	void internalChangeHealth( Real delta );
	void evaluateVisualCondition();
	/* batch A32 (another object's: the hash's) */
	Real getHealth() const;
	Real getMaxHealth() const;
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
	/* batch A15 (logged) */
	void resetDynamicPhysics();
	void scrubVelocityZ(Real desiredVelocity);
	void setAllowAirborneFriction(Bool allow);
	void setStickToGround(Bool stick);
	/* batch A2 */
	Bool getAllowToFall() const;
	/* batch A37 (an item's force logged; the turning the scene's) */
	void applyForce( const Coord3D *force );
	PhysicsTurningType getTurning(void) const;
};

class GeometryInfo
{
public:
	Real getMajorRadius() const;
	Real getBoundingCircleRadius() const;
	/* batch A14 */
	Real getMinorRadius() const;
	/* batch A38: the scene's */
	Real getBoundingSphereRadius() const;
	GeometryType getGeomType() const;
	/* batch A7 */
	Real getMaxHeightAbovePosition() const;
	/* batch A10b: the position raised by half the scene's height above position */
	void getCenterPosition(const Coord3D& pos, Coord3D& center) const;
};

/* ObjectStatusMaskType, as far as Locomotor.cpp and the AI use it */
struct ObjectStatusMaskType
{
	UnsignedInt64 m_bits;
	Bool test(Int bit) const { return ((m_bits >> bit) & 1) != 0; }
	/* batch A38 */
	void set(Int bit) { m_bits |= 1ULL << bit; }
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
/* batch A20: TunnelTracker.h's, as far as AITNGuard.cpp uses it (the scene's; updateNemesis logged) */
class TunnelTracker
{
public:
	Object *getCurNemesis(void);
	void updateNemesis(const Object *target);
	const std::list<ObjectID> *getContainerList() const;
};
class Player
{
public:
	/* batch A20 (always one: the player makes it) */
	TunnelTracker *getTunnelSystem(void);
	PlayerType getPlayerType() const;
	Bool hasUpgradeComplete(const UpgradeTemplate *upgradeTemplate);
	/* batch A9 */
	Bool isSkirmishAIPlayer( void );
	/* batch A18: AIGuardState::onEnter's, after its return (never reached) */
	class AcademyStats *getAcademyStats();
	/* batch A34 (the hacker's: the scene's) */
	class Money *getMoney();
	class ScoreKeeper *getScoreKeeper();
	/* batch A35 (the supply truck's: logged) */
	class ResourceGatheringManager *getResourceGatheringManager();
	/* batch A37 (the scene's one team) */
	class Team *getDefaultTeam();
	/* batch A38 (logged; the index the scene's) */
	void onStructureConstructionComplete( Object *builder, Object *structure, Bool isRebuild );
	void onStructureCreated( Object *builder, Object *structure );
	Int getPlayerIndex() const;
	Object *findClosestByKindOf( Object *queryObject, KindOfMaskType setKindOf, KindOfMaskType clearKindOf );
	/* batch A12 */
	Bool getUnitsShouldHunt() const;
	/* batch A17: the existing other objects, in id order */
	void iterateObjects( void (*func)( Object *obj, void *userData ), void *userData ) const;
	/* batch A16 */
	GameDifficulty getPlayerDifficulty(void) const;
};
struct TeamTemplateInfo { Bool m_attackCommonTarget; };
class TeamPrototype { public: const TeamTemplateInfo *getTemplateInfo() const; };
class Team
{
public:
	/* batch A34: the scene's one team, "team0" */
	const AsciiString& getName() const;
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
/* batch A34: TeamFactory.h's findTeam (the scene's one team) */
class TeamFactory { public: Team *findTeam( const AsciiString& name ); };
extern TeamFactory *TheTeamFactory;
typedef std::list<Object*> ContainedItemsList;
/* batch A13: the contain's exit (logged; its answers the scene's) */
struct AiTemplateView;
class ExitInterface
{
public:
	/* batch A17 (not logged: the scene's) */
	const Coord3D *getRallyPoint( void ) const;
	Bool isExitBusy() const;
	ExitDoorType reserveDoorForExit( const AiTemplateView *objType, Object *specificObject );
	void exitObjectViaDoor( Object *newObj, ExitDoorType exitDoor );
	/* batch A20 (logged) */
	void exitObjectInAHurry( Object *newObj );
};
class ContainModuleInterface
{
public:
	/* batch A13 */
	void onObjectWantsToEnterOrExit(Object *obj, ObjectEnterExitType wantsToEnterOrExit);
	/* batch A14 (logged) */
	void addToContain( Object *obj );
	/* batch A15 (logged) */
	Bool isValidContainerFor( const Object* obj, Bool checkCapacity ) const;
	void removeFromContain( Object *obj, Bool exposeStealthUnits = FALSE );
	ExitInterface *getContainExitInterface();
	Bool isPassengerAllowedToFire(ObjectID id = INVALID_ID) const;
	/* batch A10a (logged, from the hash; the scene's) */
	Bool isEnclosingContainerFor( const Object *obj ) const;
	Bool attemptBestFirePointPosition( Object *source, Weapon *weapon, Object *victim );
	Bool attemptBestFirePointPosition( Object *source, Weapon *weapon, const Coord3D *targetPos );
	Bool isGarrisonable() const;
	UnsignedInt getContainCount() const;
	/* batch A36 (the scene's) */
	Bool isImmuneToClearBuildingAttacks() const;
	/* batch A5 (logged) */
	const ContainedItemsList* getContainedItemsList() const;
	void removeAllContained( Bool exposeStealthUnits = FALSE );
	void markAllPassengersDetected();
	void orderAllPassengersToExit( CommandSourceType, Bool instantly );
	/* batch A37 (an item's, logged) */
	void setOverrideDestination( const Coord3D * );
};
class StealthUpdate { public: Bool isGrantedBySpecialPower(); };
/* batch A36: the missile's weapons, particle systems (renamed Ai* in PreRTS.h: other builds compile these classes): the scene's three
** detonation weapon templates ("wt0".."wt2") and exhaust templates ("ps0".."ps2"), logged */
class WeaponTemplate
{
public:
	Real getWeaponSpeed() const;
	Bool shouldProjectileCollideWith( const Object* projectileLauncher, const Object* projectile, const Object* thingWeCollidedWith,
		ObjectID intendedVictimID ) const;
	Bool getDieOnDetonate() const;
	const AsciiString& getName() const;
	/* batch A37 (the scene's) */
	const class ParticleSystemTemplate* getProjectileExhaust(VeterancyLevel v) const;
	Int m_index; AsciiString m_name;
};
class WeaponStore
{
public:
	void handleProjectileDetonation( const WeaponTemplate* w, const Object *source, const Coord3D* pos, WeaponBonusConditionFlags extraBonusFlags,
		Bool inflictDamage = TRUE );
	const WeaponTemplate *findWeaponTemplate( AsciiString name ) const;
};
extern WeaponStore *TheWeaponStore;
enum ParticleSystemID { INVALID_PARTICLE_SYSTEM_ID = 0, PARTICLE_SYSTEM_ID_MAX = 0x7fffffff };
class ParticleSystemTemplate { public: const AsciiString& getName() const; Int m_index; AsciiString m_name; };
class ParticleSystemManager
{
public:
	ParticleSystemID createAttachedParticleSystemID( const ParticleSystemTemplate *sysTemplate, Object* attachTo, Bool createSlaves = FALSE );
	void destroyParticleSystemByID( ParticleSystemID id );
	const ParticleSystemTemplate *findTemplate( const AsciiString &name ) const;
};
extern ParticleSystemManager *TheParticleSystemManager;
class Weapon
{
public:
	/* batch A36 (logged) */
	static void positionProjectileForLaunch( Object *projectile, const Object *launcher, WeaponSlotType wslot, Int specificBarrelToUse );
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
	/* batch A10b (logged, from the hash) */
	Bool isTooClose(const Object *source, const Object *target) const;
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
	/* batch A33: the second module's, when the scene says so */
	class RailedTransportDockUpdateInterface *getRailedTransportDockUpdateInterface();
	/* batch A38: a goal object's create modules (the scene's) */
	class CreateModuleInterface *getCreate();
	/* batch A37: a delivered payload's projectile module's */
	class ProjectileUpdateInterface *getProjectileUpdateInterface();
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
	/* batch A18: the scene's */
	Bool isEnterGuard() const;
	Bool isHijackGuard() const;
	/* batch A38: the scene's */
	Int calcTimeToBuild( const Player* player ) const;
	Int calcCostToBuild( const Player* player ) const;
	/* batch A15: none (the FX call is logged) */
	const class FXList *getPerUnitFX(const char *fxName) const { return NULL; }
	/* batch A31: "Deploy" and "Undeploy" ("s4", "s5", or empty: the scene's) */
	const class AudioEventRTS *getPerUnitSound(const AsciiString& soundName) const;
};
/* batch A31: Drawable.h's, as far as DeployStyleAIUpdate reaches it (logged) */
class Drawable
{
public:
	void setAnimationFrame( int frame );
	/* batch A34 (logged) */
	void setAnimationLoopDuration( UnsignedInt numFrames );
	/* batch A35 (logged) */
	void updateDrawableSupplyStatus( Int maxSupply, Int currentSupply );
	/* batch A36 (logged) */
	void setDrawableHidden( Bool hidden );
	/* batch A37 (logged; the bones the hash's) */
	void showSubObject( const AsciiString& name, Bool show );
	void updateSubObjects();
	Int getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const;
	void convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const;
};
/* batch A34: Money.h's, ScoreKeeper.h's, ExperienceTracker.h's, as far as the hacker reaches them (logged) */
class Money { public: void deposit( UnsignedInt amountToDeposit, Bool playSound = TRUE );
	/* batch A38 (logged) */
	void withdraw( UnsignedInt amountToWithdraw, Bool playSound = TRUE ); };
class ScoreKeeper { public: void addMoneyEarned( Int earned ); };
class ExperienceTracker
{
public:
	VeterancyLevel getVeterancyLevel() const;
	void addExperiencePoints( Int experienceGain, Bool canScaleForBonus = TRUE );
};
/* batch A17: UpdateModule.h's DockUpdateInterface (logged but isDockOpen, isAllowPassthroughType, isRallyPointAfterDockType: the
** scene's) and the supply truck's (never one here) */
class DockUpdateInterface
{
public:
	Bool reserveApproachPosition( Object* docker, Coord3D *position, Int *index );
	Bool advanceApproachPosition( Object* docker, Coord3D *position, Int *index );
	Bool isClearToEnter( Object const* docker ) const;
	Bool isClearToAdvance( Object const* docker, Int dockerIndex ) const;
	void getEnterPosition( Object* docker, Coord3D *position );
	void getDockPosition( Object* docker, Coord3D *position );
	void getExitPosition( Object* docker, Coord3D *position );
	void onApproachReached( Object* docker );
	void onEnterReached( Object* docker );
	void onDockReached( Object* docker );
	void onExitReached( Object* docker );
	Bool action( Object* docker, Object *drone = NULL );
	void cancelDock( Object* docker );
	Bool isDockOpen( void );
	Bool isAllowPassthroughType();
	Bool isRallyPointAfterDockType();
	/* batch A33 (logged) */
	void setDockOpen( Bool open );
};
/* batch A33: RailedTransportDockUpdate's interface, as far as RailedTransportAIUpdate reaches it (logged; the answer the hash's) */
class RailedTransportDockUpdateInterface
{
public:
	Bool isLoadingOrUnloading( void );
	void unloadAll( void );
};
/* batch A35: SupplyTruckAIUpdate.h's SupplyTruckAIInterface and WorkerAIUpdate.h's WorkerAIInterface, verbatim (the stand-in
** before batch A35 answered the dock's delay 0) */
#include "ai_supply_iface_a35.inc"
/* batch A35: the player's ResourceGatheringManager, as far as the supply truck reaches it (logged) */
class ResourceGatheringManager
{
public:
	Object *findBestSupplyWarehouse( Object *queryObject );
	Object *findBestSupplyCenter( Object *queryObject );
};
class SupplyWarehouseDockUpdate;
class SupplyCenterDockUpdate;
/* batch A18: never reached (see Player::getAcademyStats) */
class AcademyStats { public: void recordGuardAbilityUsed();
	/* batch A38 (logged) */
	void recordProduction( const Object *obj, const Object *constructor ); };
/* batch A15: FXList.h's doFXObj (logged) */
class FXList { public: static void doFXObj(const FXList* fx, const Object* primary, const Object* secondary = NULL);
	/* batch A37 (logged) */
	static void doFXPos(const FXList* fx, const Coord3D *primary, const Matrix3D* primaryMtx = NULL, const Real primarySpeed = 0.0f,
		const Coord3D *secondary = NULL, const Real overrideRadius = 0.0f); };
/* batch A12: ScriptEngine.h's attack priority info, as far as the hunt state uses it (getPriority logged); two of them */
class AttackPriorityInfo
{
public:
	Int getPriority(const AiTemplateView *tThing) const;
	/* batch A21: "ap<index>" */
	AsciiString getName() const;
	Int m_index;
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
	/* batch A17 */
	class DockUpdateInterface *getDockUpdateInterface( void );
	ObjectID getProducerID() const;
	ExitInterface *getObjectExitInterface() const;
	inline AIUpdateInterface *getAIUpdateInterface() { return getAI(); }
	inline const AIUpdateInterface *getAIUpdateInterface() const { return getAI(); }
	/* batch A15 (logged) */
	void setLayer( PathfindLayerEnum layer );
	void scoreTheKill( const Object *victim );
	void kill();
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
	/* batch A32: Object.h's const getter */
	BodyModuleInterface *getBodyModule() const { return const_cast<Object *>(this)->getBodyModule(); }
	Bool isContained() const;
	PathfindLayerEnum getLayer() const;
	Bool isKindOf(KindOfType t) const;
	ObjectStatusMaskType getStatusBits() const;
	void setStatus(ObjectStatusMaskType objectStatus, Bool set = TRUE);
	Real getCarrierDeckHeight() const;
	const GeometryInfo &getGeometryInfo() const;
	Bool isDisabledByType(DisabledType type) const;
	void setModelConditionState(ModelConditionFlagType a);
	void clearModelConditionState(ModelConditionFlagType a);
	/* batch A31 */
	void clearModelConditionFlags(const ModelConditionFlags& clr);
	Drawable *getDrawable() const;
	/* batch A34 */
	ExperienceTracker *getExperienceTracker() const;
	Bool isLocallyControlled() const;
	/* batch A38 (the dozer's: logged; a goal object's answers the hash's) */
	void setWeaponSetFlag( WeaponSetType wst );
	void clearWeaponSetFlag( WeaponSetType wst );
	ObjectID getSoleHealingBenefactor( void ) const;
	ObjectID getBuilderID() const;
	void setBuilder( const Object *obj );
	void setProducer( const Object *obj );
	void setConstructionPercent( Real percent );
	Real getConstructionPercent() const;
	void handlePartitionCellMaintenance();
	void updateUpgradeModules();
	Bool attemptHealingFromSoleBenefactor( Real amount, const Object* source, UnsignedInt duration );
	Bool isUsingAirborneLocomotor( void ) const;
	Real m_constructionPercent;
	Bool m_constructionPercentSet;
	/* batch A37 (the delivery's: logged; Object.cpp's) */
	Bool setWeaponLock( WeaponSlotType weaponSlot, WeaponLockType lockType );
	VeterancyLevel getVeterancyLevel() const;
	void clearAndSetModelConditionState( ModelConditionFlagType clr, ModelConditionFlagType set );
	/* batch A36 (the missile's: the scene's, logged) */
	void attemptDamage( DamageInfo *damageInfo );
	WeaponBonusConditionFlags getWeaponBonusCondition() const;
	ObjectID calculateCountermeasureToDivertTo( const Object& victim );
	Bool isKindOfMulti( const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear ) const;
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

/* (batch A21: a snapshot) */
#include "Common/Snapshot.h"
/* UpdateModule (layer 8), as the AI's base: its object and the sleepy update's wake frame (a stand-in kept in the scene) */
class UpdateModule : public Snapshot
{
public:
	UpdateModule(class Thing *thing, const class ModuleData *moduleData) {}
	/* batch A21: the base's snapshot (the stand-in's: its version and the sleepy update's next call frame) */
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
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
	/* batch A14 */
	CanAttackResult getCanAttackObject( const Object *obj, const Object *objectToAttack, CommandSourceType commandSource, AbleToAttackType attackType );
	/* batch A38 */
	Bool canRepairObject( const Object *obj, const Object *objectToRepair, CommandSourceType commandSource );
	Bool canResumeConstructionOf( const Object *obj, const Object *objectBeingConstructed, CommandSourceType commandSource );
};
enum CanEnterType { CHECK_CAPACITY, DONT_CHECK_CAPACITY, COMBATDROP_INTO };	/* ActionManager.h, verbatim */
extern ActionManager *TheActionManager;
/* (batch A37: DeliverPayloadAIUpdate is the real class, wrap_ai.cpp; privateMoveToAndEvacuateAndExit's call of it a cut point) */
class DeliverPayloadAIUpdate;
void deliverPayloadViaModuleDataOf(DeliverPayloadAIUpdate *dp, const Coord3D *moveToPos);
class ProneUpdate { public: void goProne( const DamageInfo *damageInfo ); };
/* (batch A34: HackInternetAIUpdate is the real class, wrap_ai.cpp) */
class HackInternetAIUpdate;
class PolygonTrigger { public: void getCenterPoint(Coord3D* pOutCoord) const; Int m_index;
	/* batch A18 */
	Real getRadius(void) const; AsciiString getTriggerName(void) const; };
/* batch A12: the attack-area state's filter (its polygon kept for the stand-in search) */
class PartitionFilterPolygonTrigger : public PartitionFilter
{
public:
	/* (upstream keeps the pointer: the guard makes one with no area, batch A18) */
	PartitionFilterPolygonTrigger(const PolygonTrigger *trigger) : m_index(trigger ? trigger->m_index : -1) {}
	Int m_index;
};
class CommandButton { public: Int getCommandType() const; Int m_index;
	/* batch A34: "btn" and the index */
	const AsciiString& getName() const; AsciiString m_name; };
class CommandSet { public: const CommandButton* getCommandButton(Int i) const; };
class ControlBar { public: const CommandSet *findCommandSet( const AsciiString& name );
	/* batch A34 */
	const CommandButton *findCommandButton( const AsciiString& name ); };
extern ControlBar *TheControlBar;

#endif
