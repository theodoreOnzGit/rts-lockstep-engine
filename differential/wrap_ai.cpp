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
** Batch A5 adds the commands: AI.h's AICommandParms and AICommandInterface (ai_commands_decl.inc), AIUpdate.cpp's aiDoCommand and
** every private* command (ai_update_a5.inc), the machine's squad setters, and Squad.cpp unchanged beside this file; the stand-ins
** they reach are logged (the action manager, the containers, the weapon's shot count and ranges, the partition's search and
** position finder, the move-away path, the allies, the quick path check, the command set, the special modules).
**
** Batch A6 adds the paths (ai_update_a6.inc: doPathfind, the requests, the path computations, setPathFromWaypoint), over a stand-in
** PathfindServicesInterface whose searches make their paths from the hash, and logged stand-ins of the pathfinder's other answers,
** the weapon's range tests, the waypoints' locations and links, and the bridge's attack points.
**
** Batch A7 makes the move states real (ai_states_a7.inc: isSamePosition, AIInternalMoveToState to AIMoveAwayFromRepulsorsState),
** over the AI build's own audio stand-in (TheAudio's handles logged), the template's move sounds, the pathfinder's
** snapClosestGoalPosition and removeGoal, the formation and the group's speed.
**
** Batch A8 adds the follow-path, move-and-evacuate and move-and-delete states (ai_states_a8.inc), AIUpdate.h's goal-path inlines,
** and logged stand-ins of the team's setActive and TheGameLogic->destroyObject.
**
** Batch A9 adds the waypoint, wander and panic states (ai_states_a9.inc, from `#define ALLOW_BACKTRACK`), AIUpdate.h's waypoint-id
** setters, and stand-ins of the waypoints' second link, ids and lookup by id, the team's current waypoint (logged when set), the
** groups' centres (the team's near the object), the skirmish player, the pathfinder's wall test (and its height from the scene).
**
** Batch A10a adds the attack machine (ai_states_a10.inc: the conditions, AttackStateMachine with its pursue and approach states as
** placeholders 1000 and 1001, the aim and fire states, AIAttackState) and AIUpdate's turret members (ai_update_a10.inc), over
** stand-ins of the weapons by slot (status, leech range, max shots, aim delta, firing), the objects' answers (airborne, destroyed,
** crush, relationship, two teams), the contain's fire points, the relative angle and the turrets' targets (LOG_ATTACK).
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
#include "GameLogic/AIDock.h"
#include "GameLogic/AIGuard.h"
#include "GameLogic/AIGuardRetaliate.h"
#include "GameLogic/AITNGuard.h"

/* ---- the AI ------------------------------------------------------------------------------------------------------------------ */

class Thing;
class ModuleData;
class PolygonTrigger;
class AttackPriorityInfo;
class Waypoint;
class TurretAIData;
/* TurretAI (batch T1, its own build): the AI makes one per turret its data has; here an empty object, whose presence the mood
** matrix asks */
/* batch A23: TurretAI.h's */
enum TurretTargetType { TARGET_NONE, TARGET_OBJECT, TARGET_POSITION };
class TurretAI : public Snapshot
{
public:
	TurretAI(Object *owner, const TurretAIData *data, WhichTurretType tur) : m_which((Int)tur) {}
	/* batch A21: its snapshot (batch T1's, its own build): logged; the version and its turret */
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void ) {}
	/* batch A4: the AI's update runs it (batch T1's, its own build): logged, its sleep from the hash */
	UpdateSleepTime updateTurretAI();
	/* batch A10a: the targets logged; the turn rate and whether it holds the current weapon the scene's */
	void setTurretTargetObject(Object *o, Bool forceAttacking);
	void setTurretTargetPosition(const Coord3D *pos);
	Real getTurnRate() const;
	Bool isOwnersCurWeaponOnTurret() const;
	/* batch A23 (batch T1's, its own build): the mutators and the target's query logged (LOG_AI_SNAPSHOT 3..5); the answers the
	** hash's */
	TurretTargetType friend_getTurretTarget( Object*& obj, Coord3D& pos, Bool clearDeadTargets = TRUE ) const;
	void setTurretEnabled( Bool enabled );
	void recenterTurret();
	Bool isTurretEnabled() const;
	Bool isTurretInNaturalPosition() const;
	Bool isWeaponSlotOnTurret(WeaponSlotType wslot) const;
	Bool isTryingToAimAtTarget(const Object* victim) const;
	Real getTurretAngle() const;
	Real getTurretPitch() const;
	Int m_which;
};

/* batch A4: AIStateMachine's neighbours. Squad: the real Squad.h (Squad.cpp is compiled unchanged beside this file, batch A5).
** Waypoint: a stand-in with an index (no goal waypoint's name is ever saved: xfer writes it empty and loads none). */
#include "GameLogic/Squad.h"
class Waypoint
{
public:
	/* batch A5: "wp" and the index (TerrainLogic::getWaypointByName reads it back) */
	AsciiString getName() const { AsciiString n; n.format("wp%d", m_index); return n; }
	Int m_index;
	/* batch A6: at (15 i, 25 i, 1), linked to the next when the scene says so */
	const Coord3D *getLocation( void ) const;
	Waypoint *getLink(Int ndx) const;
	Coord3D m_location;
	/* batch A9: a first link, then a second to (i+5)%8 when the scene says so; the id is the index */
	Int getNumLinks(void) const;
	UnsignedInt getID(void) const { return (UnsignedInt)m_index; }
};
/* TerrainLogic.h's */
enum { INVALID_WAYPOINT_ID = 0x7FFFFFFF };
/* batch A5: AI.h's AICommandParms and AICommandInterface, verbatim */
#include "ai_commands_decl.inc"

/* batch A4: the AI states not yet translated. Each is this class with its id (build.rs edits AIStateMachine's constructor): its
** isIdle/isAttack/isBusy are its class's static answers (AIAttackState's TRUE for the four attack states; FALSE for the others,
** whose answers are their inner machines' or the base's); its onEnter, update and onExit are logged, and onEnter and update
** answer from the hash (the scene's stateRate); its snapshot is the version only. The Rust side is AiState::Untranslated with the
** host's callbacks. (Since batch A20 build.rs leaves no placeholder: the class remains, unused.) */
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

class AIUpdateInterface : public UpdateModule, public AICommandInterface
{
public:
	AIUpdateInterface( Thing *thing, const ModuleData* moduleData );
	/* batch A21: the snapshot (AIUpdate.cpp's, verbatim in ai_update_a21.inc) */
	virtual void crc( Xfer *x );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
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
	/* batch A12 (AIUpdate.h's; getAreaToGuard virtual upstream, no subclass here) */
	void setAttackInfo(const AttackPriorityInfo *info) {m_attackInfo = info;}
	/* batch A19 (AIUpdate.h); batch A20: another object's AI (no machine here) answers the scene's */
	inline Object* getGoalObject() { return isOtherAi(this) ? otherAiGoalObject() : getStateMachine()->getGoalObject(); }
	/* batch A18 (AIUpdate.h's; virtual upstream, no subclass here) */
	const Coord3D *getGuardLocation( void ) const { return &m_locationToGuard;	}
	const ObjectID getGuardObject( void ) const { return m_objectToGuard; }
	GuardTargetType getGuardTargetType() const { return m_guardTargetType[1]; }
	void clearGuardTargetType() { m_guardTargetType[1] = m_guardTargetType[0]; m_guardTargetType[0] = GUARDTARGET_NONE; }
	GuardMode getGuardMode() const { return m_guardMode; }
	ObjectID getCrateID(void) const { return m_crateCreated; }
	/* batch A17 (AIUpdate.h's, virtual upstream; no supply truck here) */
	/* (batch A35: virtual upstream, and SupplyTruckAIUpdate answers itself; here dispatched explicitly, as isIdle (see it)) */
	SupplyTruckAIInterface* getSupplyTruckAIInterface();
	/* (batch A38: virtual upstream, and DozerAIUpdate answers itself; here dispatched explicitly) */
	class DozerAIInterface* getDozerAIInterface();
	/* batch A35 (AIUpdate.h's, virtual upstream: none for the classes of this build) */
	WorkerAIInterface* getWorkerAIInterface() { return NULL; }
	/* batch A15 (AIUpdate.h) */
	Real getDesiredSpeed() const { return m_desiredSpeed; }
	const PolygonTrigger *getAreaToGuard( void ) const { return m_areaToGuard; }
	/* batch A13: another object's AI's (virtual upstream: the base's FREE_TO_EXIT, a subclass's other answers; here the hash's, logged) */
	AIFreeToExitType getAiFreeToExit(const Object* exiter) const;
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
	/* batch A8: AIUpdate.h's inlines, verbatim */
	inline const Coord3D* friend_getGoalPathPosition( Int index ) const { return getStateMachine()->getGoalPathPosition( index ); }
	inline void friend_setCurrentGoalPathIndex( Int index ) { m_nextGoalPathIndex = index; }
	/* batch A10a: AIUpdate.h's inlines, verbatim, and the members of ai_update_a10.inc */
	Bool areTurretsLinked() const { return getAIUpdateModuleData()->m_turretsLinked; }
	/* (virtual upstream; plain here: no class of this build overrides them, and the other objects' AIs are not constructed) */
	CommandSourceType getLastCommandSource() const { return m_lastCommandSource; }
	Bool isTemporarilyPreventingAimSuccess() const { return false; }
	void notifyVictimIsDead() { }
	void setTurretTargetObject(WhichTurretType tur, Object* o, Bool forceAttacking);
	void setTurretTargetPosition(WhichTurretType tur, const Coord3D* pos);
	Real getTurretTurnRate(WhichTurretType tur) const;
	WhichTurretType getWhichTurretForCurWeapon() const;
	/* batch A11: AIUpdate.h's (virtual upstream; no subclass here: none) */
	JetAIUpdate* getJetAIUpdate() { return NULL; }
	/* AIUpdate.h's inline, verbatim */
	void friend_setLastCommandSource( CommandSourceType source ) {m_lastCommandSource = source;}
	/* batch A10b: AIUpdate.h's inlines, verbatim */
	Bool isAttackPath(void) const {return m_isAttackPath;}
	inline Bool isAllowedToChase() const { return m_allowedToChase; }
	/* batch A9: AIUpdate.h's inlines, verbatim */
	void setPriorWaypointID( UnsignedInt id )   { m_priorWaypointID = id; };
	void setCurrentWaypointID( UnsignedInt id ) { m_currentWaypointID = id; };
	void setCompletedWaypoint(const Waypoint *pWay) {m_completedWaypoint = pWay;}
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
	/* (batch A34: virtual upstream, and HackInternetAIUpdate overrides it; here its dispatch is explicit, since another object's AI
	** is not constructed (no vtable): defined after HackInternetAIUpdate, the base's body is baseIsIdle) */
	Bool isIdle() const;
	Bool baseIsIdle() const;
	/* batch A23 */
	void joinTeam( void );
	Object* getTurretTargetObject( WhichTurretType tur, Bool clearDeadTargets = TRUE );
	void setTurretEnabled(WhichTurretType tur, Bool enabled);
	void recenterTurret(WhichTurretType tur);
	Bool isTurretEnabled( WhichTurretType tur ) const;
	Bool isTurretInNaturalPosition(WhichTurretType tur) const;
	Bool isWeaponSlotOnTurretAndAimingAtTarget(WeaponSlotType wslot, const Object* victim) const;
	Bool getTurretRotAndPitch(WhichTurretType tur, Real* turretAngle, Real* turretPitch) const;
	WhichTurretType getWhichTurretForWeaponSlot(WeaponSlotType wslot, Real* turretAngle, Real* turretPitch) const;
	Bool isAttacking() const;
	Bool isBusy() const;
	Bool isClearingMines() const;
	Bool isMovingAwayFrom(Object *obj) const;
	Bool isMoving() const;
	AIGroup *getGroup(void);

	/* batch A4's members (ai_update_a4.inc); batch A30: makeStateMachine and update virtual again (as upstream: the subclasses
	** override them) */
	virtual AIStateMachine* makeStateMachine();
	void onObjectCreated();
	virtual UpdateSleepTime update( void );

	/* batch A5's members (ai_update_a5.inc), and the header's inlines they call, verbatim */
	virtual void aiDoCommand(const AICommandParms* parms);
	virtual Bool isAllowedToMoveAwayFromUnit() const { return true; }
	/* batch A7: AIUpdate.h's inlines the move states call, verbatim */
	virtual Bool isAllowedToAdjustDestination() const { return true; }
	void setDesiredSpeed( Real speed ) { m_desiredSpeed = speed; }	///< how fast we want to go
	Int getNumFramesBlocked(void) const {return m_blockedFrames;}
	Bool isBlockedAndStuck(void) const {return m_isBlockedAndStuck;}
	inline void clearMoveOutOfWay(void) {m_moveOutOfWay1 = INVALID_ID; m_moveOutOfWay2 = INVALID_ID;}
	inline Bool getRetryPath(void) {return m_retryPath;}
	void setPathExtraDistance(Real dist) {m_pathExtraDistance = dist;}
	Bool canPathThroughUnits( void ) const { return m_canPathThroughUnits; }
	void setCanPathThroughUnits( Bool canPath ) { m_canPathThroughUnits = canPath; if (canPath) m_isBlockedAndStuck=false;}
	void setIgnoreCollisionTime(Int frames) { m_ignoreCollisionsUntil = TheGameLogic->getFrame() + frames; }
	void setGoalPositionClipped(const Coord3D* in, CommandSourceType cmdSource);
	Bool isPathAvailable( const Coord3D *destination ) const;
	/* (batch A37: virtual upstream, and DeliverPayloadAIUpdate overrides it; here its dispatch is explicit (see isIdle), the base's
	** body baseIsAllowedToRespondToAiCommands) */
	Bool isAllowedToRespondToAiCommands(const AICommandParms* parms) const;
	Bool baseIsAllowedToRespondToAiCommands(const AICommandParms* parms) const;
	void doQuickExit( const std::vector<Coord3D>* path );
	void privateMoveToPosition( const Coord3D *pos, CommandSourceType cmdSource );
	void privateMoveToObject( Object *obj, CommandSourceType cmdSource );
	void privateFaceObject( Object *obj, CommandSourceType cmdSource );
	void privateFacePosition( const Coord3D *pos, CommandSourceType cmdSource );
	void privateRappelInto( Object *target, const Coord3D& pos, CommandSourceType cmdSource );
	void privateMoveToAndEvacuate( const Coord3D *pos, CommandSourceType cmdSource );
	void privateMoveToAndEvacuateAndExit( const Coord3D *pos, CommandSourceType cmdSource );
	/* (batch A35: virtual again, as upstream: SupplyTruckAIUpdate overrides it and privateDock) */
	virtual void privateIdle(CommandSourceType cmdSource);
	void privateTightenToPosition( const Coord3D *pos, CommandSourceType cmdSource );
	void privateMoveAwayFromUnit( Object *unit, CommandSourceType cmdSource );
	void privateFollowWaypointPath( const Waypoint *way, CommandSourceType cmdSource );
	void privateFollowWaypointPathExact( const Waypoint *way, CommandSourceType cmdSource );
	void privateFollowWaypointPathAsTeam( const Waypoint *way, CommandSourceType cmdSource );
	void privateFollowWaypointPathAsTeamExact( const Waypoint *way, CommandSourceType cmdSource );
	void privateFollowPathAppend( const Coord3D *pos, CommandSourceType cmdSource );
	void privateFollowPath( const std::vector<Coord3D>* path, Object *ignoreObject, CommandSourceType cmdSource, Bool exitProduction );
	/* (batch A30: the attacks virtual again, as upstream: TransportAIUpdate overrides them) */
	virtual void privateAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
	virtual void privateForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
	void privateGuardRetaliate( Object *victim, const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	void privateAttackTeam( const Team *team, Int maxShotsToFire, CommandSourceType cmdSource );
	virtual void privateAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	void privateAttackMoveToPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	void privateAttackFollowWaypointPath( const Waypoint *way, Int maxShotsToFire, Bool asTeam, CommandSourceType cmdSource );
	void privateHunt( CommandSourceType cmdSource );
	void privateAttackArea( const PolygonTrigger *areaToGuard, CommandSourceType cmdSource );
	void privateRepair( Object *obj, CommandSourceType cmdSource );
	void privateResumeConstruction( Object *obj, CommandSourceType cmdSource );
	void privateGetHealed( Object *healDepot, CommandSourceType cmdSource );
	void privateGetRepaired( Object *repairDepot, CommandSourceType cmdSource );
	void privateEnter( Object *obj, CommandSourceType cmdSource );
	virtual void privateDock( Object *obj, CommandSourceType cmdSource );
	void privateCombatDrop( Object *target, const Coord3D& pos, CommandSourceType cmdSource );
	void privateExit( Object *objectToExit, CommandSourceType cmdSource );
	void privateExitInstantly( Object *objectToExit, CommandSourceType cmdSource );
	/* (batch A33: virtual again, as upstream: RailedTransportAIUpdate overrides them) */
	virtual void privateEvacuate( Int exposeStealthUnits, CommandSourceType cmdSource );
	void privateEvacuateInstantly( Int exposeStealthUnits, CommandSourceType cmdSource );
	virtual void privateExecuteRailedTransport( CommandSourceType cmdSource );
	void privateGoProne( const DamageInfo *damageInfo, CommandSourceType );
	void privateWander( const Waypoint *way, CommandSourceType cmdSource );
	void privateWanderInPlace( CommandSourceType cmdSource );
	void privatePanic( const Waypoint *way, CommandSourceType cmdSource );
	void privateBusy( CommandSourceType cmdSource );
	void privateGuardPosition( const Coord3D *pos, GuardMode guardMode, CommandSourceType cmdSource );
	void privateGuardTunnelNetwork( GuardMode guardMode, CommandSourceType cmdSource );
	void privateGuardObject( Object *objectToGuard, GuardMode guardMode, CommandSourceType cmdSource );
	void privateGuardArea( const PolygonTrigger *areaToGuard, GuardMode guardMode, CommandSourceType cmdSource );
	void privateHackInternet( CommandSourceType cmdSource );
	void privateCommandButton( const CommandButton *commandButton, CommandSourceType cmdSource );
	void privateCommandButtonPosition( const CommandButton *commandButton, const Coord3D *pos, CommandSourceType cmdSource );
	void privateCommandButtonObject( const CommandButton *commandButton, Object *obj, CommandSourceType cmdSource );

	/* batch A6's members (ai_update_a6.inc) */
	void doPathfind( PathfindServicesInterface *pathfinder );
	void requestPath( Coord3D *destination, Bool isFinalGoal );
	void requestAttackPath( ObjectID victimID, const Coord3D* victimPos );
	void requestApproachPath( Coord3D *destination );
	void requestSafePath( ObjectID repulsor );
	void setPathFromWaypoint(const Waypoint *way, const Coord2D *offset);
	Bool canComputeQuickPath( void );
	Bool computeQuickPath( const Coord3D *destination );
	Bool computePath( PathfindServicesInterface *pathServices, Coord3D *destination );
	Bool computeAttackPath( PathfindServicesInterface *pathServices, const Object *victim, const Coord3D* victimPos );
	void friend_setGoalObject(Object *obj);
	Bool isQuickPathAvailable( const Coord3D *destination ) const;

#include "ai_update_members.inc"
};

#include "ai_update_a2.inc"
#include "ai_update_a3.inc"
#include "ai_update_a4.inc"
#include "ai_states_a4.inc"
#include "ai_states_a7.inc"
#include "ai_states_a8.inc"
#include "ai_states_a9.inc"
#include "ai_states_a10.inc"
#include "ai_states_a11.inc"
#include "ai_states_a12.inc"
#include "ai_dock_a17.inc"
#include "ai_guard_a18.inc"
#include "ai_guard_retaliate_a19.inc"
#include "ai_tn_guard_a20.inc"
/* batch A30: privateIdle's passengers' aiIdle, a logged cut point (LOG_PASSENGER kind 3; defined at the end of this file) */
void passengerAiCommand(unsigned kind, const Object *passenger, const Object *victim, const Coord3D *pos, Int shots, CommandSourceType src);
inline void passengerAiIdle(const Object *p, AIUpdateInterface *, CommandSourceType src) { passengerAiCommand(3, p, NULL, NULL, 0, src); }
/* batch A34: HackInternetAIUpdate, declared before the commands (privateHackInternet finds it): the stored damage (Damage.h's
** classes and Damage.cpp's xfer, renamed Stored*; a template found by name in none), the command storage (AI.h's and AIStates.cpp's,
** its damage upstream's default), HackInternetAIUpdate.h's states and machine (verbatim), its module data (the header's fields and
** constructor; the field parse is the INI's: the scene's, g_hackData) and the class as the header declares it without the pool and
** module macros and without HackInternetAIInterface (the control bar's); its .cpp after the other subclasses. */
#define ThingTemplate AiTemplateView
static const AiTemplateView *storedDamageTemplate(const AsciiString &) { return NULL; }
#include "ai_stored_damage_a34.inc"
#undef ThingTemplate
#include "ai_parms_storage_a34.inc"
#include "ai_hack_internet_h_a34.inc"
class HackInternetAIUpdateModuleData
{
public:
	UnsignedInt		m_unpackTime;
	UnsignedInt		m_packTime;
	UnsignedInt		m_cashUpdateDelay;
	UnsignedInt		m_cashUpdateDelayFast;
	UnsignedInt		m_regularCashAmount;
	UnsignedInt		m_veteranCashAmount;
	UnsignedInt		m_eliteCashAmount;
	UnsignedInt		m_heroicCashAmount;
	UnsignedInt		m_xpPerCashUpdate;
	Real					m_packUnpackVariationFactor;

	HackInternetAIUpdateModuleData()
	{
		m_unpackTime = 0;
		m_packTime = 0;
		m_cashUpdateDelay = 0;
		m_cashUpdateDelayFast = 0;
		m_regularCashAmount = 0;
		m_veteranCashAmount = 0;
		m_eliteCashAmount = 0;
		m_heroicCashAmount = 0;
		m_xpPerCashUpdate = 0;
		m_packUnpackVariationFactor = 0.0f;
	}
};
static HackInternetAIUpdateModuleData g_hackData;
class HackInternetAIUpdate : public AIUpdateInterface
{
public:
	HackInternetAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~HackInternetAIUpdate( void );
	const HackInternetAIUpdateModuleData *getHackInternetAIUpdateModuleData() const { return &g_hackData; }

 	virtual void aiDoCommand(const AICommandParms* parms);

	Real getPackUnpackVariationFactor() const { return getHackInternetAIUpdateModuleData()->m_packUnpackVariationFactor; }
	UnsignedInt getUnpackTime()					const;
	UnsignedInt getPackTime()						const;
	UnsignedInt getCashUpdateDelay()		const;
	UnsignedInt getRegularCashAmount()	const { return getHackInternetAIUpdateModuleData()->m_regularCashAmount; }
	UnsignedInt getVeteranCashAmount()	const { return getHackInternetAIUpdateModuleData()->m_veteranCashAmount; }
	UnsignedInt getEliteCashAmount()		const { return getHackInternetAIUpdateModuleData()->m_eliteCashAmount; }
	UnsignedInt getHeroicCashAmount()		const { return getHackInternetAIUpdateModuleData()->m_heroicCashAmount; }
	UnsignedInt getXpPerCashUpdate()		const { return getHackInternetAIUpdateModuleData()->m_xpPerCashUpdate; }

	void hackInternet();
	virtual UpdateSleepTime update();

	Bool hackIsIdle() const;	/* (its isIdle: see AIUpdateInterface's) */

	virtual Bool isHacking() const;
	virtual Bool isHackingPackingOrUnpacking() const;

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

	virtual AIStateMachine* makeStateMachine();
	
	AICommandParmsStorage		m_pendingCommand;
	Bool m_hasPendingCommand;	
};
void hackCashText(const Object *owner, UnsignedInt amount);
#include "ai_update_a5.inc"
#include "ai_update_a6.inc"
#include "ai_update_a10.inc"
/* batch A21: what the snapshot's text names: CRCGEN_LOG (a release build's: nothing), GameState.h's SC_INVALID_DATA (upstream's
** value), AIUpdate.h's TheLocomotorSetNames (DEFINE_LOCOMOTORSET_NAMES's: ai_defs.h was included without it) */
#define CRCGEN_LOG(x)
enum { SC_INVALID_DATA = 6 };
static const char *TheLocomotorSetNames[] = { "SET_NORMAL", "SET_NORMAL_UPGRADED", "SET_FREEFALL", "SET_WANDER", "SET_PANIC", "SET_TAXIING",
	"SET_SUPERSONIC", "SET_SLUGGISH", NULL };
#include "ai_update_a21.inc"

/* batch A30: WanderAIUpdate and TransportAIUpdate, declared as their headers declare them (without the pool and module macros),
** and their .cpp files from makeStateMachine, verbatim (ai_subclasses_a30.inc). The transport's passengers' commands are cut
** points: LOG_PASSENGER { 0 attack object, 1 force attack object, 2 attack position; the passenger; the victim; the position's
** bits; the shot count; the source }. */
inline void passengerAiAttackObject(const Object *p, AIUpdateInterface *, Object *v, Int shots, CommandSourceType src) { passengerAiCommand(0, p, v, NULL, shots, src); }
inline void passengerAiForceAttackObject(const Object *p, AIUpdateInterface *, Object *v, Int shots, CommandSourceType src) { passengerAiCommand(1, p, v, NULL, shots, src); }
inline void passengerAiAttackPosition(const Object *p, AIUpdateInterface *, const Coord3D *pos, Int shots, CommandSourceType src) { passengerAiCommand(2, p, NULL, pos, shots, src); }
class WanderAIUpdate : public AIUpdateInterface
{
public:
	WanderAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~WanderAIUpdate( void );
	virtual UpdateSleepTime update();
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
protected:
	virtual AIStateMachine* makeStateMachine();
};
class TransportAIUpdate : public AIUpdateInterface
{
public:
	TransportAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~TransportAIUpdate( void );
	AIFreeToExitType getAiFreeToExit(const Object* exiter) const;
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
protected:
	virtual AIStateMachine* makeStateMachine();
	virtual void privateAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
	virtual void privateAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );
	virtual void privateForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );
};
#include "ai_subclasses_a30.inc"

/* batch A31: DeployStyleAIUpdate. DeployStateTypes is cut from its header verbatim (ai_deploy_style_a31.inc, with the .cpp); the
** module data is the header's fields and constructor (its field parse is the INI's), and the class as the header declares it
** without the pool and module macros: MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA's getter returns the scene's data
** (g_deployData, set by setScene from deployTimes and deployFlags). ThingTemplate is the object's template view here. */
class DeployStyleAIUpdateModuleData
{
public:
	UnsignedInt			m_unpackTime;
	UnsignedInt			m_packTime;
	Bool						m_resetTurretBeforePacking;
	Bool						m_turretsFunctionOnlyWhenDeployed;
	Bool						m_turretsMustCenterBeforePacking;
	Bool						m_manualDeployAnimations;

	DeployStyleAIUpdateModuleData()
	{
		m_unpackTime = 0;
		m_packTime = 0;
		m_resetTurretBeforePacking = false;
		m_turretsFunctionOnlyWhenDeployed = false;
		m_turretsMustCenterBeforePacking = FALSE;
		m_manualDeployAnimations = FALSE;
	}
};
static DeployStyleAIUpdateModuleData g_deployData;
/* (its xfer of a version before 4 reaches AI.h's AICommandParmsStorage, compiled for batch A34 above; not reached: this harness
** writes version 4) */
#include "ai_deploy_style_enum_a31.inc"
class DeployStyleAIUpdate : public AIUpdateInterface
{
public:
	DeployStyleAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~DeployStyleAIUpdate( void );
	const DeployStyleAIUpdateModuleData *getDeployStyleAIUpdateModuleData() const { return &g_deployData; }

 	virtual void aiDoCommand(const AICommandParms* parms);
	virtual Bool isIdle() const;
	virtual UpdateSleepTime update();

	UnsignedInt getUnpackTime()					const { return getDeployStyleAIUpdateModuleData()->m_unpackTime; }
	UnsignedInt getPackTime()						const { return getDeployStyleAIUpdateModuleData()->m_packTime; }
	Bool doTurretsFunctionOnlyWhenDeployed() const { return getDeployStyleAIUpdateModuleData()->m_turretsFunctionOnlyWhenDeployed; }
	Bool doTurretsHaveToCenterBeforePacking() const { return getDeployStyleAIUpdateModuleData()->m_turretsMustCenterBeforePacking; }
	void setMyState( DeployStateTypes StateID, Bool reverseDeploy = FALSE );

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

	DeployStateTypes				m_state;
	UnsignedInt							m_frameToWaitForDeploy;
};
#define ThingTemplate AiTemplateView
#include "ai_deploy_style_a31.inc"
#undef ThingTemplate

