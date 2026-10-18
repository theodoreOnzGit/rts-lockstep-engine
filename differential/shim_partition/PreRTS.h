/* Shim for GameEngine/Include/Precompiled/PreRTS.h, for the PartitionManager build only
** (differential tests; build.rs, differential/wrap_partition.cpp).
**
** What the real precompiled header brings to PartitionManager.cpp and Line2D.cpp, as far as
** those files need it:
**  1. the REAL Lib/BaseType.h (through the forwarder of differential/shim_terrain), whose MSVC
**     `__asm` helpers behind REAL_TO_INT_FLOOR / REAL_TO_INT_CEIL / REAL_TO_INT cannot be emitted
**     by clang; those macros (and REAL_TO_UNSIGNEDINT, which the threat and value maps use) are redefined to the GNU-asm transcriptions of wrap_base_type.cpp
**     (ref_fast_float*), which are the same instruction sequences;
**  2. the BitFlags build's machinery (the INI prelude and the REAL BitFlagsIO.h / KindOf.h /
**     ObjectStatusTypes.h through shim_flags), so KindOfMaskType and ObjectStatusMaskType are the
**     real classes;
**  3. the names of the stand-ins of this directory, renamed by macro BEFORE any header is read, so
**     that none of them is, to the linker, a class another fixture defines with another layout:
**     every header here says which upstream class it stands in for;
**  4. MSVC spellings: __min, __max, and the case-insensitive string names. */
#ifndef SHIM_PARTITION_PRERTS_H
#define SHIM_PARTITION_PRERTS_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <math.h>
#include <limits.h>
#include <new>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <queue>
#include <algorithm>
#include <initializer_list>
#include <bitset>

/* ---- renames (stand-ins in this directory) */
#define Object PMObject
#define Player PMPlayer
#define Team PMTeam
#define PlayerList PMPlayerList
#define ThePlayerList PMThePlayerList
#define ThingTemplate PMThingTemplate
#define ModuleInfo PMModuleInfo
#define GameLogic PMGameLogic
#define TheGameLogic PMTheGameLogic
#define TerrainLogic PMTerrainLogic
#define TheTerrainLogic PMTheTerrainLogic
#define AI PMAI
#define TheAI PMTheAI
#define Pathfinder PMPathfinder
#define PathfindCell PMPathfindCell
#define Squad PMSquad
#define GhostObject PMGhostObject
#define GhostObjectManager PMGhostObjectManager
#define TheGhostObjectManager PMTheGhostObjectManager
#define Display PMDisplay
#define TheDisplay PMTheDisplay
#define Radar PMRadar
#define TheRadar PMTheRadar
#define ActionManager PMActionManager
#define TheActionManager PMTheActionManager
#define CommandButton PMCommandButton
#define PolygonTrigger PMPolygonTrigger
#define ContainModuleInterface PMContainModuleInterface
#define StealthUpdate PMStealthUpdate
#define BodyModuleInterface PMBodyModuleInterface
#define AIUpdateInterface PMAIUpdateInterface
#define DamageInfo PMDamageInfo
#define MapObject PMMapObject
#define Drawable PMDrawable
#define LocomotorSet PMLocomotorSet

class INI;
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/GameMemory.h"

extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
float ref_fast_float_floor(float f);
float ref_fast_float_ceil(float f);
}
#undef REAL_TO_INT
#undef REAL_TO_INT_FLOOR
#undef REAL_TO_INT_CEIL
#undef REAL_TO_UNSIGNEDINT
#undef FAST_REAL_TRUNC
#undef FAST_REAL_FLOOR
#undef FAST_REAL_CEIL
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define REAL_TO_UNSIGNEDINT(x) ((UnsignedInt)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))
#define REAL_TO_INT_CEIL(x) (ref_fast_float2long_round(ref_fast_float_ceil(x)))
#define FAST_REAL_TRUNC(x) ref_fast_float_trunc(x)
#define FAST_REAL_FLOOR(x) ref_fast_float_floor(x)
#define FAST_REAL_CEIL(x) ref_fast_float_ceil(x)

/* MSVC's names */
#ifndef __min
#define __min(a, b) (((a) < (b)) ? (a) : (b))
#endif
#ifndef __max
#define __max(a, b) (((a) > (b)) ? (a) : (b))
#endif
#define _isnan isnan
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp

/* MemoryPoolObjectHolder, as GameMemory.h lines 772-781 */
class MemoryPoolObjectHolder
{
private:
	MemoryPoolObject *m_mpo;
public:
	MemoryPoolObjectHolder(MemoryPoolObject *mpo = NULL) : m_mpo(mpo) { }
	void hold(MemoryPoolObject *mpo) { m_mpo = mpo; }
	void release() { m_mpo = NULL; }
	~MemoryPoolObjectHolder() { if (m_mpo) m_mpo->deleteInstance(); }
};

#include "../shim_ini/ini_prelude.h"
#include "Common/BitFlagsIO.h"
#include "Common/SubsystemInterface.h"
#endif
