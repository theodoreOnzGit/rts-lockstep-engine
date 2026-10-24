/* Forced include (-include) for the Module build, differential tests only
** (p1w3-modules). It does three things before any other line of the
** translation unit:
**
** 1. Includes the standard headers the wrapper and the upstream text need.
** 2. Defines the macros of differential/wrap_module.cpp (see its header):
**    - the Upgrade classes keep the renames of the Upgrade build
**      (shim_upgrade/upgrade_prelude.h), so that this unit's references
**      (`TheUpgradeCenter->findUpgrade`) resolve to the REAL UpgradeCenter that
**      differential/wrap_upgrade.cpp compiles from Upgrade.cpp;
**    - the classes this unit stands in for (Object, Thing, FXList, FXListStore,
**      TheFXListStore, GameLogic, TheGameLogic, Drawable) get names of their
**      own, so that no definition here can clash with another unit's stand-in
**      of the same class;
**    - `private`/`protected` made public for this build, so that the wrapper
**      can read the modules' members.
**    They are defined HERE, not in the wrapper, because the INI prelude's
**    BitFlagsIO.h includes Xfer.h, which includes the REAL Upgrade.h.
** 3. Runs the BitFlags build's prelude (the INI prelude, then the real
**    BitFlagsIO.h after KindOf.h) with what the real precompiled header
**    PreRTS.h brings before Upgrade.h: memory pools, SubsystemInterface, the
**    name keys, and the AcademyClassificationType enum (as the Upgrade build). */
#ifndef SHIM_MODULE_PRELUDE_H
#define SHIM_MODULE_PRELUDE_H

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

/* the Upgrade classes: the Upgrade build's names (wrap_upgrade.cpp defines them) */
#define UpgradeCenter UpTestUpgradeCenter
#define UpgradeTemplate UpTestUpgradeTemplate
#define Upgrade UpTestUpgrade
#define TheUpgradeCenter UpTestTheUpgradeCenter
#define TheUpgradeTypeNames UpTestTheUpgradeTypeNames
/* the Upgrade build's other stand-ins keep their own names there; here the
** Player of Upgrade.h's declarations is a name of this unit's own */
#define Player ModTestPlayer
#define TeamFactory ModTestTeamFactory   /* batch R11: the contain driver's stand-in */
#define TheTeamFactory ModTestTheTeamFactory
#define CaveSystem ModTestCaveSystem   /* p1w13-sonnet, batch R11: CaveSystem.cpp in the contain driver (the tunnel tracker test links its own) */
#define TheCaveSystem ModTestTheCaveSystem
#define TunnelTracker ModTestTunnelTracker   /* p1w13-sonnet, batch R10: TunnelTracker.cpp in the contain driver (the tunnel tracker test links its own) */
#define Money ModTestMoney
#define InGameUI ModTestInGameUI
#define TheInGameUI ModTestTheInGameUI
/* this unit's stand-ins */
#define Object ModTestObject
#define Thing ModTestThing
#define Drawable ModTestDrawable
#define FXList ModTestFXList
#define FXListStore ModTestFXListStore
#define TheFXListStore ModTestTheFXListStore
/* --- p1w10-sonnet --- (upgrade batch B: the module interfaces the six upgrade modules call, shim_module/GameLogic/Module/{AIUpdate,SpawnBehavior}.h) */
#define AIUpdateInterface ModTestAIUpdateInterface
#define SpawnBehaviorInterface ModTestSpawnBehaviorInterface
/* --- end p1w10-sonnet --- */
/* --- p1w12-sonnet --- (ReplaceObjectUpgrade: the thing factory and the pathfinder, shim_module/Common/ThingFactory.h and GameLogic/AI.h) */
#define ThingFactory ModTestThingFactory
#define TheThingFactory ModTestTheThingFactory
#define AI ModTestAI
#define TheAI ModTestTheAI
#define Pathfinder ModTestPathfinder
/* die batch D: the template, the audio manager and the script engine the dies call */
#define ThingTemplate ModTestThingTemplate
#define AudioManager ModTestAudioManager
#define TheAudio ModTestTheAudio
#define ScriptEngine ModTestScriptEngine
#define TheScriptEngine ModTestTheScriptEngine
/* crate batch F: the miscellaneous audio, the partition manager and the 2D animations the crates call */
#define MiscAudio ModTestMiscAudio
#define ScoreKeeper ModTestScoreKeeper
#define Eva ModTestEva
#define TheEva ModTestTheEva
#define PlayerList ModTestPlayerList
#define ThePlayerList ModTestThePlayerList
#define PartitionManager ModTestPartitionManager
#define ThePartitionManager ModTestThePartitionManager
#define Anim2DTemplate ModTestAnim2DTemplate
#define Anim2DCollection ModTestAnim2DCollection
#define TheAnim2DCollection ModTestTheAnim2DCollection
/* --- end p1w12-sonnet --- */
/* --- p1w12-sonnet --- (update batch L: AutoDepositUpdate's floating text) */
#define UnicodeString ModTestUnicodeString
#define GameTextInterface ModTestGameTextInterface
#define TheGameText ModTestTheGameText
/* --- end p1w12-sonnet --- */
/* --- p1w12-sonnet --- (behavior batch K: FireWeaponWhenDeadBehavior fires a temporary weapon) */
#define WeaponStore ModTestWeaponStore
#define TheWeaponStore ModTestTheWeaponStore
#define WeaponTemplate ModTestWeaponTemplate
#define Weapon ModTestWeapon   /* update batch M */
#define ObjectIterator ModTestObjectIterator
#define SimpleObjectIterator ModTestSimpleObjectIterator
#define PartitionFilter ModTestPartitionFilter
#define PartitionFilterRelationship ModTestPartitionFilterRelationship
#define PartitionFilterAlive ModTestPartitionFilterAlive
#define PartitionFilterSameMapStatus ModTestPartitionFilterSameMapStatus
#define PartitionFilterAcceptByKindOf ModTestPartitionFilterAcceptByKindOf   /* p1w13-sonnet, batch R27: its filters run (the partition build has the real class) */
#define MemoryPoolObjectHolder ModTestMemoryPoolObjectHolder
#define IterOrderType ModTestIterOrderType
#define PlayerTemplate ModTestPlayerTemplate
#define ParticleSystemManager ModTestParticleSystemManager
#define ParticleSystem ModTestParticleSystem
#define ParticleSystemTemplate ModTestParticleSystemTemplate
#define TheParticleSystemManager ModTestTheParticleSystemManager
#define TerrainLogic ModTestTerrainLogic
#define TheTerrainLogic ModTestTheTerrainLogic
#define DistanceCalculationType ModTestDistanceCalculationType
/* --- end p1w12-sonnet --- */
/* --- p1w13-sonnet --- (update batch O: DynamicShroudClearingRangeUpdate's grid decals; the ObjectCreationList fixture has classes of the same names) */
#define RadiusDecal ModTestRadiusDecal
#define RadiusDecalTemplate ModTestRadiusDecalTemplate
#define GameLODManager ModTestGameLODManager   /* batch Q (SlowDeathBehavior) */
#define TheGameLODManager ModTestTheGameLODManager
/* batch P (ActiveBody): the Armor build's DamageFX is compiled against ITS Object; this unit's has the functions of DamageFX.cpp cut in (build.rs), under names of its own */
#define DamageFX ModTestDamageFX
#define DamageFXStore ModTestDamageFXStore
#define TheDamageFXStore ModTestTheDamageFXStore
/* --- end p1w13-sonnet --- */
/* --- p1w13-sonnet --- (batch R14c: CreateCrateDie; the terrain build compiles the real CrateSystem.cpp) */
#define CrateSystem ModTestCrateSystem
#define TheCrateSystem ModTestTheCrateSystem
#define CrateTemplate ModTestCrateTemplate
#define crateCreationEntry ModTestCrateCreationEntry
/* --- end p1w13-sonnet --- */
#define GameLogic ModTestGameLogic
#define TheGameLogic ModTestTheGameLogic

