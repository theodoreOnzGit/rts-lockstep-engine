/* Shim for GameEngine/Include/Common/GameMemory.h, differential tests only:
** shim/Common/GameMemory.h (the next one on the include path) with `NEW` and
** `newInstance` spelled so that `NEW GlobalData;` and
** `newInstance(WeaponBonusSet)` parse as placement new, not as `new` of the
** function type `ShimZeroFill ()`: the placement argument is `ShimZeroFill{}`. */
#ifndef SHIM_GLOBALDATA_GAMEMEMORY_H
#define SHIM_GLOBALDATA_GAMEMEMORY_H
#include_next "Common/GameMemory.h"
#undef newInstance
#undef NEW
#undef MSGNEW
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
#define NEW new(ShimZeroFill{})
#define MSGNEW(MSG) new(ShimZeroFill{})
#endif
