/* Stand-in for GameEngine/Include/Common/GlobalData.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): the five members that
** ThingTemplate.cpp reads. (Renamed TtGlobalData by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_GLOBALDATA_H
#define SHIM_TT_GLOBALDATA_H
#include "Lib/BaseType.h"
class GlobalData
{
public:
	UnsignedInt m_defaultOcclusionDelay;
	Real m_LowEnergyPenaltyModifier;
	Real m_MinLowEnergyProductionSpeed;
	Real m_MaxLowEnergyProductionSpeed;
	Real m_MultipleFactory;
};
extern GlobalData *TheGlobalData;
#endif
