/* Shim for GameEngine/Include/Precompiled/PreRTS.h, for the TerrainLogic build
** only (differential tests; build.rs, differential/wrap_terrain_logic.cpp).
**
** What the real precompiled header brings to TerrainLogic.cpp, as far as that
** file needs it:
**  1. the REAL Lib/BaseType.h (through the forwarder of differential/
**     shim_terrain), whose MSVC `__asm` helpers behind REAL_TO_INT_FLOOR /
**     FAST_REAL_FLOOR / REAL_TO_INT cannot be emitted by clang; those three
**     macros are redefined to the GNU-asm transcriptions of
**     wrap_base_type.cpp (the x87 `fld`/`fistp` and the integer `trunc`),
**     which are the same instruction sequences;
**  2. the names of the stand-ins of this directory, renamed by macro (so that
**     none of them is, to the linker, a class another fixture defines): every
**     header here says which upstream class it stands in for;
**  3. MemoryPoolObjectHolder (GameMemory.h, whose shim does not have it).
**
** The renames are a macro per name, defined BEFORE any header is read, so the
** forward declarations in the real TerrainLogic.h and DataChunk.h name the
** renamed classes consistently. */
#ifndef SHIM_TERRAINLOGIC_PRERTS_H
#define SHIM_TERRAINLOGIC_PRERTS_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <math.h>
#include <new>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <set>
#include <algorithm>
#include <initializer_list>
#include <bitset>

/* ---- renames (stand-ins in this directory, and TerrainLogic's own friends) */
#define Object TLObject
#define ThingTemplate TLThingTemplate
#define ThingFactory TLThingFactory
#define TheThingFactory TLTheThingFactory
#define Team TLTeam
#define MapObject TLMapObject
#define PolygonTrigger TLPolygonTrigger
#define WaterHandle TLWaterHandle
#define Drawable TLDrawable
#define GameLogic TLGameLogic
#define TheGameLogic TLTheGameLogic
#define AI TLAI
#define TheAI TLTheAI
#define Pathfinder TLPathfinder
#define TerrainVisual TLTerrainVisual
#define TheTerrainVisual TLTheTerrainVisual
#define TerrainRoads TLTerrainRoads
#define TheTerrainRoads TLTheTerrainRoads
#define TerrainRoadType TLTerrainRoadType
#define PartitionManager TLPartitionManager
#define ThePartitionManager TLThePartitionManager
#define ShroudStatusStoreRestore TLShroudStatusStoreRestore
#define ObjectIterator TLObjectIterator
#define GhostObjectManager TLGhostObjectManager
#define TheGhostObjectManager TLTheGhostObjectManager
#define Radar TLRadar
#define TheRadar TLTheRadar
#define View TLView
#define TheTacticalView TLTheTacticalView
#define GlobalData TLGlobalData
#define TheGlobalData TLTheGlobalData
#define BodyModuleInterface TLBodyModuleInterface
#define BridgeBehaviorInterface TLBridgeBehaviorInterface
#define BridgeBehavior TLBridgeBehavior
#define BridgeTowerBehaviorInterface TLBridgeTowerBehaviorInterface
#define BridgeTowerBehavior TLBridgeTowerBehavior
#define DamageInfo TLDamageInfo
#define DamageInfoInput TLDamageInfoInput
#define WorldHeightMap TLWorldHeightMap

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
#undef FAST_REAL_TRUNC
#undef FAST_REAL_FLOOR
#undef FAST_REAL_CEIL
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))
#define REAL_TO_INT_FLOOR(x) (ref_fast_float2long_round(ref_fast_float_floor(x)))
#define REAL_TO_INT_CEIL(x) (ref_fast_float2long_round(ref_fast_float_ceil(x)))
#define FAST_REAL_TRUNC(x) ref_fast_float_trunc(x)
#define FAST_REAL_FLOOR(x) ref_fast_float_floor(x)
#define FAST_REAL_CEIL(x) ref_fast_float_ceil(x)

/* MemoryPoolObjectHolder, as GameMemory.h lines 772-781 */
class MemoryPoolObjectHolder
{
private:
	MemoryPoolObject *m_mpo;
public:
	MemoryPoolObjectHolder(MemoryPoolObject *mpo = NULL) : m_mpo(mpo) { }
	void hold(MemoryPoolObject *mpo) { m_mpo = mpo; }
	void release() { m_mpo = NULL; }
	~MemoryPoolObjectHolder() { m_mpo->deleteInstance(); }
};

/* TerrainLogic.cpp assigns strrchr's result to a `char *` (MSVC's C-only overload) */
#define strrchr(s, c) ((char *)strrchr((s), (c)))

#include "Common/SubsystemInterface.h"
#endif
