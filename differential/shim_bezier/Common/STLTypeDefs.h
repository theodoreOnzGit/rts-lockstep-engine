/* Bezier tests only: BezierSegment.h names this header STLTypeDefs.h (the
** upstream file is STLTypedefs.h, and Linux file names are case-sensitive).
** VecCoord3D is upstream's typedef (STLTypedefs.h line 95). */
#ifndef SHIM_BEZIER_STLTYPEDEFS_H
#define SHIM_BEZIER_STLTYPEDEFS_H
#include <vector>
#include "Lib/BaseType.h"
typedef std::vector<Coord3D> VecCoord3D;
#endif
