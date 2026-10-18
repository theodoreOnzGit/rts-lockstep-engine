/* Stand-in for GameLogic/Module/ContainModule.h in the sides/scripts build:
** EvacDisposition (ContainModule.h lines 57-64), which Parameter::getUiText names. */
#ifndef SHIM_SIDES_CONTAINMODULE_H
#define SHIM_SIDES_CONTAINMODULE_H
enum EvacDisposition
{
	EVAC_INVALID = 0,
	EVAC_TO_LEFT,
	EVAC_TO_RIGHT,
	EVAC_BURST_FROM_CENTER,
};
#endif
