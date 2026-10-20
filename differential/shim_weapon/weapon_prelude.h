/* Forced include (-include) for the Weapon builds, differential tests only: the Armor build's prelude (the BitFlags build's: the INI
** prelude, then the real BitFlagsIO.h after KindOf.h) with this directory's PreRTS.h first, so that the first Lib/BaseType.h any of
** them reaches is this build's (whose Coord3D is the real one) and not the root shim's. The other copies have the same include
** guards, so they find the work done. */
#ifndef SHIM_WEAPON_PRELUDE_H
#define SHIM_WEAPON_PRELUDE_H
#include "PreRTS.h"
#include "../shim_armor/armor_prelude.h"
#endif
