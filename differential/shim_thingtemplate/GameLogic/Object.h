/* Stand-in for GameEngine/Include/GameLogic/Object.h, ThingTemplate build
** only (differential tests, p1w4-thingtemplate): what ThingFactory::newObject
** does with the Object the game logic makes: its behavior modules (none in
** the tests: Object is layer 8) and initObject. Also the forward references
** the real header brings. (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_OBJECT_H
#define SHIM_TT_OBJECT_H
#include "Common/ObjectStatusTypes.h"
#include "GameLogic/Module/CreateModule.h"
class Team;
class Object
{
public:
	BehaviorModule **getBehaviorModules() { return m_modules; }
	void initObject() { m_initialised = true; }
	BehaviorModule *m_modules[2];
	bool m_initialised;
	const void *m_template;
};
#include "GameLogic/GameLogic.h"
#endif
