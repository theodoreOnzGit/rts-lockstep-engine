/* Stand-in for GameEngine/Include/Common/ThingTemplate.h, differential tests
** only (StateMachine build). StateMachine.cpp names the owner's template only
** in STATE_MACHINE_DEBUG logging, which a release build does not compile. */
#ifndef SM_SHIM_THINGTEMPLATE_H
#define SM_SHIM_THINGTEMPLATE_H
#include "Common/AsciiString.h"
class ThingTemplate
{
public:
	AsciiString getName() const { return AsciiString("template"); }
};
#endif
