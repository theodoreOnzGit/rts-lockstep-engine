/* Stand-in for GameEngine/Include/GameLogic/PartitionManager.h, Module build only (differential tests, p1w11-sonnet, crate batch F): the calls the
** crate modules make, logged by differential/l9_crates_f.inc. Renamed ModTestPartitionManager by module_prelude.h. */
#ifndef SHIM_MODULE_PARTITIONMANAGER_H
#define SHIM_MODULE_PARTITIONMANAGER_H
#include "Lib/BaseType.h"
class PartitionManager
{
public:
	void revealMapForPlayer( Int playerIndex );
};
extern PartitionManager *ThePartitionManager;
#endif
