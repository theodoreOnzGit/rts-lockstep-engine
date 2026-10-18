/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), energy and
** score/stats tests only (p1w6-energy). Energy.cpp, ScoreKeeper.cpp,
** MissionStats.cpp and AcademyStats.cpp reach a Player for: its index
** (getPlayerIndex), the brown-out notification (onPowerBrownOutChange, logged
** by the wrapper), and, for the academy statistics, its template, money,
** energy and object iteration (each wrapper says which it uses). Renamed
** per archive by the wrapper's macros. */
#ifndef SHIM_ENERGY_PLAYER_H
#define SHIM_ENERGY_PLAYER_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
class Player
{
public:
	Player() : m_index(0), m_brownOutNotices(0), m_lastBrownOut(0) {}
	Int getPlayerIndex() const { return m_index; }
	void onPowerBrownOutChange(Bool brownOut);
	Int m_index;
	Int m_brownOutNotices;
	Bool m_lastBrownOut;
};
#endif
