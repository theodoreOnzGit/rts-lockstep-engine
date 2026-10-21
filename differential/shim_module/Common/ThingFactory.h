/* Stand-in for GameEngine/Include/Common/ThingFactory.h, Module build only (differential tests, p1w12-sonnet, ReplaceObjectUpgrade): the two calls
** that module makes, answered by differential/l9_replace_object.inc. Renamed ModTestThingFactory by module_prelude.h. */
#ifndef SHIM_MODULE_THINGFACTORY_H
#define SHIM_MODULE_THINGFACTORY_H
#include "Common/AsciiString.h"
#include "Common/ObjectStatusTypes.h"
class ThingTemplate;
class Object;
class Team;
class ThingFactory
{
public:
	const ThingTemplate *findTemplate(const AsciiString &name, Bool check = TRUE);
	Object *newObject(const ThingTemplate *tmplate, Team *team, ObjectStatusMaskType statusMask = ObjectStatusMaskType());
};
extern ThingFactory *TheThingFactory;
#endif
