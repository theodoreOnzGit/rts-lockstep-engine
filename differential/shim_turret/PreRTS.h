/* Shim for GameEngine/Include/Precompiled/PreRTS.h, for the turret build only (differential tests, p1w13-sonnet, layer 10
** batch T1; build.rs, differential/wrap_turret.cpp). A copy of differential/shim_pathfind/PreRTS.h with this build's renames.
**
** What the real precompiled header brings to TurretAI.cpp, AIStates.cpp's fire state and StateMachine.cpp, as far as they need
** it: the REAL Lib/BaseType.h (its MSVC helpers redefined to the GNU-asm transcriptions), the logic random-value declarations
** (the stand-ins of turret_world.h define them), and the names of this directory's stand-ins renamed by macro BEFORE any header
** is read, so that none of them is, to the linker, a class or a global another fixture defines. `State` and `StateMachine` are
** renamed too: StateMachine.cpp is compiled again in this unit, over this build's Object and GameLogic. */
#ifndef SHIM_TURRET_PRERTS_H
#define SHIM_TURRET_PRERTS_H
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

/* ---- renames */
#define Object TuObject
#define TerrainLogic TuTerrainLogic
#define TheTerrainLogic TuTheTerrainLogic
#define GameLogic TuGameLogic
#define TheGameLogic TuTheGameLogic
#define AI TuAI
#define TheAI TuTheAI
#define Pathfinder TuPathfinder
#define ThingTemplate TuThingTemplate
#define AIUpdateInterface TuAIUpdateInterface
#define Player TuPlayer
#define Team TuTeam
#define TeamPrototype TuTeamPrototype
#define TeamTemplateInfo TuTeamTemplateInfo
#define PartitionManager TuPartitionManager
#define ThePartitionManager TuThePartitionManager
#define PartitionFilter TuPartitionFilter
#define PartitionFilterSamePlayer TuPartitionFilterSamePlayer
#define PartitionFilterSameMapStatus TuPartitionFilterSameMapStatus
#define PartitionFilterPossibleToAttack TuPartitionFilterPossibleToAttack
#define Weapon TuWeapon
#define GeometryInfo TuGeometryInfo
#define AudioEventRTS TuAudioEventRTS
#define AudioManager TuAudioManager
#define TheAudio TuTheAudio
#define GlobalData TuGlobalData
#define TheGlobalData TuTheGlobalData
#define State TuState
#define StateMachine TuStateMachine
#define TurretAI TuTurretAI
#define TurretAIData TuTurretAIData
#define TurretStateMachine TuTurretStateMachine
#define TurretState TuTurretState
#define AIAttackFireWeaponState TuAIAttackFireWeaponState
#define NotifyWeaponFiredInterface TuNotifyWeaponFiredInterface
#define outOfWeaponRangeObject tuOutOfWeaponRangeObject
#define GetGameLogicRandomValue tuGetGameLogicRandomValue
#define GetGameLogicRandomValueReal tuGetGameLogicRandomValueReal

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
#include "turret_world.h"
#endif
