/* Forced include (-include) for the Player build, differential tests only (p1w10-sonnet). It runs before any other line of the
** translation unit and does three things:
** 1. defines the macros of differential/wrap_player.cpp: the renames of every class and global this archive defines or
**    stands in for (so that one test binary can link every fixture), and `private`/`protected` made public so that the
**    wrapper can dump the players and teams. They are defined HERE, not in the wrapper, because the headers the weapon
**    prelude reaches (Xfer.h and others) read the real classes' names before the wrapper's first line;
** 2. includes what the real Science.h and Upgrade.h (shim_player/Common forwarders) need before they are read: the memory
**    pools, SubsystemInterface and the name keys;
** 3. runs the Weapon build's prelude (the PreRTS stand-in, then the BitFlags build's: the INI prelude and the real
**    BitFlagsIO.h after KindOf.h). */
#ifndef SHIM_PLAYER_PRELUDE_H
#define SHIM_PLAYER_PRELUDE_H

#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <strings.h>
#include <string>
#include <map>
#include <vector>
#include <list>
#include <set>
#include <bitset>
#include <algorithm>
#include <functional>
#include <exception>
#include <stdexcept>
#include <typeinfo>
#include <cxxabi.h>

#define Object PlTestObject
#define Dict PlTestDict
#define Player PlTestPlayer
#define PlayerList PlTestPlayerList
#define ThePlayerList PlTestThePlayerList
#define PlayerTemplate PlTestPlayerTemplate
#define ThingTemplate PlTestThingTemplate
#define ThingFactory PlTestThingFactory
#define TheThingFactory PlTestTheThingFactory
#define Team PlTestTeam
#define TeamPrototype PlTestTeamPrototype
#define TeamFactory PlTestTeamFactory
#define TheTeamFactory PlTestTheTeamFactory
#define TeamRelationMap PlTestTeamRelationMap
#define TeamTemplateInfo PlTestTeamTemplateInfo
#define PlayerRelationMap PlTestPlayerRelationMap
#define AIGroup PlTestAIGroup
#define AIUpdateInterface PlTestAIUpdateInterface
#define ContainModuleInterface PlTestContainModuleInterface
#define LocomotorSet PlTestLocomotorSet
#define Script PlTestScript
#define ScriptAction PlTestScriptAction
#define ScriptEngine PlTestScriptEngine
#define TheScriptEngine PlTestTheScriptEngine
#define Waypoint PlTestWaypoint
#define TerrainLogic PlTestTerrainLogic
#define TheTerrainLogic PlTestTheTerrainLogic
#define GameLogic PlTestGameLogic
#define TheGameLogic PlTestTheGameLogic
#define PartitionManager PlTestPartitionManager
#define ThePartitionManager PlTestThePartitionManager
#define PartitionFilter PlTestPartitionFilter
#define PartitionFilterRelationship PlTestPartitionFilterRelationship
#define PartitionFilterAlive PlTestPartitionFilterAlive
#define PartitionFilterSameMapStatus PlTestPartitionFilterSameMapStatus
#define PolygonTrigger PlTestPolygonTrigger
#define SidesList PlTestSidesList
#define TeamsInfo PlTestTeamsInfo
#define DistanceCalculationType PlTestDistanceCalculationType
#define AttitudeType PlTestAttitudeType
#define LocomotorSurfaceType PlTestLocomotorSurfaceType
#define LocomotorSurfaceTypeMask PlTestLocomotorSurfaceTypeMask
#define Rec PlTestRec
#define fbits PlTestFbits
#define g_lines PlTestLines

