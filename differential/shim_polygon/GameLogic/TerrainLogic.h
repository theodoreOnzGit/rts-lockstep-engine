/* Stand-in for GameEngine/Include/GameLogic/TerrainLogic.h (p1w4-polygon): the
** one member PolygonTrigger.cpp calls, getCenterPoint's ground height, with
** upstream's signature. The definitions are in wrap_polygon_trigger.cpp,
** which renames the class and the singleton by macro. */
#ifndef SHIM_POLYGON_TERRAINLOGIC_H
#define SHIM_POLYGON_TERRAINLOGIC_H
#include "Lib/BaseType.h"
class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y, Coord3D* normal = NULL) const;
};
extern TerrainLogic *TheTerrainLogic;
#endif
