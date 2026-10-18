/* Forced into the polygon-trigger build as PolygonTrigger.cpp's precompiled
** header (differential/wrap_polygon_trigger.cpp, p1w4-polygon). The real
** PreRTS.h brings in windows.h and most of the engine. This one brings the
** root shim's BaseType.h and Debug.h (the release forms of the macros), and
** the two engine singletons the .cpp names, GlobalData (the water extents of
** the version-1 path) and TerrainLogic (the ground height of getCenterPoint),
** as the stand-ins of this directory. */
#ifndef SHIM_POLYGON_PRERTS_H
#define SHIM_POLYGON_PRERTS_H
#include <stdio.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/GlobalData.h"
#endif
