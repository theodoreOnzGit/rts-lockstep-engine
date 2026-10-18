/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8), energy and
** stats tests only: TheGameLogic->getFrame(), the frame the wrapper sets. */
#ifndef SHIM_ENERGY_GAMELOGIC_H
#define SHIM_ENERGY_GAMELOGIC_H
#include "Lib/BaseType.h"
class GameLogic
{
public:
	UnsignedInt getFrame() const;
	Bool isScoringEnabled() const;
};
extern GameLogic *TheGameLogic;
#endif
