/* Stand-in for GameEngine/Include/GameLogic/Object.h (layer 8), energy and
** stats tests only: getTemplate() (Energy.cpp's objectEnteringInfluence and
** objectLeavingInfluence, addPowerBonus and removePowerBonus read the template's
** energy numbers). The wrapper sets the template of each object it makes. */
#ifndef SHIM_ENERGY_OBJECT_H
#define SHIM_ENERGY_OBJECT_H
#include "Common/ThingTemplate.h"
class Object
{
public:
	Object() {}
	const ThingTemplate *getTemplate() const { return &m_template; }
	ThingTemplate m_template;
};
#endif
