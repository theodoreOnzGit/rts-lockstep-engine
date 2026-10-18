/* Stand-in for GameEngine/Include/Common/Energy.h (layer 7), academy tests only:
** this shadows the real header for this archive (the real Energy is the
** energy wrapper's), so that AcademyStats.cpp's power check reads the value
** the wrapper sets. */
#ifndef SHIM_ACADEMY_ENERGY_H
#define SHIM_ACADEMY_ENERGY_H
#include "Lib/BaseType.h"
class Energy
{
public:
	Energy() : m_sufficient(true) {}
	Bool hasSufficientPower() const { return m_sufficient; }
	Bool m_sufficient;
};
#endif
