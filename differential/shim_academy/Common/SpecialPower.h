/* Stand-in for GameEngine/Include/Common/SpecialPower.h (layer 5), academy tests
** only: SpecialPowerTemplate::getAcademyClassificationType(), which the wrapper
** sets per template. */
#ifndef SHIM_ACADEMY_SPECIALPOWER_H
#define SHIM_ACADEMY_SPECIALPOWER_H
#include "Common/AcademyStats.h"
class SpecialPowerTemplate
{
public:
	SpecialPowerTemplate() : m_academyClassificationType(ACT_NONE) {}
	AcademyClassificationType getAcademyClassificationType() const { return m_academyClassificationType; }
	AcademyClassificationType m_academyClassificationType;
};
#endif
