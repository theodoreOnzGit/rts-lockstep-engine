/* Stand-in for GameEngine/Include/Common/ThingFactory.h, Weapon tests only: `findTemplate`, which
** `WeaponTemplate::postProcessLoad` calls. The wrapper answers a template for the names that start with
** "Proj" and none for the others (the real factory is a later layer's). */
#ifndef SHIM_WEAPON_THINGFACTORY_H
#define SHIM_WEAPON_THINGFACTORY_H
#include "Common/AsciiString.h"
class ThingTemplate;
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name, Bool check = TRUE);
};
extern ThingFactory *TheThingFactory;
#endif
