/* Shim for GameEngine/Include/Precompiled/PreRTS.h, for the AI build only (differential tests, p1w13-sonnet, layer 10, batch A2;
** build.rs, differential/wrap_ai.cpp). A copy of differential/shim_locomotor/PreRTS.h with this build's renames.
**
** The AI build compiles, in one translation unit, upstream's Locomotor.cpp UNCHANGED, the paths of AIPathfind.cpp (batch P1's cut)
** and verbatim members of AIUpdate.cpp, so that the AI drives the REAL locomotors along REAL paths. Every class those files define
** is defined by another fixture too (the Locomotor build, the pathfinder build), and so is every stand-in: all of them are renamed
** here by macro, BEFORE any header is read, so that none of them is, to the linker, a class or a global another fixture defines.
** The real Locomotor.h, LocomotorSet.h and AIPathfind.h name the renamed classes consistently. */
#ifndef SHIM_AI_PRERTS_H
#define SHIM_AI_PRERTS_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <limits.h>
#include <math.h>
#include <new>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <bitset>

/* ---- renames: the stand-ins of this directory */
#define Object AiObject
#define PhysicsBehavior AiPhysicsBehavior
#define BodyModuleInterface AiBodyModuleInterface
#define TerrainLogic AiTerrainLogic
#define TheTerrainLogic AiTheTerrainLogic
#define GameLogic AiGameLogic
#define TheGameLogic AiTheGameLogic
#define AI AiAI
#define TheAI AiTheAI
#define PartitionManager AiPartitionManager
#define ThePartitionManager AiThePartitionManager
#define GlobalData AiGlobalData
#define TheGlobalData AiTheGlobalData
#define GeometryInfo AiGeometryInfo
#define AIGroup AiAIGroup
#define AIStateMachine AiAIStateMachine
#define UpdateModule AiUpdateModule
#define AIUpdateInterface AiAIUpdateInterface
#define AIUpdateModuleData AiAIUpdateModuleData
#define TurretAI AiTurretAI
#define TurretAIData AiTurretAIData
/* batch A3 */
#define UpgradeTemplate AiUpgradeTemplate
#define UpgradeCenter AiUpgradeCenter
#define TheUpgradeCenter AiTheUpgradeCenter
#define TeamTemplateInfo AiTeamTemplateInfo
#define TeamPrototype AiTeamPrototype
#define Team AiTeam
#define ContainModuleInterface AiContainModuleInterface
#define StealthUpdate AiStealthUpdate
#define HordeUpdateInterface AiHordeUpdateInterface
#define BehaviorModule AiBehaviorModule
#define CollideModuleInterface AiCollideModuleInterface
#define DamageInfo AiDamageInfo
#define AiData AiAiData
/* batch A4: StateMachine.cpp is compiled again in this archive (over this build's Object and GameLogic), the real
** AIStateMachine.h is read, and AIStates.cpp's machine and first states are compiled */
#define State AiSmState
#define StateMachine AiSmStateMachine
#define ScriptEngine AiScriptEngine
#define TheScriptEngine AiTheScriptEngine
#define Squad AiSquad
#define AIIdleState AiAIIdleState
#define AIDeadState AiAIDeadState
#define AIWaitState AiAIWaitState
#define AIBusyState AiAIBusyState
#define AIPlaceholderState AiAIPlaceholderState
/* batch A5 */
#define AICommandParms AiAICommandParms
#define AICommandInterface AiAICommandInterface
#define ActionManager AiActionManager
#define TheActionManager AiTheActionManager
#define DeliverPayloadAIUpdate AiDeliverPayloadAIUpdate
#define ProneUpdate AiProneUpdate
#define HackInternetAIUpdate AiHackInternetAIUpdate
#define PolygonTrigger AiPolygonTrigger
#define CommandButton AiCommandButton
#define CommandSet AiCommandSet
#define ControlBar AiControlBar
#define TheControlBar AiTheControlBar
#define PartitionFilter AiPartitionFilter
#define PartitionFilterPossibleToAttack AiPartitionFilterPossibleToAttack
#define PartitionFilterSameMapStatus AiPartitionFilterSameMapStatus
#define FindPositionOptions AiFindPositionOptions
/* batch A10a: the attack machine (the turret build has its own fire state and condition) */
#define AttackStateMachine AiAttackStateMachine
#define AIAttackState AiAIAttackState
#define AIAttackAimAtTargetState AiAIAttackAimAtTargetState
#define AIAttackFireWeaponState AiAIAttackFireWeaponState
#define AIAttackApproachTargetState AiAIAttackApproachTargetState
#define AIAttackPursueTargetState AiAIAttackPursueTargetState
/* batch A11 */
#define AIAttackMoveStateMachine AiAIAttackMoveStateMachine
#define AIAttackMoveToState AiAIAttackMoveToState
#define AIPickUpCrateState AiAIPickUpCrateState
/* batch A12 */
#define AIAttackThenIdleStateMachine AiAIAttackThenIdleStateMachine
#define AIHuntState AiAIHuntState
#define AIAttackAreaState AiAIAttackAreaState
#define AIAttackFollowWaypointPathState AiAIAttackFollowWaypointPathState
/* batch A13 */
#define AIExitState AiAIExitState
#define AIExitInstantlyState AiAIExitInstantlyState
#define AIFaceState AiAIFaceState
#define JetAIUpdate AiJetAIUpdate
#define outOfWeaponRangeObject aiOutOfWeaponRangeObject
#define outOfWeaponRangePosition aiOutOfWeaponRangePosition
#define wantToSquishTarget aiWantToSquishTarget
#define PartitionFilterSamePlayer AiPartitionFilterSamePlayer
/* batch A7 */
#define AudioEventRTS AiAudioEventRTS
#define DynamicAudioEventRTS AiDynamicAudioEventRTS
#define AudioManager AiAudioManager
#define TheAudio AiTheAudio
/* ---- renames: Locomotor.cpp's classes, its global and its INI block handler */
#define Locomotor AiLocomotor
#define LocomotorTemplate AiLocomotorTemplate
#define LocomotorStore AiLocomotorStore
#define TheLocomotorStore AiTheLocomotorStore
#define LocomotorSet AiLocomotorSet
#define parseLocomotorTemplateDefinition aiParseLocomotorTemplateDefinition
/* ---- renames: every class of AIPathfind.h (as differential/shim_pathfind/PreRTS.h renames them) */
#define Pathfinder AiPathfinder
#define PathfindServicesInterface AiPathfindServicesInterface
#define PathfindCell AiPathfindCell
#define PathfindCellInfo AiPathfindCellInfo
#define PathfindLayer AiPathfindLayer
#define PathfindZoneManager AiPathfindZoneManager
#define ZoneBlock AiZoneBlock
#define PathNode AiPathNode
#define Path AiPath
#define Bridge AiBridge
#define BridgeInfo AiBridgeInfo
#define Weapon AiWeapon
#define Player AiPlayer
#define TAiData AiTAiData

