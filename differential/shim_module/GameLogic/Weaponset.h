/* Stand-in for GameEngine/Include/GameLogic/WeaponSet.h (layer 8), Module build only (update batch M, p1w12-sonnet): FireOCLAfterWeaponCooldownUpdate.cpp
** includes it as "GameLogic/Weaponset.h" (a spelling a case-insensitive file system forgives) for the slot-name lookup list that
** DEFINE_WEAPONSLOTTYPE_NAMES asks for; the real header needs the weapon layer. Values as upstream WeaponSet.h. */
#ifndef SHIM_MODULE_WEAPONSET_LC_H
#define SHIM_MODULE_WEAPONSET_LC_H
#include "Common/GameType.h"
#include "Common/INI.h"
#ifdef DEFINE_WEAPONSLOTTYPE_NAMES
static LookupListRec TheWeaponSlotTypeNamesLookupList[] =
{
	{ "PRIMARY", PRIMARY_WEAPON },
	{ "SECONDARY", SECONDARY_WEAPON },
	{ "TERTIARY", TERTIARY_WEAPON },
	{ 0, 0 }
};
#endif
#endif
