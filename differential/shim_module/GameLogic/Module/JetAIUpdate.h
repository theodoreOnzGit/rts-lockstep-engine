/* Stand-in for GameEngine/Include/GameLogic/Module/JetAIUpdate.h, Module build only (differential tests, p1w13-sonnet, batch R74:
** ParkingPlaceBehavior finds a parked jet's update by name and asks it two questions; the real header needs the whole AI layer, layer 10's).
** The jet is an update module whose answers are the test's (differential/l9_parking_r74.inc). */
#ifndef SHIM_MODULE_JETAIUPDATE_H
#define SHIM_MODULE_JETAIUPDATE_H
#include "GameLogic/Module/UpdateModule.h"
class JetAIUpdate : public UpdateModule
{
public:
	JetAIUpdate( Thing *thing, const ModuleData *moduleData ) : UpdateModule( thing, moduleData ) {}
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual void crc( Xfer * ) {}
	virtual void xfer( Xfer * ) {}
	virtual void loadPostProcess() {}
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }
	/* JetAIUpdate.h lines 119 and 130: the module data's parking offset; whether a takeoff or a landing is in progress (the test's) */
	Real friend_getParkingOffset() const { return m_parkingOffset; }
	Bool friend_isTakeoffOrLandingInProgress() const { return m_takeoffOrLanding; }
	Real m_parkingOffset = 0.0f;
	Bool m_takeoffOrLanding = false;
};
#endif