#define Money PlTestMoney
#define Handicap PlTestHandicap
#define Energy PlTestEnergy
#define MissionStats PlTestMissionStats
#define ScoreKeeper PlTestScoreKeeper
#define AcademyStats PlTestAcademyStats
#define AcademyAdviceInfo PlTestAcademyAdviceInfo
#define Upgrade PlTestUpgrade
#define UpgradeTemplate PlTestUpgradeTemplate
#define UpgradeCenter PlTestUpgradeCenter
#define TheUpgradeCenter PlTestTheUpgradeCenter
#define ScienceStore PlTestScienceStore
#define ScienceInfo PlTestScienceInfo
#define TheScienceStore PlTestTheScienceStore
#define AudioEventRTS PlTestAudioEventRTS
#define MiscAudio PlTestMiscAudio
#define AudioManager PlTestAudioManager
#define TheAudio PlTestTheAudio
#define GlobalData PlTestGlobalData
#define TheGlobalData PlTestTheGlobalData
#define GameInfo PlTestGameInfo
#define TheGameInfo PlTestTheGameInfo
#define GameMessage PlTestGameMessage
#define MessageStream PlTestMessageStream
#define TheMessageStream PlTestTheMessageStream
#define AIPlayer PlTestAIPlayer
#define AISkirmishPlayer PlTestAISkirmishPlayer
#define AIData PlTestAIData
#define AI PlTestAI
#define TheAI PlTestTheAI
#define Squad PlTestSquad
#define BuildListInfo PlTestBuildListInfo
#define Eva PlTestEva
#define TheEva PlTestTheEva
#define ControlBar PlTestControlBar
#define TheControlBar PlTestTheControlBar
#define InGameUI PlTestInGameUI
#define TheInGameUI PlTestTheInGameUI
#define GameText PlTestGameText
#define TheGameText PlTestTheGameText
#define VictoryConditions PlTestVictoryConditions
#define TheVictoryConditions PlTestTheVictoryConditions
#define RankInfo PlTestRankInfo
#define RankInfoStore PlTestRankInfoStore
#define TheRankInfoStore PlTestTheRankInfoStore
#define SpecialPowerModuleInterface PlTestSpecialPowerModuleInterface
#define BehaviorModule PlTestBehaviorModule
#define SpecialPowerTemplate PlTestSpecialPowerTemplate
#define PtColor PlTestPtColor
#define TimeOfDay PlTestTimeOfDay
#define EvaMessage PlTestEvaMessage
#define SpecialPowerReadyTimerType PlTestSpecialPowerReadyTimerType
#define KindOfPercentProductionChange PlTestKindOfPercentProductionChange
#define ClosestKindOfData PlTestClosestKindOfData
#define PlayerObjectFindInfo PlTestPlayerObjectFindInfo
#define VisionSpiedStruct PlTestVisionSpiedStruct
#define TypeCountData PlTestTypeCountData
#define ScienceAvailabilityType PlTestScienceAvailabilityType
#define private public
#define protected public

/* the INI prelude (shim_ini/ini_prelude.h) done by hand (without the .big prelude it starts with: that one defines a GlobalData of its own), with the real Science.h in its place and the pieces in the order the
** real headers need: the memory pools, SubsystemInterface and the name keys before Science.h (Overridable.h), the enums
** Upgrade.h and Player.h forward-declare before they do */
#include "PreRTS.h"
#include "Common/AsciiString.h"
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _MAX_PATH 260
#include <bitset>
#include <ctype.h>
#include <float.h>
#include "Common/Errors.h"
#define _isnan isnan
#ifndef PI
#define PI 3.14159265359f
#endif
#include "Common/GameMemory.h"
/* clang reads `new(ShimZeroFill()) T` as a new of the function type `ShimZeroFill()`; braces cannot be a type-id */
#undef newInstance
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
#include "Common/SubsystemInterface.h"
#include "Common/NameKeyGenerator.h"
#include "Common/Science.h"
#include "Common/GameCommon.h"
/* STLTypedefs.h lines 112 and 113, verbatim (the Armor build's STLTypedefs shim has no ProductionChangeMap) */
typedef std::map< NameKeyType, Real, std::less<NameKeyType> > ProductionChangeMap;
typedef std::map< NameKeyType, VeterancyLevel, std::less<NameKeyType> > ProductionVeterancyMap;
#include "Common/AcademyStats.h"
#define SHIM_INI_PRELUDE_H
#include "../shim_weapon/weapon_prelude.h"
#include "Common/SpecialPowerType.h"
#include "enums_gen.inc"
extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_floor(float f);
float ref_fast_float_ceil(float f);
}
#ifndef REAL_TO_INT_CEIL
#define REAL_TO_INT_CEIL(x) (ref_fast_float2long_round(ref_fast_float_ceil(x)))
#endif
using std::min;
using std::max;
#define INT_TO_REAL(x) ((Real)(x))
#endif
