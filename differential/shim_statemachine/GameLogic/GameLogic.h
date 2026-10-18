/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h, differential tests
** only (StateMachine build; the class is renamed SmGameLogic by the prelude).
** The two calls StateMachine.cpp makes: TheGameLogic->getFrame() and
** TheGameLogic->findObjectByID(). Defined in differential/wrap_statemachine.cpp
** over a frame counter and a registry of live objects the test controls. */
#ifndef SM_SHIM_GAMELOGIC_H
#define SM_SHIM_GAMELOGIC_H
#include "Lib/BaseType.h"
#include "Common/GameType.h"
class Object;
class GameLogic
{
public:
	UnsignedInt getFrame();
	Object *findObjectByID(ObjectID id);
};
extern GameLogic *TheGameLogic;
#endif
