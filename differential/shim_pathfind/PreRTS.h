/* Shim for GameEngine/Include/Precompiled/PreRTS.h, for the pathfinder build
** only (differential tests, p1w13-sonnet, layer 10; build.rs, differential/wrap_pathfind.cpp).
** A copy of differential/shim_locomotor/PreRTS.h with this build's renames.
**
** What the real precompiled header brings to AIPathfind.cpp, as far as that
** file needs it:
**  1. the REAL Lib/BaseType.h (through the forwarder of differential/
**     shim_locomotor/Lib), whose MSVC `__asm` helpers cannot be emitted by
**     clang; the macros that use them are redefined to the GNU-asm
**     transcriptions of wrap_base_type.cpp (as shim_terrain_logic/PreRTS.h
**     does; AIPathfind.cpp calls none of them);
**  2. the logic random-value declarations (GameLogicRandomValue,
**     GameLogicRandomValueReal), unchanged upstream headers;
**  3. the names of the stand-ins of this directory, renamed by macro BEFORE
**     any header is read, so that none of them is, to the linker, a class or
**     a global that another fixture defines. The real Locomotor.h and
**     LocomotorSet.h name the renamed classes consistently. */
#ifndef SHIM_PATHFIND_PRERTS_H
#define SHIM_PATHFIND_PRERTS_H
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

/* ---- renames (the stand-ins of this directory, and every class of AIPathfind.h: other builds define stand-in
** members of Pathfinder, Path and PathNode, which must not meet these at link time) */
#define Object PfObject
#define TerrainLogic PfTerrainLogic
#define TheTerrainLogic PfTheTerrainLogic
#define GameLogic PfGameLogic
#define TheGameLogic PfTheGameLogic
#define AI PfAI
#define TheAI PfTheAI
#define Pathfinder PfPathfinder
#define PathfindServicesInterface PfPathfindServicesInterface
#define PathfindCell PfPathfindCell
#define PathfindCellInfo PfPathfindCellInfo
#define PathfindLayer PfPathfindLayer
#define PathfindZoneManager PfPathfindZoneManager
#define ZoneBlock PfZoneBlock
#define PathNode PfPathNode
#define Path PfPath
#define Bridge PfBridge
#define Weapon PfWeapon
#define GlobalData PfGlobalData
#define TheGlobalData PfTheGlobalData

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
#endif
