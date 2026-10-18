/* Stand-in for GameEngine/Include/GameLogic/Module/AIUpdate.h, ThingTemplate
** build only (differential tests, p1w4-thingtemplate): AIUpdateModuleData,
** the module data of the AI (layer 9), as far as ThingTemplate.cpp uses it:
** a ModuleData for which isAiModuleData() is true, and the Locomotor field's
** parser. No module of the tests is one. (Renamed TtAIUpdateModuleData by
** thingtemplate_prelude.h.) */
#ifndef SHIM_TT_AIUPDATE_H
#define SHIM_TT_AIUPDATE_H
#include "Common/Module.h"
class AIUpdateModuleData : public ModuleData
{
public:
	virtual Bool isAiModuleData() const { return true; }
	static void parseLocomotorSet(INI *ini, void *instance, void *store, const void *userData);
};
#endif
