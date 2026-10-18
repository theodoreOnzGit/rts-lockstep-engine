/* Stand-in for GameEngine/Include/Common/GlobalData.h, Thing tests only: the
** one field Thing.cpp reads (m_gravity, in isSignificantlyAboveTerrain). The
** wrapper renames the class and TheGlobalData (wrap_thing.cpp). */
#ifndef SHIM_THING_GLOBALDATA_H
#define SHIM_THING_GLOBALDATA_H
#include "Lib/BaseType.h"
class GlobalData
{
public:
	Real m_gravity;
};
extern GlobalData *TheGlobalData;
#endif
