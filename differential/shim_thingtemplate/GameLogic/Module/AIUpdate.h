/* Stand-in for GameEngine/Include/GameLogic/Module/AIUpdate.h, ThingTemplate
** build only (differential tests, p1w4-thingtemplate; batch A1): the module
** data of the AI as upstream declares it, cut out verbatim by build.rs
** (ai_update_decl.inc: LocomotorSetType to the end of AIUpdateModuleData, and
** TurretAIData from TurretAI.h, whose header needs the turret machine). The
** rest of the header is AIUpdateInterface (layer 10), not compiled here.
** (AIUpdateModuleData and TurretAIData are renamed by thingtemplate_prelude.h.) */
#ifndef SHIM_TT_AIUPDATE_H
#define SHIM_TT_AIUPDATE_H
#include "Common/Module.h"
#include "Common/GameMemory.h"
#include "Common/GameCommon.h"
#include "GameLogic/WeaponSet.h"
/* BehaviorModule.h lines 87-98 and UpdateModule.h lines 122-130 (the headers themselves declare the modules, which
** CreateModule.h's stand-in stands in for) */
class BehaviorModuleData : public ModuleData
{
public:
	BehaviorModuleData()
	{
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    ModuleData::buildFieldParse(p);
	}
};
class UpdateModuleData : public BehaviorModuleData
{
public:

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
		BehaviorModuleData::buildFieldParse(p);
	}
};
class LocomotorTemplate;
#include "ai_update_decl.inc"
#endif
