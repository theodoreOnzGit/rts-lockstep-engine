/* Stand-in for GameEngine/Include/GameLogic/Module/CreateModule.h, ThingTemplate
** build only (differential tests, p1w4-thingtemplate): the interface that
** ThingFactory::newObject asks each behavior module for. The tests give the
** Object no modules. (Renamed Tt* by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_CREATEMODULE_H
#define SHIM_TT_CREATEMODULE_H
class CreateModuleInterface
{
public:
	virtual void onCreate() = 0;
};
class BehaviorModule
{
public:
	virtual ~BehaviorModule() {}
	virtual CreateModuleInterface *getCreate() { return 0; }
};
#endif
