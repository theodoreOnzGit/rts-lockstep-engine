/* Shim for GameEngine/Include/Common/ThingTemplate.h, for the Armor/Damage/
** DamageFX build only (differential tests): the one member Damage.cpp calls,
** getName(). */
#ifndef SHIM_ARMOR_THINGTEMPLATE_H
#define SHIM_ARMOR_THINGTEMPLATE_H
#include "Common/AsciiString.h"
class ThingTemplate
{
public:
	ThingTemplate(const AsciiString &name) : m_name(name) {}
	const AsciiString &getName() const { return m_name; }
private:
	AsciiString m_name;
};
#endif
