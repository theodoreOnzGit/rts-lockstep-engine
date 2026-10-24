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
	/* batch R26: StealthUpdate drops a temporary grant on a player's command (differential/l9_stealth_r26.inc) */
	CommandSourceType getLastCommandSource() const { return (CommandSourceType)m_lastCmdSource; }
	Int m_lastCmdSource = 0;
	/* --- end p1w13-sonnet --- */
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
