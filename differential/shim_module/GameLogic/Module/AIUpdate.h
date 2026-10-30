/* Stand-in for GameEngine/Include/GameLogic/Module/AIUpdate.h, Module build only (differential tests, p1w10-sonnet, upgrade batch B): the one call
** LocomotorSetUpgrade makes. Renamed ModTestAIUpdateInterface by module_prelude.h. */
#ifndef SHIM_MODULE_AIUPDATE_H
#define SHIM_MODULE_AIUPDATE_H
#include "Lib/BaseType.h"
#include "Common/GameCommon.h"
#include <vector>
class Object;
/* --- p1w13-sonnet --- (batch R2: TransportContain) */
#include "GameLogic/AI.h"
/* batch R5 (RiderChangeContain): AIUpdate.h, verbatim */
enum LocomotorSetType
{
	LOCOMOTORSET_INVALID = -1,

	LOCOMOTORSET_NORMAL = 0,
	LOCOMOTORSET_NORMAL_UPGRADED,
	LOCOMOTORSET_FREEFALL,
	LOCOMOTORSET_WANDER,
	LOCOMOTORSET_PANIC,
	LOCOMOTORSET_TAXIING,			// set used for normally-airborne items while taxiing on ground
	LOCOMOTORSET_SUPERSONIC,	// set used for high-speed attacks
	LOCOMOTORSET_SLUGGISH,		// set used for abnormally slow (but not damaged) speeds

