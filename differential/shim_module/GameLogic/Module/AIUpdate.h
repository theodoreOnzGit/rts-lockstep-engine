/* Stand-in for GameEngine/Include/GameLogic/Module/AIUpdate.h, Module build only (differential tests, p1w10-sonnet, upgrade batch B): the one call
** LocomotorSetUpgrade makes. Renamed ModTestAIUpdateInterface by module_prelude.h. */
#ifndef SHIM_MODULE_AIUPDATE_H
#define SHIM_MODULE_AIUPDATE_H
#include "Lib/BaseType.h"
class Object;
class AIUpdateInterface { public: void setLocomotorUpgrade(Bool set);
	/* --- p1w13-sonnet --- (update batch O: SquishCollide asks what the AI is going for) */
	Object *getGoalObject() const { return m_goal; }
	Object *m_goal = 0;
	/* --- end p1w13-sonnet --- */
};
#endif
