/* Stand-in for GameEngine/Include/GameLogic/GameLogic.h (layer 8), score keeper
** tests only: isScoringEnabled(), the flag the wrapper sets for each call. */
#ifndef SHIM_SCORE_GAMELOGIC_H
#define SHIM_SCORE_GAMELOGIC_H
#include "Lib/BaseType.h"
class GameLogic
{
public:
	Bool isScoringEnabled() const;
};
extern GameLogic *TheGameLogic;
#endif
