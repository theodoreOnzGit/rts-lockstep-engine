/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8), academy tests
** only: getFrame(), the frame the wrapper sets for each call. */
#ifndef SHIM_ACADEMY_GAMELOGIC_H
#define SHIM_ACADEMY_GAMELOGIC_H
#include "Lib/BaseType.h"
class GameLogic
{
public:
	UnsignedInt getFrame() const;
};
extern GameLogic *TheGameLogic;
#endif
