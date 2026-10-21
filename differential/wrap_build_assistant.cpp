/* Differential-test wrapper for BuildAssistant.cpp (upstream commit 0a05454d), p1w10-sonnet:
**   GeneralsMD/Code/GameEngine/Include/Common/BuildAssistant.h          (unchanged: the whole class)
**   GeneralsMD/Code/GameEngine/Source/Common/System/BuildAssistant.cpp  (every function, whole and unchanged)
** with ONE mechanical edit of BuildAssistant.cpp, made by build.rs into OUT_DIR/build_assistant_gen/BuildAssistant_cut.cpp and asserted to
** apply exactly once: the include block (PreRTS.h .. ParkingPlaceBehavior.h) is replaced by a comment, the wrapper including what the
** stand-ins need before it.
** Every class of the world that BuildAssistant.cpp reaches (objects and their interfaces, the thing templates, the player, the terrain, the
** terrain visual, the partition manager's queries, the pathfinder's cells, the control bar, the audio, the game logic, the global data, the
** Xfer's four calls) is a stand-in (shim_buildassist/ba_standins_decl.h, ba_driver.h) that answers from the program's
** configuration and writes a line for every call that has an effect; all of them, and the classes of BuildAssistant.cpp itself, are renamed by
** the #define lines below so that no symbol of this archive collides with another fixture's. Lib/BaseType.h, Geometry.h, KindOf.h,
** ObjectStatusTypes.h, ModelState.h, GameCommon.h, the random value generator and WWMath's Matrix3D/Vector3 are the real ones.
**
** Entry point (extern "C", tests/differential_build_assistant.rs): ref_ba_run. */
#define Object BaTestObject
#define Player BaTestPlayer
#define Team BaTestTeam
#define ThingTemplate BaTestThingTemplate
#define ThingFactory BaTestThingFactory
#define TheThingFactory BaTestTheThingFactory
#define AIUpdateInterface BaTestAIUpdateInterface
#define ProductionUpdateInterface BaTestProductionUpdateInterface
#define ContainModuleInterface BaTestContainModuleInterface
#define ParkingPlaceBehaviorInterface BaTestParkingPlaceBehaviorInterface
#define CreateModuleInterface BaTestCreateModuleInterface
#define BehaviorModule BaTestBehaviorModule
#define Drawable BaTestDrawable
#define CommandSet BaTestCommandSet
#define CommandButton BaTestCommandButton
#define ControlBar BaTestControlBar
#define TheControlBar BaTestTheControlBar
#define TerrainLogic BaTestTerrainLogic
#define TheTerrainLogic BaTestTheTerrainLogic
#define TerrainType BaTestTerrainType
#define TerrainVisual BaTestTerrainVisual
#define TheTerrainVisual BaTestTheTerrainVisual
#define PathfindCell BaTestPathfindCell
#define Pathfinder BaTestPathfinder
#define AI BaTestAI
#define TheAI BaTestTheAI
#define MemoryPoolObjectHolder BaTestMemoryPoolObjectHolder
#define ObjectIterator BaTestObjectIterator
#define DistanceCalculationType BaTestDistanceCalculationType
#define PartitionFilter BaTestPartitionFilter
#define PartitionFilterAcceptByKindOf BaTestPartitionFilterAcceptByKindOf
#define PartitionManager BaTestPartitionManager
#define ThePartitionManager BaTestThePartitionManager
#define AudioEventRTS BaTestAudioEventRTS
#define AudioManager BaTestAudioManager
#define TheAudio BaTestTheAudio
#define GameLogic BaTestGameLogic
#define TheGameLogic BaTestTheGameLogic
#define GlobalData BaTestGlobalData
#define TheGlobalData BaTestTheGlobalData
#define Money BaTestMoney
#define GUICommandType BaTestGUICommandType
#define CanMakeInt BaTestCanMakeInt
#define BuildAssistant BaTestBuildAssistant
#define TheBuildAssistant BaTestTheBuildAssistant
#define ObjectSellInfo BaTestObjectSellInfo
#define SampleBuildData BaTestSampleBuildData
#define Rec BaTestRec
#define fbits BaTestFbits
#define g_lines BaTestLines

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
#include <algorithm>
#include <functional>
#include "Common/GameMemory.h"
#undef newInstance
#define newInstance(ARGCLASS) new(ShimZeroFill{}) ARGCLASS
#undef NEW
#define NEW new(ShimZeroFill{})
#include "Common/AsciiString.h"
#include "matrix3d.h"
#include "vector3.h"

/* the real header, the stand-ins of the world, the cut BuildAssistant.cpp, with its members readable (the dumps read them) */
#define private public
#define protected public
#include "Common/BuildAssistant.h"
#include "BuildAssistant_cut.cpp"
#include "ba_driver.h"
#undef private
#undef protected
