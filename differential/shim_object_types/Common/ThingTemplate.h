/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5, not
** translated), ObjectTypes tests only: a template is its name, which is the
** only member ObjectTypes.cpp reads (getName). The wrapper renames the class. */
#ifndef SHIM_OBJECTTYPES_THINGTEMPLATE_H
#define SHIM_OBJECTTYPES_THINGTEMPLATE_H
#include "Common/AsciiString.h"
class ThingTemplate
{
public:
	ThingTemplate() : m_index(0) {}
	const AsciiString &getName() const { return m_name; }
	AsciiString m_name;
	int m_index;
};
#endif
