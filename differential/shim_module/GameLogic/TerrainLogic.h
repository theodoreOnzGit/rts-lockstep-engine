/* Stand-in for GameEngine/Include/GameLogic/TerrainLogic.h, Module build only (differential tests, p1w12-sonnet, update batch N): the one call OCLUpdate
** makes, answered by differential/l9_ocl_update_n.inc. Renamed ModTestTerrainLogic by module_prelude.h. */
#ifndef SHIM_MODULE_TERRAINLOGIC_H
#define SHIM_MODULE_TERRAINLOGIC_H
#include "Lib/BaseType.h"
class TerrainLogic
{
public:
	Coord3D findClosestEdgePoint( const Coord3D *closestTo ) const;
};
extern TerrainLogic *TheTerrainLogic;
#endif
