/* Stand-in for GameEngine/Include/Common/GlobalData.h (p1w4-polygon), with the
** two fields PolygonTrigger.cpp reads: m_waterExtentX and m_waterExtentY (the
** version-1 water area). The wrapper renames the class and the singleton by
** macro (wrap_polygon_trigger.cpp), so it cannot clash with another fixture's. */
#ifndef SHIM_POLYGON_GLOBALDATA_H
#define SHIM_POLYGON_GLOBALDATA_H
#include "Lib/BaseType.h"
class GlobalData
{
public:
	Real m_waterExtentX;
	Real m_waterExtentY;
};
extern GlobalData *TheGlobalData;
#endif
