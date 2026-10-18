/* Stand-in for GameEngine/Include/Common/ThingFactory.h (layer 5, not
** translated), ObjectTypes tests only: findTemplate, the one member
** ObjectTypes.cpp calls, and TheThingFactory. The wrapper renames both
** (wrap_object_types.cpp). */
#ifndef SHIM_OBJECTTYPES_THINGFACTORY_H
#define SHIM_OBJECTTYPES_THINGFACTORY_H
#include "Common/AsciiString.h"
class ThingTemplate;
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name) const;
};
extern ThingFactory *TheThingFactory;
#endif
