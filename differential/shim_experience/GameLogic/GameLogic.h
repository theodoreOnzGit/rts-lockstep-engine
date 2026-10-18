/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8 singleton, not translated),
** ExperienceTracker tests only: findObjectByID, the one member ExperienceTracker.cpp calls.
** The wrapper renames the class and the global (wrap_experience_tracker.cpp). */
#ifndef SHIM_EXPERIENCE_GAMELOGIC_H
#define SHIM_EXPERIENCE_GAMELOGIC_H
#include "Common/GameType.h"
class Object;
class GameLogic
{
public:
	Object *findObjectByID(ObjectID id);
};
extern GameLogic *TheGameLogic;
#endif
