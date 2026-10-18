/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), score keeper tests
** only (p1w6-energy): the one method ScoreKeeper.cpp calls, getPlayerIndex(). */
#ifndef SHIM_SCORE_PLAYER_H
#define SHIM_SCORE_PLAYER_H
#include "Lib/BaseType.h"
class Player
{
public:
	Player(Int index) : m_index(index) {}
	Int getPlayerIndex() const { return m_index; }
	Int m_index;
};
#endif
