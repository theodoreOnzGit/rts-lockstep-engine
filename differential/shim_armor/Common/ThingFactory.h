/* Shim for GameEngine/Include/Common/ThingFactory.h, for the Armor/Damage/
** DamageFX build only (differential tests): the one lookup Damage.cpp calls
** (the real one has the same parameters). It is defined in
** differential/wrap_armor.cpp: an empty name finds nothing, any other name
** finds a stand-in template that carries it. */
#ifndef SHIM_ARMOR_THINGFACTORY_H
#define SHIM_ARMOR_THINGFACTORY_H
#include "Common/AsciiString.h"
#include "Common/ThingTemplate.h"
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name, Bool check = TRUE);
};
extern ThingFactory *TheThingFactory;
#endif