	LOCOMOTORSET_COUNT	///< keep last, please
};
enum AIFreeToExitType // Note - written out in save/load xfer, don't change these numbers.  jba.
{
	FREE_TO_EXIT=0,
	NOT_FREE_TO_EXIT=1,
	WAIT_TO_EXIT=2
};
/* --- end p1w13-sonnet --- */
class DozerAIInterface;
class SupplyTruckAIInterface;
/* --- p1w13-sonnet --- (batch R57: SlavedUpdate reads the last node of its master's path; AIPathfind.h's PathNode::getPosition and
** Path::getLastNode, verbatim, on stand-ins that hold only those fields) */
class DamageInfo;
class PathNode
{
public:
	Coord3D *getPosition( void ) { return &m_pos; }			///< return position of this node
	const Coord3D *getPosition( void ) const { return &m_pos; }			///< return position of this node
	Coord3D m_pos;
};
class Path
{
public:
	PathNode *getLastNode( void ) { return m_pathTail; }
	PathNode *m_pathTail = 0;
};
/* --- end p1w13-sonnet --- */
class AIUpdateInterface { public: void setLocomotorUpgrade(Bool set);
	/* p1w13-sonnet, batch R44: the supply docks' truck (differential/l9_supply_docks_r44.inc); null for the other drivers */
	SupplyTruckAIInterface *getSupplyTruckAIInterface() { return m_supplyTruck; }
	SupplyTruckAIInterface *m_supplyTruck = 0;
	DozerAIInterface *getDozerAIInterface();   /* p1w13-sonnet, batch R13e: differential/l9_crates_r13e.inc */
	void transferAttack(ObjectID fromID, ObjectID toID);   /* p1w13-sonnet, batch R14a: differential/l9_dies_r14.inc */
	void notifyCrate(ObjectID id);
	/* p1w13-sonnet, batch R19: AutoFindHealingUpdate, PilotFindVehicleUpdate (differential/l9_find_r19.inc) */
	void aiGetHealed(Object *healDepot, CommandSourceType cmdSource);
	void aiEnter(Object *obj, CommandSourceType cmdSource);
	void evaluateMoraleBonus( void );   /* batch R25: HordeUpdate (differential/l9_horde_r25.inc) */
	/* p1w13-sonnet, batch R18: the production exits (differential/l9_exits_r18.inc) */
	Bool isDoingGroundMovement() const { return m_groundMovement; }
	Bool m_groundMovement = false;
	void aiFollowExitProductionPath(const std::vector<Coord3D> *path, Object *ignoreObject, CommandSourceType cmdSource);
	/* batch R15: RebuildHoleBehavior (differential/l9_hole_r15.inc) */
	Object *construct(const ThingTemplate *what, const Coord3D *pos, Real angle, Player *owner, Bool isRebuild);
	void aiResumeConstruction(Object *obj, CommandSourceType cmdSource);   /* p1w13-sonnet, batch R14c: differential/l9_dies_r14.inc */
	/* --- p1w13-sonnet --- (update batch O: SquishCollide asks what the AI is going for) */
	Object *getGoalObject() const { return m_goal; }
	Object *m_goal = 0;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch P: ActiveBody; the calls are lines of the effect log, differential/l9_active_body_p.inc) */
	Bool isMoving() const { return m_moving; }
	Bool isIdle() const { return m_idle; }
	void aiIdle( CommandSourceType cmdSource );
	void aiEvacuateInstantly( Bool exposeStealthUnits, CommandSourceType cmdSource );
	void chooseLocomotorSet( LocomotorSetType wst );   /* p1w13-sonnet, batch R5: differential/l9_open_contain_r.inc logs it */
	void aiGuardRetaliate( Object *victim, const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	Bool m_moving = false, m_idle = false;
	Object *m_owner = 0;
	Bool isAiInDeadState() const { return m_dead; }   /* SlowDeathBehavior */
	void markAsDead();
	Bool m_dead = false;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R: OpenContain; differential/l9_open_contain_r.inc) */
	void aiExit( Object *objectToExit, CommandSourceType cmdSource );
	void aiExitInstantly( Object *objectToExit, CommandSourceType cmdSource );
	void aiHackInternet( CommandSourceType cmdSource );
	void aiMoveToPosition( const Coord3D *pos, CommandSourceType cmdSource );
	/* batch R81: SpecialAbilityUpdate (lines: differential/l9_special_ability_r81.inc) */
	void aiMoveToObject( Object *obj, CommandSourceType cmdSource );
	void aiFaceObject( Object *obj, CommandSourceType cmdSource );
	void aiFacePosition( const Coord3D *pos, CommandSourceType cmdSource );
	void ignoreObstacle( const Object *obj );
	void setIgnoreCollisionTime( Int frames );
	void aiFollowPath( const std::vector<Coord3D> *path, Object *ignoreObject, CommandSourceType cmdSource );
	void doQuickExit( const std::vector<Coord3D> *path );
	Object *getEnterTarget() const { return m_enterTarget; }
	const class LocomotorSet *getLocomotorSet() const { return 0; }
	Object *m_enterTarget = 0;
	/* --- p1w13-sonnet --- (batch R2: TransportContain) */
	void setAttitude( AttitudeType tude );
	void wakeUpAndAttemptToTarget( void );
	AIFreeToExitType getAiFreeToExit(const Object* exiter) const { return (AIFreeToExitType)m_freeToExit; }
	const Locomotor *getCurLocomotor() const { return m_hasLoco ? (const Locomotor *)this : 0; }   /* the test's locomotor is the AI itself */
	Locomotor *getCurLocomotor() { return m_hasLoco ? (Locomotor *)this : 0; }   /* batch R8 */
	void aiHunt( CommandSourceType cmdSource );   /* batch R8 */
	/* batch R12: GarrisonContain asks what the rider is shooting at */
	Object *getCurrentVictim() const { return m_victim; }
	const Coord3D *getCurrentVictimPos() const { return m_victimPosHas ? &m_victimPos : 0; }
	Object *m_victim = 0;
	Bool m_victimPosHas = false;
	Coord3D m_victimPos;
	Int m_freeToExit = 0;
	Bool m_hasLoco = false;
	Real m_r60Height = 0.0f;   /* p1w13-sonnet, batch R60: the current locomotor's preferred height (differential/l9_spectre_deploy_r60.inc) */
	/* batch R26: StealthUpdate drops a temporary grant on a player's command (differential/l9_stealth_r26.inc) */
	CommandSourceType getLastCommandSource() const { return (CommandSourceType)m_lastCmdSource; }
	Int m_lastCmdSource = 0;
	const class AttackPriorityInfo *getAttackInfo(void) { return m_attackInfo; }   /* batch R69: CommandButtonHuntUpdate */
	const class AttackPriorityInfo *m_attackInfo = 0;
	void aiFollowWaypointPath( const class Waypoint *way, CommandSourceType cmdSource );   /* batch R70: lines (differential/l9_wave_guide_r70.inc) */
	void setPathExtraDistance( Real dist );
	/* batch R71: BattlePlanUpdate's turret (the test's answers; the orders lines: differential/l9_battle_plan_r71.inc) */
	WhichTurretType getWhichTurretForCurWeapon() const { return (WhichTurretType)m_curTurret; }
	void setTurretEnabled( WhichTurretType tur, Bool enabled );
	void recenterTurret( WhichTurretType tur );
	Bool isTurretInNaturalPosition( WhichTurretType tur ) const;
	Int m_curTurret = -1;
	Bool m_turretNatural = false;
	/* batch R58: MobMemberSlavedUpdate's questions (the test's answers: differential/l9_mob_member_r58.inc) */
	Real getLocomotorDistanceToGoal() { return m_r58Dist; }
	const Coord3D* getGoalPosition() const { return &m_r58Goal; }
	Bool isAttacking() const { return m_r58Attacking; }
	Object *getNextMoodTarget( Bool calledByAI, Bool calledDuringIdle ) { return m_r58Mood; }
	Real m_r58Dist = 0.0f;
	Coord3D m_r58Goal{};
	Bool m_r58Attacking = false;
	Object *m_r58Mood = 0;
	/* batch R57: SlavedUpdate follows its master's path and goes prone when the master is hit (differential/l9_slaved_r57.inc) */
	Path *getPath() { return m_r57Path; }
	Path *m_r57Path = 0;
	void aiGoProne( const DamageInfo *damageInfo, CommandSourceType cmdSource );
	/* batch R56: CleanupHazardUpdate keeps the AI busy and orders the attack (lines: differential/l9_cleanup_hazard_r56.inc) */
	Bool isBusy() const { return m_busy; }
	Bool m_busy = false;
	void aiBusy( CommandSourceType cmdSource );
	void aiAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
	void aiForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );   /* batch R73: SpawnBehavior (a line) */
	/* batch R63: FireWeaponPower's attack and turrets (lines: differential/l9_powers_h.inc) */
	void aiAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	/* batch R82: FlightDeckBehavior passes its orders on (lines: differential/l9_flight_deck_r82.inc) */
	void aiGuardPosition( const Coord3D *pos, GuardMode guardMode, CommandSourceType cmdSource );
	void aiAttackMoveToPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	void setTurretTargetObject( WhichTurretType tur, Object *o, Bool isForceAttacking = FALSE );
	void setTurretTargetPosition( WhichTurretType tur, const Coord3D *pos );
	/* --- end p1w13-sonnet --- */
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R50: PhysicsBehavior's questions, differential/l9_physics_r50.inc) */
	LocomotorSetType getCurLocomotorSetType() const { return (LocomotorSetType)m_r50LocoSet; }
	ObjectID getIgnoredObstacleID( void ) const { return m_r50Ignored; }
	Bool processCollision(class PhysicsBehavior *physics, Object *other);   /* a line */
	Bool hasLocomotorForSurface(Int surfaceType) { return (m_r50Surfaces & (unsigned)surfaceType) != 0; }
	Int m_r50LocoSet = -1;
	ObjectID m_r50Ignored = (ObjectID)0;
	Bool m_r50Process = true;
	unsigned m_r50Surfaces = 0;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch R40: MinefieldBehavior grants a mine clearer immunity) */
	Bool isClearingMines() const { return m_clearingMines; }
	Bool m_clearingMines = false;
	/* --- end p1w13-sonnet --- */
};
#endif

/* --- p1w13-sonnet --- (batch R5: RiderChangeContain.cpp defines DEFINE_LOCOMOTORSET_NAMES and includes this header after others have; the names sit
** outside the include guard so that inclusion still sees them. AIUpdate.h, verbatim.) */
#if defined(DEFINE_LOCOMOTORSET_NAMES) && !defined(SHIM_MODULE_LOCOMOTORSET_NAMES)
#define SHIM_MODULE_LOCOMOTORSET_NAMES
static const char *TheLocomotorSetNames[] = 
{
	"SET_NORMAL",
	"SET_NORMAL_UPGRADED",
	"SET_FREEFALL",
	"SET_WANDER",
	"SET_PANIC",
	"SET_TAXIING",
	"SET_SUPERSONIC",
	"SET_SLUGGISH",

	NULL
};
#endif
/* --- end p1w13-sonnet --- */
