/* Stand-in for GameEngine/Include/GameLogic/Module/SupplyTruckAIUpdate.h, Module build only (differential tests, p1w13-sonnet, batch R44: the
** supply docks). The real header needs the AI layer; this is the interface's calls the docks make (SupplyTruckAIUpdate.h:166-182, the
** declarations verbatim), answered by differential/l9_supply_docks_r44.inc's truck. */
#ifndef SHIM_MODULE_SUPPLYTRUCKAIUPDATE_H
#define SHIM_MODULE_SUPPLYTRUCKAIUPDATE_H
#include "Lib/BaseType.h"
class SupplyTruckAIInterface
{
public:
	virtual Bool loseOneBox() = 0;
	virtual Bool gainOneBox( Int remainingStock ) = 0;
	virtual void setForceWantingState(Bool v) = 0;
	virtual Int getUpgradedSupplyBoost() const = 0;
};
#endif
