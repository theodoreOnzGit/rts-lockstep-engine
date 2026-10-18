/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8 singleton, not translated), FiringTracker
** tests only: getFrame and findObjectByID, the two members FiringTracker.cpp calls, and TheGameLogic.
** The wrapper renames the class and the global and defines the members (wrap_firing_tracker.cpp). */
#ifndef SHIM_FIRING_GAMELOGIC_H
#define SHIM_FIRING_GAMELOGIC_H
#include "Common/GameType.h"
class Object;
class GameLogic
{
public:
	UnsignedInt getFrame() const;
	Object *findObjectByID(ObjectID id) const;
};
extern GameLogic *TheGameLogic;
#endif