#define private public
#define protected public

#include "../shim_ini/ini_prelude.h"
#include "Common/GameMemory.h"
/* clang reads `new(ShimZeroFill()) T` as a new of the function type
** `ShimZeroFill()` (the shim's newInstance, fine for g++); braces cannot be a
** type-id */
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
/* Module.h forward-declares `enum StaticGameLODLevel;` and returns it by value (MSVC allows an
** incomplete enum there). The real GameLOD.h needs the particle system, so the enum is copied
** from it (Common/GameLOD.h lines 45-53, commit 0a05454d), before Module.h. */
enum StaticGameLODLevel
{
	STATIC_GAME_LOD_UNKNOWN=-1,
	STATIC_GAME_LOD_LOW,
	STATIC_GAME_LOD_MEDIUM,
	STATIC_GAME_LOD_HIGH,
	STATIC_GAME_LOD_CUSTOM,	//make sure this remains last!
	STATIC_GAME_LOD_COUNT
};
/* UpdateModule.h forward-declares `enum CommandOption;`. The real definition is in
** GameClient/ControlBar.h (the client); the module files of this build use the type
** nowhere, so only its first enumerator is copied, to make the type complete. */
enum CommandOption
{
	COMMAND_OPTION_NONE = 0x00000000
};
/* --- p1w13-sonnet, batch R33 --- EMPUpdate.cpp's BaseType.h conversions, on the x87 helpers' transcriptions
** (differential/wrap_base_type.cpp), as the OCL fixture does (BaseType.h lines 232 and 243, commit 0a05454d) */
extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_ceil(float f);
}
#ifndef REAL_TO_UNSIGNEDINT
#define REAL_TO_UNSIGNEDINT(x) ((UnsignedInt)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#endif
#ifndef REAL_TO_INT_CEIL
#define REAL_TO_INT_CEIL(x) (ref_fast_float2long_round(ref_fast_float_ceil(x)))
#endif
#endif
