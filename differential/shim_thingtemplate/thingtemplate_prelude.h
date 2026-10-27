/* Forced include (-include) for the ThingTemplate build, differential tests
** only (p1w4-thingtemplate): ThingTemplate.cpp, ThingFactory.cpp,
** INIObject.cpp and ProductionPrerequisite.cpp compiled unchanged, with the
** pieces of WeaponSet.cpp, ModuleFactory.cpp and INI.cpp that build.rs cuts
** out verbatim (see build.rs), and differential/wrap_thingtemplate.cpp.
**
** It first renames, by macro, every class this archive defines or stands in
** for, so that no symbol of it clashes with another fixture's (several
** fixtures stand in for ThingTemplate, ThingFactory, Object, ... under the
** same names, or define the same INI field parsers): the definition of a
** renamed class here is not "a second strong definition of an upstream
** symbol", it is a different symbol. The INI block handlers and the three INI
** field parsers that reach the stand-ins are renamed too, and their text is
** upstream's (INIObject.cpp unchanged; the field parsers cut from INI.cpp):
** the real INI::load reaches them through the hook of wrap_ini.cpp
** (ref_object_block_hook).
**
** Then the Science build's prelude: the real Science.h with `private` and
** `protected` made public (the wrapper reads the members), the INI prelude
** and the real BitFlagsIO.h after KindOf.h. */
#ifndef SHIM_THINGTEMPLATE_PRELUDE_H
#define SHIM_THINGTEMPLATE_PRELUDE_H

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

/* this archive's classes */
#define ThingTemplate TtThingTemplate
#define ThingFactory TtThingFactory
#define TheThingFactory TtTheThingFactory
#define ModuleFactory TtModuleFactory
#define TheModuleFactory TtTheModuleFactory
#define WeaponTemplateSet TtWeaponTemplateSet
#define ProductionPrerequisite TtProductionPrerequisite
/* the block handlers and field parsers of INI that these files define or reach */
#define parseObjectDefinition ttParseObjectDefinition
#define parseObjectReskinDefinition ttParseObjectReskinDefinition
#define parseDynamicAudioEventRTS ttParseDynamicAudioEventRTS
#define parseWeaponTemplate ttParseWeaponTemplate
/* the stand-ins of this archive */
#define Object TtObject
#define Thing TtThing
#define Drawable TtDrawable
#define Player TtPlayer
#define PlayerList TtPlayerList
#define ThePlayerList TtThePlayerList
#define Handicap TtHandicap
#define Energy TtEnergy
#define GameLogic TtGameLogic
#define TheGameLogic TtTheGameLogic
#define GameClient TtGameClient
#define TheGameClient TtTheGameClient
#define PartitionManager TtPartitionManager
#define ThePartitionManager TtThePartitionManager
#define AudioEventRTS TtAudioEventRTS
#define DynamicAudioEventRTS TtDynamicAudioEventRTS
#define AudioManager TtAudioManager
#define TheAudio TtTheAudio
#define FXList TtFXList
#define FXListStore TtFXListStore
#define TheFXListStore TtTheFXListStore
#define Image TtImage
#define ImageCollection TtImageCollection
#define TheMappedImageCollection TtTheMappedImageCollection
#define GameTextInterface TtGameTextInterface
#define TheGameText TtTheGameText
#define WeaponStore TtWeaponStore
#define WeaponTemplate TtWeaponTemplate
#define TheWeaponStore TtTheWeaponStore
#define AIUpdateModuleData TtAIUpdateModuleData
#define TurretAIData TtTurretAIData
#define BehaviorModuleData TtBehaviorModuleData
#define UpdateModuleData TtUpdateModuleData
#define LocomotorStore TtLocomotorStore
#define TheLocomotorStore TtTheLocomotorStore
#define LocomotorTemplate TtLocomotorTemplate
#define BehaviorModule TtBehaviorModule
#define BehaviorModuleInterface TtBehaviorModuleInterface
#define CreateModuleInterface TtCreateModuleInterface
#define Team TtTeam
#define MapObject TtMapObject

/* the Science build's setup (its archive prelude declares a GlobalData of its own: not renamed yet) */
#include "../shim_science/science_prelude.h"

/* BaseType.h lines 101 and 105 (the shim of Lib/BaseType.h has neither); after every standard header */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))

/* ThingTemplate.h forward-declares `enum EditorSortingType;`, `enum ShadowType;` and `enum RadarPriorityType;`
** and returns them by value (MSVC allows an incomplete enum there; clang does not). So they are defined
** here, ahead of it, with the name lists ThingTemplate.cpp defines its DEFINE_*_NAMES macros for (the
** same empty definitions: a redefinition with the same tokens is allowed). */
#define DEFINE_SHADOW_NAMES
#define DEFINE_RADAR_PRIORITY_NAMES
#define DEFINE_EDITOR_SORTING_NAMES
#include "Common/ThingSort.h"
#include "GameClient/Shadow.h"
#include "Common/Radar.h"

#define GlobalData TtGlobalData
#define TheGlobalData TtTheGlobalData

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
#endif