/* batch A32: AssaultTransportAIUpdate. The module data is the header's fields and constructor (its field parse is the INI's); the
** class as the header declares it, without the pool and module macros and without AssaultTransportAIInterface (whose one member,
** beginAssault, the weapon calls: here op 100): MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA's getter returns the scene's data.
** Its .cpp from the constructor, verbatim (ai_assault_transport_a32.inc) but for its members' AIs, other objects' and not constructed
** in this build: build.rs makes each an AssaultMemberAi (memberAiOf), whose every call is a logged cut point (LOG_MEMBER_AI
** { kind, the member, words.. }; defined at the end of this file): 0 getLastCommandSource (CMD_FROM_AI at the scene's object rate,
** else CMD_FROM_PLAYER), 1 setAllowedToChase, 2 aiExit, 3 aiEnter, 4 getAIStateType (the other AIs' machine's state), 5 isMoving
** (the hash), 6 getGoalObject (the scene's), 7 aiAttackObject, 8 aiAttackMoveToPosition. */
class AssaultTransportAIUpdateModuleData
{
public:
	Real m_membersGetHealedAtLifeRatio;
	Real m_clearRangeRequiredToContinueAttackMove;

	AssaultTransportAIUpdateModuleData()
	{
		m_membersGetHealedAtLifeRatio = 0.0f;
		m_clearRangeRequiredToContinueAttackMove = 50.0f;
	}
};
static AssaultTransportAIUpdateModuleData g_assaultData;
class AssaultMemberAi
{
public:
	const Object *m_member;
	CommandSourceType getLastCommandSource() const;
	void setAllowedToChase(Bool allowed);
	void aiExit(Object *objectToExit, CommandSourceType cmdSource);
	void aiEnter(Object *obj, CommandSourceType cmdSource);
	AIStateType getAIStateType() const;
	Bool isMoving() const;
	Object *getGoalObject();
	void aiAttackObject(Object *victim, Int maxShotsToFire, CommandSourceType cmdSource);
	void aiAttackMoveToPosition(const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource);
	/* batch A37 (a delivered item's) */
	void aiMoveToPosition(const Coord3D *pos, CommandSourceType cmdSource);
};
AssaultMemberAi *memberAiOf(const Object *o);
#include "ai_assault_transport_enum_a32.inc"
class AssaultTransportAIUpdate : public AIUpdateInterface
{
public:
	AssaultTransportAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~AssaultTransportAIUpdate( void );
	const AssaultTransportAIUpdateModuleData *getAssaultTransportAIUpdateModuleData() const { return &g_assaultData; }

 	virtual void aiDoCommand(const AICommandParms* parms);
	virtual Bool isIdle() const;
	virtual UpdateSleepTime update();
	virtual void beginAssault( const Object *designatedTarget ) const;

	UpdateSleepTime calcSleepTime();

	void reset();
	Bool isMemberWounded( const Object *member ) const;	//Member requires medical attention?
	Bool isMemberHealthy( const Object *member ) const; //Member has full health?
	void retrieveMembers();
	void giveFinalOrders();
  Bool isAttackPointless() const;

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

  ObjectID					m_memberIDs[ MAX_TRANSPORT_SLOTS ];
	Bool							m_memberHealing[ MAX_TRANSPORT_SLOTS ];
	Bool							m_newMember[ MAX_TRANSPORT_SLOTS ];
  Coord3D						m_attackMoveGoalPos;
  mutable ObjectID	m_designatedTarget;
	AssaultStateTypes	m_state;
	UnsignedInt				m_framesRemaining;
	Int								m_currentMembers;
	Bool							m_isAttackMove;
	Bool							m_isAttackObject;
	Bool							m_newOccupantsAreNewMembers;
};
#include "ai_assault_transport_a32.inc"

/* batch A33: RailedTransportAIUpdate. The module data is the header's field (its prefix "rail" here; the field parse is the INI's);
** the class as the header declares it without the pool and module macros; its .cpp from the constructor, verbatim but for the
** object's own dock (`us->getDockUpdateInterface()` is railedOwnDock(us): the scene's, since the AI build's other paths ask other
** objects only). The waypoints "railStartNN"/"railEndNN" exist when the scene's masks say so (TerrainLogic::getWaypointByName). */
class RailedTransportAIUpdateModuleData
{
public:
	RailedTransportAIUpdateModuleData( void ) { m_pathPrefixName = "rail"; }
	AsciiString m_pathPrefixName;		///< prefix to use for waypont start and end points we'll look for
};
static RailedTransportAIUpdateModuleData g_railedData;
DockUpdateInterface *railedOwnDock(Object *us);
class RailedTransportAIUpdate : public AIUpdateInterface
{
public:

	RailedTransportAIUpdate( Thing *thing, const ModuleData *moduleData );
	virtual ~RailedTransportAIUpdate( void );
	const RailedTransportAIUpdateModuleData *getRailedTransportAIUpdateModuleData() const { return &g_railedData; }

	// AIUpdate interface methods
	virtual void aiDoCommand( const AICommandParms *parms );
	virtual UpdateSleepTime update( void );

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

	// ai module methods
	virtual void privateExecuteRailedTransport( CommandSourceType cmdSource );
	virtual void privateEvacuate( Int exposeStealthUnits, CommandSourceType cmdSource );

	// our methods
	void setInTransit( Bool inTransit );
	void loadWaypointData( void );
	void pickAndMoveToInitialLocation( void );

	// our data
	Bool m_inTransit;								///< in transit
	struct WaypointPathInfo
	{
		UnsignedInt startWaypointID;
		UnsignedInt endWaypointID;
	};
	enum { MAX_WAYPOINT_PATHS = 32 };
	WaypointPathInfo m_path[ MAX_WAYPOINT_PATHS ];		///< transit paths we can use
	Int m_numPaths;								///< how many waypoint paths are in m_path
	Int m_currentPath;						///< index into m_path for our current path

	Bool m_waypointDataLoaded;		///< TRUE once we've searched the map to load m_path

};
#include "ai_railed_transport_a33.inc"
/* batch A34: HackInternetAIUpdate.cpp (declared above) */
#include "ai_hack_internet_a34.inc"
/* batch A35: SupplyTruckAIUpdate. Its header's machine, states and state ids, verbatim (ai_supply_truck_h_a35.inc); its module data
** the header's fields and constructor (the field parse is the INI's: the scene's, g_supplyData; its depleted voice "s7" when the
** scene names one); the class as the header declares it without the pool and module macros; its .cpp from makeStateMachine,
** verbatim, with its REGROUP_SUCCESS_DISTANCE_SQUARED (ai_supply_truck_a35.inc). */
#include "ai_supply_truck_h_a35.inc"
class SupplyTruckAIUpdateModuleData
{
public:
	Int m_maxBoxesData;
	UnsignedInt m_centerDelay;
	UnsignedInt m_warehouseDelay;
	Real m_warehouseScanDistance;
 	AudioEventRTS m_suppliesDepletedVoice;						///< Sound played when I take the last box.

	SupplyTruckAIUpdateModuleData()
	{
		m_maxBoxesData = 0;
		m_centerDelay = 0;
		m_warehouseDelay = 0;
		m_warehouseScanDistance = 100;
	}
};
static SupplyTruckAIUpdateModuleData g_supplyData;
class SupplyTruckAIUpdate : public AIUpdateInterface, public SupplyTruckAIInterface
{
public:

	SupplyTruckAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~SupplyTruckAIUpdate( void );
	const SupplyTruckAIUpdateModuleData *getSupplyTruckAIUpdateModuleData() const { return &g_supplyData; }

	virtual Int getNumberBoxes() const { return m_numberBoxes; }
	virtual Bool loseOneBox();
	virtual Bool gainOneBox( Int remainingStock );

	virtual Bool isAvailableForSupplying() const;
	virtual Bool isCurrentlyFerryingSupplies() const;
	virtual Real getWarehouseScanDistance() const; ///< How far can I look for a warehouse?

	virtual void setForceWantingState(Bool v) { m_forcePending = v; } // When a Supply Center creates us (or maybe other sources later), we need to hop into autopilot mode.
	virtual Bool isForcedIntoWantingState() const { return m_forcePending; }

	virtual void setForceBusyState(Bool v) { m_forcedBusyPending = v; } 
	virtual Bool isForcedIntoBusyState() const { return m_forcedBusyPending; }

	virtual ObjectID getPreferredDockID() const { return m_preferredDock; }
	virtual UnsignedInt getActionDelayForDock( Object *dock );
	virtual Int getUpgradedSupplyBoost() const { return 0; }
	
	virtual UpdateSleepTime update();

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

	virtual AIStateMachine* makeStateMachine();
	virtual void privateDock( Object *obj, CommandSourceType cmdSource );
	virtual void privateIdle(CommandSourceType cmdSource);						///< Enter idle state.	

private:
	SupplyTruckStateMachine*	m_supplyTruckStateMachine;
	ObjectID									m_preferredDock;			///< Instead of searching, try this one first
	Int												m_numberBoxes;
	Bool											m_forcePending;				// To prevent a function from doing a setState, 
	Bool											m_forcedBusyPending;	// A supply truck can't tell the difference between Idle since
 	AudioEventRTS m_suppliesDepletedVoice;						///< Sound played when I take the last box.

};
#include "ai_supply_truck_a35.inc"
/* batch A36: MissileAIUpdate. Its module data the header's fields and constructor (MissileAIUpdate.cpp's, verbatim; the field
** parse is the INI's: the scene's, g_missileData); the class as the header declares it, without the pool and module macros and
** without ProjectileUpdateInterface (the weapon's: here ops 104-108); its .cpp from the constructor, verbatim (its BIGNUM is
** Locomotor.cpp's, the same) (ai_missile_a36.inc). */
#include "ai_missile_data_a36.inc"
static MissileAIUpdateModuleData g_missileData;
class MissileAIUpdate : public AIUpdateInterface
{
public:
	MissileAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~MissileAIUpdate( void );
	const MissileAIUpdateModuleData *getMissileAIUpdateModuleData() const { return &g_missileData; }

	enum MissileStateType	 // Stored in save file, don't renumber.
	{
		PRELAUNCH			= 0,
		LAUNCH				= 1, ///< released from launcher, falling
		IGNITION			= 2, ///< engines ignite
		ATTACK_NOTURN	= 3, ///< fly toward victim
		ATTACK				= 4, ///< fly toward victim
		DEAD					= 5,
		KILL					= 6, ///< Hit victim (cheat).
		KILL_SELF			= 7, ///< Destroy self.
	};

	virtual void projectileFireAtObjectOrPosition( const Object *victim, const Coord3D *victimPos, const WeaponTemplate *detWeap, const ParticleSystemTemplate* exhaustSysOverride );
	virtual void projectileLaunchAtObjectOrPosition(const Object *victim, const Coord3D* victimPos, const Object *launcher, WeaponSlotType wslot, Int specificBarrelToUse, const WeaponTemplate* detWeap, const ParticleSystemTemplate* exhaustSysOverride);
	virtual Bool projectileHandleCollision( Object *other );
	virtual Bool projectileIsArmed() const { return m_isArmed; }
	virtual ObjectID projectileGetLauncherID() const { return m_launcherID; }
	virtual void setFramesTillCountermeasureDiversionOccurs( UnsignedInt frames ); ///< Number of frames till missile diverts to countermeasures.
	virtual void projectileNowJammed();///< We lose our Object target and scatter to the ground

	virtual Bool processCollision(PhysicsBehavior *physics, Object *other); ///< Returns true if the physics collide should apply the force.  Normally not.  jba.

	virtual UpdateSleepTime update();
	virtual void onDelete( void );

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:
	void detonate();

private:
	MissileStateType			m_state;									///< the behavior state of the missile
	UnsignedInt						m_stateTimestamp;					///< time of state change
	UnsignedInt						m_nextTargetTrackTime;		///< if nonzero, how often we update our target pos
	ObjectID							m_launcherID;							///< ID of object that launched us (INVALID_ID if not yet launched)
	ObjectID							m_victimID;								///< ID of object that I am rocketing towards (INVALID_ID if not yet launched)
	UnsignedInt						m_fuelExpirationDate;			///< how long 'til we run out of fuel
	Real									m_noTurnDistLeft;					///< when zero, ok to start turning
	Real									m_maxAccel;
	Coord3D								m_originalTargetPos;			///< When firing uphill, we aim high to clear the brow of the hill.  jba.
	Coord3D								m_prevPos;
	WeaponBonusConditionFlags		m_extraBonusFlags;
	const WeaponTemplate*	m_detonationWeaponTmpl;		///< weapon to fire at end (or null)
	const ParticleSystemTemplate* m_exhaustSysTmpl;
	ParticleSystemID			m_exhaustID;								///< our exhaust particle system (if any)
	UnsignedInt						m_framesTillDecoyed;			///< Number of frames before missile will get distracted by decoy countermeasures.
	Bool									m_isTrackingTarget;				///< Was I originally shot at a moving object?
	Bool									m_isArmed;								///< if true, missile will explode on contact
	Bool									m_noDamage;								///< if true, missile will not cause damage when it detonates. (Used for flares).
	Bool									m_isJammed;								///< No target, just shooting at a scattered position

	void doPrelaunchState();
	void doLaunchState();
	void doIgnitionState();
	void doAttackState(Bool turnOK);
	void doKillState();
	void doKillSelfState();
	void doDeadState();

	void airborneTargetGone();											///< My airborne target has died, so I have to do something cool to make up for that
	void tossExhaust();
	void switchToState(MissileStateType s);
};
#include "ai_missile_a36.inc"
extern WeaponTemplate g_weaponTemplates[3];
extern ParticleSystemTemplate g_particleTemplates[3];
extern FXList g_missileFx[2];
#define ThingTemplate AiTemplateView
/* batch A37: DeliverPayloadAIUpdate. The client's RadiusDecal and RadiusDecalTemplate as RadiusDecal.h declares them (renamed Ai*
** in PreRTS.h: other builds compile them), their constructor, xferRadiusDecalTemplate and xferRadiusDecal verbatim
** (ai_radius_decal_a37.inc), createRadiusDecal, clear and update logged stand-ins (LOG_DELIVER 0, 2, 1: the client draws them);
** the header's machine, states, state ids and DeliverPayloadData, verbatim (ai_deliver_payload_h_a37.inc); the module data the
** header's fields and constructor (the field parse is the INI's: the scene's, g_deliverData); the class as the header declares it
** without the pool and module macros, its isAllowedToRespondToAiCommands `deliverIsAllowedToRespondToAiCommands` (the base's
** dispatches to it: see AIUpdateInterface::isAllowedToRespondToAiCommands); its .cpp from makeStateMachine, verbatim
** (ai_deliver_payload_a37.inc) but for its cut points: the items' AIs are AssaultMemberAis (memberAiOf, LOG_MEMBER_AI), an item's
** setPosition is setItemPosition (logged), and the visible payload a PayloadObject (logged). */
typedef Int Color;
enum ShadowType { SHADOW_ALPHA_DECAL = 0x20 };
struct FieldParse;
class RadiusDecalTemplate;
class RadiusDecal
{
	friend class RadiusDecalTemplate;
public:
	RadiusDecal() : m_empty(true) {}
	void xferRadiusDecal( Xfer *xfer );
	Bool isEmpty() const { return m_empty; }
	void clear();
	void update();
	Bool m_empty;
};
class RadiusDecalTemplate
{
	friend class RadiusDecal;
public:
	AsciiString		m_name;
	ShadowType		m_shadowType;
	Real					m_minOpacity;
	Real					m_maxOpacity;
	UnsignedInt		m_opacityThrobTime;
	Color					m_color;
	Bool					m_onlyVisibleToOwningPlayer;
	RadiusDecalTemplate();
	Bool valid() const { return m_name.isNotEmpty(); }
	void xferRadiusDecalTemplate( Xfer *xfer );
	void createRadiusDecal(const Coord3D& pos, Real radius, const Player* owningPlayer, RadiusDecal& result) const;
};
#include "ai_radius_decal_a37.inc"
/* the items' modules the delivery aims (another object's: the scene's masks; logged) */
class GenerateMinefieldBehavior { public: void setMinefieldTarget(const Coord3D* pos); ObjectID m_id; };
class SmartBombTargetHomingUpdate { public: void SetTargetPosition( const Coord3D& target ); ObjectID m_id; };
void setItemPosition(Object *item, const Coord3D *pos);
/* the visible payload: a new object of the scene's (ids from 40 in each case), its physics, behaviour modules and AI (logged) */
class PayloadPhysics
{
public:
	Real getMass() const;
	void applyMotiveForce(const Coord3D *force);
	Real getCenterOfMassOffset() const;
	void setPitchRate(Real pitch);
};
class PayloadAi { public: void aiMoveToPosition(const Coord3D *pos, CommandSourceType cmdSource); };
class ProjectileUpdateInterface
{
public:
	void projectileFireAtObjectOrPosition( const Object *victim, const Coord3D *victimPos, const WeaponTemplate *detWeap,
		const ParticleSystemTemplate* exhaustSysOverride );
};
class PayloadObject
{
public:
	void setProducer(const Object* obj);
	void setPosition(const Coord3D *pos);
	const Coord3D *getPosition() const { return &m_pos; }
	void setOrientation(Real angle);
	PayloadPhysics *getPhysics() { return &m_physics; }
	BehaviorModule **getBehaviorModules();
	PayloadAi *getAIUpdateInterface();
	ObjectID m_id;
	Coord3D m_pos;
	PayloadPhysics m_physics;
	PayloadAi m_ai;
};
class ThingFactory
{
public:
	const ThingTemplate *findTemplate( const AsciiString& name );
	PayloadObject *newObject( const ThingTemplate *tmplate, Team *team );
	/* batch A38: the dozer's construction */
	Object *newObject( const ThingTemplate *tmplate, Team *team, ObjectStatusMaskType statusBits );
};
extern ThingFactory *TheThingFactory;
#include "ai_deliver_payload_h_a37.inc"
class DeliverPayloadAIUpdateModuleData
{
public:
	UnsignedInt		m_doorDelay;
	Real					m_maxDistanceToTarget;				///< How far away from target I can unload, plus how far after target I need to turn around at
	Int						m_maxNumberAttempts;					///< How many times I can re-approach
	UnsignedInt		m_dropDelay;									///< How long to wait after entering Deliver state (to allow for doors opening)
	Coord3D				m_dropOffset;									///< where to disgorge the guys, relative to me
	Coord3D				m_dropVariance;								///< variance in dropping position among guys that I am dropping
	AsciiString		m_putInContainerName;
	RadiusDecalTemplate	m_deliveryDecalTemplate;
	Real					m_deliveryDecalRadius;

	DeliverPayloadAIUpdateModuleData()
	{
		m_doorDelay = 0;
		m_maxDistanceToTarget = 0.0f;
		m_maxNumberAttempts = 0;
		m_dropDelay = 0;
		m_dropOffset.zero();
		m_dropVariance.zero();
		m_deliveryDecalRadius = 0;
		m_putInContainerName.clear();
	}
};
static DeliverPayloadAIUpdateModuleData g_deliverData;
class DeliverPayloadAIUpdate : public AIUpdateInterface
{
public:

	DeliverPayloadAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~DeliverPayloadAIUpdate( void );
	const DeliverPayloadAIUpdateModuleData *getDeliverPayloadAIUpdateModuleData() const { return &g_deliverData; }

	virtual AIFreeToExitType getAiFreeToExit(const Object* exiter) const;

	const Coord3D* getTargetPos() const { return &m_targetPos; }
	const Coord3D* getMoveToPos() const { return &m_moveToPos; }
	UnsignedInt getDoorDelay() const { return getDeliverPayloadAIUpdateModuleData()->m_doorDelay; }
	Bool isDeliveringPayload() const { return m_deliverPayloadStateMachine != NULL; }
	const ThingTemplate* getPutInContainerTemplateViaModuleData() const;

	Real getAllowedDistanceToTarget() const { return m_data.m_distToTarget; }
	Real getPreOpenDistance() const { return m_data.m_preOpenDistance; }
	Int getMaxNumberAttempts() const { return m_data.m_maxAttempts; }
	UnsignedInt getDropDelay() const { return m_data.m_dropDelay; }
	const Coord3D& getDropOffset() const { return m_data.m_dropOffset; }
	const Coord3D& getDropVariance() const { return m_data.m_dropVariance; }
	Bool isFireWeapon() const { return m_data.m_fireWeapon; }
	Int getVisibleItemsDelivered() const { return m_visibleItemsDelivered; }
	void setVisibleItemsDelivered( Int num ) { m_visibleItemsDelivered = num; }

	Bool isCloseEnoughToTarget();
	Bool isOffMap() const;
	Real calcMinTurnRadius(Real* timeToTravelThatDist) const;

	void deliverPayload( const Coord3D *moveToPos, const Coord3D *targetPos, const DeliverPayloadData *data );
	void deliverPayloadViaModuleData( const Coord3D *moveToPos );
	const DeliverPayloadData* getData() { return &m_data; }

	virtual UpdateSleepTime update();

	void killDeliveryDecal();

	void friend_setFreeToExit(Bool f) { m_freeToExit = f; }
	void friend_setAcceptingCommands(Bool f) { m_acceptingCommands = f; }

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

	virtual AIStateMachine* makeStateMachine();
	/* (its isAllowedToRespondToAiCommands, virtual upstream: see AIUpdateInterface's) */
	Bool deliverIsAllowedToRespondToAiCommands(const AICommandParms* parms) const;

	DeliverPayloadStateMachine*		m_deliverPayloadStateMachine;	///< Controls my special logic
	Coord3D												m_targetPos;									///< Where I plan to deliver my little friends, if obj is null
	Coord3D												m_moveToPos;									///< Where I am moving to.
	DeliverPayloadData						m_data;
	Int														m_visibleItemsDelivered;
	RadiusDecal										m_deliveryDecal;
	Real													m_previousDistanceSqr;
	Bool													m_freeToExit;
	Bool													m_acceptingCommands;

	enum DiveState	// Stored in save file as int, don't renumber!  jba.
	{
		DIVESTATE_PREDIVE=0,
		DIVESTATE_DIVING=1,
		DIVESTATE_POSTDIVE=2,
	};
	DiveState											m_diveState;
};
#include "ai_deliver_payload_a37.inc"
/* the scene's delivery: a template's name (0 none, 1 "pt0", 2 "pt1", 3 "ptx"), the decals' template, the data op 109 delivers */
static AsciiString deliverTemplateName(Int k)
{
	static const char *const n[4] = { "", "pt0", "pt1", "ptx" };
	return AsciiString(n[k & 3]);
}
static void sceneDecalTemplate(RadiusDecalTemplate *t)
{
	t->m_name = g_aiEnv.deliverDecal[0] ? "decal0" : "";
	t->m_shadowType = (ShadowType)g_aiEnv.deliverDecal[1];
	t->m_opacityThrobTime = (UnsignedInt)g_aiEnv.deliverDecal[2];
	t->m_color = g_aiEnv.deliverDecal[3];
	t->m_onlyVisibleToOwningPlayer = g_aiEnv.deliverDecal[4] != 0;
	t->m_minOpacity = g_aiEnv.deliverOpacity[0];
	t->m_maxOpacity = g_aiEnv.deliverOpacity[1];
}
static void sceneDeliverData(DeliverPayloadData *d)
{
	const Real *r = g_aiEnv.deliverDataReals;
	const Int *k = g_aiEnv.deliverDataInts;
	const Int f = g_aiEnv.deliverFlags;
	d->m_distToTarget = r[0];
	d->m_preOpenDistance = r[1];
	d->m_dropOffset.set(r[2], r[3], r[4]);
	d->m_dropVariance.set(r[5], r[6], r[7]);
	d->m_diveStartDistance = r[8];
	d->m_diveEndDistance = r[9];
	d->m_exitPitchRate = r[10];
	d->m_strafeLength = r[11];
	d->m_deliveryDecalRadius = r[12];
	d->m_maxAttempts = k[0];
	d->m_dropDelay = (UnsignedInt)k[1];
	d->m_visibleNumBones = k[2];
	d->m_strafingWeaponSlot = (WeaponSlotType)k[3];
	d->m_visibleItemsDroppedPerInterval = k[4];
	d->m_visiblePayloadWeaponTemplate = k[5] >= 0 ? &g_weaponTemplates[k[5]] : NULL;
	d->m_strafeFX = k[6] >= 0 ? &g_missileFx[k[6]] : NULL;
	d->m_visibleDropBoneName = k[7] ? "bone" : "";
	d->m_visibleSubObjectName = k[8] ? "sub" : "";
	d->m_visiblePayloadTemplateName = deliverTemplateName(k[9]);
	d->m_fireWeapon = ((f >> 3) & 1) != 0;
	d->m_selfDestructObject = ((f >> 4) & 1) != 0;
	d->m_inheritTransportVelocity = ((f >> 5) & 1) != 0;
	d->m_isParachuteDirectly = ((f >> 6) & 1) != 0;
	sceneDecalTemplate(&d->m_deliveryDecalTemplate);
}
/* batch A38: DozerAIUpdate. Its header's primary machine, task enums and DozerAIInterface, verbatim (ai_dozer_h_a38.inc); its module
** data the header's fields and the .cpp's constructor (the field parse is the INI's: the scene's, g_dozerData); the class as the
** header declares it without the pool and module macros (its getDozerAIInterface the base's, dispatched explicitly: see
** AIUpdateInterface::getDozerAIInterface); its .cpp verbatim but its module data's field parse (ai_dozer_a38.inc), with its cut
** points: the locally controlled player's completion UI (dozerConstructionCompleteUi, logged) and a constructed object's placement
** (newStructureSetPosition, newStructureSetOrientation, logged). Its world: LOG_DOZER (see wrap_ai.cpp's end). */
class BridgeTowerBehaviorInterface { public: ObjectID getBridgeID( void ); ObjectID m_tower; };
class BridgeTowerBehavior { public: static BridgeTowerBehaviorInterface *getBridgeTowerBehaviorInterfaceFromObject( Object *obj ); };
enum BridgeTowerType { BRIDGE_TOWER_FROM_LEFT = 0, BRIDGE_TOWER_FROM_RIGHT, BRIDGE_TOWER_TO_LEFT, BRIDGE_TOWER_TO_RIGHT, BRIDGE_MAX_TOWERS };
class BridgeBehaviorInterface
{
public:
	ObjectID getTowerID( BridgeTowerType towerType );
	Bool isScaffoldInMotion( void );
	void createScaffolding( void );
	void removeScaffolding( void );
	ObjectID m_bridge;
};
class BridgeBehavior { public: static BridgeBehaviorInterface *getBridgeBehaviorInterfaceFromObject( Object *obj ); };
class CreateModuleInterface { public: void onBuildComplete(); int m_index; };
class InGameUI
{
public:
	void message( const char *stringToLookup );
	void addIdleWorker( Object *obj );
	void removeIdleWorker( Object *obj, Int playerNumber );
};
extern InGameUI *TheInGameUI;
enum CanMakeType { CANMAKE_OK, CANMAKE_NO_PREREQ, CANMAKE_NO_MONEY, CANMAKE_FACTORY_IS_DISABLED, CANMAKE_QUEUE_FULL, CANMAKE_PARKING_PLACES_FULL,
	CANMAKE_MAXED_OUT_FOR_PLAYER };
enum LegalBuildCode { LBC_OK = 0, LBC_RESTRICTED_TERRAIN, LBC_NOT_FLAT_ENOUGH, LBC_OBJECTS_IN_THE_WAY, LBC_NO_CLEAR_PATH, LBC_SHROUD,
	LBC_TOO_CLOSE_TO_SUPPLIES };
class BuildAssistant
{
public:
	enum { TERRAIN_RESTRICTIONS = 0x00000001, CLEAR_PATH = 0x00000002, NO_OBJECT_OVERLAP = 0x00000004, USE_QUICK_PATHFIND = 0x00000008,
		SHROUD_REVEALED = 0x00000010, IGNORE_STEALTHED = 0x00000020, FAIL_STEALTHED_WITHOUT_FEEDBACK = 0x00000040 };
	CanMakeType canMakeUnit( Object *builder, const ThingTemplate *whatToBuild ) const;
	LegalBuildCode isLocationLegalToBuild( const Coord3D *worldPos, const ThingTemplate *build, Real angle, UnsignedInt options,
		Object *builderObject, Player *player );
};
extern BuildAssistant *TheBuildAssistant;
enum { CONSTRUCTION_COMPLETE = -1 };
inline KindOfMaskType makeKindOfMask(KindOfType k) { KindOfMaskType m; m.set(k); return m; }
#define MAKE_KINDOF_MASK(k) makeKindOfMask(k)
#define KINDOFMASK_NONE KindOfMaskType()
class PartitionFilterAcceptByKindOf : public PartitionFilter
{
public:
	PartitionFilterAcceptByKindOf(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) : m_mustBeSet(mustBeSet), m_mustBeClear(mustBeClear) {}
	KindOfMaskType m_mustBeSet, m_mustBeClear;
};
/* (the bored dozer's search: the scene's repair candidates, in id order) */
class ObjectIterator
{
public:
	Object *first();
	Object *next();
	std::vector<ObjectID> m_ids;
	size_t m_at;
};
class MemoryPoolObjectHolder { public: MemoryPoolObjectHolder( ObjectIterator * ) {} };
void dozerConstructionCompleteUi(Object *dozer, Object *goalObject);
void newStructureSetPosition(Object *obj, const Coord3D *pos);
void newStructureSetOrientation(Object *obj, Real angle);
#include "ai_dozer_h_a38.inc"
class DozerAIUpdateModuleData
{
public:
	DozerAIUpdateModuleData( void );

	Real m_repairHealthPercentPerSecond;	///< how many health points per second the dozer repairs at
	Real m_boredTime;											///< after this many frames, a dozer will try to find something to do on its own
	Real m_boredRange;										///< range the dozers try to auto repair when they're bored
};
static DozerAIUpdateModuleData g_dozerData;
class DozerAIUpdate : public AIUpdateInterface, public DozerAIInterface
{
public:
	static Bool findGoodBuildOrRepairPosition(const Object* me, const Object* target, Coord3D& positionOut);
	static Object* findGoodBuildOrRepairPositionAndTarget(Object* me, Object* target, Coord3D& positionOut);