class INI;
#include "Lib/BaseType.h"
/* the enums first: Xfer.h forward-declares `enum KindOfType;`, which standard C++ accepts only
** for an enum already declared (locomotor_gen_defs.h: verbatim cuts of the upstream headers) */
#include "locomotor_gen_defs.h"
/* what INI.h relies on (differential/shim_ini/ini_prelude.h): the MSVC string names, the error
** codes of Common/Errors.h (INI.h's status codes are ERROR_BAD_INI), the ScienceType enum,
** `isnan` for the MSVC `_isnan`, and BaseType.h's PI */
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _MAX_PATH 260
#include <ctype.h>
#include "Common/Errors.h"
#include "Common/Science.h"
#define _isnan isnan
#ifndef PI
#define PI 3.14159265359f
#endif
#include "Common/Debug.h"
#include "Common/GameMemory.h"
#include "GameLogic/LogicRandomValue.h"
#include "Lib/trig.h"

extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_floor(float f);
float ref_fast_float_ceil(float f);
}
#undef REAL_TO_INT
#undef REAL_TO_INT_FLOOR
#undef REAL_TO_INT_CEIL
#undef FAST_REAL_TRUNC
#undef FAST_REAL_FLOOR
#undef FAST_REAL_CEIL
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))
#define REAL_TO_INT_CEIL(x) (ref_fast_float2long_round(ref_fast_float_ceil(x)))
#define FAST_REAL_TRUNC(x) ref_fast_float_trunc(x)
#define FAST_REAL_FLOOR(x) ref_fast_float_floor(x)
#define FAST_REAL_CEIL(x) ref_fast_float_ceil(x)

#include "Common/SubsystemInterface.h"
/* batch A5: Squad.cpp includes Squad.h first, which names ObjectID */
#include "Common/GameType.h"
#endif
