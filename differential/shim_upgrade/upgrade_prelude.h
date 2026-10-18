/* Forced include (-include) for the Upgrade/SpecialPower build, differential
** tests only (p1w2-upgrade). It does three things before any other line of
** the translation unit:
**
** 1. Includes the standard headers the wrapper and the upstream text need,
**    so that the macro below does not reach into them.
** 2. Defines the macros of differential/wrap_upgrade.cpp (see its header):
**    the renames, and `private`/`protected` made public so that the wrapper
**    can dump the templates. They are defined HERE, not in the wrapper,
**    because the INI prelude's BitFlagsIO.h includes Xfer.h, which includes
**    the REAL Upgrade.h (through shim_upgrade/Common/Upgrade.h), so Upgrade.h
**    is read during the prelude and must already see them.
** 3. Runs the BitFlags build's prelude (the INI prelude, then the real
**    BitFlagsIO.h after KindOf.h) with what the real precompiled header
**    PreRTS.h brings before Upgrade.h and SpecialPower.h in between: memory
**    pools, SubsystemInterface, the name keys (`enum NameKeyType;` is a
**    forward reference to an enum that must be declared), and the
**    AcademyClassificationType enum (from the real AcademyStats.h, which
**    Player.h includes upstream). */
#ifndef SHIM_UPGRADE_PRELUDE_H
#define SHIM_UPGRADE_PRELUDE_H

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

#define UpgradeCenter UpTestUpgradeCenter
#define UpgradeTemplate UpTestUpgradeTemplate
#define Upgrade UpTestUpgrade
#define TheUpgradeCenter UpTestTheUpgradeCenter
#define TheUpgradeTypeNames UpTestTheUpgradeTypeNames
#define SpecialPowerStore UpTestSpecialPowerStore
#define SpecialPowerTemplate UpTestSpecialPowerTemplate
#define TheSpecialPowerStore UpTestTheSpecialPowerStore
#define Player UpTestPlayer
#define Money UpTestMoney
#define Object UpTestObject
#define InGameUI UpTestInGameUI
#define TheInGameUI UpTestTheInGameUI

#define private public
#define protected public

#include "../shim_ini/ini_prelude.h"
#include "Common/GameMemory.h"
/* clang reads `new(ShimZeroFill()) T` as a new of the function type
** `ShimZeroFill()` (the shim's newInstance, fine for g++); braces cannot be a
** type-id */
#undef newInstance
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
#include "Common/SubsystemInterface.h"
#include "Common/NameKeyGenerator.h"
#include "Common/AcademyStats.h"
#include "Common/BitFlagsIO.h"
#endif
