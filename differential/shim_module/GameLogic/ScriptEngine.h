/* Stand-in for GameEngine/Include/GameLogic/ScriptEngine.h, Module build only (differential tests, p1w11-sonnet, die batch D): the one call
** SpecialPowerCompletionDie makes, answered by differential/l9_dies_d.inc. Renamed ModTestScriptEngine by module_prelude.h. */
#ifndef SHIM_MODULE_SCRIPTENGINE_H
#define SHIM_MODULE_SCRIPTENGINE_H
#include "Common/AsciiString.h"
#include "Common/GameType.h"
class ScriptEngine
{
public:
	void notifyOfCompletedSpecialPower(Int playerIndex, const AsciiString &specialPowerName, ObjectID sourceObject);
};
extern ScriptEngine *TheScriptEngine;
#endif