	DozerAIUpdate( Thing *thing, const ModuleData* moduleData );
	virtual ~DozerAIUpdate( void );
	const DozerAIUpdateModuleData *getDozerAIUpdateModuleData() const { return &g_dozerData; }

	virtual void onDelete( void );

	virtual Real getRepairHealthPerSecond( void ) const;	///< get health to repair per second
	virtual Real getBoredTime( void ) const;							///< how long till we're bored
	virtual Real getBoredRange( void ) const;							///< when we're bored, we look this far away to do things

	virtual Object* construct( const ThingTemplate *what,
														 const Coord3D *pos, Real angle,
														 Player *owningPlayer,
														 Bool isRebuild );								///< construct an object

	virtual DozerTask getMostRecentCommand( void );				///< return task that was most recently issued
	virtual Bool isTaskPending( DozerTask task );					///< is there a desire to do the requested task
	virtual ObjectID getTaskTarget( DozerTask task );			///< get target of task
	virtual Bool isAnyTaskPending( void );								///< is there any dozer task pending
	virtual DozerTask getCurrentTask( void ) const { return m_currentTask; }	///< return the current task we're doing
	virtual void setCurrentTask( DozerTask task ) { m_currentTask = task; }	///< set the current task of the dozer

	virtual Bool getIsRebuild( void ) { return m_isRebuild; }	///< get whether or not this building is a rebuild.

	virtual void newTask( DozerTask task, Object *target );	///< set a desire to do the requrested task
	virtual void cancelTask( DozerTask task );							///< cancel this task from the queue, if it's the current task the dozer will stop working on it

	virtual void internalTaskComplete( DozerTask task );					///< set a dozer task as successfully completed
	virtual void internalCancelTask( DozerTask task );						///< cancel this task from the dozer
	virtual void internalTaskCompleteOrCancelled( DozerTask task );	///< this is called when tasks are cancelled or completed

	virtual const Coord3D* getDockPoint( DozerTask task, DozerDockPoint point );

	virtual void setBuildSubTask( DozerBuildSubTask subTask ) { m_buildSubTask = subTask; };
	virtual DozerBuildSubTask getBuildSubTask( void ) { return m_buildSubTask; }

	virtual UpdateSleepTime update( void );											///< the update entry point

	virtual Bool canAcceptNewRepair( Object *obj );
	virtual void createBridgeScaffolding( Object *bridgeTower );
	virtual void removeBridgeScaffolding( Object *bridgeTower );

	virtual void startBuildingSound( const AudioEventRTS *sound, ObjectID constructionSiteID );
	virtual void finishBuildingSound();

	virtual void aiDoCommand(const AICommandParms* parms);

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

protected:

	virtual void privateRepair( Object *obj, CommandSourceType cmdSource );	///< repair the target
	virtual void privateResumeConstruction( Object *obj, CommandSourceType cmdSource );  ///< resume construction on obj

	struct DozerTaskInfo
	{
		ObjectID m_targetObjectID;				///< target object ID of task
		UnsignedInt m_taskOrderFrame;			///< logic frame we decided we wanted to do this task
	} m_task[ DOZER_NUM_TASKS ];				///< tasks we want to do indexed by DozerTask

	DozerPrimaryStateMachine *m_dozerMachine;  ///< the custom state machine for Dozer behavior
	DozerTask m_currentTask;						///< current task the dozer is attending to (if any)
	AudioEventRTS	m_buildingSound;			///< sound is pulled from the object we are building!
	Bool m_isRebuild;										///< is this a rebuild of a previous building?

	struct DozerDockPointInfo
	{
		Bool valid;						///< this point has been set and is valid
		Coord3D location;			///< WORLD location
	} m_dockPoint[ DOZER_NUM_TASKS ][ DOZER_NUM_DOCK_POINTS ];

	DozerBuildSubTask m_buildSubTask;		///< for building and actually docking for the build

private:

	void createMachines( void );		///< create our behavior machines we need

};
#include "ai_dozer_a38.inc"
/* (the delivery's world's objects, defined with it below) */
static AiTemplateView g_payloadTemplates[2];
static PayloadObject g_payload;
static unsigned g_nextPayloadId = 40;
static GenerateMinefieldBehavior g_minefield;
static SmartBombTargetHomingUpdate g_smartBomb;
#undef ThingTemplate

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
AIStateMachine *g_otherMachine = NULL;	/* batch A23: the other AIs' one machine (see ref_ai_a4_xfer_path) */
AiData g_aiData;
Player g_player;
TeamTemplateInfo g_teamInfo;
TeamPrototype g_teamPrototype;
Team g_team;
Team g_otherTeams[2];	/* batch A10a: the other objects' teams (by the hash of their id and the scene's epoch) */
ContainModuleInterface g_contain;
StealthUpdate g_stealth;
Weapon g_weapon;
/* batch A10a: the weapons by slot (the current weapon is the current slot's: a locked weapon is a slot) */
Weapon g_slotWeapons[3];
DamageInfo g_lastDamage;
UpgradeTemplate g_nationalism, g_fanaticism, g_otherUpgrade;
UpgradeCenter g_upgradeCenter;
HordeUpdateInterface g_horde[3];
BehaviorModule g_modules[3];
BehaviorModule *g_moduleList[4];
ScriptEngine g_scriptEngine;
/* batch A5 */
ActionManager g_actionManager;
ControlBar g_controlBar;
CommandSet g_commandSet;
CommandButton g_buttons[4];
PolygonTrigger g_polygons[8];
AttackPriorityInfo g_attackInfos[2] = { { 0 }, { 1 } };
Waypoint g_waypoints[8];
unsigned char g_moduleDummy[3];
/* the action manager's answer: the hash of the question, the target and the source */
Bool actionAnswer(unsigned kind, const Object *dest, CommandSourceType src);
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
std::vector<unsigned char> g_aiSaved;	/* batch A21: the AI's last save */

}  // namespace

GlobalData *TheGlobalData = &g_globalData;
GameLogic *TheGameLogic = &g_gameLogic;
TerrainLogic *TheTerrainLogic = &g_terrain;
PartitionManager *ThePartitionManager = &g_partition;
AI *TheAI = &g_ai;
UpgradeCenter *TheUpgradeCenter = &g_upgradeCenter;
ScriptEngine *TheScriptEngine = &g_scriptEngine;
ActionManager *TheActionManager = &g_actionManager;
ControlBar *TheControlBar = &g_controlBar;

Pathfinder *AI::pathfinder() { return (Pathfinder *)g_pathfinderMem; }

UnsignedInt GameLogic::getFrame() const { return g_aiEnv.frame; }

/* ---- batch A7: the move sounds (TheAudio's handles count from 1 in each case) */
namespace { AudioHandle g_nextAudioHandle = 1; AudioEventRTS g_sounds[4]; AudioManager g_audio;
/* batch A36: the particle systems' ids count from 1 in each case */
unsigned g_nextParticleId = 1; }
AudioManager *TheAudio = &g_audio;
AudioHandle AudioManager::addAudioEvent(const AudioEventRTS *e)
{
	int k = -1;
	sscanf(e->getEventName().str(), "s%d", &k);
	AudioHandle h = g_nextAudioHandle++;
	L(LOG_AUDIO_ADD, { (unsigned)k, (unsigned)e->getObjectID(), (unsigned)h });
	return h;
}
void AudioManager::removeAudioEvent(AudioHandle h) { L(LOG_AUDIO_REMOVE, { (unsigned)h }); }
static const AudioEventRTS *moveSound(int k)
{
	g_sounds[k] = AudioEventRTS();
	if ((g_aiEnv.sounds >> k) & 1) { AsciiString n; n.format("s%d", k); g_sounds[k].setEventName(n); }
	return &g_sounds[k];
}
const AudioEventRTS *AiTemplateView::getSoundMoveStart() const { return moveSound(0); }
const AudioEventRTS *AiTemplateView::getSoundMoveStartDamaged() const { return moveSound(1); }
const AudioEventRTS *AiTemplateView::getSoundMoveLoop() const { return moveSound(2); }
const AudioEventRTS *AiTemplateView::getSoundMoveLoopDamaged() const { return moveSound(3); }
Real GeometryInfo::getMaxHeightAbovePosition() const { return g_aiEnv.maxHeightAbovePosition; }
FormationID Object::getFormationID(void) const { return (FormationID)g_aiEnv.formationId; }
Real AIGroup::getSpeed(void) { return g_aiEnv.groupSpeed; }

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

