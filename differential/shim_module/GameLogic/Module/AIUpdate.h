/* Stand-in for GameEngine/Include/GameLogic/Module/AIUpdate.h, Module build only (differential tests, p1w10-sonnet, upgrade batch B): the one call
** LocomotorSetUpgrade makes. Renamed ModTestAIUpdateInterface by module_prelude.h. */
#ifndef SHIM_MODULE_AIUPDATE_H
#define SHIM_MODULE_AIUPDATE_H
#include "Lib/BaseType.h"
#include "Common/GameCommon.h"
#include <vector>
class Object;
class AIUpdateInterface { public: void setLocomotorUpgrade(Bool set);
	/* --- p1w13-sonnet --- (update batch O: SquishCollide asks what the AI is going for) */
	Object *getGoalObject() const { return m_goal; }
	Object *m_goal = 0;
	/* --- end p1w13-sonnet --- */
	/* --- p1w13-sonnet --- (batch P: ActiveBody; the calls are lines of the effect log, differential/l9_active_body_p.inc) */
	Bool isMoving() const { return m_moving; }
	Bool isIdle() const { return m_idle; }
	void aiIdle( CommandSourceType cmdSource );
	void aiEvacuateInstantly( Bool exposeStealthUnits, CommandSourceType cmdSource );
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
	/* --- end p1w13-sonnet --- */
};
#endif
