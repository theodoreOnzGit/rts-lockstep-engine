/* Stand-in for GameEngine/Include/Common/GlobalData.h, p1l9-updates build only (differential/wrap_l9updates.cpp): the two
** fields BaseRenerateUpdate.cpp reads, m_baseRegenHealthPercentPerSecond and m_baseRegenDelay, in a class and global renamed
** so that they cannot clash with the GlobalData of the globaldata build (TheGlobalData is defined in wrap_l9updates.cpp). */
#ifndef SHIM_UPDATES_GLOBALDATA_H
#define SHIM_UPDATES_GLOBALDATA_H
#include "Lib/BaseType.h"
#define GlobalData L9UGlobalData
#define TheGlobalData L9UTheGlobalData
class GlobalData
{
public:
	Real m_baseRegenHealthPercentPerSecond;
	UnsignedInt m_baseRegenDelay;
};
extern GlobalData *TheGlobalData;
#endif