Waypoint *TerrainLogic::getWaypointByName(AsciiString name)
{
	int i = -1;
	if (sscanf(name.str(), "wp%d", &i) == 1 && i >= 0 && i < 8) return &g_waypoints[i];
	/* batch A33: the railed transport's paths' ends (waypoint (3 NN) % 8 and (3 NN + 1) % 8) */
	if (sscanf(name.str(), "railStart%d", &i) == 1 && i >= 1 && i <= 32 && ((g_aiEnv.railStartMask >> (i - 1)) & 1)) return &g_waypoints[(3 * i) % 8];
	if (sscanf(name.str(), "railEnd%d", &i) == 1 && i >= 1 && i <= 32 && ((g_aiEnv.railEndMask >> (i - 1)) & 1)) return &g_waypoints[(3 * i + 1) % 8];
	return NULL;
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

/* (batch A36: the missile's distances, defined at the end of this file) */
static Real missileDistance(int kind, unsigned it, const Coord3D *a, const Coord3D *b);
/* PartitionManager::getDistanceSquared(obj, pos, FROM_CENTER_3D) (layer 8, translated): the 3-D distance from the object's centre */
Real PartitionManager::getDistanceSquared(const Object *obj, const Coord3D *pos, DistanceCalculationType dc, Coord3D *vec)
{
	/* (batch A36: FROM_CENTER_2D, the missile's) */
	if (dc == FROM_CENTER_2D) return missileDistance(0, 0, obj->getPosition(), pos);
	/* (batch A38: FROM_BOUNDINGSPHERE_2D, the dozer's: the centres' 2-D, logged) */
	if (dc == FROM_BOUNDINGSPHERE_2D) {
		const Coord3D *q = obj->getPosition();
		Real dx = pos->x - q->x, dy = pos->y - q->y;
		Real d = dx * dx + dy * dy;
		L(LOG_DOZER, { 35u, bitsOf(pos->x), bitsOf(pos->y), bitsOf(d) });
		return d;
	}
	/* (batch A6: FROM_BOUNDINGSPHERE_3D too, computed alike by the stand-in) */
	if (dc != FROM_CENTER_3D && dc != FROM_BOUNDINGSPHERE_3D) abort();
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
Real GeometryInfo::getMinorRadius() const { return g_aiEnv.minorRadius; }
GeometryType GeometryInfo::getGeomType() const { return g_aiEnv.geomBox ? GEOMETRY_BOX : GEOMETRY_SPHERE; }
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
	m_constructionPercent = 0.0f;
	m_constructionPercentSet = FALSE;
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
	/* batch A6 */
	if (t == KINDOF_BRIDGE) return (g_aiEnv.kindMask & 256) != 0;
	/* batch A10a */
	if (t == KINDOF_PORTABLE_STRUCTURE) return (g_aiEnv.kindMask & 512) != 0;
	if (t == KINDOF_CAN_ATTACK) return (g_aiEnv.kindMask & 1024) != 0;
	if (t == KINDOF_MINE) return (g_aiEnv.kindMask & 2048) != 0;
	if (t == KINDOF_SPAWNS_ARE_THE_WEAPONS) return (g_aiEnv.kindMask & 4096) != 0;
	if (t == KINDOF_STRUCTURE) return (g_aiEnv.kindMask & 8192) != 0;
	if (t == KINDOF_AIRCRAFT) return (g_aiEnv.kindMask & 16384) != 0;
	if (t == KINDOF_DONT_AUTO_CRUSH_INFANTRY) return (g_aiEnv.kindMask & 32768) != 0;
	/* batch A15 */
	if (t == KINDOF_CAN_RAPPEL) return (g_aiEnv.kindMask & 65536) != 0;
	/* batch A17 */
	if (t == KINDOF_DRONE) return (g_aiEnv.kindMask & 131072) != 0;
	/* batch A38 */
	if (t == KINDOF_BRIDGE_TOWER) return (g_aiEnv.kindMask & 262144) != 0;
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
	/* batch A14: another object's from the hash */
	if (m_otherId) return p1Pick(p1Hash(73, { (unsigned)m_otherId }), 50u);
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
void GameLogic::destroyObject(Object *obj) { L(LOG_DESTROY_OBJECT, { obj->getID() }); }
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
	g_aiData.m_repulsedDistance = g_aiEnv.repulsedDistance;
	g_aiData.m_skirmishGroupFudgeValue = g_aiEnv.skirmishFudge;
	g_aiData.m_aiCrushesInfantry = g_aiEnv.crushesInfantry != 0;
	/* batch A38 */
	g_aiData.m_aiDozerBoredRadiusModifier = g_aiEnv.dozerReals[3];
	g_aiData.m_guardChaseUnitFrames = g_aiEnv.guardChaseFrames;
	g_aiData.m_guardEnemyScanRate = g_aiEnv.guardScanRate;
	g_aiData.m_guardEnemyReturnScanRate = g_aiEnv.guardReturnScanRate;
	return &g_aiData;
}
Real AI::getAdjustedVisionRangeForObject(const Object *object, Int factorsToConsider)
{
	L(LOG_VISION_RANGE, { (unsigned)object->getID(), (unsigned)factorsToConsider });
	return g_aiEnv.visionRange;
}
Object *AI::findClosestEnemy(const Object *me, Real range, UnsignedInt qualifiers, const AttackPriorityInfo *info, PartitionFilter *optionalFilter)
{
	/* batch A12: the info by index + 1, the polygon filter's polygon + 1 (0: none) */
	unsigned poly = optionalFilter ? (unsigned)((const PartitionFilterPolygonTrigger *)optionalFilter)->m_index + 1u : 0u;
	L(LOG_CLOSEST_ENEMY, { (unsigned)me->getID(), bitsOf(range), qualifiers, info ? (unsigned)info->m_index + 1u : 0u, poly });
	return findObject((ObjectID)g_aiEnv.closestEnemy);
}
/* batch A20: another object's body (its id; 0: the AI's own) */
unsigned bodyOwner(const BodyModuleInterface *body)
{
	for (unsigned k = 1; k < 32; ++k)
		if (body == g_others[k].getBodyModule()) return k;
	return 0;
}
DamageInfo g_otherDamage;
const DamageInfo *BodyModuleInterface::getLastDamageInfo() const
{
	/* batch A20: another object's: a source by hash, the scene's no-effect bit */
	if (unsigned k = bodyOwner(this)) {
		g_otherDamage.in.m_damageType = (DamageType)0;
		g_otherDamage.in.m_sourceID = (ObjectID)(p1Hash(73, { k }) % 32u);
		g_otherDamage.out.m_noEffect = ((g_aiEnv.tunnelNoEffect >> k) & 1u) != 0;
		return &g_otherDamage;
	}
	g_lastDamage.in.m_damageType = (DamageType)g_aiEnv.lastDamageType;
	g_lastDamage.in.m_sourceID = (ObjectID)g_aiEnv.lastDamageSource;
	g_lastDamage.out.m_noEffect = false;
	return &g_lastDamage;
}
UnsignedInt BodyModuleInterface::getLastDamageTimestamp() const
{
	unsigned k = bodyOwner(this);
	return k ? g_aiEnv.tunnelStamp + p1Hash(74, { k }) % 64u : 0u;
}
/* batch A20: the tunnel network (the owner's container, if any, then the scene's tunnels) */
AiTunnelTracker g_tunnels;
std::list<ObjectID> g_tunnelList;
TunnelTracker *Player::getTunnelSystem(void) { return &g_tunnels; }
Object *TunnelTracker::getCurNemesis(void) { return findObject((ObjectID)g_aiEnv.tunnelNemesis); }
void TunnelTracker::updateNemesis(const Object *target) { L(LOG_TUNNEL, { 0u, target ? (unsigned)target->getID() : 0u }); }
const std::list<ObjectID> *TunnelTracker::getContainerList() const
{
	g_tunnelList.clear();
	if (g_aiEnv.containerId) g_tunnelList.push_back((ObjectID)g_aiEnv.containerId);
	for (int i = 0; i < 4; ++i)
		if (g_aiEnv.tunnelIds[i]) g_tunnelList.push_back((ObjectID)g_aiEnv.tunnelIds[i]);
	return &g_tunnelList;
}
Object *otherAiGoalObject() { return findObject((ObjectID)g_aiEnv.otherAiGoal); }
Bool isOtherAi(const AIUpdateInterface *ai) { return ai == (const AIUpdateInterface *)g_otherAiMem; }
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
void Team::setActive() { L(LOG_TEAM_ACTIVE, {}); }
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
Team *Object::getTeam()
{
	if (!m_otherId) return &g_team;
	return &g_otherTeams[(p1Hash(68, { (unsigned)m_otherId, (unsigned)g_aiEnv.teamEpoch }) >> 8) & 1];
}
CanAttackResult Object::getAbleToAttackSpecificObject(AbleToAttackType t, const Object *target, CommandSourceType commandSource) const
{
	unsigned h = p1Hash(9, { (unsigned)t, (unsigned)target->getID(), (unsigned)commandSource });
	CanAttackResult r = p1Pick(h, g_aiEnv.ableRate) ? ATTACKRESULT_POSSIBLE
		: p1Pick(p1Hash(10, { h }), g_aiEnv.ableRate) ? ATTACKRESULT_POSSIBLE_AFTER_MOVING : ATTACKRESULT_NOT_POSSIBLE;
	L(LOG_ABLE_TO_ATTACK, { (unsigned)t, (unsigned)target->getID(), (unsigned)commandSource, (unsigned)r });
	return r;
}
Object *Object::getContainedBy() { return m_otherId ? NULL : findObject((ObjectID)g_aiEnv.containerId); }
/* (batch A5: the AI's own object has one if the scene says so; another object always has one) */
ContainModuleInterface *Object::getContain() const { return (m_otherId || g_aiEnv.hasContain) ? &g_contain : NULL; }
StealthUpdate *Object::getStealth() const { return g_aiEnv.stealth >= 0 ? &g_stealth : NULL; }
Weapon *Object::getCurrentWeapon() { return g_aiEnv.weaponAnti >= 0 ? &g_slotWeapons[g_aiEnv.curSlot] : NULL; }
const Weapon *Object::getCurrentWeapon() const { return g_aiEnv.weaponAnti >= 0 ? &g_slotWeapons[g_aiEnv.curSlot] : NULL; }
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
/* (batch A7: the const overload, as Object.h's) */
const AIUpdateInterface *Object::getAI() const { return const_cast<Object *>(this)->getAI(); }
AIUpdateInterface *Object::getAI()
{
	if (!m_otherId) return g_ai_update;
	return ((g_aiEnv.aiMask >> m_otherId) & 1) ? (AIUpdateInterface *)g_otherAiMem : NULL;
}

/* ---- batch A4's stand-ins */
Real Object::getVisionRange() const { return g_aiEnv.objVisionRange; }
namespace {
Bool actionAnswer(unsigned kind, const Object *dest, CommandSourceType src)
{
	unsigned id = dest ? (unsigned)dest->getID() : 0u;
	bool r = p1Pick(p1Hash(13, { kind, id, (unsigned)src }), g_aiEnv.actionRate);
	L(LOG_ACTION, { kind, id, (unsigned)src, r ? 1u : 0u });
	return r;
}
}  // namespace
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
/* batch A31: DeployStyleAIUpdate's reach: the drawable's animation frame (LOG_ANIM_FRAME; the object has a drawable when the scene
** says so), the cleared model conditions (LOG_CLEAR_MODEL_FLAGS, in increasing order), the per-unit sounds ("Deploy" is "s4",
** "Undeploy" "s5", when the scene's sound bits 4 and 5 say so; else the empty sound, an unknown name's) */
namespace { Drawable g_drawable; AudioEventRTS g_unitSound; }
/* (batch A34: a hacker always has one: its states use it unchecked) */
/* (batch A37: a delivering AI's object when the scene says so) */
Drawable *Object::getDrawable() const
{
	return (g_aiEnv.subclass == 6 || ((g_aiEnv.deployFlags >> 4) & 1) || (g_aiEnv.subclass == 9 && (g_aiEnv.deliverFlags & 1))) ? &g_drawable : NULL;
}
void Drawable::setAnimationFrame(int frame) { L(LOG_ANIM_FRAME, { (unsigned)frame }); }
void Object::clearModelConditionFlags(const ModelConditionFlags &clr)
{
	std::vector<unsigned> w;
	for (Int i = 0; i < MODELCONDITION_COUNT; ++i) if (clr.test(i)) w.push_back((unsigned)i);
	if (g_aiQuiet) return;
	g_aiLog->push_back((unsigned)LOG_CLEAR_MODEL_FLAGS);
	g_aiLog->push_back((unsigned)w.size());
	for (unsigned x : w) g_aiLog->push_back(x);
}
const AudioEventRTS *AiTemplateView::getPerUnitSound(const AsciiString& soundName) const
{
	g_unitSound = AudioEventRTS();
	/* (batch A34: "UnitUnpack" is "s4" too, "UnitPack" "s5", "UnitCashPing" "s6") */
	const char *n0 = soundName.str();
	/* (batch A37: "StartDive" is "s8") */
	int k = (strcmp(n0, "Deploy") == 0 || strcmp(n0, "UnitUnpack") == 0) ? 4
		: (strcmp(n0, "Undeploy") == 0 || strcmp(n0, "UnitPack") == 0) ? 5 : strcmp(n0, "UnitCashPing") == 0 ? 6
		: strcmp(n0, "StartDive") == 0 ? 8 : strcmp(n0, "UnderConstruction") == 0 ? 9 : -1;
	if (k >= 0 && ((g_aiEnv.sounds >> k) & 1)) { AsciiString n; n.format("s%d", k); g_unitSound.setEventName(n); }
	return &g_unitSound;
}
/* ---- batch A5's stand-ins (the commands' reach) */
Bool Object::isMobile() const { return g_aiEnv.mobile != 0; }
void TerrainLogic::getExtent(Region3D *r) const
{
	r->lo.x = 0.0f; r->lo.y = 0.0f; r->lo.z = 0.0f;
	r->hi.x = g_aiEnv.extentHi[0]; r->hi.y = g_aiEnv.extentHi[1]; r->hi.z = 0.0f;
}
/* the closest point of the extent's edge (the stand-in: clamped into the extent) */
Coord3D TerrainLogic::findClosestEdgePoint(const Coord3D *pos) const
{
	L(LOG_EDGE_POINT, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	Coord3D r = *pos;
	if (r.x < 0.0f) r.x = 0.0f;
	if (r.x > g_aiEnv.extentHi[0]) r.x = g_aiEnv.extentHi[0];
	if (r.y < 0.0f) r.y = 0.0f;
	if (r.y > g_aiEnv.extentHi[1]) r.y = g_aiEnv.extentHi[1];
	return r;
}
Object *Object::dlink_next_TeamMemberList() const
{
	for (unsigned id = (unsigned)m_otherId + 1; id < 32; ++id)
		if ((g_aiEnv.teamMask >> id) & 1) { g_others[id].m_otherId = (ObjectID)id; return &g_others[id]; }
	return NULL;
}
DLINK_ITERATOR<Object> Team::iterate_TeamMemberList() const
{
	Object *head = NULL;
	for (unsigned id = 1; id < 32 && !head; ++id)
		if ((g_aiEnv.teamMask >> id) & 1) { g_others[id].m_otherId = (ObjectID)id; head = &g_others[id]; }
	return DLINK_ITERATOR<Object>(head, &Object::dlink_next_TeamMemberList);
}
Bool Object::isSelectable() const { return TRUE; }
const std::vector<ObjectID>& AIGroup::getAllIDs() const { static std::vector<ObjectID> none; return none; }
void AIGroup::add(Object *obj) { abort(); }
Bool ActionManager::canGetRepairedAt(const Object *obj, const Object *dest, CommandSourceType src)
{
	return actionAnswer(0, dest, src);
}
Bool ActionManager::canGetHealedAt(const Object *obj, const Object *dest, CommandSourceType src)
{
	return actionAnswer(1, dest, src);
}
Bool ActionManager::canEnterObject(const Object *obj, const Object *dest, CommandSourceType src, Int mode)
{
	return actionAnswer(2, dest, src);
}
/* batch A14: possible (kind 3), else possible after moving (kind 4), else not */
CanAttackResult ActionManager::getCanAttackObject(const Object *obj, const Object *dest, CommandSourceType src, AbleToAttackType attackType)
{
	if (actionAnswer(3, dest, src)) return ATTACKRESULT_POSSIBLE;
	return actionAnswer(4, dest, src) ? ATTACKRESULT_POSSIBLE_AFTER_MOVING : ATTACKRESULT_NOT_POSSIBLE;
}
/* another AI's path: getMoveAwayFromPath's stand-in makes a two-node path (from the object to an offset) at the scene's rate */
Path *Pathfinder::getMoveAwayFromPath(Object *obj, Object *otherObj, Path *pathToAvoid, Object *otherObj2, Path *pathToAvoid2)
{
	unsigned canPath = g_ai_update->m_canPathThroughUnits ? 1u : 0u;
	unsigned id2 = otherObj2 ? (unsigned)otherObj2->getID() : 0u;
	unsigned h = p1Hash(14, { (unsigned)otherObj->getID(), id2, pathToAvoid2 ? 1u : 0u, canPath });
	bool found = p1Pick(h, g_aiEnv.moveAwayRate);
	L(LOG_MOVE_AWAY_PATH, { (unsigned)otherObj->getID(), id2, pathToAvoid2 ? 1u : 0u, canPath, found ? 1u : 0u });
	if (!found) return NULL;
	Path *path = newInstance(Path);
	Coord3D a = *obj->getPosition();
	Coord3D b = a;
	b.x += (Real)((Int)((h >> 12) % 41u) - 20);
	b.y += (Real)((Int)((h >> 20) % 41u) - 20);
	path->appendNode(&a, LAYER_GROUND);
	path->appendNode(&b, LAYER_GROUND);
	return path;
}
Bool Pathfinder::moveAllies(Object *obj, Path *path)
{
	unsigned count = 0;
	for (const PathNode *n = path->getFirstNode(); n; n = n->getNext()) ++count;
	L(LOG_MOVE_ALLIES, { count });
	return TRUE;
}
const AsciiString& Object::getName() const { return AsciiString::TheEmptyString; }
const AsciiString& AiTemplateView::getName() const { return AsciiString::TheEmptyString; }
Bool Pathfinder::clientSafeQuickDoesPathExist(const LocomotorSet& locomotorSet, const Coord3D *from, const Coord3D *to)
{
	bool r = p1Pick(p1Hash(15, { bitsOf(to->x), bitsOf(to->y) }), g_aiEnv.pathExistsRate);
	L(LOG_PATH_EXISTS, { bitsOf(to->x), bitsOf(to->y), bitsOf(to->z), r ? 1u : 0u });
	return r;
}
Int g_maxShots = 0;	/* batch A10a: the current weapon's max shot count */
void Weapon::setMaxShotCount(Int maxShots) { L(LOG_MAX_SHOTS, { (unsigned)maxShots }); g_maxShots = maxShots; }
Real Weapon::getContinueAttackRange() const { return g_aiEnv.continueRange; }
Bool Weapon::isContactWeapon() const { return g_aiEnv.contactWeapon != 0; }
Object *PartitionManager::getClosestObject(const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters,
	Real *closestDist, Coord3D *closestDistVec)
{
	/* batch A18: the guard's search (filters other than the attack-position command's): logged with what they are */
	if (!dynamic_cast<const PartitionFilterPossibleToAttack *>(filters[0])) {
		unsigned mask = 0, poly = 0;
		for (int i = 0; filters[i]; ++i) {
			const PartitionFilterRelationship *rel = dynamic_cast<const PartitionFilterRelationship *>(filters[i]);
			if (rel) mask |= (rel->m_flags == PartitionFilterRelationship::ALLOW_ENEMIES) ? 1u : 32u;
			else if (dynamic_cast<const PartitionFilterPossibleToAttack *>(filters[i])) mask |= 2u;
			else if (dynamic_cast<const PartitionFilterSameMapStatus *>(filters[i])) mask |= 4u;
			else if (const PartitionFilterPolygonTrigger *pt = dynamic_cast<const PartitionFilterPolygonTrigger *>(filters[i])) { mask |= 8u; poly = (unsigned)pt->m_index + 1u; }
			else if (dynamic_cast<const PartitionFilterIsFlying *>(filters[i])) mask |= 16u;
			else if (dynamic_cast<const PartitionFilterPossibleToEnter *>(filters[i])) mask |= 64u;
			else if (dynamic_cast<const PartitionFilterPossibleToHijack *>(filters[i])) mask |= 128u;
			else if (dynamic_cast<const PartitionFilterRejectBuildings *>(filters[i])) mask |= 256u;
		}
		L(LOG_CLOSEST_OBJECT, { 0xfffffffeu, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(maxDist), (unsigned)dc, mask, poly });
		return findObject((ObjectID)g_aiEnv.closestAttack);
	}
	const PartitionFilterPossibleToAttack *f = (const PartitionFilterPossibleToAttack *)filters[0];
	L(LOG_CLOSEST_OBJECT, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(maxDist), (unsigned)dc, (unsigned)f->m_commandSource });
	return findObject((ObjectID)g_aiEnv.closestAttack);
}
/* batch A16: the squad's existing member with the smallest hash (the first of equals), logged with the owner and the answer */
Object *PartitionManager::getClosestObject(const Object *obj, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters,
	Real *closestDist, Coord3D *closestDistVec)
{
	/* (batch A38: the bored dozer's mine: the scene's attack answer, logged with the filters) */
	if (dynamic_cast<const PartitionFilterRelationship *>(filters[0])) {
		const PartitionFilterRelationship *rel = (const PartitionFilterRelationship *)filters[0];
		const PartitionFilterPossibleToAttack *att = dynamic_cast<const PartitionFilterPossibleToAttack *>(filters[1]);
		Object *o = findObject((ObjectID)g_aiEnv.closestAttack);
		L(LOG_DOZER, { 34u, (unsigned)obj->getID(), bitsOf(maxDist), (unsigned)dc, (unsigned)rel->m_flags, att ? (unsigned)att->m_commandSource : 0xffffffffu,
			o ? (unsigned)o->getID() : 0u });
		return o;
	}
	const PartitionFilterAcceptOnSquad *f = (const PartitionFilterAcceptOnSquad *)filters[0];
	Object *best = NULL;
	unsigned bestHash = 0;
	for (size_t i = 0; i < f->m_squad->m_objectIDs.size(); ++i) {
		Object *o = findObject(f->m_squad->m_objectIDs[i]);
		if (!o) continue;
		unsigned h = p1Hash(75, { (unsigned)o->getID() });
		if (!best || h < bestHash) { best = o; bestHash = h; }
	}
	L(LOG_CLOSEST_OBJECT, { 0xffffffffu, (unsigned)obj->getID(), bitsOf(maxDist), (unsigned)dc, best ? (unsigned)best->getID() : 0u });
	return best;
}
Bool PartitionManager::findPositionAround(const Coord3D *center, const FindPositionOptions *options, Coord3D *result)
{
	bool r = p1Pick(p1Hash(16, { bitsOf(center->x), bitsOf(center->y) }), g_aiEnv.findPosRate);
	L(LOG_FIND_POSITION, { bitsOf(center->x), bitsOf(center->y), bitsOf(center->z), bitsOf(options->minRadius), bitsOf(options->maxRadius),
		options->sourceToPathToDest ? (unsigned)options->sourceToPathToDest->getID() : 0u, r ? 1u : 0u });
	/* (batch A38: the start angle, the z delta, the object ignored, when not the defaults: the dozer's) */
	if (options->startAngle != RANDOM_START_ANGLE || options->maxZDelta != 1e10f || options->ignoreObject)
		L(LOG_DOZER, { 33u, bitsOf(options->startAngle), bitsOf(options->maxZDelta), options->ignoreObject ? (unsigned)options->ignoreObject->getID() : 0u });
	if (r) { *result = *center; result->x += 5.0f; result->y += 5.0f; }
	return r;
}
/* batch A15: the existing objects without an AI of the scene's mask, in id order (A5's passengers' aiIdle reaches no AI) */
/* batch A30: for the transport, those with an AI too, and the list even when empty (the real contain's) */
const ContainedItemsList* ContainModuleInterface::getContainedItemsList() const
{
	static ContainedItemsList items;
	items.clear();
	Bool transport = g_aiEnv.subclass == 2;
	/* batch A32: for the assault transport, those with an AI only (its members' AIs are reached unchecked upstream), the list
	** even when empty */
	Bool assault = g_aiEnv.subclass == 4;
	/* batch A37: for the delivering AI, those with an AI too, the list even when empty */
	Bool deliver = g_aiEnv.subclass == 9;
	for (unsigned i = 1; i < 32; ++i) {
		if (!((g_aiEnv.containedMask >> i) & 1)) continue;
		Bool hasAi = ((g_aiEnv.aiMask >> i) & 1) != 0;
		if (assault ? !hasAi : (!transport && !deliver && hasAi)) continue;
		Object *o = findObject((ObjectID)i);
		if (o) items.push_back(o);
	}
	return items.empty() && !transport && !assault && !deliver ? NULL : &items;
}
/* batch A15: LOG_RAPPEL's kinds: 0 resetDynamicPhysics, 1 scrubVelocityZ, 2 setLayer, 3 scoreTheKill, 4 kill, 5 doFXObj,
** 6 isValidContainerFor, 7 removeFromContain */
Bool ContainModuleInterface::isValidContainerFor(const Object *obj, Bool checkCapacity) const
{
	bool r = p1Pick(p1Hash(74, { (unsigned)obj->getID(), (unsigned)checkCapacity }), 50u);
	L(LOG_RAPPEL, { 6u, (unsigned)checkCapacity, r ? 1u : 0u });
	return r;
}
void ContainModuleInterface::removeFromContain(Object *obj, Bool exposeStealthUnits) { L(LOG_RAPPEL, { 7u, (unsigned)obj->getID() }); }
void PhysicsBehavior::resetDynamicPhysics() { L(LOG_RAPPEL, { 0u }); }
void PhysicsBehavior::scrubVelocityZ(Real desiredVelocity) { L(LOG_RAPPEL, { 1u, bitsOf(desiredVelocity) }); }
void Object::setLayer(PathfindLayerEnum layer) { L(LOG_RAPPEL, { 2u, (unsigned)layer }); }
void Object::scoreTheKill(const Object *victim) { L(LOG_RAPPEL, { 3u, (unsigned)victim->getID() }); }
void Object::kill() { L(LOG_RAPPEL, { 4u, (unsigned)getID() }); }
/* batch A17: LOG_DOCK's kinds: 0 cancelDock, 1 reserveApproachPosition, 2 advanceApproachPosition, 3 isClearToEnter,
** 4 isClearToAdvance, 5 getEnterPosition, 6 getDockPosition, 7 getExitPosition, 8 onApproachReached, 9 onEnterReached,
** 10 onDockReached, 11 onExitReached, 12 action */
DockUpdateInterface g_dock;
namespace {
bool dockAnswer(unsigned kind, std::initializer_list<unsigned> words)
{
	return p1Pick(p1Hash(80 + kind, words), g_aiEnv.dockRate);
}
void dockPosition(unsigned kind, Coord3D *position)
{
	position->x = g_aiEnv.dockPos[0] + 0.1f * (Real)(kind - 4);
	position->y = g_aiEnv.dockPos[1];
	position->z = g_aiEnv.dockPos[2];
	L(LOG_DOCK, { kind });
}
}  // namespace
DockUpdateInterface *Object::getDockUpdateInterface()
{
	return (m_otherId && ((g_aiEnv.dockMask >> m_otherId) & 1)) ? &g_dock : NULL;
}
ObjectID Object::getProducerID() const
{
	return (m_otherId && ((g_aiEnv.droneMask >> m_otherId) & 1)) ? (ObjectID)g_aiEnv.objectId : INVALID_ID;
}
extern ExitInterface g_exit;
ExitInterface *Object::getObjectExitInterface() const { return (g_aiEnv.dockFlags & 8) ? &g_exit : NULL; }
Coord3D g_rallyPoint;
const Coord3D *ExitInterface::getRallyPoint() const
{
	if (!(g_aiEnv.dockFlags & 16)) return NULL;
	g_rallyPoint.x = g_aiEnv.dockPos[0] - 15.0f; g_rallyPoint.y = g_aiEnv.dockPos[1] + 5.0f; g_rallyPoint.z = g_aiEnv.dockPos[2];
	return &g_rallyPoint;
}
void Player::iterateObjects(void (*func)(Object *obj, void *userData), void *userData) const
{
	for (unsigned i = 1; i < 32; ++i) { Object *o = findObject((ObjectID)i); if (o) func(o, userData); }
}
Bool DockUpdateInterface::reserveApproachPosition(Object *docker, Coord3D *position, Int *index)
{
	bool r = dockAnswer(1, { TheGameLogic->getFrame() });
	if (r) { position->x = g_aiEnv.dockPos[0]; position->y = g_aiEnv.dockPos[1]; position->z = g_aiEnv.dockPos[2]; *index = g_aiEnv.dockIndex; }
	L(LOG_DOCK, { 1u, r ? 1u : 0u });
	return r;
}
Bool DockUpdateInterface::advanceApproachPosition(Object *docker, Coord3D *position, Int *index)
{
	bool r = dockAnswer(2, { TheGameLogic->getFrame() });
	if (r) { position->x = g_aiEnv.dockPos[0]; position->y = g_aiEnv.dockPos[1]; position->z = g_aiEnv.dockPos[2]; *index = g_aiEnv.dockIndex; }
	L(LOG_DOCK, { 2u, r ? 1u : 0u });
	return r;
}
Bool DockUpdateInterface::isClearToEnter(Object const *docker) const
{
	bool r = dockAnswer(3, { TheGameLogic->getFrame() });
	L(LOG_DOCK, { 3u, r ? 1u : 0u });
	return r;
}
Bool DockUpdateInterface::isClearToAdvance(Object const *docker, Int dockerIndex) const
{
	bool r = dockAnswer(4, { TheGameLogic->getFrame(), (unsigned)dockerIndex });
	L(LOG_DOCK, { 4u, (unsigned)dockerIndex, r ? 1u : 0u });
	return r;
}
void DockUpdateInterface::getEnterPosition(Object *docker, Coord3D *position) { dockPosition(5, position); }
void DockUpdateInterface::getDockPosition(Object *docker, Coord3D *position) { dockPosition(6, position); }
void DockUpdateInterface::getExitPosition(Object *docker, Coord3D *position) { dockPosition(7, position); }
void DockUpdateInterface::onApproachReached(Object *docker) { L(LOG_DOCK, { 8u }); }
void DockUpdateInterface::onEnterReached(Object *docker) { L(LOG_DOCK, { 9u }); }
void DockUpdateInterface::onDockReached(Object *docker) { L(LOG_DOCK, { 10u }); }
void DockUpdateInterface::onExitReached(Object *docker) { L(LOG_DOCK, { 11u }); }
Bool DockUpdateInterface::action(Object *docker, Object *drone)
{
	unsigned d = drone ? (unsigned)drone->getID() : 0u;
	bool r = dockAnswer(12, { TheGameLogic->getFrame(), d });
	L(LOG_DOCK, { 12u, d, r ? 1u : 0u });
	return r;
}
void DockUpdateInterface::cancelDock(Object *docker) { L(LOG_DOCK, { 0u }); }
Bool DockUpdateInterface::isDockOpen() { return (g_aiEnv.dockFlags & 1) != 0; }
Bool DockUpdateInterface::isAllowPassthroughType() { return (g_aiEnv.dockFlags & 2) != 0; }
Bool DockUpdateInterface::isRallyPointAfterDockType() { return (g_aiEnv.dockFlags & 4) != 0; }
void FXList::doFXObj(const FXList *fx, const Object *primary, const Object *secondary) { L(LOG_RAPPEL, { 5u, (unsigned)primary->getID() }); }
void ContainModuleInterface::removeAllContained(Bool exposeStealthUnits) { L(LOG_CONTAIN, { 0u, (unsigned)exposeStealthUnits }); }
void ContainModuleInterface::markAllPassengersDetected() { L(LOG_CONTAIN, { 1u }); }
/* batch A13: LOG_CONTAIN 3 onObjectWantsToEnterOrExit, 4 getAiFreeToExit, 5 reserveDoorForExit, 6 exitObjectViaDoor */
ExitInterface g_exit;
void ContainModuleInterface::onObjectWantsToEnterOrExit(Object *obj, ObjectEnterExitType wants)
{
	L(LOG_CONTAIN, { 3u, (unsigned)obj->getID(), (unsigned)wants });
}
void ContainModuleInterface::addToContain(Object *obj) { L(LOG_CONTAIN, { 7u, (unsigned)obj->getID() }); }
ExitInterface *ContainModuleInterface::getContainExitInterface() { return (g_aiEnv.exitFlags & 1) ? &g_exit : NULL; }
Bool ExitInterface::isExitBusy() const { return (g_aiEnv.exitFlags & 2) != 0; }
ExitDoorType ExitInterface::reserveDoorForExit(const AiTemplateView *objType, Object *specificObject)
{
	L(LOG_CONTAIN, { 5u, (unsigned)specificObject->getID(), (unsigned)g_aiEnv.exitDoor });
	return (ExitDoorType)g_aiEnv.exitDoor;
}
void ExitInterface::exitObjectViaDoor(Object *newObj, ExitDoorType exitDoor)
{
	L(LOG_CONTAIN, { 6u, (unsigned)newObj->getID(), (unsigned)exitDoor });
}
void ExitInterface::exitObjectInAHurry(Object *newObj) { L(LOG_CONTAIN, { 8u, (unsigned)newObj->getID() }); }
AIFreeToExitType AIUpdateInterface::getAiFreeToExit(const Object* exiter) const
{
	unsigned r = p1Hash(71, { (unsigned)exiter->getID(), TheGameLogic->getFrame() }) % 3u;
	L(LOG_CONTAIN, { 4u, (unsigned)exiter->getID(), r });
	return (AIFreeToExitType)r;
}
void ContainModuleInterface::orderAllPassengersToExit(CommandSourceType src, Bool instantly) { L(LOG_CONTAIN, { 2u, (unsigned)src, (unsigned)instantly }); }
UpdateModule *Object::findUpdateModule(UnsignedInt key) const
{
	/* batch A35: a dock's supply modules (another object's: the scene's masks) */
	static const char *const supplyNames[2] = { "SupplyWarehouseDockUpdate", "SupplyCenterDockUpdate" };
	for (int i = 0; i < 2; ++i)
		if (key == (UnsignedInt)TheNameKeyGenerator->nameToKey(AsciiString(supplyNames[i])))
			return (m_otherId && ((g_aiEnv.supplyDockMasks[i] >> m_otherId) & 1)) ? (UpdateModule *)&g_moduleDummy[0] : NULL;
	/* batch A37: a delivered item's minefield and smart bomb modules (another object's: the scene's masks) */
	static const char *const itemNames[2] = { "GenerateMinefieldBehavior", "SmartBombTargetHomingUpdate" };
	for (int i = 0; i < 2; ++i)
		if (key == (UnsignedInt)TheNameKeyGenerator->nameToKey(AsciiString(itemNames[i]))) {
			if (!m_otherId || !((g_aiEnv.deliverModuleMasks[i] >> m_otherId) & 1)) return NULL;
			if (i == 0) { g_minefield.m_id = m_otherId; return (UpdateModule *)(void *)&g_minefield; }
			g_smartBomb.m_id = m_otherId;
			return (UpdateModule *)(void *)&g_smartBomb;
		}
	static const char *const names[3] = { "DeliverPayloadAIUpdate", "ProneUpdate", "HackInternetAIUpdate" };
	for (int i = 0; i < 3; ++i)
		if (key == (UnsignedInt)TheNameKeyGenerator->nameToKey(AsciiString(names[i]))) {
			/* (batch A34: the HackInternetAIUpdate is the hacker's own AI, and none another class's) */
			if (i == 2) return g_aiEnv.subclass == 6 ? (UpdateModule *)g_ai_update : NULL;
			/* (batch A37: the DeliverPayloadAIUpdate likewise the delivering AI's own; the scene's module bit 0 is unused) */
			if (i == 0) return (!m_otherId && g_aiEnv.subclass == 9) ? (UpdateModule *)g_ai_update : NULL;
			return ((g_aiEnv.modules >> i) & 1) ? (UpdateModule *)&g_moduleDummy[i] : NULL;
		}
	return NULL;
}
void ProneUpdate::goProne(const DamageInfo *damageInfo) { L(LOG_MODULE, { 1u }); }
const AsciiString& Object::getCommandSetString() const { static AsciiString name("AiCommandSet"); return name; }
const CommandSet *ControlBar::findCommandSet(const AsciiString& name) { return g_aiEnv.hasCommandSet ? &g_commandSet : NULL; }
const CommandButton* CommandSet::getCommandButton(Int i) const
{
	Int k = g_aiEnv.commandSlots[i];
	return k >= 0 ? &g_buttons[k] : NULL;
}
Int CommandButton::getCommandType() const { return g_aiEnv.buttonTypes[m_index]; }
/* ---- batch A6's stand-ins (the paths) */
const Coord3D *Waypoint::getLocation( void ) const { return &m_location; }
Waypoint *Waypoint::getLink(Int ndx) const
{
	if (ndx == 0 && m_index < 7 && ((g_aiEnv.waypointLinks >> m_index) & 1)) return &g_waypoints[m_index + 1];
	if (ndx == 1 && getNumLinks() == 2) return &g_waypoints[(m_index + 5) % 8];
	return NULL;
}
/* ---- batch A9's stand-ins (the waypoint states) */
Int Waypoint::getNumLinks(void) const
{
	if (!(m_index < 7 && ((g_aiEnv.waypointLinks >> m_index) & 1))) return 0;
	return 1 + (Int)((g_aiEnv.waypointLinks2 >> m_index) & 1);
}
Waypoint *TerrainLogic::getWaypointByID(UnsignedInt id) { return id < 8 ? &g_waypoints[id] : NULL; }
const Waypoint *g_teamWaypoint = NULL;
void Team::setCurrentWaypoint(const Waypoint *way)
{
	L(LOG_TEAM_WAYPOINT, { way ? (unsigned)way->m_index : 0xffffffffu });
	g_teamWaypoint = way;
}
const Waypoint *Team::getCurrentWaypoint() { return g_teamWaypoint; }
AIGroup g_teamGroup;
AIGroup *AI::createGroup(void) { g_teamGroup.m_team = 1; return &g_teamGroup; }
void Team::getTeamAsAIGroup(AIGroup *group) { L(LOG_TEAM_GROUP, {}); }
void AIGroup::getCenter(Coord3D *center)
{
	/* the team's: the object's position moved by the scene's offset (so that the skirmish bail-out's distance is near its limit) */
	if (m_team) {
		const Coord3D *p = g_object.getPosition();
		center->x = p->x + g_aiEnv.teamCenter[0]; center->y = p->y + g_aiEnv.teamCenter[1]; center->z = p->z + g_aiEnv.teamCenter[2];
	} else {
		center->x = g_aiEnv.groupCenter[0]; center->y = g_aiEnv.groupCenter[1]; center->z = g_aiEnv.groupCenter[2];
	}
	L(LOG_GROUP_CENTER, { (unsigned)m_team, bitsOf(center->x), bitsOf(center->y), bitsOf(center->z) });
}
Int AIGroup::getCount(void) { return m_team ? g_aiEnv.teamCount : 0; }
Bool Player::getUnitsShouldHunt() const { return g_aiEnv.unitsHunt != 0; }
GameDifficulty Player::getPlayerDifficulty(void) const { return (GameDifficulty)g_aiEnv.difficulty; }
Bool ScriptEngine::getChooseVictimAlwaysUsesNormal() const { return g_aiEnv.chooseNormal != 0; }
Bool Player::isSkirmishAIPlayer(void) { L(LOG_SKIRMISH, { (unsigned)g_aiEnv.skirmish }); return g_aiEnv.skirmish != 0; }
void TerrainLogic::getMaximumPathfindExtent(Region3D *r) const
{
	r->lo.x = 0.0f; r->lo.y = 0.0f; r->lo.z = 0.0f;
	r->hi.x = g_aiEnv.pfExtentHi[0]; r->hi.y = g_aiEnv.pfExtentHi[1]; r->hi.z = 0.0f;
}
void TerrainLogic::getBridgeAttackPoints(const Object *bridge, TBridgeAttackInfo *info)
{
	Real i = (Real)bridge->getID();
	info->attackPoint1.x = 7.0f * i; info->attackPoint1.y = 3.0f * i; info->attackPoint1.z = 0.0f;
	info->attackPoint2.x = 3.0f * i; info->attackPoint2.y = 7.0f * i; info->attackPoint2.z = 0.0f;
}
namespace {
/* a search's path (the stand-in's): from the object to `to` offset by the hash, sometimes blocked by an ally; or none */
Path *standinPath(unsigned kind, const Object *obj, const Coord3D *to, std::initializer_list<unsigned> words)
{
	unsigned h = p1Hash(17 + kind, words);
	bool found = p1Pick(h, g_aiEnv.serviceRate);
	std::vector<unsigned> w(1, kind);
	w.insert(w.end(), words.begin(), words.end());
	w.push_back(found ? 1u : 0u);
	if (!g_aiQuiet) {
		g_aiLog->push_back((unsigned)LOG_SERVICE);
		g_aiLog->push_back((unsigned)w.size());
		for (unsigned x : w) g_aiLog->push_back(x);
	}
	if (!found) return NULL;
	Path *path = newInstance(Path);
	Coord3D a = *obj->getPosition();
	Coord3D b = *to;
	b.x += (Real)((Int)((h >> 12) % 9u) - 4);
	b.y += (Real)((Int)((h >> 16) % 9u) - 4);
	path->appendNode(&a, LAYER_GROUND);
	path->appendNode(&b, LAYER_GROUND);
	if ((h >> 28) & 1) path->setBlockedByAlly(TRUE);
	return path;
}
bool pfAnswer(unsigned kind, std::initializer_list<unsigned> words)
{
	bool r = p1Pick(p1Hash(30 + kind, words), g_aiEnv.pfAnswerRate);
	std::vector<unsigned> w(1, kind);
	w.insert(w.end(), words.begin(), words.end());
	w.push_back(r ? 1u : 0u);
	if (!g_aiQuiet) {
		g_aiLog->push_back((unsigned)LOG_PF_ANSWER);
		g_aiLog->push_back((unsigned)w.size());
		for (unsigned x : w) g_aiLog->push_back(x);
	}
	return r;
}
bool rangeAnswer(unsigned kind, std::initializer_list<unsigned> words)
{
	bool r = p1Pick(p1Hash(40 + kind, words), g_aiEnv.rangeRate);
	std::vector<unsigned> w(1, kind);
	w.insert(w.end(), words.begin(), words.end());
	w.push_back(r ? 1u : 0u);
	if (!g_aiQuiet) {
		g_aiLog->push_back((unsigned)LOG_WEAPON_RANGE);
		g_aiLog->push_back((unsigned)w.size());
		for (unsigned x : w) g_aiLog->push_back(x);
	}
	return r;
}
unsigned idOf(const Object *o) { return o ? (unsigned)o->getID() : 0u; }
/* PathfindServicesInterface: the searches doPathfind and the path computations ask (the stand-in's paths) */
class StandinServices : public PathfindServicesInterface
{
public:
	virtual Path *findPath( Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, const Coord3D *to )
	{
		return standinPath(0, obj, to, { bitsOf(to->x), bitsOf(to->y), bitsOf(to->z) });
	}
	virtual Path *findClosestPath( Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, Coord3D *to, Bool blocked,
		Real pathCostMultiplier, Bool moveAllies )
	{
		return standinPath(1, obj, to, { bitsOf(to->x), bitsOf(to->y), bitsOf(to->z), (unsigned)blocked, bitsOf(pathCostMultiplier),
			(unsigned)moveAllies });
	}
	virtual Path *findAttackPath( const Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, const Object *victim,
		const Coord3D* victimPos, const Weapon *weapon )
	{
		return standinPath(2, obj, victimPos, { idOf(victim), bitsOf(victimPos->x), bitsOf(victimPos->y), bitsOf(victimPos->z) });
	}
	virtual Path *patchPath( const Object *obj, const LocomotorSet& locomotorSet, Path *originalPath, Bool blocked )
	{
		return standinPath(3, obj, obj->getPosition(), { originalPath ? 1u : 0u, (unsigned)blocked });
	}
	virtual Path *findSafePath( const Object *obj, const LocomotorSet& locomotorSet, const Coord3D *from, const Coord3D* repulsorPos1,
		const Coord3D* repulsorPos2, Real repulsorRadius )
	{
		return standinPath(4, obj, obj->getPosition(), { bitsOf(repulsorPos1->x), bitsOf(repulsorPos1->y), bitsOf(repulsorPos2->x),
			bitsOf(repulsorPos2->y), bitsOf(repulsorRadius) });
	}
};
StandinServices g_services;
}  // namespace
/* the pathfinder's line test (computePath asks it): batch P1's (the paths' calls are edited to it) */
Bool Pathfinder::isLinePassable(const Object *obj, LocomotorSurfaceTypeMask acceptableSurfaces, PathfindLayerEnum layer,
	const Coord3D& startWorld, const Coord3D& endWorld, Bool blocked, Bool allowPinched)
{
	return p1IsLinePassable(this, obj, acceptableSurfaces, layer, startWorld, endWorld, blocked, allowPinched);
}
Path *Pathfinder::getAircraftPath(const Object *obj, const Coord3D *to)
{
	return standinPath(5, obj, to, { bitsOf(to->x), bitsOf(to->y), bitsOf(to->z) });
}
/* batch A7 */
void Pathfinder::snapClosestGoalPosition(Object *obj, Coord3D *pos)
{
	if (pfAnswer(5, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) })) pos->x += 1.0f;
}
void Pathfinder::removeGoal(Object *obj) { pfAnswer(6, {}); }
/* ---- batch A10a's stand-ins (the attack machine). LOG_ATTACK's kinds: 0 setPreAttackFinishedFrame, 1 fireWeapon, 2 releaseWeaponLock,
** 3 notifyFiringTrackerShotFired, 4 preFireCurrentWeapon, 5 setFiringConditionForCurrentWeapon, 6 fireCurrentWeapon at an object, 7 at a
** position, 8 chooseBestWeaponForTarget, 9 adjustModelConditionForWeaponStatus, 10 clearLeechRangeModeForAllWeapons, 11
** setTeamTargetObject, 12 attemptBestFirePointPosition, 13 getRelativeAngle2D, 14 a turret's target object, 15 its target position */
namespace {
bool objectAnswer(unsigned kind, std::initializer_list<unsigned> words) { return p1Pick(p1Hash(60 + kind, words), g_aiEnv.objectRate); }
}
WeaponStatus Weapon::getStatus() const { return (WeaponStatus)g_aiEnv.weaponStatus; }
Bool Weapon::hasLeechRange() const { return g_aiEnv.leechRange != 0; }
Int Weapon::getMaxShotCount() const { return g_maxShots; }
Real Weapon::getAimDelta() const { return g_aiEnv.aimDelta; }
Bool Weapon::fireWeapon(const Object *source, const Coord3D* pos)
{
	bool r = objectAnswer(7, { (unsigned)m_slot, bitsOf(pos->x), bitsOf(pos->y) });
	L(LOG_ATTACK, { 1u, (unsigned)m_slot, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), r ? 1u : 0u });
	return r;
}
void Weapon::setPreAttackFinishedFrame(UnsignedInt frame) { L(LOG_ATTACK, { 0u, (unsigned)m_slot, frame }); }
/* batch A12: LOG_ATTACK kind 16, getPriority (the info, the template's object, the answer: the hash's, 0..3) */
Int AttackPriorityInfo::getPriority(const AiTemplateView *tThing) const
{
	unsigned id = (unsigned)g_aiEnv.objectId;
	for (int i = 0; i < 32; ++i) if (&g_others[i].m_template == tThing) id = (unsigned)i;
	Int r = (Int)(p1Hash(70, { (unsigned)m_index, id }) % 4u);
	L(LOG_ATTACK, { 16u, (unsigned)m_index, id, (unsigned)r });
	return r;
}
Bool Object::isAirborneTarget() const { return objectAnswer(0, { (unsigned)getID() }); }
Bool Object::isDestroyed() const { return objectAnswer(1, { (unsigned)getID() }); }
Bool Object::canCrushOrSquish(Object *otherObj, CrushSquishTestType testType) const
{
	return objectAnswer(2, { (unsigned)getID(), (unsigned)otherObj->getID(), (unsigned)testType });
}
Relationship Object::getRelationship(const Object *that) const
{
	if (!that) return NEUTRAL;	/* (batch A20: upstream's answer without one) */
	return (Relationship)(p1Hash(63, { (unsigned)getID(), (unsigned)that->getID() }) % 3u);
}
Bool Object::isAbleToAttack() const { return g_aiEnv.ableToAttack != 0; }
Bool Object::isOutOfAmmo() const { return g_aiEnv.outOfAmmo != 0; }
Bool Object::isCurWeaponLocked() const { return g_aiEnv.curWeaponLocked != 0; }
Bool Object::hasAnyWeapon() const { return g_aiEnv.hasAnyWeapon != 0; }
Weapon *Object::getCurrentWeapon(WeaponSlotType* wslot)
{
	if (wslot) *wslot = (WeaponSlotType)g_aiEnv.curSlot;
	return getCurrentWeapon();
}
Weapon *Object::getWeaponInWeaponSlot(WeaponSlotType wslot) const
{
	if (!((g_aiEnv.slotMask >> (Int)wslot) & 1)) return NULL;
	return &g_slotWeapons[wslot];
}
Bool Object::chooseBestWeaponForTarget(const Object* target, WeaponChoiceCriteria criteria, CommandSourceType cmdSource)
{
	unsigned id = target ? (unsigned)target->getID() : 0u;
	bool r = objectAnswer(6, { id, (unsigned)criteria, (unsigned)cmdSource });
	L(LOG_ATTACK, { 8u, id, (unsigned)criteria, (unsigned)cmdSource, r ? 1u : 0u });
	return r;
}
void Object::adjustModelConditionForWeaponStatus() { L(LOG_ATTACK, { 9u }); }
void Object::clearLeechRangeModeForAllWeapons() { L(LOG_ATTACK, { 10u }); }
void Object::setFiringConditionForCurrentWeapon() const { L(LOG_ATTACK, { 5u }); }
void Object::preFireCurrentWeapon(const Object *victim) { L(LOG_ATTACK, { 4u, victim ? (unsigned)victim->getID() : 0u }); }
Bool Object::fireCurrentWeapon(Object *target) { L(LOG_ATTACK, { 6u, target ? (unsigned)target->getID() : 0u }); return FALSE; }
Bool Object::fireCurrentWeapon(const Coord3D* pos) { L(LOG_ATTACK, { 7u, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); return FALSE; }
void Object::releaseWeaponLock(WeaponLockType lockType) { L(LOG_ATTACK, { 2u, (unsigned)lockType }); }
void Object::notifyFiringTrackerShotFired(const Weapon* weaponFired, ObjectID victimID)
{
	L(LOG_ATTACK, { 3u, (unsigned)weaponFired->m_slot, (unsigned)victimID });
}
void Team::setTeamTargetObject(const Object *target) { L(LOG_ATTACK, { 11u, target ? (unsigned)target->getID() : 0u }); }
Bool ContainModuleInterface::isEnclosingContainerFor(const Object *obj) const { return objectAnswer(4, { (unsigned)obj->getID() }); }
Bool ContainModuleInterface::attemptBestFirePointPosition(Object *source, Weapon *weapon, Object *victim)
{
	bool r = objectAnswer(5, { (unsigned)victim->getID() });
	L(LOG_ATTACK, { 12u, (unsigned)victim->getID(), r ? 1u : 0u });
	return r;
}
Bool ContainModuleInterface::attemptBestFirePointPosition(Object *source, Weapon *weapon, const Coord3D *targetPos)
{
	bool r = objectAnswer(5, { bitsOf(targetPos->x), bitsOf(targetPos->y) });
	L(LOG_ATTACK, { 12u, bitsOf(targetPos->x), bitsOf(targetPos->y), r ? 1u : 0u });
	return r;
}
Bool ContainModuleInterface::isGarrisonable() const { return g_aiEnv.garrisonable != 0; }
UnsignedInt ContainModuleInterface::getContainCount() const { return (UnsignedInt)g_aiEnv.containCount; }
Real PartitionManager::getRelativeAngle2D(const Object *obj, const Object *otherObj)
{
	L(LOG_ATTACK, { 13u, otherObj ? (unsigned)otherObj->getID() : 0u });
	return g_aiEnv.relAngle;
}
Real PartitionManager::getRelativeAngle2D(const Object *obj, const Coord3D *pos)
{
	L(LOG_ATTACK, { 13u, bitsOf(pos->x), bitsOf(pos->y) });
	return g_aiEnv.relAngle;
}
void TurretAI::setTurretTargetObject(Object *o, Bool forceAttacking)
{
	L(LOG_ATTACK, { 14u, (unsigned)m_which, o ? (unsigned)o->getID() : 0u, (unsigned)forceAttacking });
}
void TurretAI::setTurretTargetPosition(const Coord3D *pos)
{
	if (pos) L(LOG_ATTACK, { 15u, (unsigned)m_which, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
	else L(LOG_ATTACK, { 15u, (unsigned)m_which });
}
Real TurretAI::getTurnRate() const { return g_aiEnv.turretTurnRate[m_which]; }
/* ---- batch A10b's stand-ins (the chase states) */
Bool Weapon::isTooClose(const Object *source, const Object *target) const { return rangeAnswer(3, { idOf(target) }); }
void GeometryInfo::getCenterPosition(const Coord3D& pos, Coord3D& center) const
{
	center = pos;
	center.z += g_aiEnv.maxHeightAbovePosition * 0.5f;
}
Bool TurretAI::isOwnersCurWeaponOnTurret() const { return ((g_aiEnv.turretCurWeapon >> m_which) & 1) != 0; }
/* batch A23 */
TurretTargetType TurretAI::friend_getTurretTarget(Object*& obj, Coord3D& pos, Bool clearDeadTargets) const
{
	unsigned h = p1Hash(76, { (unsigned)m_which, clearDeadTargets ? 1u : 0u });
	L(LOG_AI_SNAPSHOT, { 5u, (unsigned)m_which, clearDeadTargets ? 1u : 0u });
	obj = findObject((ObjectID)((h >> 4) % 32u));
	return (TurretTargetType)(h % 3u);
}
void TurretAI::setTurretEnabled(Bool enabled) { L(LOG_AI_SNAPSHOT, { 3u, (unsigned)m_which, enabled ? 1u : 0u }); }
void TurretAI::recenterTurret() { L(LOG_AI_SNAPSHOT, { 4u, (unsigned)m_which }); }
Bool TurretAI::isTurretEnabled() const { return (p1Hash(77, { (unsigned)m_which }) & 1) != 0; }
Bool TurretAI::isTurretInNaturalPosition() const { return (p1Hash(78, { (unsigned)m_which }) & 1) != 0; }
Bool TurretAI::isWeaponSlotOnTurret(WeaponSlotType wslot) const { return (p1Hash(79, { (unsigned)m_which, (unsigned)wslot }) & 1) != 0; }
Bool TurretAI::isTryingToAimAtTarget(const Object* victim) const
{
	return (p1Hash(80, { (unsigned)m_which, victim ? (unsigned)victim->getID() : 0u }) & 1) != 0;
}
Real TurretAI::getTurretAngle() const { return (Real)(p1Hash(81, { (unsigned)m_which }) % 628u) / 100.0f - 3.0f; }
Real TurretAI::getTurretPitch() const { return (Real)(p1Hash(82, { (unsigned)m_which }) % 100u) / 100.0f; }
/* batch A9 */
Bool Pathfinder::isPointOnWall(const Coord3D *pos) { return pfAnswer(7, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
void Pathfinder::moveAlliesAwayFromDestination(Object *obj, const Coord3D& destination)
{
	pfAnswer(0, { bitsOf(destination.x), bitsOf(destination.y), bitsOf(destination.z) });
}
Bool Pathfinder::adjustToPossibleDestination(Object *obj, const LocomotorSet& locomotorSet, Coord3D *dest)
{
	bool r = pfAnswer(1, { idOf(obj), bitsOf(dest->x), bitsOf(dest->y), bitsOf(dest->z) });
	if (r) { dest->x += 2.0f; dest->y -= 2.0f; }
	return r;
}
Bool Pathfinder::adjustDestination(Object *obj, const LocomotorSet& locomotorSet, Coord3D *dest, const Coord3D *groupDest)
{
	bool r = pfAnswer(2, { bitsOf(dest->x), bitsOf(dest->y), bitsOf(dest->z), groupDest ? 1u : 0u });
	if (r) { dest->x -= 3.0f; dest->y += 3.0f; }
	return r;
}
Bool Pathfinder::clientSafeQuickDoesPathExistForUI(const LocomotorSet& locomotorSet, const Coord3D *from, const Coord3D *to)
{
	return pfAnswer(3, { bitsOf(to->x), bitsOf(to->y), bitsOf(to->z) });
}
Bool Pathfinder::isAttackViewBlockedByObstacle(const Object* obj, const Coord3D& attackerPos, const Object* victim, const Coord3D& victimPos)
{
	return pfAnswer(4, { idOf(victim), bitsOf(attackerPos.x), bitsOf(attackerPos.y), bitsOf(victimPos.x), bitsOf(victimPos.y) });
}
Bool Weapon::isWithinAttackRange(const Object *source, const Object *target) const { return rangeAnswer(0, { idOf(target) }); }
Bool Weapon::isWithinAttackRange(const Object *source, const Coord3D* pos) const
{
	return rangeAnswer(1, { bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
}
Bool Weapon::isGoalPosWithinAttackRange(const Object *source, const Coord3D* goalPos, const Object *target, const Coord3D* targetPos) const
{
	return rangeAnswer(2, { bitsOf(goalPos->x), bitsOf(goalPos->y), bitsOf(goalPos->z), idOf(target), bitsOf(targetPos->x), bitsOf(targetPos->y) });
}
/* the approach point: the position moved toward the object by a quarter (logged) */
Bool Weapon::computeApproachTarget(const Object *source, const Object *target, const Coord3D *pos, Real angleOffset, Coord3D& approachTargetPos) const
{
	const Coord3D *s = source->getPosition();
	Coord3D r = *pos;
	r.x += (s->x - pos->x) * 0.25f;
	r.y += (s->y - pos->y) * 0.25f;
	rangeAnswer(3, { idOf(target), bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(angleOffset) });
	approachTargetPos = r;
	return TRUE;
}
/* batch A18 */
Real PolygonTrigger::getRadius(void) const { return 30.0f + 5.0f * (Real)m_index; }
AsciiString PolygonTrigger::getTriggerName(void) const { AsciiString n; n.format("poly%d", m_index); return n; }
PolygonTrigger *TerrainLogic::getTriggerAreaByName(AsciiString name)
{
	for (int i = 0; i < 8; ++i) if (g_polygons[i].getTriggerName() == name) return &g_polygons[i];
	return NULL;
}
UnsignedInt GameLogic::getFrameObjectsChangedTriggerAreas(void) { return g_aiEnv.objectsChangedFrame; }
AcademyStats *Player::getAcademyStats() { return NULL; }
void AcademyStats::recordGuardAbilityUsed() {}
ObjectID BodyModuleInterface::getClearableLastAttacker() const { return (ObjectID)g_aiEnv.clearableAttacker; }
void BodyModuleInterface::clearLastAttacker() { L(LOG_ATTACK, { 17u }); g_aiEnv.clearableAttacker = 0; }
Bool AiTemplateView::isEnterGuard() const { return (g_aiEnv.guardFlags & 1) != 0; }
Bool AiTemplateView::isHijackGuard() const { return (g_aiEnv.guardFlags & 2) != 0; }
void PolygonTrigger::getCenterPoint(Coord3D* pOutCoord) const
{
	pOutCoord->x = 10.0f * (Real)m_index;
	pOutCoord->y = 20.0f * (Real)m_index;
	pOutCoord->z = 0.0f;
}

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
/* (batch A21: the goal's z is not logged: upstream's updateGoal does not read it, and AIUpdateInterface::loadPostProcess leaves it
** unset) */
void Pathfinder::updateGoal(Object *obj, const Coord3D *newGoalPos, PathfindLayerEnum layer)
{
	L(LOG_PF_UPDATE_GOAL, { bitsOf(newGoalPos->x), bitsOf(newGoalPos->y), (unsigned)layer });
}
/* batch A21: LOG_AI_SNAPSHOT's kinds: 0 updatePos {x, y, z}, 1 a turret's crc {which}, 2 its xfer {which} */
void Pathfinder::updatePos(Object *obj, const Coord3D *newPos) { L(LOG_AI_SNAPSHOT, { 0u, bitsOf(newPos->x), bitsOf(newPos->y), bitsOf(newPos->z) }); }
void TurretAI::crc(Xfer *xfer) { L(LOG_AI_SNAPSHOT, { 1u, (unsigned)m_which }); }
void TurretAI::xfer(Xfer *xfer)
{
	L(LOG_AI_SNAPSHOT, { 2u, (unsigned)m_which });
	XferVersion v = 1;
	xfer->xferVersion(&v, 1);
	xfer->xferInt(&m_which);
}
void UpdateModule::crc(Xfer *xfer) {}
void UpdateModule::xfer(Xfer *xfer)
{
	XferVersion v = 1;
	xfer->xferVersion(&v, 1);
	xfer->xferUnsignedInt(&g_nextCallFrame);
}
void UpdateModule::loadPostProcess(void) {}
AsciiString AttackPriorityInfo::getName() const { AsciiString n; n.format("ap%d", m_index); return n; }
const AttackPriorityInfo *ScriptEngine::getAttackInfo(const AsciiString& name)
{
	for (int i = 0; i < 2; ++i) if (g_attackInfos[i].getName() == name) return &g_attackInfos[i];
	return NULL;
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
	/* batch A31: a DeployStyleAIUpdate's data */
	g_deployData.m_unpackTime = env->deployTimes[0];
	g_deployData.m_packTime = env->deployTimes[1];
	g_deployData.m_resetTurretBeforePacking = (env->deployFlags & 1) != 0;
	g_deployData.m_turretsFunctionOnlyWhenDeployed = (env->deployFlags & 2) != 0;
	g_deployData.m_turretsMustCenterBeforePacking = (env->deployFlags & 4) != 0;
	g_deployData.m_manualDeployAnimations = (env->deployFlags & 8) != 0;
	/* batch A32: an AssaultTransportAIUpdate's data */
	g_assaultData.m_membersGetHealedAtLifeRatio = env->assaultRatio;
	/* batch A36: a MissileAIUpdate's data (the FX lists the scene's two, the kinds by bit) */
	for (int i = 0; i < 3; ++i) { g_weaponTemplates[i].m_index = i; g_particleTemplates[i].m_index = i; }
	g_missileData = MissileAIUpdateModuleData();
	g_missileData.m_tryToFollowTarget = (env->missileFlags & 1) != 0;
	g_missileData.m_useWeaponSpeed = (env->missileFlags & 2) != 0;
	g_missileData.m_detonateOnNoFuel = (env->missileFlags & 4) != 0;
	g_missileData.m_detonateCallsKill = (env->missileFlags & 8) != 0;
	g_missileData.m_ignitionFX = (env->missileFlags & 16) ? &g_missileFx[0] : NULL;
	g_missileData.m_garrisonHitKillFX = (env->missileFlags & 32) ? &g_missileFx[1] : NULL;
	g_missileData.m_fuelLifetime = env->missileTimes[0];
	g_missileData.m_ignitionDelay = env->missileTimes[1];
	g_missileData.m_initialVel = env->missileReals[0];
	g_missileData.m_initialDist = env->missileReals[1];
	g_missileData.m_diveDistance = env->missileReals[2];
	g_missileData.m_distanceScatterWhenJammed = env->missileReals[3];
	g_missileData.m_lockDistance = env->missileReals[4];
	g_missileData.m_garrisonHitKillCount = env->missileGarrison[0];
	for (int k = 0; k < 31; ++k) {
		if ((env->missileGarrison[1] >> k) & 1) g_missileData.m_garrisonHitKillKindof.set((KindOfType)k);
		if ((env->missileGarrison[2] >> k) & 1) g_missileData.m_garrisonHitKillKindofNot.set((KindOfType)k);
	}
	/* batch A37: a DeliverPayloadAIUpdate's data (the template names the scene's) */
	g_deliverData = DeliverPayloadAIUpdateModuleData();
	g_deliverData.m_doorDelay = env->deliverTimes[0];
	g_deliverData.m_dropDelay = env->deliverTimes[1];
	g_deliverData.m_maxDistanceToTarget = env->deliverReals[0];
	g_deliverData.m_dropOffset.set(env->deliverReals[1], env->deliverReals[2], env->deliverReals[3]);
	g_deliverData.m_dropVariance.set(env->deliverReals[4], env->deliverReals[5], env->deliverReals[6]);
	g_deliverData.m_deliveryDecalRadius = env->deliverReals[7];
	g_deliverData.m_maxNumberAttempts = env->deliverInts[0];
	g_deliverData.m_putInContainerName = deliverTemplateName(env->deliverInts[1]);
	sceneDecalTemplate(&g_deliverData.m_deliveryDecalTemplate);
	/* batch A38: a DozerAIUpdate's data */
	g_dozerData.m_repairHealthPercentPerSecond = env->dozerReals[0];
	g_dozerData.m_boredTime = env->dozerReals[1];
	g_dozerData.m_boredRange = env->dozerReals[2];
	g_missileData.m_killSelfDelay = env->missileKillSelfDelay;
	/* batch A35: a SupplyTruckAIUpdate's data */
	g_supplyData.m_maxBoxesData = env->supplyMaxBoxes;
	g_supplyData.m_centerDelay = env->supplyDelays[0];
	g_supplyData.m_warehouseDelay = env->supplyDelays[1];
	g_supplyData.m_warehouseScanDistance = env->supplyScan;
	g_supplyData.m_suppliesDepletedVoice = AudioEventRTS();
	if (env->supplyFlags & 1) g_supplyData.m_suppliesDepletedVoice.setEventName(AsciiString("s7"));
	/* batch A34: a HackInternetAIUpdate's data */
	g_hackData.m_unpackTime = env->hackTimes[0];
	g_hackData.m_packTime = env->hackTimes[1];
	g_hackData.m_cashUpdateDelay = env->hackTimes[2];
	g_hackData.m_cashUpdateDelayFast = env->hackTimes[3];
	g_hackData.m_regularCashAmount = env->hackCash[0];
	g_hackData.m_veteranCashAmount = env->hackCash[1];
	g_hackData.m_eliteCashAmount = env->hackCash[2];
	g_hackData.m_heroicCashAmount = env->hackCash[3];
	g_hackData.m_xpPerCashUpdate = env->hackXp;
	g_hackData.m_packUnpackVariationFactor = env->hackVariation;
	/* batch A9 */
	TheAI->pathfinder()->m_wallHeight = env->wallHeight;
	/* batch A10a */
	for (int i = 0; i < 3; ++i) g_slotWeapons[i].m_slot = i;
	TheGlobalData->m_movementPenaltyDamageState = env->penalty;
	/* batch A5 */
	TheGlobalData->m_partitionCellSize = env->partitionCellSize;
	((AIUpdateInterface *)g_otherAiMem)->m_path = env->otherAiPath ? (Path *)g_otherAiMem : NULL;
	/* batch A23: the other objects' AIs' one machine: a real AIStateMachine (made once, on another object), its current state set
	** from the scene without entering it, its goal position the scene's (its goal object stays the scene's otherAiGoal) */
	{
		AIStateMachine *otherMachine = g_otherMachine;
		((AIUpdateInterface *)g_otherAiMem)->m_stateMachine = otherMachine;
		otherMachine->m_currentState = otherMachine->internalGetState((StateID)env->otherAiState);
		otherMachine->m_goalPosition.x = env->otherAiGoalPos[0];
		otherMachine->m_goalPosition.y = env->otherAiGoalPos[1];
		otherMachine->m_goalPosition.z = env->otherAiGoalPos[2];
	}
	for (int i = 0; i < 4; ++i) g_buttons[i].m_index = i;
	for (int i = 0; i < 8; ++i) { g_polygons[i].m_index = i; g_waypoints[i].m_index = i; }
	/* batch A6: the waypoints' locations; the other objects anew (their cached heights are asked) */
	for (int i = 0; i < 8; ++i) { g_waypoints[i].m_location.x = 15.0f * i; g_waypoints[i].m_location.y = 25.0f * i; g_waypoints[i].m_location.z = 1.0f; }
	for (int i = 0; i < 32; ++i) g_others[i] = Object();
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
	/* batch A5: the modules the commands look up, first (their keys are kept in function-local statics, which outlive a reset) */
	gen->nameToKey(AsciiString("DeliverPayloadAIUpdate"));
	gen->nameToKey(AsciiString("ProneUpdate"));
	gen->nameToKey(AsciiString("HackInternetAIUpdate"));
	/* batch A37: the delivered items' modules */
	gen->nameToKey(AsciiString("GenerateMinefieldBehavior"));
	gen->nameToKey(AsciiString("SmartBombTargetHomingUpdate"));
	/* batch A38: the dozer's goal objects' creation dispatches no module by name */
	g_ai_update = 0;	/* (leaked: its locomotors point into the store deleted below) */
	delete TheLocomotorStore;
	TheLocomotorStore = new LocomotorStore;
	static std::vector<unsigned> log;
	g_aiLog = &log;
	log.clear();
}

/* batch A4: the file the machine's xfer goes through (one per test thread) */
/* (batch A23: and the other AIs' machine, made once here, before a case seeds the logic's random values: AIIdleState's constructor
** draws one) */
void ref_ai_a4_xfer_path(const char *path)
{
	g_xferPath = path; g_saved.clear(); g_aiSaved.clear();
	if (!g_otherMachine) {
		g_others[1].m_otherId = (ObjectID)1;
		g_otherMachine = newInstance(AIStateMachine)(&g_others[1], AsciiString("other"));
		((AIUpdateInterface *)g_otherAiMem)->m_stateMachine = g_otherMachine;
	}
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

/* A new AI: its data's locomotor sets from `sets` (count, then per set: its type, its number of templates, their names' keys,
** then (batch A3) the data's auto-acquire bits and mood check rate, (batch A5) its forbidden player commands),
** the scene `env`, the object at the scene's start (identity, at `pos` facing `angle`), the AI made by its constructor
** (AIUpdateInterface(NULL, NULL)); the log of the construction is returned (length in words, may exceed cap). */
int ref_ai_a2_new(const unsigned *sets, const AiEnv *env, const float *pos, float angle, unsigned *log, int cap)
{
	setScene(env);
	g_aiLog->clear();
	makeMap();
	g_nextCallFrame = 0;
	g_nextAudioHandle = 1;
	g_nextParticleId = 1;
	/* batch A37: the visible payloads' ids count from 40 in each case */
	g_nextPayloadId = 40;
	/* batch A9 */
	g_teamWaypoint = env->teamWaypoint >= 0 ? &g_waypoints[env->teamWaypoint] : NULL;
	/* batch A10a */
	g_maxShots = env->maxShotsInit;
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
	/* batch A5: the data's forbidden player commands */
	g_data->m_forbidPlayerCommands = sets[k++] != 0;
	/* batch A10a: the data's linked turrets */
	g_data->m_turretsLinked = sets[k++] != 0;
	g_object = Object();
	++g_aiQuiet;
	Coord3D p = { pos[0], pos[1], pos[2] };
	g_object.setPosition(&p);
	g_object.setOrientation(angle);
	--g_aiQuiet;
	/* batch A30: the scene's class (1 WanderAIUpdate, 2 TransportAIUpdate; batch A31: 3 DeployStyleAIUpdate) */
	switch (g_aiEnv.subclass) {
		case 1: g_ai_update = new (ShimZeroFill{}) WanderAIUpdate(NULL, NULL); break;
		case 2: g_ai_update = new (ShimZeroFill{}) TransportAIUpdate(NULL, NULL); break;
		case 3: g_ai_update = new (ShimZeroFill{}) DeployStyleAIUpdate(NULL, NULL); break;
		case 4: g_ai_update = new (ShimZeroFill{}) AssaultTransportAIUpdate(NULL, NULL); break;
		case 5: g_ai_update = new (ShimZeroFill{}) RailedTransportAIUpdate(NULL, NULL); break;
		case 6: g_ai_update = new (ShimZeroFill{}) HackInternetAIUpdate(NULL, NULL); break;
		/* (batch A35: the object has no AI while its AI is made: the machine's default state asks) */
		case 7: g_ai_update = NULL; g_ai_update = new (ShimZeroFill{}) SupplyTruckAIUpdate(NULL, NULL); break;
		case 8: g_ai_update = new (ShimZeroFill{}) MissileAIUpdate(NULL, NULL); break;
		case 9: g_ai_update = new (ShimZeroFill{}) DeliverPayloadAIUpdate(NULL, NULL); break;
		/* (batch A38: the object has no AI while its AI is made: the machine's default state's transitions ask) */
		case 10: g_ai_update = NULL; g_ai_update = new (ShimZeroFill{}) DozerAIUpdate(NULL, NULL); break;
		default: g_ai_update = new (ShimZeroFill{}) AIUpdateInterface(NULL, NULL); break;
	}
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
	/* batch A32: the weapon's beginAssault (an assault transport's; nothing for another class) */
	case 100: if (g_aiEnv.subclass == 4) ((AssaultTransportAIUpdate *)ai)->beginAssault(findObject((ObjectID)ia[0])); break;
	/* batch A35: the docks' calls to a supply truck (nothing for another class) */
	case 101: if (g_aiEnv.subclass == 7) r.push_back(((SupplyTruckAIUpdate *)ai)->gainOneBox(ia[0]) ? 1u : 0u); break;
	case 102: if (g_aiEnv.subclass == 7) r.push_back(((SupplyTruckAIUpdate *)ai)->loseOneBox() ? 1u : 0u); break;
	case 103: if (g_aiEnv.subclass == 7) ((SupplyTruckAIUpdate *)ai)->setForceWantingState(ia[0] != 0); break;
	/* batch A36: the weapon's calls to a missile (ia: the victim, the launcher, the slot, the barrel, the detonation weapon (-1 none),
	** the exhaust (-1 none); fa: the victim's position) (nothing for another class) */
	case 104: case 105: case 106: case 107: case 108:
		if (g_aiEnv.subclass == 8) {
			MissileAIUpdate *m = (MissileAIUpdate *)ai;
			const WeaponTemplate *det = ia[4] >= 0 ? &g_weaponTemplates[ia[4]] : NULL;
			const ParticleSystemTemplate *exhaust = ia[5] >= 0 ? &g_particleTemplates[ia[5]] : NULL;
			Object *victim = findObject((ObjectID)ia[0]);
			if (op == 104) m->projectileLaunchAtObjectOrPosition(victim, &p, findObject((ObjectID)ia[1]), (WeaponSlotType)ia[2], ia[3], det, exhaust);
			if (op == 105) m->projectileFireAtObjectOrPosition(victim, &p, det, exhaust);
			if (op == 106) r.push_back(m->projectileHandleCollision(victim) ? 1u : 0u);
			if (op == 107) m->setFramesTillCountermeasureDiversionOccurs((UnsignedInt)ia[0]);
			if (op == 108) m->projectileNowJammed();
		}
		break;
	/* batch A37: a delivering AI's members (ia[0]: the exiter; fa[3..5]: the target; the data op 109 delivers the scene's)
	** (nothing for another class) */
	case 109: case 110: case 111: case 112: case 113: case 114: case 115: case 116: case 117:
		if (g_aiEnv.subclass == 9) {
			DeliverPayloadAIUpdate *d = (DeliverPayloadAIUpdate *)ai;
			if (op == 109) {
				DeliverPayloadData data;
				sceneDeliverData(&data);
				Coord3D t = { fa[3], fa[4], fa[5] };
				d->deliverPayload(&p, &t, &data);
			}
			if (op == 110) d->deliverPayloadViaModuleData(&p);
			if (op == 111) r.push_back((unsigned)d->getAiFreeToExit(findObject((ObjectID)ia[0])));
			if (op == 112) r.push_back(d->isCloseEnoughToTarget() ? 1u : 0u);
			if (op == 113) { Real t = 0.0f; Real radius = d->calcMinTurnRadius(&t); putF(r, radius); putF(r, t); }
			if (op == 114) d->killDeliveryDecal();
			if (op == 115) { const AiTemplateView *t = d->getPutInContainerTemplateViaModuleData(); r.push_back(t ? (unsigned)(t - g_payloadTemplates) : 0xffffffffu); }
			if (op == 116) r.push_back(d->isOffMap() ? 1u : 0u);
			if (op == 117) r.push_back(d->isDeliveringPayload() ? 1u : 0u);
		}
		break;
	/* batch A38: a dozer's members (ia[0]: a task, an object, a template (-1 none); ia[1]: an object, a point, the rebuild flag;
	** fa[3]: the angle) (nothing for another class) */
	case 118: case 119: case 120: case 121: case 122: case 123: case 124: case 125: case 126: case 127: case 128: case 129: case 130:
	case 131: case 132:
		if (g_aiEnv.subclass == 10) {
			DozerAIUpdate *d = (DozerAIUpdate *)ai;
			DozerTask t = (DozerTask)ia[0];
			if (op == 118) {
				Object *o = d->construct(ia[0] >= 0 ? &g_payloadTemplates[ia[0]] : NULL, &p, fa[3], g_object.getControllingPlayer(), ia[1] != 0);
				r.push_back(o ? (unsigned)o->getID() : 0u);
			}
			if (op == 119) r.push_back((unsigned)d->getMostRecentCommand());
			if (op == 120) r.push_back(d->isTaskPending(t) ? 1u : 0u);
			if (op == 121) r.push_back((unsigned)d->getTaskTarget(t));
			if (op == 122) r.push_back(d->isAnyTaskPending() ? 1u : 0u);
			if (op == 123) d->newTask(t, findObject((ObjectID)ia[1]));
			if (op == 124) d->cancelTask(t);
			if (op == 125) d->internalTaskComplete(t);
			if (op == 126) d->internalCancelTask(t);
			if (op == 127) { const Coord3D *q = d->getDockPoint(t, (DozerDockPoint)ia[1]); r.push_back(q ? 1u : 0u); if (q) putPos(r, *q); }
			if (op == 128) r.push_back(d->canAcceptNewRepair(findObject((ObjectID)ia[0])) ? 1u : 0u);
			if (op == 129) d->onDelete();
			if (op == 130) { putF(r, d->getRepairHealthPerSecond()); putF(r, d->getBoredTime()); putF(r, d->getBoredRange()); }
			if (op == 131) d->createBridgeScaffolding(findObject((ObjectID)ia[0]));
			if (op == 132) d->removeBridgeScaffolding(findObject((ObjectID)ia[0]));
		}
		break;
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
	case 72: {
		/* batch A5: aiDoCommand of the parms: ia[0] the command, ia[1] its source, ia[2] its object (findObject), ia[3] the team (>= 0),
		** ia[4] the waypoint, ia[5] the polygon, ia[6] the int value, ia[7] the command button (each -1: none), ia[8] coords at fa[3..] */
		AICommandParms parms((AICommandType)ia[0], (CommandSourceType)ia[1]);
		parms.m_pos = p;
		parms.m_obj = findObject((ObjectID)ia[2]);
		parms.m_team = ia[3] >= 0 ? &g_team : NULL;
		parms.m_waypoint = ia[4] >= 0 ? &g_waypoints[ia[4]] : NULL;
		parms.m_polygon = ia[5] >= 0 ? &g_polygons[ia[5]] : NULL;
		parms.m_intValue = ia[6];
		parms.m_commandButton = ia[7] >= 0 ? &g_buttons[ia[7]] : NULL;
		for (int i = 0; i < ia[8]; ++i) { Coord3D c = { fa[3 + 3 * i], fa[4 + 3 * i], fa[5 + 3 * i] }; parms.m_coords.push_back(c); }
		ai->aiDoCommand(&parms);
		break;
	}
	case 73: {
		/* batch A5: doQuickExit of the coords (ia[8] at fa[3..]) */
		std::vector<Coord3D> path;
		for (int i = 0; i < ia[8]; ++i) { Coord3D c = { fa[3 + 3 * i], fa[4 + 3 * i], fa[5 + 3 * i] }; path.push_back(c); }
		ai->doQuickExit(&path);
		break;
	}
	/* batch A6: the paths */
	case 74: ai->requestPath(&p, ia[0] != 0); break;
	case 75: ai->requestAttackPath((ObjectID)ia[0], &p); break;
	case 76: ai->requestApproachPath(&p); break;
	case 77: ai->requestSafePath((ObjectID)ia[0]); break;
	case 78: ai->doPathfind(&g_services); break;
	case 79: { Coord2D off = { fa[3], fa[4] }; ai->setPathFromWaypoint(ia[0] >= 0 ? &g_waypoints[ia[0]] : NULL, &off); break; }
	case 80: r.push_back(ai->computePath(&g_services, &p) ? 1u : 0u); break;
	case 81: r.push_back(ai->computeAttackPath(&g_services, findObject((ObjectID)ia[0]), ia[1] ? &p : NULL) ? 1u : 0u); break;
	case 82: ai->friend_setGoalObject(findObject((ObjectID)ia[0])); break;
	case 83: r.push_back(ai->isQuickPathAvailable(&p) ? 1u : 0u); break;
	case 84: r.push_back(ai->canComputeQuickPath() ? 1u : 0u); break;
	case 85: r.push_back(ai->computeQuickPath(&p) ? 1u : 0u); break;
	/* what the move states set on the current locomotor (not yet translated): ultra-accurate ia[0] */
	case 86: if (ai->m_curLocomotor) ai->m_curLocomotor->setUltraAccurate(ia[0] != 0); break;
	/* batch A12: setAttackInfo (ia[0]: the info, -1 none) */
	case 87: ai->setAttackInfo(ia[0] >= 0 ? &g_attackInfos[ia[0]] : NULL); break;
	case 88: {
		/* batch A21: the AI saved (xferSnapshot: AIUpdateInterface::xfer): the status, then the bytes */
		XferSave x;
		int st = 0;
		try { x.open(AsciiString(g_xferPath.c_str())); x.xferSnapshot(ai); } catch (...) { st = 1; }
		try { x.close(); } catch (...) {}
		g_aiSaved.clear();
		if (FILE *f = fopen(g_xferPath.c_str(), "rb")) {
			unsigned char b[65536];
			size_t m = fread(b, 1, sizeof b, f);
			fclose(f);
			g_aiSaved.assign(b, b + m);
		}
		r.push_back((unsigned)st);
		r.push_back((unsigned)g_aiSaved.size());
		for (unsigned char c : g_aiSaved) r.push_back(c);
		break;
	}
	case 89: {
		/* batch A21: the AI's last save loaded into it (xferSnapshot, then loadPostProcess): the status */
		if (FILE *f = fopen(g_xferPath.c_str(), "wb")) { if (!g_aiSaved.empty()) fwrite(&g_aiSaved[0], 1, g_aiSaved.size(), f); fclose(f); }
		XferLoad x;
		int st = 0;
		try { x.open(AsciiString(g_xferPath.c_str())); x.xferSnapshot(ai); } catch (...) { st = 1; }
		try { x.close(); } catch (...) {}
		ai->loadPostProcess();
		r.push_back((unsigned)st);
		break;
	}
	case 91: ai->joinTeam(); break;
	/* batch A23: the turrets' members (ia[0] the turret or the weapon slot, ia[1] the flag or the victim) */
	case 92: { Object *o = ai->getTurretTargetObject((WhichTurretType)ia[0], ia[1] != 0); r.push_back(o ? (unsigned)o->getID() : 0u); break; }
	case 93: ai->setTurretEnabled((WhichTurretType)ia[0], ia[1] != 0); break;
	case 94: ai->recenterTurret((WhichTurretType)ia[0]); break;
	case 95: r.push_back(ai->isTurretEnabled((WhichTurretType)ia[0]) ? 1u : 0u); break;
	case 96: r.push_back(ai->isTurretInNaturalPosition((WhichTurretType)ia[0]) ? 1u : 0u); break;
	case 97: r.push_back(ai->isWeaponSlotOnTurretAndAimingAtTarget((WeaponSlotType)ia[0], findObject((ObjectID)ia[1])) ? 1u : 0u); break;
	case 98: {
		Real angle = -9.0f, pitch = -9.0f;
		r.push_back(ai->getTurretRotAndPitch((WhichTurretType)ia[0], &angle, &pitch) ? 1u : 0u);
		r.push_back(bitsOf(angle)); r.push_back(bitsOf(pitch));
		break;
	}
	case 99: {
		Real angle = -9.0f, pitch = -9.0f;
		r.push_back((unsigned)ai->getWhichTurretForWeaponSlot((WeaponSlotType)ia[0], &angle, &pitch));
		r.push_back(bitsOf(angle)); r.push_back(bitsOf(pitch));
		break;
	}
	case 90: {
		/* batch A21: the AI's CRC (XferCRC::xferSnapshot: AIUpdateInterface::crc) */
		XferCRC x;
		x.open(AsciiString("ai_a21_crc"));
		x.xferSnapshot(ai);
		x.close();
		r.push_back(x.getCRC());
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
	/* batch A5 */
	putU32(v, ai->m_areaToGuard ? (unsigned)ai->m_areaToGuard->m_index : 0xffffffffu);
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
	/* batch A6: the pathfinder's ignored obstacle */
	putU32(v, (unsigned)TheAI->pathfinder()->m_ignoreObstacleID);
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
		/* batch A5: the goal waypoint and squad */
		putU32(v, sm->m_goalWaypoint ? (unsigned)sm->m_goalWaypoint->m_index : 0xffffffffu);
		putU32(v, sm->m_goalSquad ? 1u : 0u);
		if (sm->m_goalSquad) {
			putU32(v, (unsigned)sm->m_goalSquad->m_objectIDs.size());
			for (size_t i = 0; i < sm->m_goalSquad->m_objectIDs.size(); ++i) putU32(v, (unsigned)sm->m_goalSquad->m_objectIDs[i]);
		}
		/* batch A7: the move states' members */
		const StateID moves[4] = { AI_MOVE_TO, AI_MOVE_OUT_OF_THE_WAY, AI_MOVE_AND_TIGHTEN, AI_MOVE_AWAY_FROM_REPULSORS };
		for (int i = 0; i < 4; ++i) {
			const AIInternalMoveToState *m = (const AIInternalMoveToState *)sm->m_stateMap.find(moves[i])->second;
			putPos(v, m->m_goalPosition);
			putU32(v, (unsigned)m->m_goalLayer);
			putPos(v, m->m_pathGoalPosition);
			putU32(v, (unsigned)m->m_ambientPlayingHandle);
			putU32(v, m->m_pathTimestamp);
			putU32(v, m->m_blockedRepathTimestamp);
			putU32(v, m->m_adjustDestinations ? 1u : 0u);
			putU32(v, m->m_waitingForPath ? 1u : 0u);
			putU32(v, m->m_tryOneMoreRepath ? 1u : 0u);
			if (i >= 2) {
				const AIMoveAndTightenState *t = (const AIMoveAndTightenState *)m;
				putU32(v, (unsigned)t->m_okToRepathTimes);
				putU32(v, t->m_checkForPath ? 1u : 0u);
			}
		}
		/* batch A8: the follow-path, move-and-evacuate and move-and-delete states' members */
		const StateID moves8[5] = { AI_FOLLOW_PATH, AI_FOLLOW_EXITPRODUCTION_PATH, AI_MOVE_AND_EVACUATE, AI_MOVE_AND_EVACUATE_AND_EXIT, AI_MOVE_AND_DELETE };
		for (int i = 0; i < 5; ++i) {
			const AIInternalMoveToState *m = (const AIInternalMoveToState *)sm->m_stateMap.find(moves8[i])->second;
			putPos(v, m->m_goalPosition);
			putU32(v, (unsigned)m->m_goalLayer);
			putPos(v, m->m_pathGoalPosition);
			putU32(v, (unsigned)m->m_ambientPlayingHandle);
			putU32(v, m->m_pathTimestamp);
			putU32(v, m->m_blockedRepathTimestamp);
			putU32(v, m->m_adjustDestinations ? 1u : 0u);
			putU32(v, m->m_waitingForPath ? 1u : 0u);
			putU32(v, m->m_tryOneMoreRepath ? 1u : 0u);
			if (i < 2) {
				const AIFollowPathState *f = (const AIFollowPathState *)m;
				putU32(v, (unsigned)f->m_index);
				putU32(v, f->m_adjustFinal ? 1u : 0u);
				putU32(v, f->m_adjustFinalOverride ? 1u : 0u);
				putU32(v, (unsigned)f->m_retryCount);
			} else if (i < 4) {
				putPos(v, ((const AIMoveAndEvacuateState *)m)->m_origin);
			} else {
				putU32(v, ((const AIMoveAndDeleteState *)m)->m_appendGoalPosition ? 1u : 0u);
			}
		}
		/* batch A9: the team's waypoint; the waypoint, wander and panic states' members */
		putU32(v, g_teamWaypoint ? (unsigned)g_teamWaypoint->m_index : 0xffffffffu);
		putU32(v, ai->m_completedWaypoint ? (unsigned)ai->m_completedWaypoint->m_index : 0xffffffffu);
		const StateID moves9[7] = { AI_FOLLOW_WAYPOINT_PATH_AS_TEAM, AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS,
			AI_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT, AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS_EXACT, AI_WANDER, AI_PANIC, AI_WANDER_IN_PLACE };
		for (int i = 0; i < 7; ++i) {
			const AIInternalMoveToState *m = (const AIInternalMoveToState *)sm->m_stateMap.find(moves9[i])->second;
			putPos(v, m->m_goalPosition);
			putU32(v, (unsigned)m->m_goalLayer);
			putPos(v, m->m_pathGoalPosition);
			putU32(v, (unsigned)m->m_ambientPlayingHandle);
			putU32(v, m->m_pathTimestamp);
			putU32(v, m->m_blockedRepathTimestamp);
			putU32(v, m->m_adjustDestinations ? 1u : 0u);
			putU32(v, m->m_waitingForPath ? 1u : 0u);
			putU32(v, m->m_tryOneMoreRepath ? 1u : 0u);
			if (i == 2 || i == 3) {
				const AIFollowWaypointPathExactState *x = (const AIFollowWaypointPathExactState *)m;
				putU32(v, x->m_lastWaypoint ? (unsigned)x->m_lastWaypoint->m_index : 0xffffffffu);
				putU32(v, x->m_moveAsGroup ? 1u : 0u);
			} else if (i == 6) {
				const AIWanderInPlaceState *w = (const AIWanderInPlaceState *)m;
				putPos(v, w->m_origin);
				putU32(v, (unsigned)w->m_waitFrames);
				putU32(v, (unsigned)w->m_timer);
			} else {
				const AIFollowWaypointPathState *f = (const AIFollowWaypointPathState *)m;
				putU32(v, bitsOf(f->m_groupOffset.x));
				putU32(v, bitsOf(f->m_groupOffset.y));
				putU32(v, bitsOf(f->m_angle));
				putU32(v, (unsigned)f->m_framesSleeping);
				putU32(v, f->m_currentWaypoint ? (unsigned)f->m_currentWaypoint->m_index : 0xffffffffu);
				putU32(v, f->m_priorWaypoint ? (unsigned)f->m_priorWaypoint->m_index : 0xffffffffu);
				putU32(v, f->m_appendGoalPosition ? 1u : 0u);
				if (i == 4) {
					putU32(v, (unsigned)((const AIWanderState *)f)->m_waitFrames);
					putU32(v, (unsigned)((const AIWanderState *)f)->m_timer);
				} else if (i == 5) {
					putU32(v, (unsigned)((const AIPanicState *)f)->m_waitFrames);
					putU32(v, (unsigned)((const AIPanicState *)f)->m_timer);
				} else {
					putU32(v, f->m_moveAsGroup ? 1u : 0u);
					putU32(v, f->m_isFollowWaypointPathState ? 1u : 0u);
				}
			}
		}
		/* batch A10a: the attack states, their machines and their aim states */
		const StateID attacks[4] = { AI_ATTACK_POSITION, AI_ATTACK_OBJECT, AI_FORCE_ATTACK_OBJECT, AI_ATTACK_AND_FOLLOW_OBJECT };
		for (int i = 0; i < 4; ++i) {
			const AIAttackState *a = (const AIAttackState *)sm->m_stateMap.find(attacks[i])->second;
			putU32(v, a->m_attackMachine ? 1u : 0u);
			putPos(v, a->m_originalVictimPos);
			putU32(v, a->m_victimTeam ? 1u : 0u);
			putU32(v, a->m_lockedWeaponOnEnter ? (unsigned)a->m_lockedWeaponOnEnter->m_slot : 0xffffffffu);
			if (a->m_attackMachine) {
				const AttackStateMachine *m = a->m_attackMachine;
				putU32(v, (unsigned)m->getCurrentStateID());
				putU32(v, (unsigned)m->m_goalObjectID);
				putPos(v, m->m_goalPosition);
				putU32(v, m->m_sleepTill);
				const AIAttackAimAtTargetState *aim = (const AIAttackAimAtTargetState *)m->m_stateMap.find(AttackStateMachine::AIM_AT_TARGET)->second;
				putU32(v, aim->m_canTurnInPlace ? 1u : 0u);
				putU32(v, aim->m_setLocomotor ? 1u : 0u);
				/* batch A10b: the chase states' members (when the machine has them: not the continue or failure states) */
				for (int c = 0; c < 2; ++c) {
					std::map<StateID, State *>::const_iterator it = m->m_stateMap.find((StateID)c);
					if (it == m->m_stateMap.end()) continue;	/* (no approach state for an immobile or portable owner) */
					const State *st = it->second;
					const AIAttackApproachTargetState *ap = dynamic_cast<const AIAttackApproachTargetState *>(st);
					const AIAttackPursueTargetState *pu = dynamic_cast<const AIAttackPursueTargetState *>(st);
					if (!ap && !pu) continue;
					const AIInternalMoveToState *mt = (const AIInternalMoveToState *)st;
					putPos(v, mt->m_goalPosition);
					putU32(v, (unsigned)mt->m_goalLayer);
					putPos(v, mt->m_pathGoalPosition);
					putU32(v, (unsigned)mt->m_ambientPlayingHandle);
					putU32(v, mt->m_pathTimestamp);
					putU32(v, mt->m_blockedRepathTimestamp);
					putU32(v, mt->m_adjustDestinations ? 1u : 0u);
					putU32(v, mt->m_waitingForPath ? 1u : 0u);
					putU32(v, mt->m_tryOneMoreRepath ? 1u : 0u);
#define PUT_CHASE(x) \
					putPos(v, x->m_prevVictimPos); putU32(v, x->m_approachTimestamp); putU32(v, x->m_follow ? 1u : 0u); \
					putU32(v, x->m_isAttackingObject ? 1u : 0u); putU32(v, x->m_stopIfInRange ? 1u : 0u); \
					putU32(v, x->m_isInitialApproach ? 1u : 0u); putU32(v, x->m_isForceAttacking ? 1u : 0u);
					if (ap) { PUT_CHASE(ap) } else { PUT_CHASE(pu) }
#undef PUT_CHASE
				}
			}
		}
		/* batch A11: the attack-move state (its machine: state, goal, its crate and attack states) and the crate state */
		{
			const AIAttackMoveToState *am = (const AIAttackMoveToState *)sm->m_stateMap.find(AI_ATTACK_MOVE_TO)->second;
#define PUT_MOVE(mt) \
			putPos(v, (mt)->m_goalPosition); putU32(v, (unsigned)(mt)->m_goalLayer); putPos(v, (mt)->m_pathGoalPosition); \
			putU32(v, (unsigned)(mt)->m_ambientPlayingHandle); putU32(v, (mt)->m_pathTimestamp); putU32(v, (mt)->m_blockedRepathTimestamp); \
			putU32(v, (mt)->m_adjustDestinations ? 1u : 0u); putU32(v, (mt)->m_waitingForPath ? 1u : 0u); \
			putU32(v, (mt)->m_tryOneMoreRepath ? 1u : 0u);
			PUT_MOVE(am)
			putU32(v, am->m_frameToSleepUntil);
			putU32(v, (unsigned)am->m_retryCount);
			putU32(v, (unsigned)am->m_commandSrc);
			const StateMachine *m = am->m_attackMoveMachine;
			putU32(v, (unsigned)m->getCurrentStateID());
			putU32(v, (unsigned)m->m_goalObjectID);
			putU32(v, m->m_sleepTill);
			putPos(v, m->m_goalPosition);
			const AIPickUpCrateState *ic = (const AIPickUpCrateState *)m->m_stateMap.find(AI_PICK_UP_CRATE)->second;
			PUT_MOVE(ic)
			putU32(v, (unsigned)ic->m_delayCounter);
			const AIAttackState *ia = (const AIAttackState *)m->m_stateMap.find(AI_ATTACK_OBJECT)->second;
			putU32(v, ia->m_attackMachine ? 1u : 0u);
			putPos(v, ia->m_originalVictimPos);
			if (ia->m_attackMachine) {
				putU32(v, (unsigned)ia->m_attackMachine->getCurrentStateID());
				putU32(v, (unsigned)ia->m_attackMachine->m_goalObjectID);
			}
			const AIPickUpCrateState *pc = (const AIPickUpCrateState *)sm->m_stateMap.find(AI_PICK_UP_CRATE)->second;
			PUT_MOVE(pc)
			putU32(v, (unsigned)pc->m_delayCounter);
			/* batch A12: the hunt and attack-area states (their machines: state, goal, sleep, goal position, the crate state, the
			** attack state's machine) */
			for (int k = 0; k < 2; ++k) {
				const State *st = sm->m_stateMap.find(k ? AI_ATTACK_AREA : AI_HUNT)->second;
				const StateMachine *hm = k ? ((const AIAttackAreaState *)st)->m_attackMachine : ((const AIHuntState *)st)->m_huntMachine;
				putU32(v, k ? ((const AIAttackAreaState *)st)->m_nextEnemyScanTime : ((const AIHuntState *)st)->m_nextEnemyScanTime);
				putU32(v, hm ? 1u : 0u);
				if (!hm) continue;
				putU32(v, (unsigned)hm->getCurrentStateID());
				putU32(v, (unsigned)hm->m_goalObjectID);
				putU32(v, hm->m_sleepTill);
				putPos(v, hm->m_goalPosition);
				const AIPickUpCrateState *hc = (const AIPickUpCrateState *)hm->m_stateMap.find(AI_PICK_UP_CRATE)->second;
				PUT_MOVE(hc)
				putU32(v, (unsigned)hc->m_delayCounter);
				const AIAttackState *ha = (const AIAttackState *)hm->m_stateMap.find(AI_ATTACK_OBJECT)->second;
				putU32(v, ha->m_attackMachine ? 1u : 0u);
				putPos(v, ha->m_originalVictimPos);
				if (ha->m_attackMachine) {
					putU32(v, (unsigned)ha->m_attackMachine->getCurrentStateID());
					putU32(v, (unsigned)ha->m_attackMachine->m_goalObjectID);
				}
			}
			/* batch A12b: the attack-follow-waypoint states (as individuals, as a team): the waypoint path's members, their machines
			** (state, goal, sleep, goal position, the crate state, the attack state's machine) */
			for (int k = 0; k < 2; ++k) {
				const AIAttackFollowWaypointPathState *af = (const AIAttackFollowWaypointPathState *)sm->m_stateMap.find(
					k ? AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM : AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS)->second;
				PUT_MOVE(af)
				putU32(v, bitsOf(af->m_groupOffset.x));
				putU32(v, bitsOf(af->m_groupOffset.y));
				putU32(v, bitsOf(af->m_angle));
				putU32(v, (unsigned)af->m_framesSleeping);
				putU32(v, af->m_currentWaypoint ? (unsigned)af->m_currentWaypoint->m_index : 0xffffffffu);
				putU32(v, af->m_priorWaypoint ? (unsigned)af->m_priorWaypoint->m_index : 0xffffffffu);
				putU32(v, af->m_appendGoalPosition ? 1u : 0u);
				putU32(v, af->m_moveAsGroup ? 1u : 0u);
				putU32(v, af->m_isFollowWaypointPathState ? 1u : 0u);
				const StateMachine *fm = af->m_attackFollowMachine;
				putU32(v, (unsigned)fm->getCurrentStateID());
				putU32(v, (unsigned)fm->m_goalObjectID);
				putU32(v, fm->m_sleepTill);
				putPos(v, fm->m_goalPosition);
				const AIPickUpCrateState *fc = (const AIPickUpCrateState *)fm->m_stateMap.find(AI_PICK_UP_CRATE)->second;
				PUT_MOVE(fc)
				putU32(v, (unsigned)fc->m_delayCounter);
				const AIAttackState *fa = (const AIAttackState *)fm->m_stateMap.find(AI_ATTACK_OBJECT)->second;
				putU32(v, fa->m_attackMachine ? 1u : 0u);
				putPos(v, fa->m_originalVictimPos);
				if (fa->m_attackMachine) {
					putU32(v, (unsigned)fa->m_attackMachine->getCurrentStateID());
					putU32(v, (unsigned)fa->m_attackMachine->m_goalObjectID);
				}
			}
			/* batch A18: the guard state (its machine: state, goal, sleep, members; its states' members) */
			{
				const AIGuardState *gs = (const AIGuardState *)sm->m_stateMap.find(AI_GUARD)->second;
				const AIGuardMachine *gm = gs->m_guardMachine;
				putU32(v, gm ? 1u : 0u);
				if (gm) {
					putU32(v, (unsigned)gm->getCurrentStateID());
					putU32(v, (unsigned)gm->m_goalObjectID);
					putU32(v, gm->m_sleepTill);
					putU32(v, (unsigned)gm->m_targetToGuard);
					putU32(v, gm->m_areaToGuard ? (unsigned)gm->m_areaToGuard->m_index : 0xffffffffu);
					putPos(v, gm->m_positionToGuard);
					putU32(v, (unsigned)gm->m_nemesisToAttack);
					putU32(v, (unsigned)gm->m_guardMode);
#define PUT_EXIT(x) \
					putU32(v, (unsigned)(x).m_conditionsToConsider); putPos(v, (x).m_center); putU32(v, bitsOf((x).m_radiusSqr)); \
					putU32(v, (x).m_attackGiveUpFrame);
					const AIGuardInnerState *gi = (const AIGuardInnerState *)gm->m_stateMap.find(AI_GUARD_INNER)->second;
#define PUT_EMBEDDED(a) \
					if (a) { putU32(v, (a)->m_follow ? 1u : 0u); putU32(v, (a)->m_attackMachine ? (unsigned)(a)->m_attackMachine->getCurrentStateID() : 0xffffffffu); }
					PUT_EXIT(gi->m_exitConditions)
					putU32(v, gi->m_attackState ? 1u : gi->m_enterState ? 2u : 0u);
					PUT_EMBEDDED(gi->m_attackState)
					const AIGuardOuterState *go = (const AIGuardOuterState *)gm->m_stateMap.find(AI_GUARD_OUTER)->second;
					PUT_EXIT(go->m_exitConditions)
					putU32(v, go->m_attackState ? 1u : 0u);
					PUT_EMBEDDED(go->m_attackState)
					const AIGuardAttackAggressorState *ga = (const AIGuardAttackAggressorState *)gm->m_stateMap.find(AI_GUARD_ATTACK_AGGRESSOR)->second;
					PUT_EXIT(ga->m_exitConditions)
					putU32(v, ga->m_attackState ? 1u : 0u);
					PUT_EMBEDDED(ga->m_attackState)
#undef PUT_EMBEDDED
#undef PUT_EXIT
					const AIGuardReturnState *gr = (const AIGuardReturnState *)gm->m_stateMap.find(AI_GUARD_RETURN)->second;
					PUT_MOVE(gr)
					putU32(v, gr->m_nextReturnScanTime);
					const AIGuardIdleState *gl = (const AIGuardIdleState *)gm->m_stateMap.find(AI_GUARD_IDLE)->second;
					putU32(v, gl->m_nextEnemyScanTime);
					putPos(v, gl->m_guardeePos);
					const AIGuardPickUpCrateState *gc = (const AIGuardPickUpCrateState *)gm->m_stateMap.find(AI_GUARD_GET_CRATE)->second;
					PUT_MOVE(gc)
					putU32(v, (unsigned)gc->m_delayCounter);
				}
			}
			/* batch A19: the guard-retaliate state (its machine: state, goal, sleep, members; its states' members) */
			{
				const AIGuardRetaliateState *rs = (const AIGuardRetaliateState *)sm->m_stateMap.find(AI_GUARD_RETALIATE)->second;
				const AIGuardRetaliateMachine *rm = rs->m_guardRetaliateMachine;
				putU32(v, rm ? 1u : 0u);
				if (rm) {
					putU32(v, (unsigned)rm->getCurrentStateID());
					putU32(v, (unsigned)rm->m_goalObjectID);
					putU32(v, rm->m_sleepTill);
					putPos(v, rm->m_positionToGuard);
					putU32(v, (unsigned)rm->m_nemesisToAttack);
#define PUT_EXIT(x) \
					putU32(v, (unsigned)(x).m_conditionsToConsider); putPos(v, (x).m_center); putU32(v, bitsOf((x).m_radiusSqr)); \
					putU32(v, (x).m_attackGiveUpFrame);
#define PUT_EMBEDDED(a) \
					if (a) { putU32(v, (a)->m_follow ? 1u : 0u); putU32(v, (a)->m_attackMachine ? (unsigned)(a)->m_attackMachine->getCurrentStateID() : 0xffffffffu); }
					const AIGuardRetaliateInnerState *ri = (const AIGuardRetaliateInnerState *)rm->m_stateMap.find(AI_GUARD_RETALIATE_INNER)->second;
					PUT_EXIT(ri->m_exitConditions)
					putU32(v, ri->m_attackState ? 1u : ri->m_enterState ? 2u : 0u);
					PUT_EMBEDDED(ri->m_attackState)
					const AIGuardRetaliateOuterState *ro = (const AIGuardRetaliateOuterState *)rm->m_stateMap.find(AI_GUARD_RETALIATE_OUTER)->second;
					PUT_EXIT(ro->m_exitConditions)
					putU32(v, ro->m_attackState ? 1u : 0u);
					PUT_EMBEDDED(ro->m_attackState)
					const AIGuardRetaliateAttackAggressorState *ra = (const AIGuardRetaliateAttackAggressorState *)rm->m_stateMap.find(AI_GUARD_RETALIATE_ATTACK_AGGRESSOR)->second;
					PUT_EXIT(ra->m_exitConditions)
					putU32(v, ra->m_attackState ? 1u : 0u);
					PUT_EMBEDDED(ra->m_attackState)
#undef PUT_EMBEDDED
#undef PUT_EXIT
					const AIGuardRetaliateReturnState *rr = (const AIGuardRetaliateReturnState *)rm->m_stateMap.find(AI_GUARD_RETALIATE_RETURN)->second;
					PUT_MOVE(rr)
					putU32(v, rr->m_nextReturnScanTime);
					putU32(v, ((const AIGuardRetaliateIdleState *)rm->m_stateMap.find(AI_GUARD_RETALIATE_IDLE)->second)->m_nextEnemyScanTime);
					const AIGuardRetaliatePickUpCrateState *rc = (const AIGuardRetaliatePickUpCrateState *)rm->m_stateMap.find(AI_GUARD_RETALIATE_GET_CRATE)->second;
					PUT_MOVE(rc)
					putU32(v, (unsigned)rc->m_delayCounter);
				}
			}
			/* batch A20: the tunnel-network guard state (its machine: state, goal, sleep, members; its states' members) */
			{
				const AITunnelNetworkGuardState *ts = (const AITunnelNetworkGuardState *)sm->m_stateMap.find(AI_GUARD_TUNNEL_NETWORK)->second;
				const AITNGuardMachine *tm = ts->m_guardMachine;
				putU32(v, tm ? 1u : 0u);
				if (tm) {
					putU32(v, (unsigned)tm->getCurrentStateID());
					putU32(v, (unsigned)tm->m_goalObjectID);
					putU32(v, tm->m_sleepTill);
					putPos(v, tm->m_positionToGuard);
					putU32(v, (unsigned)tm->m_nemesisToAttack);
					putU32(v, (unsigned)tm->m_guardMode);
#define PUT_EMBEDDED(a) \
					putU32(v, (a) ? 1u : 0u); \
					if (a) { putU32(v, (a)->m_follow ? 1u : 0u); putU32(v, (a)->m_attackMachine ? (unsigned)(a)->m_attackMachine->getCurrentStateID() : 0xffffffffu); }
					const AITNGuardInnerState *ti = (const AITNGuardInnerState *)tm->m_stateMap.find(AI_TN_GUARD_INNER)->second;
					putU32(v, ti->m_exitConditions.m_attackGiveUpFrame);
					putU32(v, ti->m_scanForEnemy ? 1u : 0u);
					PUT_EMBEDDED(ti->m_attackState)
					const AITNGuardOuterState *to = (const AITNGuardOuterState *)tm->m_stateMap.find(AI_TN_GUARD_OUTER)->second;
					putU32(v, to->m_exitConditions.m_attackGiveUpFrame);
					PUT_EMBEDDED(to->m_attackState)
					const AITNGuardAttackAggressorState *ta = (const AITNGuardAttackAggressorState *)tm->m_stateMap.find(AI_TN_GUARD_ATTACK_AGGRESSOR)->second;
					putU32(v, ta->m_exitConditions.m_attackGiveUpFrame);
					PUT_EMBEDDED(ta->m_attackState)
#undef PUT_EMBEDDED
					const AITNGuardReturnState *tr = (const AITNGuardReturnState *)tm->m_stateMap.find(AI_TN_GUARD_RETURN)->second;
					PUT_MOVE(tr)
					putU32(v, (unsigned)tr->m_entryToClear);
					putU32(v, tr->m_nextReturnScanTime);
					putU32(v, ((const AITNGuardIdleState *)tm->m_stateMap.find(AI_TN_GUARD_IDLE)->second)->m_nextEnemyScanTime);
					const AITNGuardPickUpCrateState *tc = (const AITNGuardPickUpCrateState *)tm->m_stateMap.find(AI_TN_GUARD_GET_CRATE)->second;
					PUT_MOVE(tc)
					putU32(v, (unsigned)tc->m_delayCounter);
				}
			}
			/* batch A17: the dock state (its machine: state, goal, sleep, lock, approach position; its states' members) */
			{
				const AIDockState *dk = (const AIDockState *)sm->m_stateMap.find(AI_DOCK)->second;
				const AIDockMachine *dm = (const AIDockMachine *)dk->m_dockMachine;
				putU32(v, dm ? 1u : 0u);
				putU32(v, dk->m_usingPrecisionMovement ? 1u : 0u);
				if (dm) {
					putU32(v, (unsigned)dm->getCurrentStateID());
					putU32(v, (unsigned)dm->m_goalObjectID);
					putU32(v, dm->m_sleepTill);
					putU32(v, dm->m_locked ? 1u : 0u);
					putU32(v, (unsigned)dm->m_approachPosition);
					const StateID moves[6] = { AI_DOCK_APPROACH, AI_DOCK_ADVANCE_POSITION, AI_DOCK_MOVE_TO_ENTRY, AI_DOCK_MOVE_TO_DOCK,
						AI_DOCK_MOVE_TO_EXIT, AI_DOCK_MOVE_TO_RALLY };
					for (int k = 0; k < 6; ++k) {
						const AIInternalMoveToState *dmv = (const AIInternalMoveToState *)dm->m_stateMap.find(moves[k])->second;
						PUT_MOVE(dmv)
					}
					putU32(v, ((const AIDockWaitForClearanceState *)dm->m_stateMap.find(AI_DOCK_WAIT_FOR_CLEARANCE)->second)->m_enterFrame);
					const AIDockProcessDockState *pd = (const AIDockProcessDockState *)dm->m_stateMap.find(AI_DOCK_PROCESS_DOCK)->second;
					putU32(v, pd->m_nextDockActionFrame);
					putU32(v, (unsigned)pd->m_droneID);
				}
			}
			/* batch A16: the attack-squad state's machine (state, goal, its attack state's machine) */
			{
				const AIAttackSquadState *sq = (const AIAttackSquadState *)sm->m_stateMap.find(AI_ATTACK_SQUAD)->second;
				const StateMachine *qm = sq->m_attackSquadMachine;
				putU32(v, qm ? 1u : 0u);
				if (qm) {
					putU32(v, (unsigned)qm->getCurrentStateID());
					putU32(v, (unsigned)qm->m_goalObjectID);
					putU32(v, qm->m_sleepTill);
					const AIAttackState *qa = (const AIAttackState *)qm->m_stateMap.find(AI_ATTACK_OBJECT)->second;
					putU32(v, qa->m_attackMachine ? 1u : 0u);
					if (qa->m_attackMachine) putU32(v, (unsigned)qa->m_attackMachine->getCurrentStateID());
				}
			}
			/* batch A15: the rappel state's members */
			{
				const AIRappelState *ra = (const AIRappelState *)sm->m_stateMap.find(AI_RAPPEL_INTO)->second;
				putU32(v, bitsOf(ra->m_rappelRate));
				putU32(v, bitsOf(ra->m_destZ));
				putU32(v, ra->m_targetIsBldg ? 1u : 0u);
			}
			/* batch A14: the enter state's move members and entry to clear */
			{
				const AIEnterState *en = (const AIEnterState *)sm->m_stateMap.find(AI_ENTER)->second;
				PUT_MOVE(en)
				putU32(v, (unsigned)en->m_entryToClear);
			}
			/* batch A13: the exit states' entries to clear, the face states' turn-in-place flags */
			putU32(v, (unsigned)((const AIExitState *)sm->m_stateMap.find(AI_EXIT)->second)->m_entryToClear);
			putU32(v, (unsigned)((const AIExitInstantlyState *)sm->m_stateMap.find(AI_EXIT_INSTANTLY)->second)->m_entryToClear);
			putU32(v, ((const AIFaceState *)sm->m_stateMap.find(AI_FACE_OBJECT)->second)->m_canTurnInPlace ? 1u : 0u);
			putU32(v, ((const AIFaceState *)sm->m_stateMap.find(AI_FACE_POSITION)->second)->m_canTurnInPlace ? 1u : 0u);
#undef PUT_MOVE
		}
		const AIIdleState *idle = (const AIIdleState *)sm->m_stateMap.find(AI_IDLE)->second;
		putU32(v, (unsigned)idle->m_initialSleepOffset);
		putU32(v, idle->m_shouldLookForTargets ? 1u : 0u);
		putU32(v, idle->m_inited ? 1u : 0u);
	}
	/* batch A31: a DeployStyleAIUpdate's members */
	if (g_aiEnv.subclass == 3) {
		const DeployStyleAIUpdate *d = (const DeployStyleAIUpdate *)ai;
		putU32(v, (unsigned)d->m_state);
		putU32(v, d->m_frameToWaitForDeploy);
	}
	/* batch A32: an AssaultTransportAIUpdate's members */
	if (g_aiEnv.subclass == 4) {
		const AssaultTransportAIUpdate *a = (const AssaultTransportAIUpdate *)ai;
		putU32(v, (unsigned)a->m_currentMembers);
		for (int i = 0; i < MAX_TRANSPORT_SLOTS; ++i) {
			putU32(v, (unsigned)a->m_memberIDs[i]);
			putU32(v, a->m_memberHealing[i] ? 1u : 0u);
			putU32(v, a->m_newMember[i] ? 1u : 0u);
		}
		putPos(v, a->m_attackMoveGoalPos);
		putU32(v, (unsigned)a->m_designatedTarget);
		putU32(v, (unsigned)a->m_state);
		putU32(v, a->m_framesRemaining);
		putU32(v, a->m_isAttackMove ? 1u : 0u);
		putU32(v, a->m_isAttackObject ? 1u : 0u);
		putU32(v, a->m_newOccupantsAreNewMembers ? 1u : 0u);
	}
	/* batch A38: a DozerAIUpdate's members and machines */
	if (g_aiEnv.subclass == 10) {
		const DozerAIUpdate *d = (const DozerAIUpdate *)ai;
		for (int i = 0; i < DOZER_NUM_TASKS; ++i) { putU32(v, (unsigned)d->m_task[i].m_targetObjectID); putU32(v, d->m_task[i].m_taskOrderFrame); }
		putU32(v, (unsigned)d->m_currentTask);
		for (int i = 0; i < DOZER_NUM_TASKS; ++i)
			for (int j = 0; j < DOZER_NUM_DOCK_POINTS; ++j) { putU32(v, d->m_dockPoint[i][j].valid ? 1u : 0u); putPos(v, d->m_dockPoint[i][j].location); }
		putU32(v, (unsigned)d->m_buildSubTask);
		putU32(v, d->m_isRebuild ? 1u : 0u);
		putU32(v, d->m_buildingSound.m_playingHandle);
		const DozerPrimaryStateMachine *m = d->m_dozerMachine;
		putU32(v, m->m_currentState ? (unsigned)m->m_currentState->getID() : (unsigned)INVALID_STATE_ID);
		putU32(v, m->m_sleepTill);
		putU32(v, m->m_defaultStateInited ? 1u : 0u);
		const DozerPrimaryIdleState *idle = (const DozerPrimaryIdleState *)m->m_stateMap.find(DOZER_PRIMARY_IDLE)->second;
		putU32(v, idle->m_idleTooLongTimestamp);
		putU32(v, (unsigned)idle->m_idlePlayerNumber);
		putU32(v, idle->m_isMarkedAsIdle ? 1u : 0u);
		for (int s = DOZER_PRIMARY_BUILD; s <= DOZER_PRIMARY_FORTIFY; ++s) {
			const DozerActionState *a = (const DozerActionState *)m->m_stateMap.find(s)->second;
			const StateMachine *am = a->m_actionMachine;
			putU32(v, am->m_currentState ? (unsigned)am->m_currentState->getID() : (unsigned)INVALID_STATE_ID);
			putU32(v, am->m_sleepTill);
			putU32(v, (unsigned)am->m_goalObjectID);
			putPos(v, am->m_goalPosition);
			putU32(v, (unsigned)((const DozerActionPickActionPosState *)am->m_stateMap.find(DOZER_ACTION_PICK_ACTION_POS)->second)->m_failedAttempts);
			putU32(v, ((const DozerActionDoActionState *)am->m_stateMap.find(DOZER_ACTION_DO_ACTION)->second)->m_enterFrame);
		}
	}
	/* batch A37: a DeliverPayloadAIUpdate's members, data and machine */
	if (g_aiEnv.subclass == 9) {
		const DeliverPayloadAIUpdate *d = (const DeliverPayloadAIUpdate *)ai;
		putPos(v, d->m_targetPos);
		putPos(v, d->m_moveToPos);
		putU32(v, (unsigned)d->m_visibleItemsDelivered);
		putU32(v, d->m_deliveryDecal.m_empty ? 1u : 0u);
		putF(v, d->m_previousDistanceSqr);
		putU32(v, d->m_freeToExit ? 1u : 0u);
		putU32(v, d->m_acceptingCommands ? 1u : 0u);
		putU32(v, (unsigned)d->m_diveState);
		const DeliverPayloadData &a = d->m_data;
		putU32(v, (unsigned)a.m_visibleDropBoneName.getLength());
		putU32(v, (unsigned)a.m_visibleSubObjectName.getLength());
		putU32(v, (unsigned)a.m_visiblePayloadTemplateName.getLength());
		putF(v, a.m_distToTarget);
		putF(v, a.m_preOpenDistance);
		putU32(v, (unsigned)a.m_maxAttempts);
		putPos(v, a.m_dropOffset);
		putPos(v, a.m_dropVariance);
		putU32(v, a.m_dropDelay);
		putU32(v, a.m_fireWeapon ? 1u : 0u);
		putU32(v, a.m_selfDestructObject ? 1u : 0u);
		putU32(v, (unsigned)a.m_visibleNumBones);
		putF(v, a.m_diveStartDistance);
		putF(v, a.m_diveEndDistance);
		putU32(v, (unsigned)a.m_strafingWeaponSlot);
		putU32(v, (unsigned)a.m_visibleItemsDroppedPerInterval);
		putU32(v, a.m_inheritTransportVelocity ? 1u : 0u);
		putU32(v, a.m_isParachuteDirectly ? 1u : 0u);
		putF(v, a.m_exitPitchRate);
		putU32(v, a.m_strafeFX ? (unsigned)(a.m_strafeFX - g_missileFx) : 0xffffffffu);
		putF(v, a.m_strafeLength);
		putU32(v, a.m_visiblePayloadWeaponTemplate ? (unsigned)a.m_visiblePayloadWeaponTemplate->m_index : 0xffffffffu);
		const RadiusDecalTemplate &t = a.m_deliveryDecalTemplate;
		putU32(v, (unsigned)t.m_name.getLength());
		putU32(v, (unsigned)t.m_shadowType);
		putF(v, t.m_minOpacity);
		putF(v, t.m_maxOpacity);
		putU32(v, t.m_opacityThrobTime);
		putU32(v, (unsigned)t.m_color);
		putU32(v, t.m_onlyVisibleToOwningPlayer ? 1u : 0u);
		putF(v, a.m_deliveryDecalRadius);
		const DeliverPayloadStateMachine *m = d->m_deliverPayloadStateMachine;
		putU32(v, m ? 1u : 0u);
		if (m) {
			putU32(v, m->m_currentState ? (unsigned)m->m_currentState->getID() : (unsigned)INVALID_STATE_ID);
			putU32(v, m->m_sleepTill);
			putU32(v, m->m_defaultStateInited ? 1u : 0u);
			const DeliveringState *ds = (const DeliveringState *)m->m_stateMap.find(DELIVERING)->second;
			putU32(v, ds->m_dropDelayLeft);
			putU32(v, ds->m_didOpen ? 1u : 0u);
			putU32(v, (unsigned)((const ConsiderNewApproachState *)m->m_stateMap.find(CONSIDER_NEW_APPROACH)->second)->m_numberEntriesToState);
			putU32(v, ((const RecoverFromOffMapState *)m->m_stateMap.find(RECOVER_FROM_OFF_MAP)->second)->m_reEntryFrame);
			putPos(v, ((const HeadOffMapState *)m->m_stateMap.find(HEAD_OFF_MAP)->second)->facingDirectionUponDelivery);
		}
	}
	/* batch A36: a MissileAIUpdate's members */
	if (g_aiEnv.subclass == 8) {
		const MissileAIUpdate *m = (const MissileAIUpdate *)ai;
		putU32(v, (unsigned)m->m_state);
		putU32(v, m->m_stateTimestamp);
		putU32(v, m->m_nextTargetTrackTime);
		putU32(v, (unsigned)m->m_launcherID);
		putU32(v, (unsigned)m->m_victimID);
		putU32(v, m->m_fuelExpirationDate);
		putF(v, m->m_noTurnDistLeft);
		putF(v, m->m_maxAccel);
		putPos(v, m->m_originalTargetPos);
		putPos(v, m->m_prevPos);
		putU32(v, m->m_extraBonusFlags);
		putU32(v, m->m_detonationWeaponTmpl ? (unsigned)m->m_detonationWeaponTmpl->m_index : 0xffffffffu);
		putU32(v, m->m_exhaustSysTmpl ? (unsigned)m->m_exhaustSysTmpl->m_index : 0xffffffffu);
		putU32(v, (unsigned)m->m_exhaustID);
		putU32(v, m->m_framesTillDecoyed);
		const Bool flags[] = { m->m_isTrackingTarget, m->m_isArmed, m->m_noDamage, m->m_isJammed };
		for (Bool f : flags) putU32(v, f ? 1u : 0u);
	}
	/* batch A35: a SupplyTruckAIUpdate's members and its machine's */
	if (g_aiEnv.subclass == 7) {
		const SupplyTruckAIUpdate *t = (const SupplyTruckAIUpdate *)ai;
		putU32(v, (unsigned)t->m_preferredDock);
		putU32(v, (unsigned)t->m_numberBoxes);
		putU32(v, t->m_forcePending ? 1u : 0u);
		putU32(v, t->m_forcedBusyPending ? 1u : 0u);
		const SupplyTruckStateMachine *m = t->m_supplyTruckStateMachine;
		putU32(v, m->m_currentState ? (unsigned)m->m_currentState->getID() : (unsigned)INVALID_STATE_ID);
		putU32(v, m->m_sleepTill);
		putU32(v, m->m_defaultStateInited ? 1u : 0u);
	}
	/* batch A34: a HackInternetAIUpdate's members (the waiting command's storage) and its machine's three states' frames */
	if (g_aiEnv.subclass == 6) {
		const HackInternetAIUpdate *k = (const HackInternetAIUpdate *)ai;
		const AICommandParmsStorage &c = k->m_pendingCommand;
		putU32(v, k->m_hasPendingCommand ? 1u : 0u);
		putU32(v, (unsigned)c.m_cmd);
		putU32(v, (unsigned)c.m_cmdSource);
		putPos(v, c.m_pos);
		putU32(v, (unsigned)c.m_obj);
		putU32(v, (unsigned)c.m_otherObj);
		putU32(v, (unsigned)c.m_teamName.getLength());
		putU32(v, (unsigned)c.m_coords.size());
		for (size_t i = 0; i < c.m_coords.size(); ++i) putPos(v, c.m_coords[i]);
		putU32(v, c.m_waypoint ? (unsigned)c.m_waypoint->m_index : 0xffffffffu);
		putU32(v, c.m_polygon ? (unsigned)c.m_polygon->m_index : 0xffffffffu);
		putU32(v, (unsigned)c.m_intValue);
		putU32(v, c.m_commandButton ? (unsigned)c.m_commandButton->m_index : 0xffffffffu);
		putU32(v, c.m_path ? 1u : 0u);
		if (sm) {
			const StateID ids[3] = { UNPACKING, HACK_INTERNET, PACKING };
			putU32(v, ((const UnpackingState *)sm->m_stateMap.find(ids[0])->second)->m_framesRemaining);
			putU32(v, ((const HackInternetState *)sm->m_stateMap.find(ids[1])->second)->m_framesRemaining);
			putU32(v, ((const PackingState *)sm->m_stateMap.find(ids[2])->second)->m_framesRemaining);
		}
	}
	/* batch A33: a RailedTransportAIUpdate's members */
	if (g_aiEnv.subclass == 5) {
		const RailedTransportAIUpdate *t = (const RailedTransportAIUpdate *)ai;
		putU32(v, t->m_inTransit ? 1u : 0u);
		for (int i = 0; i < 32; ++i) { putU32(v, t->m_path[i].startWaypointID); putU32(v, t->m_path[i].endWaypointID); }
		putU32(v, (unsigned)t->m_numPaths);
		putU32(v, (unsigned)t->m_currentPath);
		putU32(v, t->m_waypointDataLoaded ? 1u : 0u);
	}
	int n = (int)v.size();
	if (n <= cap && n > 0) memcpy(buf, &v[0], 4 * n);
	return n;
}

}  /* extern "C" */

/* batch A30: a transport's passenger's command (LOG_PASSENGER, see the subclasses above) */
void passengerAiCommand(unsigned kind, const Object *passenger, const Object *victim, const Coord3D *pos, Int shots, CommandSourceType src)
{
	unsigned b[3] = { 0, 0, 0 };
	if (pos) { b[0] = bitsOf(pos->x); b[1] = bitsOf(pos->y); b[2] = bitsOf(pos->z); }
	L(LOG_PASSENGER, { kind, (unsigned)passenger->getID(), victim ? (unsigned)victim->getID() : 0u, b[0], b[1], b[2], (unsigned)shots, (unsigned)src });
}

/* batch A32: an assault member's AI (LOG_MEMBER_AI, see AssaultTransportAIUpdate above); other objects' bodies' health (the hash's,
** over 100), their isContained (the assault transport's passengers) */
static AssaultMemberAi g_memberAi;
AssaultMemberAi *memberAiOf(const Object *o)
{
	if (!o || !const_cast<Object *>(o)->getAI()) return NULL;
	g_memberAi.m_member = o;
	return &g_memberAi;
}
static unsigned memberId(const AssaultMemberAi *a) { return (unsigned)a->m_member->getID(); }
CommandSourceType AssaultMemberAi::getLastCommandSource() const
{
	CommandSourceType s = p1Pick(p1Hash(130, { memberId(this), g_aiEnv.frame }), g_aiEnv.objectRate) ? CMD_FROM_AI : CMD_FROM_PLAYER;
	L(LOG_MEMBER_AI, { 0, memberId(this), (unsigned)s });
	return s;
}
void AssaultMemberAi::setAllowedToChase(Bool allowed) { L(LOG_MEMBER_AI, { 1, memberId(this), allowed ? 1u : 0u }); }
void AssaultMemberAi::aiExit(Object *o, CommandSourceType s) { L(LOG_MEMBER_AI, { 2, memberId(this), o ? (unsigned)o->getID() : 0u, (unsigned)s }); }
void AssaultMemberAi::aiEnter(Object *o, CommandSourceType s) { L(LOG_MEMBER_AI, { 3, memberId(this), o ? (unsigned)o->getID() : 0u, (unsigned)s }); }
AIStateType AssaultMemberAi::getAIStateType() const
{
	L(LOG_MEMBER_AI, { 4, memberId(this), (unsigned)g_aiEnv.otherAiState });
	return (AIStateType)g_aiEnv.otherAiState;
}
Bool AssaultMemberAi::isMoving() const
{
	bool r = p1Pick(p1Hash(131, { memberId(this), g_aiEnv.frame }), 50);
	L(LOG_MEMBER_AI, { 5, memberId(this), r ? 1u : 0u });
	return r;
}
Object *AssaultMemberAi::getGoalObject()
{
	Object *g = otherAiGoalObject();
	L(LOG_MEMBER_AI, { 6, memberId(this), g ? (unsigned)g->getID() : 0u });
	return g;
}
void AssaultMemberAi::aiAttackObject(Object *v, Int shots, CommandSourceType s)
{
	L(LOG_MEMBER_AI, { 7, memberId(this), v ? (unsigned)v->getID() : 0u, (unsigned)shots, (unsigned)s });
}
void AssaultMemberAi::aiAttackMoveToPosition(const Coord3D *p, Int shots, CommandSourceType s)
{
	L(LOG_MEMBER_AI, { 8, memberId(this), bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)shots, (unsigned)s });
}
/* batch A37: a delivered item's */
void AssaultMemberAi::aiMoveToPosition(const Coord3D *p, CommandSourceType s)
{
	L(LOG_MEMBER_AI, { 9, memberId(this), bitsOf(p->x), bitsOf(p->y), bitsOf(p->z), (unsigned)s });
}
static Real healthOf(unsigned k)
{
	static const Real h[4] = { 100.0f, 100.0f, 55.0f, 15.0f };
	return h[(p1Hash(132, { k, g_aiEnv.frame >> 5 }) >> 8) & 3];
}
Real BodyModuleInterface::getHealth() const { return healthOf(bodyOwner(this)); }
Real BodyModuleInterface::getMaxHealth() const { return 100.0f; }
Bool Object::isContained() const
{
	return m_otherId && g_aiEnv.subclass == 4 && ((g_aiEnv.containedMask >> m_otherId) & 1) != 0;
}

/* batch A33: the railed transport's docks (LOG_RAILED) */
static RailedTransportDockUpdateInterface g_railedDock;
DockUpdateInterface *railedOwnDock(Object *us) { return (g_aiEnv.railFlags & 1) ? &g_dock : NULL; }
void DockUpdateInterface::setDockOpen(Bool open) { L(LOG_RAILED, { 0, open ? 1u : 0u }); }
RailedTransportDockUpdateInterface *BehaviorModule::getRailedTransportDockUpdateInterface()
{
	return (m_index == 1 && (g_aiEnv.railFlags & 2)) ? &g_railedDock : NULL;
}
Bool RailedTransportDockUpdateInterface::isLoadingOrUnloading()
{
	bool r = p1Pick(p1Hash(133, { g_aiEnv.frame }), 30);
	L(LOG_RAILED, { 1, r ? 1u : 0u });
	return r;
}
void RailedTransportDockUpdateInterface::unloadAll() { L(LOG_RAILED, { 2 }); }

/* batch A34: the hacker's world (LOG_HACK): the money, the score, the experience (the scene's), locally controlled (the scene's), the
** floating cash text (a cut point), the drawable's loop; the team's name and the factory's one team, the buttons' names */
static Money g_money;
static ScoreKeeper g_scoreKeeper;
static ExperienceTracker g_experience;
Money *Player::getMoney() { return (g_aiEnv.hackFlags & 1) ? &g_money : NULL; }
ScoreKeeper *Player::getScoreKeeper() { return &g_scoreKeeper; }
void Money::deposit(UnsignedInt amount, Bool playSound) { L(LOG_HACK, { 0, amount }); }
void ScoreKeeper::addMoneyEarned(Int earned) { L(LOG_HACK, { 1, (unsigned)earned }); }
ExperienceTracker *Object::getExperienceTracker() const { return (g_aiEnv.hackFlags & 2) ? &g_experience : NULL; }
VeterancyLevel ExperienceTracker::getVeterancyLevel() const { return (VeterancyLevel)g_aiEnv.hackLevel; }
void ExperienceTracker::addExperiencePoints(Int xp, Bool canScale) { L(LOG_HACK, { 2, (unsigned)xp, canScale ? 1u : 0u }); }
Bool Object::isLocallyControlled() const
{
	Bool r = m_otherId ? (g_aiEnv.hackFlags & 8) != 0 : (g_aiEnv.hackFlags & 4) != 0;
	L(LOG_HACK, { 3, (unsigned)getID(), r ? 1u : 0u });
	return r;
}
void hackCashText(const Object *owner, UnsignedInt amount) { L(LOG_HACK, { 4, amount }); }
void Drawable::setAnimationLoopDuration(UnsignedInt numFrames) { L(LOG_HACK, { 5, numFrames }); }
static AsciiString g_teamName("team0");
const AsciiString& Team::getName() const { return g_teamName; }
static TeamFactory g_teamFactory;
TeamFactory *TheTeamFactory = &g_teamFactory;
Team *TeamFactory::findTeam(const AsciiString& name) { return name == g_teamName ? &g_team : NULL; }
const AsciiString& CommandButton::getName() const
{
	const_cast<CommandButton *>(this)->m_name.format("btn%d", m_index);
	return m_name;
}
const CommandButton *ControlBar::findCommandButton(const AsciiString& name)
{
	for (int i = 0; i < 4; ++i) if (g_buttons[i].getName() == name) return &g_buttons[i];
	return NULL;
}

/* batch A34: the virtual isIdle's dispatch (see AIUpdateInterface's declaration): a hacker's own override, else the base's */
Bool AIUpdateInterface::isIdle() const
{
	if (this == g_ai_update && g_aiEnv.subclass == 6) return ((const HackInternetAIUpdate *)this)->hackIsIdle();
	return baseIsIdle();
}

/* batch A35: the supply truck's world (LOG_SUPPLY) and the virtual getSupplyTruckAIInterface's dispatch (a supply truck itself) */
static ResourceGatheringManager g_resources;
ResourceGatheringManager *Player::getResourceGatheringManager() { return &g_resources; }
Object *ResourceGatheringManager::findBestSupplyCenter(Object *queryObject)
{
	Object *o = findObject((ObjectID)g_aiEnv.supplyBest[0]);
	L(LOG_SUPPLY, { 0, o ? (unsigned)o->getID() : 0u });
	return o;
}
Object *ResourceGatheringManager::findBestSupplyWarehouse(Object *queryObject)
{
	Object *o = findObject((ObjectID)g_aiEnv.supplyBest[1]);
	L(LOG_SUPPLY, { 1, o ? (unsigned)o->getID() : 0u });
	return o;
}
Object *Player::findClosestByKindOf(Object *queryObject, KindOfMaskType setKindOf, KindOfMaskType clearKindOf)
{
	const KindOfType kinds[3] = { KINDOF_CASH_GENERATOR, KINDOF_COMMANDCENTER, KINDOF_STRUCTURE };
	for (int i = 0; i < 3; ++i) {
		if (!setKindOf.test(kinds[i])) continue;
		Object *o = findObject((ObjectID)g_aiEnv.supplyClosest[i]);
		L(LOG_SUPPLY, { 2, (unsigned)kinds[i], o ? (unsigned)o->getID() : 0u });
		return o;
	}
	abort();
}
Real PartitionManager::getDistanceSquared(const Object *obj, const Object *otherObj, DistanceCalculationType dc, Coord3D *vec)
{
	/* (batch A36: FROM_CENTER_2D and FROM_BOUNDINGSPHERE_3D, the missile's) */
	if (dc == FROM_CENTER_2D || dc == FROM_BOUNDINGSPHERE_3D)
		return missileDistance(dc == FROM_CENTER_2D ? 0 : 1, (unsigned)otherObj->getID(), obj->getPosition(), otherObj->getPosition());
	/* (the stand-in's: FROM_BOUNDINGSPHERE_2D as the centres' 2-D distance) */
	if (dc != FROM_BOUNDINGSPHERE_2D) abort();
	const Coord3D *a = obj->getPosition();
	const Coord3D *b = otherObj->getPosition();
	Real dx = b->x - a->x, dy = b->y - a->y;
	Real d = dx * dx + dy * dy;
	L(LOG_SUPPLY, { 3, (unsigned)otherObj->getID(), bitsOf(d) });
	return d;
}
void Drawable::updateDrawableSupplyStatus(Int maxSupply, Int currentSupply) { L(LOG_SUPPLY, { 4, (unsigned)maxSupply, (unsigned)currentSupply }); }
SupplyTruckAIInterface *AIUpdateInterface::getSupplyTruckAIInterface()
{
	if (this == g_ai_update && g_aiEnv.subclass == 7) return (SupplyTruckAIUpdate *)this;
	return NULL;
}

/* batch A36: the missile's world (LOG_MISSILE): the weapon templates and the store, the particle systems, the launcher's bonus, the
** countermeasure, the garrison's immunity, the kinds, the distances, the damage, the drawable */
WeaponTemplate g_weaponTemplates[3];
ParticleSystemTemplate g_particleTemplates[3];
FXList g_missileFx[2];
static WeaponStore g_weaponStore;
WeaponStore *TheWeaponStore = &g_weaponStore;
static ParticleSystemManager g_particles;
ParticleSystemManager *TheParticleSystemManager = &g_particles;
Real WeaponTemplate::getWeaponSpeed() const { return g_aiEnv.missileWeaponSpeeds[m_index]; }
Bool WeaponTemplate::shouldProjectileCollideWith(const Object* launcher, const Object* projectile, const Object* other, ObjectID victim) const
{
	bool r = p1Pick(p1Hash(134, { (unsigned)m_index, launcher ? (unsigned)launcher->getID() : 0u, (unsigned)other->getID(), (unsigned)victim }), g_aiEnv.missileCollideRate);
	L(LOG_MISSILE, { 3, (unsigned)m_index, launcher ? (unsigned)launcher->getID() : 0u, (unsigned)other->getID(), (unsigned)victim, r ? 1u : 0u });
	return r;
}
Bool WeaponTemplate::getDieOnDetonate() const { return ((g_aiEnv.missileDieMask >> m_index) & 1) != 0; }
const AsciiString& WeaponTemplate::getName() const { const_cast<WeaponTemplate *>(this)->m_name.format("wt%d", m_index); return m_name; }
void WeaponStore::handleProjectileDetonation(const WeaponTemplate* w, const Object *source, const Coord3D* pos, WeaponBonusConditionFlags bonus, Bool inflictDamage)
{
	L(LOG_MISSILE, { 4, (unsigned)w->m_index, (unsigned)bonus, inflictDamage ? 1u : 0u });
}
const WeaponTemplate *WeaponStore::findWeaponTemplate(AsciiString name) const
{
	for (int i = 0; i < 3; ++i) if (g_weaponTemplates[i].getName() == name) return &g_weaponTemplates[i];
	return NULL;
}
const AsciiString& ParticleSystemTemplate::getName() const { const_cast<ParticleSystemTemplate *>(this)->m_name.format("ps%d", m_index); return m_name; }
ParticleSystemID ParticleSystemManager::createAttachedParticleSystemID(const ParticleSystemTemplate *t, Object* attachTo, Bool createSlaves)
{
	unsigned id = g_nextParticleId++;
	L(LOG_MISSILE, { 5, (unsigned)t->m_index, id });
	return (ParticleSystemID)id;
}
void ParticleSystemManager::destroyParticleSystemByID(ParticleSystemID id) { L(LOG_MISSILE, { 6, (unsigned)id }); }
const ParticleSystemTemplate *ParticleSystemManager::findTemplate(const AsciiString &name) const
{
	for (int i = 0; i < 3; ++i) if (g_particleTemplates[i].getName() == name) return &g_particleTemplates[i];
	return NULL;
}
void Weapon::positionProjectileForLaunch(Object *projectile, const Object *launcher, WeaponSlotType wslot, Int barrel)
{
	L(LOG_MISSILE, { 2, launcher ? (unsigned)launcher->getID() : 0u, (unsigned)wslot, (unsigned)barrel });
}
WeaponBonusConditionFlags Object::getWeaponBonusCondition() const { return p1Hash(135, { (unsigned)getID() }) & 0xff; }
ObjectID Object::calculateCountermeasureToDivertTo(const Object& victim)
{
	Object *o = findObject((ObjectID)g_aiEnv.missileCountermeasure);
	L(LOG_MISSILE, { 8, (unsigned)victim.getID(), o ? (unsigned)o->getID() : 0u });
	return o ? o->getID() : INVALID_ID;
}
Bool ContainModuleInterface::isImmuneToClearBuildingAttacks() const { return g_aiEnv.missileGarrisonImmune != 0; }
Bool Object::isKindOfMulti(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) const
{
	for (int k = 0; k < 256; ++k) {
		if (mustBeSet.test((KindOfType)k) && !isKindOf((KindOfType)k)) return FALSE;
		if (mustBeClear.test((KindOfType)k) && isKindOf((KindOfType)k)) return FALSE;
	}
	return TRUE;
}
void Object::attemptDamage(DamageInfo *d)
{
	L(LOG_MISSILE, { 0, (unsigned)d->in.m_damageType, (unsigned)d->in.m_deathType, (unsigned)d->in.m_sourceID, bitsOf(d->in.m_amount) });
}
void Drawable::setDrawableHidden(Bool hidden) { L(LOG_MISSILE, { 1, hidden ? 1u : 0u }); }
/* (the stand-in's distances: FROM_CENTER_2D the centres' 2-D, FROM_BOUNDINGSPHERE_3D the centres' 3-D, LOG_MISSILE kind 7) */
static Real missileDistance(int kind, unsigned it, const Coord3D *a, const Coord3D *b)
{
	Real dx = b->x - a->x, dy = b->y - a->y, dz = b->z - a->z;
	Real d = kind == 1 ? dx * dx + dy * dy + dz * dz : dx * dx + dy * dy;
	L(LOG_MISSILE, { 7, (unsigned)kind, it, bitsOf(d) });
	return d;
}

/* batch A37: the delivery's world (LOG_DELIVER): the decal, the dive's sound position, the weapon lock, the strafe FX, the drawable's
** subobjects and bones, an item's position, contain, modules and force, the visible payload (a new object: its template, producer,
** position, orientation, physics, projectile, AI), the partition, the physics' turning; the explicit dispatch of
** isAllowedToRespondToAiCommands and deliverPayloadViaModuleData (the delivering AI itself, subclass 9) */
static void putName(std::vector<unsigned> &w, const char *s) { size_t n = strlen(s); w.push_back((unsigned)n); for (size_t i = 0; i < n; ++i) w.push_back((unsigned)(unsigned char)s[i]); }
static void logDeliver(std::vector<unsigned> w)
{
	if (g_aiQuiet) return;
	g_aiLog->push_back((unsigned)LOG_DELIVER);
	g_aiLog->push_back((unsigned)w.size());
	for (unsigned x : w) g_aiLog->push_back(x);
}
void RadiusDecalTemplate::createRadiusDecal(const Coord3D& pos, Real radius, const Player* owningPlayer, RadiusDecal& result) const
{
	std::vector<unsigned> w = { 0u, bitsOf(pos.x), bitsOf(pos.y), bitsOf(pos.z), bitsOf(radius), owningPlayer ? 1u : 0u, (unsigned)m_shadowType,
		bitsOf(m_minOpacity), bitsOf(m_maxOpacity), m_opacityThrobTime, (unsigned)m_color, m_onlyVisibleToOwningPlayer ? 1u : 0u };
	putName(w, m_name.str());
	logDeliver(w);
	/* (upstream: result cleared, then not empty with a player, a texture and a radius) */
	result.m_empty = !(owningPlayer && m_name.isNotEmpty() && radius > 0.0f);
}
void RadiusDecal::clear() { m_empty = true; logDeliver({ 2u }); }
void RadiusDecal::update() { logDeliver({ 1u }); }
void AudioEventRTS::setPosition(const Coord3D *pos) { logDeliver({ 3u, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
Bool Object::setWeaponLock(WeaponSlotType weaponSlot, WeaponLockType lockType)
{
	logDeliver({ 4u, (unsigned)weaponSlot, (unsigned)lockType });
	return TRUE;
}
void FXList::doFXPos(const FXList* fx, const Coord3D *primary, const Matrix3D* primaryMtx, const Real primarySpeed, const Coord3D *secondary, const Real overrideRadius)
{
	/* (FXList.h's inline: nothing without a list) */
	if (fx) logDeliver({ 5u, (unsigned)(fx - g_missileFx), bitsOf(primary->x), bitsOf(primary->y), bitsOf(primary->z) });
}
void Drawable::showSubObject(const AsciiString& name, Bool show)
{
	std::vector<unsigned> w = { 6u, show ? 1u : 0u };
	putName(w, name.str());
	logDeliver(w);
}
void Drawable::updateSubObjects() { logDeliver({ 7u }); }
/* the bones: found at the hash's rate, at (2 * index, 3, 1) */
Int Drawable::getPristineBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
{
	Int n = (p1Hash(140, { (unsigned)startIndex, g_aiEnv.frame }) & 1) ? 1 : 0;
	if (n) { positions[0].x = 2.0f * (Real)startIndex; positions[0].y = 3.0f; positions[0].z = 1.0f; }
	std::vector<unsigned> w = { 8u, (unsigned)startIndex, (unsigned)maxBones, (unsigned)n };
	putName(w, boneNamePrefix);
	logDeliver(w);
	return n;
}
/* (Thing's: the stand-in moves the bone by the owner's position) */
void Drawable::convertBonePosToWorldPos(const Coord3D* bonePos, const Matrix3D* boneTransform, Coord3D* worldPos, Matrix3D* worldTransform) const
{
	Coord3D b = *bonePos;
	const Coord3D *o = g_object.getPosition();
	worldPos->x = b.x + o->x; worldPos->y = b.y + o->y; worldPos->z = b.z + o->z;
	logDeliver({ 9u, bitsOf(worldPos->x), bitsOf(worldPos->y), bitsOf(worldPos->z) });
}
void setItemPosition(Object *item, const Coord3D *pos) { logDeliver({ 10u, (unsigned)item->getID(), bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
void ContainModuleInterface::setOverrideDestination(const Coord3D *pos) { logDeliver({ 11u, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
void GenerateMinefieldBehavior::setMinefieldTarget(const Coord3D* pos) { logDeliver({ 12u, (unsigned)m_id, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
void SmartBombTargetHomingUpdate::SetTargetPosition(const Coord3D& target) { logDeliver({ 13u, (unsigned)m_id, bitsOf(target.x), bitsOf(target.y), bitsOf(target.z) }); }
void PhysicsBehavior::applyForce(const Coord3D *force) { logDeliver({ 14u, bitsOf(force->x), bitsOf(force->y), bitsOf(force->z) }); }
/* the visible payload: "pt0" and "pt1" are templates (the scene's), others not; each new object's id counts from 40 in each case */
namespace { ThingFactory g_thingFactory; BehaviorModule g_payloadModules[2]; BehaviorModule *g_payloadModuleList[3];
ProjectileUpdateInterface g_payloadProjectile; }
ThingFactory *TheThingFactory = &g_thingFactory;
const AiTemplateView *ThingFactory::findTemplate(const AsciiString& name)
{
	if (strcmp(name.str(), "pt0") == 0) return &g_payloadTemplates[0];
	if (strcmp(name.str(), "pt1") == 0) return &g_payloadTemplates[1];
	return NULL;
}
PayloadObject *ThingFactory::newObject(const AiTemplateView *tmplate, Team *team)
{
	g_payload.m_id = (ObjectID)g_nextPayloadId++;
	g_payload.m_pos.zero();
	logDeliver({ 15u, (unsigned)(tmplate - g_payloadTemplates), (unsigned)g_payload.m_id });
	return &g_payload;
}
void PayloadObject::setProducer(const Object* obj) { logDeliver({ 16u, (unsigned)m_id, obj ? (unsigned)obj->getID() : 0u }); }
void PayloadObject::setPosition(const Coord3D *pos) { m_pos = *pos; logDeliver({ 17u, (unsigned)m_id, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) }); }
void PayloadObject::setOrientation(Real angle) { logDeliver({ 18u, (unsigned)m_id, bitsOf(angle) }); }
Real PayloadPhysics::getMass() const { return g_aiEnv.deliverWorld[1]; }
void PayloadPhysics::applyMotiveForce(const Coord3D *force) { logDeliver({ 20u, (unsigned)g_payload.m_id, bitsOf(force->x), bitsOf(force->y), bitsOf(force->z) }); }
Real PayloadPhysics::getCenterOfMassOffset() const { return g_aiEnv.deliverWorld[2]; }
void PayloadPhysics::setPitchRate(Real pitch) { logDeliver({ 23u, (unsigned)g_payload.m_id, bitsOf(pitch) }); }
/* (its modules: a projectile's when the scene says so, after another) */
BehaviorModule **PayloadObject::getBehaviorModules()
{
	g_payloadModules[0].m_index = 101;
	g_payloadModules[1].m_index = 100;
	g_payloadModuleList[0] = &g_payloadModules[0];
	g_payloadModuleList[1] = ((g_aiEnv.deliverFlags >> 1) & 1) ? &g_payloadModules[1] : NULL;
	g_payloadModuleList[2] = NULL;
	return g_payloadModuleList;
}
ProjectileUpdateInterface *BehaviorModule::getProjectileUpdateInterface() { return m_index == 100 ? &g_payloadProjectile : NULL; }
void ProjectileUpdateInterface::projectileFireAtObjectOrPosition(const Object *victim, const Coord3D *victimPos, const WeaponTemplate *detWeap,
	const ParticleSystemTemplate* exhaustSysOverride)
{
	logDeliver({ 21u, (unsigned)g_payload.m_id, victim ? (unsigned)victim->getID() : 0u, bitsOf(victimPos->x), bitsOf(victimPos->y), bitsOf(victimPos->z),
		detWeap ? (unsigned)detWeap->m_index : 0xffffffffu, exhaustSysOverride ? (unsigned)exhaustSysOverride->m_index : 0xffffffffu });
}
PayloadAi *PayloadObject::getAIUpdateInterface() { return ((g_aiEnv.deliverFlags >> 2) & 1) ? &m_ai : NULL; }
void PayloadAi::aiMoveToPosition(const Coord3D *pos, CommandSourceType cmdSource)
{
	logDeliver({ 24u, (unsigned)g_payload.m_id, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), (unsigned)cmdSource });
}
/* (the exhaust per veterancy: (template + level) % 4, none for 3) */
const ParticleSystemTemplate *WeaponTemplate::getProjectileExhaust(VeterancyLevel v) const
{
	unsigned k = ((unsigned)m_index + (unsigned)v) % 4u;
	return k == 3 ? NULL : &g_particleTemplates[k];
}
VeterancyLevel Object::getVeterancyLevel() const
{
	/* (Object.cpp's: `m_experienceTracker ? m_experienceTracker->getVeterancyLevel() : LEVEL_REGULAR`) */
	ExperienceTracker *t = getExperienceTracker();
	return t ? t->getVeterancyLevel() : LEVEL_REGULAR;
}
Team *Player::getDefaultTeam() { return g_object.getTeam(); }
void TerrainLogic::getExtentIncludingBorder(Region3D *r) const
{
	Real b = g_aiEnv.deliverWorld[0];
	r->lo.x = -b; r->lo.y = -b; r->lo.z = 0.0f;
	r->hi.x = g_aiEnv.extentHi[0] + b; r->hi.y = g_aiEnv.extentHi[1] + b; r->hi.z = 0.0f;
}
void PartitionManager::registerObject(Object *obj) { logDeliver({ 25u, (unsigned)obj->getID() }); }
void PartitionManager::unRegisterObject(Object *obj) { logDeliver({ 26u, (unsigned)obj->getID() }); }
PhysicsTurningType PhysicsBehavior::getTurning(void) const { return (PhysicsTurningType)g_aiEnv.deliverTurning; }
/* (Object.cpp's: the drawable's clearAndSetModelConditionState, if any; the stand-in's log is clearAndSetModelConditionFlags') */
void Object::clearAndSetModelConditionState(ModelConditionFlagType clr, ModelConditionFlagType set)
{
	if (getDrawable()) clearAndSetModelConditionFlags(MAKE_MODELCONDITION_MASK(clr), MAKE_MODELCONDITION_MASK(set));
}
Bool AIUpdateInterface::isAllowedToRespondToAiCommands(const AICommandParms* parms) const
{
	if (this == g_ai_update && g_aiEnv.subclass == 9) return ((const DeliverPayloadAIUpdate *)this)->deliverIsAllowedToRespondToAiCommands(parms);
	return baseIsAllowedToRespondToAiCommands(parms);
}
void deliverPayloadViaModuleDataOf(DeliverPayloadAIUpdate *dp, const Coord3D *pos)
{
	/* (findUpdateModule answered the delivering AI itself) */
	((DeliverPayloadAIUpdate *)g_ai_update)->deliverPayloadViaModuleData(pos);
}

/* batch A38: the dozer's world (LOG_DOZER): its goal objects (another object's: their healer, builder and construction percent the
** hash's, the percent set kept for the operation; their health A32's), the player's notices, the bridges (the scene's), the UI,
** the build assistant, the constructed object, the bored search; the explicit dispatch of getDozerAIInterface */
static void logDozer(std::vector<unsigned> w)
{
	if (g_aiQuiet) return;
	g_aiLog->push_back((unsigned)LOG_DOZER);
	g_aiLog->push_back((unsigned)w.size());
	for (unsigned x : w) g_aiLog->push_back(x);
}
DozerAIInterface *AIUpdateInterface::getDozerAIInterface()
{
	if (this == g_ai_update && g_aiEnv.subclass == 10) return (DozerAIUpdate *)this;
	return NULL;
}
void Object::setWeaponSetFlag(WeaponSetType wst) { logDozer({ 0u, (unsigned)wst, 1u }); }
void Object::clearWeaponSetFlag(WeaponSetType wst) { logDozer({ 0u, (unsigned)wst, 0u }); }
/* (another object's: INVALID_ID, the dozer, or the next id, by the hash, the same for 32 frames) */
ObjectID Object::getSoleHealingBenefactor() const
{
	unsigned k = p1Hash(141, { (unsigned)getID(), g_aiEnv.frame >> 5 }) % 8u;
	ObjectID r = k < 4 ? INVALID_ID : k < 7 ? (ObjectID)g_aiEnv.objectId : (ObjectID)(getID() + 1);
	logDozer({ 1u, (unsigned)getID(), (unsigned)r });
	return r;
}
ObjectID Object::getBuilderID() const
{
	ObjectID r = (p1Hash(142, { (unsigned)getID(), g_aiEnv.frame >> 5 }) & 7) ? (ObjectID)g_aiEnv.objectId : INVALID_ID;
	logDozer({ 2u, (unsigned)getID(), (unsigned)r });
	return r;
}
void Object::setBuilder(const Object *obj) { logDozer({ 3u, (unsigned)getID(), obj ? (unsigned)obj->getID() : 0u }); }
void Object::setProducer(const Object *obj) { logDozer({ 4u, (unsigned)getID(), obj ? (unsigned)obj->getID() : 0u }); }
void Object::setConstructionPercent(Real percent)
{
	m_constructionPercent = percent;
	m_constructionPercentSet = TRUE;
	logDozer({ 5u, (unsigned)getID(), bitsOf(percent) });
}
Real Object::getConstructionPercent() const
{
	static const Real start[4] = { 0.0f, 50.0f, 99.5f, 120.0f };
	Real r = m_constructionPercentSet ? m_constructionPercent : start[p1Hash(143, { (unsigned)getID(), g_aiEnv.frame }) & 3];
	logDozer({ 6u, (unsigned)getID(), bitsOf(r) });
	return r;
}
void BodyModuleInterface::internalChangeHealth(Real delta) { logDozer({ 7u, bodyOwner(this), bitsOf(delta) }); }
void BodyModuleInterface::evaluateVisualCondition() { logDozer({ 8u, bodyOwner(this) }); }
void Player::onStructureConstructionComplete(Object *builder, Object *structure, Bool isRebuild)
{
	logDozer({ 9u, builder ? (unsigned)builder->getID() : 0u, (unsigned)structure->getID(), isRebuild ? 1u : 0u });
}
void AcademyStats::recordProduction(const Object *obj, const Object *constructor)
{
	logDozer({ 10u, (unsigned)obj->getID(), constructor ? (unsigned)constructor->getID() : 0u });
}
/* (the goal object's create modules: module i when the scene's bit i says so) */
namespace { CreateModuleInterface g_creates[3]; }
CreateModuleInterface *BehaviorModule::getCreate()
{
	if (m_index < 0 || m_index >= 3 || !((g_aiEnv.dozerMasks[3] >> m_index) & 1)) return NULL;
	g_creates[m_index].m_index = m_index;
	return &g_creates[m_index];
}
void CreateModuleInterface::onBuildComplete() { logDozer({ 11u, (unsigned)m_index }); }
void Object::handlePartitionCellMaintenance() { logDozer({ 12u, (unsigned)getID() }); }
void Object::updateUpgradeModules() { logDozer({ 13u, (unsigned)getID() }); }
void dozerConstructionCompleteUi(Object *dozer, Object *goalObject) { logDozer({ 14u, (unsigned)goalObject->getID() }); }
namespace { InGameUI g_inGameUi; BuildAssistant g_buildAssistant; }
InGameUI *TheInGameUI = &g_inGameUi;
BuildAssistant *TheBuildAssistant = &g_buildAssistant;
void InGameUI::message(const char *stringToLookup)
{
	std::vector<unsigned> w = { 15u };
	size_t n = strlen(stringToLookup);
	w.push_back((unsigned)n);
	for (size_t i = 0; i < n; ++i) w.push_back((unsigned)(unsigned char)stringToLookup[i]);
	logDozer(w);
}
void InGameUI::addIdleWorker(Object *obj) { logDozer({ 16u, (unsigned)obj->getID() }); }
void InGameUI::removeIdleWorker(Object *obj, Int playerNumber) { logDozer({ 17u, (unsigned)obj->getID(), (unsigned)playerNumber }); }
/* the bridges: towers (the scene's mask; and BRIDGE_TOWER, as upstream asks) with their bridge (the scene's), bridges (the scene's
** mask) with their four towers (the scene's), the scaffold moving by the hash */
namespace { BridgeTowerBehaviorInterface g_towerInterface; BridgeBehaviorInterface g_bridgeInterface; }
BridgeTowerBehaviorInterface *BridgeTowerBehavior::getBridgeTowerBehaviorInterfaceFromObject(Object *obj)
{
	if (obj == NULL || obj->isKindOf(KINDOF_BRIDGE_TOWER) == FALSE) return NULL;
	Bool r = obj->getID() < 32 && ((g_aiEnv.dozerMasks[0] >> obj->getID()) & 1);
	logDozer({ 18u, (unsigned)obj->getID(), r ? 1u : 0u });
	if (!r) return NULL;
	g_towerInterface.m_tower = obj->getID();
	return &g_towerInterface;
}
ObjectID BridgeTowerBehaviorInterface::getBridgeID(void) { return (ObjectID)g_aiEnv.dozerBridge; }
BridgeBehaviorInterface *BridgeBehavior::getBridgeBehaviorInterfaceFromObject(Object *obj)
{
	if (obj == NULL) return NULL;
	Bool r = obj->getID() < 32 && ((g_aiEnv.dozerMasks[1] >> obj->getID()) & 1);
	logDozer({ 19u, (unsigned)obj->getID(), r ? 1u : 0u });
	if (!r) return NULL;
	g_bridgeInterface.m_bridge = obj->getID();
	return &g_bridgeInterface;
}
ObjectID BridgeBehaviorInterface::getTowerID(BridgeTowerType towerType) { return (ObjectID)g_aiEnv.dozerTowers[towerType]; }
Bool BridgeBehaviorInterface::isScaffoldInMotion(void)
{
	Bool r = (p1Hash(145, { (unsigned)m_bridge, g_aiEnv.frame }) & 1) != 0;
	logDozer({ 20u, (unsigned)m_bridge, r ? 1u : 0u });
	return r;
}
void BridgeBehaviorInterface::createScaffolding(void) { logDozer({ 21u, (unsigned)m_bridge, 1u }); }
void BridgeBehaviorInterface::removeScaffolding(void) { logDozer({ 21u, (unsigned)m_bridge, 0u }); }
Bool Object::attemptHealingFromSoleBenefactor(Real amount, const Object* source, UnsignedInt duration)
{
	Bool r = (p1Hash(144, { (unsigned)getID(), g_aiEnv.frame }) & 3) != 0;
	logDozer({ 22u, (unsigned)getID(), bitsOf(amount), source ? (unsigned)source->getID() : 0u, duration, r ? 1u : 0u });
	return r;
}
Real GeometryInfo::getBoundingSphereRadius() const { return g_aiEnv.dozerReals[4]; }
Bool Object::isUsingAirborneLocomotor( void ) const
{
	/* (Object.cpp's, its m_ai the object's AI) */
	const AIUpdateInterface *m_ai = getAI();
	return ( m_ai && m_ai->getCurLocomotor() && ((m_ai->getCurLocomotor()->getLegalSurfaces() & LOCOMOTORSURFACE_AIR) != 0) );
}
/* the bored dozer's search: the existing objects of the scene's repair mask, in id order */
ObjectIterator *PartitionManager::iterateObjectsInRange(const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters,
	IterOrderType order)
{
	static ObjectIterator iter;
	iter.m_ids.clear();
	iter.m_at = 0;
	for (unsigned i = 1; i < 32; ++i)
		if (((g_aiEnv.dozerMasks[2] >> i) & 1) && findObject((ObjectID)i)) iter.m_ids.push_back((ObjectID)i);
	logDozer({ 23u, bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z), bitsOf(maxDist), (unsigned)dc, (unsigned)iter.m_ids.size() });
	return &iter;
}
Object *ObjectIterator::first() { m_at = 0; return next(); }
Object *ObjectIterator::next() { return m_at < m_ids.size() ? findObject(m_ids[m_at++]) : NULL; }
CanMakeType BuildAssistant::canMakeUnit(Object *builder, const AiTemplateView *whatToBuild) const
{
	Bool r = (p1Hash(146, { g_aiEnv.frame }) & 3) != 0;
	logDozer({ 24u, (unsigned)(whatToBuild - g_payloadTemplates), r ? 1u : 0u });
	return r ? CANMAKE_OK : CANMAKE_NO_PREREQ;
}
LegalBuildCode BuildAssistant::isLocationLegalToBuild(const Coord3D *worldPos, const AiTemplateView *build, Real angle, UnsignedInt options,
	Object *builderObject, Player *player)
{
	Bool r = (p1Hash(147, { g_aiEnv.frame }) & 3) != 0;
	logDozer({ 25u, (unsigned)(build - g_payloadTemplates), bitsOf(worldPos->x), bitsOf(worldPos->y), bitsOf(worldPos->z), bitsOf(angle),
		options, builderObject ? (unsigned)builderObject->getID() : 0u, player ? 1u : 0u, r ? 1u : 0u });
	return r ? LBC_OK : LBC_RESTRICTED_TERRAIN;
}
/* the constructed object: the scene's id (an object of the scene, or else one only this operation knows) */
namespace { Object g_newStructure; }
Object *ThingFactory::newObject(const AiTemplateView *tmplate, Team *team, ObjectStatusMaskType statusBits)
{
	ObjectID id = (ObjectID)g_aiEnv.dozerInts[3];
	Object *o = findObject(id);
	if (!o) { g_newStructure = Object(); g_newStructure.m_otherId = id; o = &g_newStructure; }
	logDozer({ 26u, (unsigned)(tmplate - g_payloadTemplates), (unsigned)statusBits.m_bits, (unsigned)id });
	return o;
}
Int AiTemplateView::calcTimeToBuild(const Player *player) const { return g_aiEnv.dozerInts[0]; }
Int AiTemplateView::calcCostToBuild(const Player *player) const { return g_aiEnv.dozerInts[1]; }
void Money::withdraw(UnsignedInt amountToWithdraw, Bool playSound) { logDozer({ 27u, amountToWithdraw }); }
void TerrainLogic::flattenTerrain(Object *obj) { logDozer({ 28u, (unsigned)obj->getID() }); }
/* (Pathfinder::addObjectToPathfindMap is AIPathfind.h's inline: classifyObjectFootprint(obj, true), here logged) */
void Pathfinder::classifyObjectFootprint(Object *obj, Bool insert) { logDozer({ 29u, (unsigned)obj->getID(), insert ? 1u : 0u }); }
void Player::onStructureCreated(Object *builder, Object *structure)
{
	logDozer({ 30u, builder ? (unsigned)builder->getID() : 0u, (unsigned)structure->getID() });
}
void newStructureSetPosition(Object *obj, const Coord3D *pos)
{
	obj->m_cachedPos = *pos;
	logDozer({ 31u, (unsigned)obj->getID(), bitsOf(pos->x), bitsOf(pos->y), bitsOf(pos->z) });
}
void newStructureSetOrientation(Object *obj, Real angle) { logDozer({ 32u, (unsigned)obj->getID(), bitsOf(angle) }); }
Int Player::getPlayerIndex() const { return g_aiEnv.dozerInts[2]; }
Bool ActionManager::canRepairObject(const Object *obj, const Object *objectToRepair, CommandSourceType commandSource)
{
	return actionAnswer(5, objectToRepair, commandSource);
}
Bool ActionManager::canResumeConstructionOf(const Object *obj, const Object *objectBeingConstructed, CommandSourceType commandSource)
{
	return actionAnswer(6, objectBeingConstructed, commandSource);
}
