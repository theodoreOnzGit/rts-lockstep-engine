/* Stand-in for GameEngine/Include/Common/Player.h (layer 7), Module build only (differential tests,
** p1l9-example): the one thing the layer-9 modules compiled here read of a Player, its completed
** upgrades. Renamed ModTestPlayer by module_prelude.h. Layer-9 batches add what their modules read,
** each in its marked block. */
#ifndef SHIM_MODULE_PLAYER_H
#define SHIM_MODULE_PLAYER_H
#include "Common/Upgrade.h"
class Player
{
public:
	UpgradeMaskType getCompletedUpgradeMask() const { return m_upgradesCompleted; }
	UpgradeMaskType m_upgradesCompleted;
	/* --- p1l9-example --- */
	/* --- end p1l9-example --- */
};
#endif
