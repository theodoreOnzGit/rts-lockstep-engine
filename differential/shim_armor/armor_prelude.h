/* Forced include (-include) for the Armor/Damage/DamageFX build, differential
** tests only: the BitFlags build's prelude (differential/shim_flags/
** flags_prelude.h: the INI prelude, then the real BitFlagsIO.h after
** KindOf.h), and <map> for the hash_map alias of shim_armor/Common/
** STLTypedefs.h. */
#ifndef SHIM_ARMOR_PRELUDE_H
#define SHIM_ARMOR_PRELUDE_H
#include "../shim_flags/flags_prelude.h"
#endif
