/* Stand-in for GameEngine/Include/GameClient/Line2D.h, Module build only (differential tests, p1w13-sonnet, batch R77): the one declaration
** BridgeBehavior uses (line 87-89, verbatim). Its definition is Line2D.cpp's, compiled with the partition build (upstream_partition) and linked. */
#ifndef SHIM_MODULE_LINE2D_H
#define SHIM_MODULE_LINE2D_H
#include "Lib/BaseType.h"
extern Bool PointInsideArea2D( const Coord3D *ptToTest,
															 const Coord3D *area,
															 Int numPointsInArea);
#endif
