/* Stand-in for GameEngine/Include/Common/GlobalData.h, Module build only (differential tests, p1w12-sonnet, special power batch H): the one field
** SpecialPowerModule::createViewObject reads (and, in the debug build, m_specialPowerUsesDelay), in a class and global renamed so that they cannot
** clash with the GlobalData of the other builds (TheGlobalData is defined in differential/l9_powers_h.inc). */
#ifndef SHIM_MODULE_GLOBALDATA_H
#define SHIM_MODULE_GLOBALDATA_H
#include "Common/AsciiString.h"
#include "Lib/BaseType.h"
#define GlobalData ModTestPowGlobalData
#define TheGlobalData ModTestPowTheGlobalData
class GlobalData
{
public:
	AsciiString m_specialPowerViewObjectName;
	Bool m_specialPowerUsesDelay;
};
extern GlobalData *TheGlobalData;
#endif
