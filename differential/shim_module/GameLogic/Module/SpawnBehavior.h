/* Stand-in for GameEngine/Include/GameLogic/Module/SpawnBehavior.h, Module build only (differential tests, p1w10-sonnet, upgrade batch B): the one
** call StealthUpgrade makes. Renamed ModTestSpawnBehaviorInterface by module_prelude.h. */
#ifndef SHIM_MODULE_SPAWNBEHAVIOR_H
#define SHIM_MODULE_SPAWNBEHAVIOR_H
#include "Lib/BaseType.h"
class Object;
/* p1w13-sonnet: getClosestSlave (HiveStructureBody, differential/l9_active_body_p.inc) answers the scene's object; m_ownerId names the owner in the effect log */
class SpawnBehaviorInterface
{
public:
	SpawnBehaviorInterface() : m_closestSlave(0), m_ownerId(0) {}
	void giveSlavesStealthUpgrade(Bool giveUpgrade);
	Object *getClosestSlave(const Coord3D *pos);
	Object *m_closestSlave;
	int m_ownerId;
	/* p1w13-sonnet, batch R26: StealthUpdate asks whether the slaves all stealth, and reveals them (a line: differential/l9_stealth_r26.inc) */
	Bool areAllSlavesStealthed() const { return m_allStealthed; }
	void revealSlaves();
	Bool m_allStealthed = false;
};
#endif
