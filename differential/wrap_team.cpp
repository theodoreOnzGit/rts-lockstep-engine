/* Differential-test wrapper for Team.cpp (upstream commit 0a05454d), p1w9-sonnet:
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp  (TeamRelationMap, TeamFactory, TeamTemplateInfo,
**   TeamPrototype, Team: every function)
**   GeneralsMD/Code/GameEngine/Include/Common/Team.h       (unchanged)
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp + Include/Common/Player.h  (PlayerRelationMap only:
**   its class and its five functions, verbatim)
** with ONE mechanical edit of Team.cpp, made by build.rs into OUT_DIR/team_gen/Team_cut.cpp and asserted to apply
** exactly once: the include block (PreRTS.h .. ScriptEngine.h) is replaced by the includes the stand-ins need.
** Every class of the world that Team.cpp reaches (objects, players, the player list, the thing templates, the AI, the
** contain module, the script engine, the terrain's waypoints, the partition manager, the game logic, the sides list)
** is a stand-in (shim_team/team_standins_decl.h, team_driver.h) that answers from the program's configuration and writes a
** line for every call that has an effect; all of them, and the classes of Team.cpp itself, are renamed by the
** #define lines below so that no symbol of this archive collides with another fixture's. The name key
** generator and its well-known keys, the Xfer classes and the memory-pool shim are the real ones of the other archives;
** Dict (verified by differential_dict.rs) is a small stand-in with the real getters' meaning.
**
** Entry point (extern "C", tests/differential_team.rs): ref_team_run. */
#define Object TmTestObject
#define Dict TmTestDict
#define Player TmTestPlayer
#define PlayerList TmTestPlayerList
#define ThePlayerList TmTestThePlayerList
#define PlayerTemplate TmTestPlayerTemplate
#define ThingTemplate TmTestThingTemplate
#define ThingFactory TmTestThingFactory
#define TheThingFactory TmTestTheThingFactory
#define Team TmTestTeam
#define TeamPrototype TmTestTeamPrototype
#define TeamFactory TmTestTeamFactory
#define TheTeamFactory TmTestTheTeamFactory
#define TeamRelationMap TmTestTeamRelationMap
#define TeamTemplateInfo TmTestTeamTemplateInfo
#define PlayerRelationMap TmTestPlayerRelationMap
#define AIGroup TmTestAIGroup
#define AIUpdateInterface TmTestAIUpdateInterface
#define ContainModuleInterface TmTestContainModuleInterface
#define LocomotorSet TmTestLocomotorSet
#define Script TmTestScript
#define ScriptAction TmTestScriptAction
#define ScriptEngine TmTestScriptEngine
#define TheScriptEngine TmTestTheScriptEngine
#define Waypoint TmTestWaypoint
#define TerrainLogic TmTestTerrainLogic
#define TheTerrainLogic TmTestTheTerrainLogic
#define GameLogic TmTestGameLogic
#define TheGameLogic TmTestTheGameLogic
#define PartitionManager TmTestPartitionManager
#define ThePartitionManager TmTestThePartitionManager
#define PartitionFilter TmTestPartitionFilter
#define PartitionFilterRelationship TmTestPartitionFilterRelationship
#define PartitionFilterAlive TmTestPartitionFilterAlive
#define PartitionFilterSameMapStatus TmTestPartitionFilterSameMapStatus
#define PolygonTrigger TmTestPolygonTrigger
#define SidesList TmTestSidesList
#define TeamsInfo TmTestTeamsInfo
#define DistanceCalculationType TmTestDistanceCalculationType
#define AttitudeType TmTestAttitudeType
#define LocomotorSurfaceType TmTestLocomotorSurfaceType
#define LocomotorSurfaceTypeMask TmTestLocomotorSurfaceTypeMask
#define Rec TmTestRec
#define fbits TmTestFbits
#define g_lines TmTestLines

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

/* the real headers, the stand-ins of the world, the cut Team.cpp, with their members readable (the dumps read them) */
#define private public
#define protected public
#include "Common/NameKeyGenerator.h"
#include "Common/INI.h"
#include "Common/Xfer.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/DisabledTypes.h"
#include "Common/WellKnownKeys.h"
#include "Common/GameCommon.h"
#include "Common/Snapshot.h"
#include "Common/XferCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/Team.h"
typedef Int PlayerIndex;
#define PLAYER_INDEX_INVALID -1
#include "player_relation_map_class.inc"
#include "Team_cut.cpp"
#include "player_relation_map_cpp.inc"
#include "team_driver.h"
#undef private
#undef protected
