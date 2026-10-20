/* Stand-in for GameEngine/Include/GameLogic/WeaponSet.h (layer 8), create-module build only (p1l9-creates): the
** WeaponLockType enum and the slot-name lookup list that DEFINE_WEAPONSLOTTYPE_NAMES defines (LockWeaponCreate.cpp
** defines it before including this file, as the real header does). WeaponSlotType is the real one, from
** Common/GameType.h. Values as upstream WeaponSet.h. */
#ifndef SHIM_CREATES_WEAPONSET_H
#define SHIM_CREATES_WEAPONSET_H
#include "Common/GameType.h"
#include "Common/INI.h"
enum WeaponLockType
{
	NOT_LOCKED = 0,
	LOCKED_TEMPORARILY = 1,
	LOCKED_PERMANENTLY = 2
};
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
