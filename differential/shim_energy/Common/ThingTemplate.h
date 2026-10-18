/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5), energy
** tests: the two energy numbers Energy.cpp reads (getEnergyProduction,
** getEnergyBonus), set by the wrapper for each object it makes. */
#ifndef SHIM_ENERGY_THINGTEMPLATE_H
#define SHIM_ENERGY_THINGTEMPLATE_H
#include "Lib/BaseType.h"
class ThingTemplate
{
public:
	ThingTemplate() : m_energyProduction(0), m_energyBonus(0) {}
	Int getEnergyProduction() const { return m_energyProduction; }
	Int getEnergyBonus() const { return m_energyBonus; }
	Int m_energyProduction;
	Int m_energyBonus;
};
#endif
