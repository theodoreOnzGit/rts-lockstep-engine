/* Shim for GameEngine/Include/GameLogic/Object.h, for the Armor/Damage/
** DamageFX build only (differential tests): an Object that has a veterancy
** level, the one member DamageFX.cpp calls. */
#ifndef SHIM_ARMOR_OBJECT_H
#define SHIM_ARMOR_OBJECT_H
#include "Common/GameCommon.h"
class Object
{
public:
	Object(VeterancyLevel level) : m_level(level) {}
	VeterancyLevel getVeterancyLevel() const { return m_level; }
private:
	VeterancyLevel m_level;
};
#endif
