/* Stand-in for GameEngine/Include/Common/GameState.h, score keeper tests only
** (p1w6-energy): the SC_* codes ScoreKeeper.cpp throws (SC_INVALID_DATA, at load
** of a count the factory does not know). Its save/load game-state members are not
** used here (the wrapper's XferLoad of the snapshot is upstream's). */
#ifndef SHIM_SCORE_GAMESTATE_H
#define SHIM_SCORE_GAMESTATE_H
enum
{
	SC_INVALID_DATA = 1,
};
#endif
