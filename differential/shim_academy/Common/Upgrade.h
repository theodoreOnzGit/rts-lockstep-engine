/* Stand-in for GameEngine/Include/Common/Upgrade.h (layer 5), academy tests only:
** UpgradeTemplate::getAcademyClassificationType(), which the wrapper sets. */
#ifndef SHIM_ACADEMY_UPGRADE_H
#define SHIM_ACADEMY_UPGRADE_H
#include "Common/AcademyStats.h"
class UpgradeTemplate
{
public:
	UpgradeTemplate() : m_academyClassificationType(ACT_NONE) {}
	AcademyClassificationType getAcademyClassificationType() const { return m_academyClassificationType; }
	AcademyClassificationType m_academyClassificationType;
};
#endif
