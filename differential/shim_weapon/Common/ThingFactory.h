/* Stand-in for GameEngine/Include/Common/ThingFactory.h, Weapon tests only: the class is in shim_weapon/fire_standins.h
** (it has findTemplate, which WeaponTemplate::postProcessLoad and Weapon::createLaser call, and newObject, which
** fireWeaponTemplate calls); this file only makes the include of Weapon.cpp resolve. */
#ifndef SHIM_WEAPON_THINGFACTORY_H
#define SHIM_WEAPON_THINGFACTORY_H
#include "Common/AsciiString.h"
class ThingTemplate;
class ThingFactory;
extern ThingFactory *TheThingFactory;
#endif
