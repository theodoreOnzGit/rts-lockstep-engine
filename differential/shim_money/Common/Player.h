/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), money and
** handicap tests only. Money::deposit calls getAcademyStats()->recordIncome()
** on the player at its index; the wrapper counts those calls. */
#ifndef SHIM_MONEY_PLAYER_H
#define SHIM_MONEY_PLAYER_H
#include "Lib/BaseType.h"
class AcademyStats
{
public:
	void recordIncome();
};
class Player
{
public:
	AcademyStats *getAcademyStats() { return &m_academyStats; }
private:
	AcademyStats m_academyStats;
};
#endif
