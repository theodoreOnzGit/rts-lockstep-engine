/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), Upgrade/
** SpecialPower build only (differential tests, p1w2-upgrade): the two
** members SpecialPowerStore::canUseSpecialPower calls, which the tests never
** reach. */
#ifndef SHIM_UPGRADE_OBJECT_H
#define SHIM_UPGRADE_OBJECT_H
#include "Common/Player.h"
class SpecialPowerTemplate;
class Object
{
public:
	void *getSpecialPowerModule(const SpecialPowerTemplate *) const { return 0; }
	Player *getControllingPlayer() const { return 0; }
};
#endif
