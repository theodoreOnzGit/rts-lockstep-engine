/* Stand-in for GameEngine/Include/Common/MapObject.h (p1w4-polygon). The real
** header declares MapObject with the render objects and TerrainRoads.h, which
** PolygonTrigger.cpp does not use: it needs MAP_XY_FACTOR only, whose value
** is the real header's (MapObject.h line 74). The object chunk build has its
** own MapObject stand-in (differential/wrap_map_object.cpp). */
#ifndef SHIM_POLYGON_MAPOBJECT_H
#define SHIM_POLYGON_MAPOBJECT_H
#define MAP_XY_FACTOR			(10.0f)
#define MAP_HEIGHT_SCALE	(MAP_XY_FACTOR/16.0f)
#endif
