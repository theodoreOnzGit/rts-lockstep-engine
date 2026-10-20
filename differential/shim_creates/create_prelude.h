/* Forced include (-include) for differential/wrap_create_modules.cpp (p1l9-creates), differential tests only.
** The create modules sit on the layer-5 base classes (CreateModule.cpp is compiled by the module fixture,
** differential/wrap_module.cpp, and linked here, so its base-class symbols are shared). This prelude makes the
** names this unit uses agree with that build where a symbol's mangled name depends on them (Thing, whose
** pointer the CreateModule constructor takes, keeps the module fixture's name `ModTestThing`), and renames
** every other class this unit stands in for, so that no definition here clashes with another unit's:
**   Object -> CrtObject, Player -> CrtPlayer, PlayerList -> CrtPlayerList, ResourceGatheringManager -> CrtRGM,
**   the Upgrade classes -> Crt* (the Upgrade build's names are UpTest*, the module build's ModTest*), AcademyStats,
**   ExperienceTracker, and the global objects of those classes.
** Like module_prelude.h, it makes private and protected members public for this build. */
#ifndef SHIM_CREATES_PRELUDE_H
#define SHIM_CREATES_PRELUDE_H

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <map>
#include <set>
#include <list>
#include <vector>
#include <bitset>
#include <algorithm>
#include <functional>
#include <exception>
#include <stdexcept>
#include <typeinfo>
#include <cxxabi.h>

#define UpgradeCenter CrtUpgradeCenter
#define UpgradeTemplate CrtUpgradeTemplate
#define Upgrade CrtUpgrade
#define TheUpgradeCenter CrtTheUpgradeCenter
#define TheUpgradeTypeNames CrtTheUpgradeTypeNames
#define Player CrtPlayer
#define PlayerList CrtPlayerList
#define ThePlayerList CrtThePlayerList
#define ResourceGatheringManager CrtRGM
#define AcademyStats CrtAcademyStats
#define ExperienceTracker CrtExperienceTracker
#define Object CrtObject
/* Thing keeps the module fixture's name: CreateModule's constructor takes a Thing*, and its mangled name must match the
** archive wrap_module.cpp builds (ModTestThing, module_prelude.h). */
#define Thing ModTestThing
#define Drawable ModTestDrawable
#define FXList ModTestFXList
#define FXListStore ModTestFXListStore
#define TheFXListStore ModTestTheFXListStore
#define Money CrtMoney
#define InGameUI CrtInGameUI
#define TheInGameUI CrtTheInGameUI

#define private public
#define protected public

#include "../shim_ini/ini_prelude.h"
#include "Common/GameMemory.h"
/* clang reads `new(ShimZeroFill()) T` as a new of a function type; braces cannot be a type-id */
#undef newInstance
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
#undef NEW
#define NEW new(ShimZeroFill{})
#undef MSGNEW
#define MSGNEW(MSG) new(ShimZeroFill{})
#include "Common/SubsystemInterface.h"
#include "Common/NameKeyGenerator.h"
#include "Common/AcademyStats.h"
#include "Common/BitFlagsIO.h"
/* Module.h forward-declares `enum StaticGameLODLevel;` (copied from Common/GameLOD.h, as module_prelude.h does) */
enum StaticGameLODLevel
{
	STATIC_GAME_LOD_UNKNOWN=-1,
	STATIC_GAME_LOD_LOW,
	STATIC_GAME_LOD_MEDIUM,
	STATIC_GAME_LOD_HIGH,
	STATIC_GAME_LOD_CUSTOM,
	STATIC_GAME_LOD_COUNT
};
/* UpdateModule.h forward-declares `enum CommandOption;` (first enumerator copied, as module_prelude.h does) */
enum CommandOption
{
	COMMAND_OPTION_NONE = 0x00000000
};
#endif
