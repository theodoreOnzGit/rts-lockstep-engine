/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), Upgrade/
** SpecialPower build only (differential tests, p1w2-upgrade). Upgrade.cpp
** and SpecialPower.cpp include it for the Player-taking methods
** (canAffordUpgrade, calcTimeToBuild's cheat, canUseSpecialPower), which are
** compiled but never called by the tests. The classes are renamed by the
** wrapper's macros so that they cannot clash with the other builds'. */
#ifndef SHIM_UPGRADE_PLAYER_H
#define SHIM_UPGRADE_PLAYER_H
#include "Lib/BaseType.h"
#include "Common/Science.h"
class Money
{
public:
	UnsignedInt countMoney() const { return 0; }
};
class Player
{
public:
	Money *getMoney() { return &m_money; }
	Bool buildsInstantly() { return FALSE; }
	Bool hasScience(ScienceType) const { return FALSE; }
private:
	Money m_money;
};
#endif
