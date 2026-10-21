/* Differential-test wrapper for ObjectCreationList.cpp (upstream commit 0a05454d), p1w10-sonnet:
**   GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectCreationList.h          (unchanged: the three classes)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp  (every function and class, whole and unchanged)
** with TWO mechanical edits of ObjectCreationList.cpp, made by build.rs into OUT_DIR/ocl_gen/ObjectCreationList_cut.cpp and asserted to
** apply exactly once: `#include "PreRTS.h"` and the include block (Common/AudioEventRTS.h .. Common/CRCDebug.h) are each replaced by a
** comment, the wrapper including what the stand-ins need before it (the DEFINE_* lines between them are kept; NO_DEBUG_CRC makes the
** DUMP* macros of CRCDebug.h empty, as the stand-in header defines them).
** The real INI (INI.cpp, in the INI archive), the real name key generator, AsciiString, the random value generator, WWMath's Matrix3D and
** Vector3, Lib/BaseType.h, GameType.h, KindOf.h, GameCommon.h and ObjectStatusTypes.h are used as they are. Every other class the file reaches
** (the objects and the modules it finds in them, the thing templates and the factory, the player list, the terrain, the pathfinder, the
** partition manager, the weapon store, the particle system manager, the audio, the script engine, the game logic and LOD, the global data,
** the radius decal and the DeliverPayloadData of the module it hands to) is a stand-in (shim_ocl/ocl_standins_decl.h, ocl_driver.h) that
** answers from the program's configuration and writes a line for every call that has an effect; all of them are renamed by the #define lines
** below so that no symbol of this archive collides with another fixture's. INI::parseObjectCreationListDefinition replaces the INI archive's
** stub (WEAK there).
**
** Entry points (extern "C", tests/differential_object_creation_list.rs): ref_ocl_parse, ref_ocl_run. */
#define Object OclTestObject
#define Player OclTestPlayer
#define PlayerList OclTestPlayerList
#define ThePlayerList OclTestThePlayerList
#define Team OclTestTeam
#define Drawable OclTestDrawable
#define DrawModule OclTestDrawModule
#define DebrisDrawInterface OclTestDebrisDrawInterface
#define ThingTemplate OclTestThingTemplate
#define ThingFactory OclTestThingFactory
#define TheThingFactory OclTestTheThingFactory
#define AIUpdateInterface OclTestAIUpdateInterface
#define PhysicsBehavior OclTestPhysicsBehavior
#define BodyModuleInterface OclTestBodyModuleInterface
#define ContainModuleInterface OclTestContainModuleInterface
#define BehaviorModule OclTestBehaviorModule
#define SlavedUpdateInterface OclTestSlavedUpdateInterface
#define LifetimeUpdate OclTestLifetimeUpdate
#define FloatUpdate OclTestFloatUpdate
#define RadiusDecalUpdate OclTestRadiusDecalUpdate
#define SpecialPowerCompletionDie OclTestSpecialPowerCompletionDie
#define DeliverPayloadAIUpdate OclTestDeliverPayloadAIUpdate
#define DeliverPayloadData OclTestDeliverPayloadData
#define Locomotor OclTestLocomotor
#define ExperienceTracker OclTestExperienceTracker
#define TerrainLogic OclTestTerrainLogic
#define TheTerrainLogic OclTestTheTerrainLogic
#define PathfindCell OclTestPathfindCell
#define Pathfinder OclTestPathfinder
#define AI OclTestAI
#define TheAI OclTestTheAI
#define PartitionManager OclTestPartitionManager
#define ThePartitionManager OclTestThePartitionManager
#define FindPositionOptions OclTestFindPositionOptions
#define WeaponStore OclTestWeaponStore
#define TheWeaponStore OclTestTheWeaponStore
#define WeaponTemplate OclTestWeaponTemplate
#define ParticleSystemManager OclTestParticleSystemManager
#define TheParticleSystemManager OclTestTheParticleSystemManager
#define ParticleSystemTemplate OclTestParticleSystemTemplate
#define ParticleSystem OclTestParticleSystem
#define AudioEventRTS OclTestAudioEventRTS
#define AudioManager OclTestAudioManager
#define TheAudio OclTestTheAudio
#define ScriptEngine OclTestScriptEngine
#define TheScriptEngine OclTestTheScriptEngine
#define GameLogic OclTestGameLogic
#define TheGameLogic OclTestTheGameLogic
#define GameLODManager OclTestGameLODManager
#define TheGameLODManager OclTestTheGameLODManager
#define GlobalData OclTestGlobalData
#define TheGlobalData OclTestTheGlobalData
#define FXList OclTestFXList
#define FXListStore OclTestFXListStore
#define TheFXListStore OclTestTheFXListStore
#define RadiusDecalTemplate OclTestRadiusDecalTemplate
#define RadiusDecal OclTestRadiusDecal
#define ObjectCreationNugget OclTestNugget
#define ObjectCreationList OclTestList
#define ObjectCreationListStore OclTestStore
#define TheObjectCreationListStore OclTestTheStore
#define Rec OclTestRec
#define fbits OclTestFbits
#define g_lines OclTestLines

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

/* the stand-ins of the world, the real header, the cut ObjectCreationList.cpp, with its members readable (the dumps read them) */
#include "ocl_standins_decl.h"
#define private public
#define protected public
#include "GameClient/RadiusDecal.h"
#include "ocl_radius_decal_template.inc"      /* RadiusDecalTemplate's constructor and parser, verbatim from RadiusDecal.cpp (build.rs) */
#include "ocl_deliver_payload_data.inc"       /* DeliverPayloadData and its field parse, verbatim from DeliverPayloadAIUpdate.h/.cpp (build.rs) */
#include "ocl_static_lod_level.inc"           /* StaticGameLODNames and INI::parseStaticGameLODLevel, verbatim from GameLOD.cpp (build.rs) */
#include "ocl_real_header.h"
#include "ObjectCreationList_cut.cpp"
#include "ocl_driver.h"
#undef private
#undef protected
