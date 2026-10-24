/* Stand-in for GameEngine/Include/GameLogic/ScriptEngine.h, Module build only (differential tests, p1w12-sonnet, die batch D): the one call
** SpecialPowerCompletionDie makes, answered by differential/l9_dies_d.inc. Renamed ModTestScriptEngine by module_prelude.h. */
#ifndef SHIM_MODULE_SCRIPTENGINE_H
#define SHIM_MODULE_SCRIPTENGINE_H
#include "Common/AsciiString.h"
#include "Common/GameType.h"
class Object;
class ScriptEngine
{
public:
	void notifyOfCompletedSpecialPower(Int playerIndex, const AsciiString &specialPowerName, ObjectID sourceObject);
	void notifyOfTriggeredSpecialPower(Int playerIndex, const AsciiString &completedPower, ObjectID sourceObj);   /* special power batch H */
	void transferObjectName(const AsciiString &unitName, Object *pNewObject);   /* p1w13-sonnet, batch R13b: VeterancyCrateCollide's pilot */
	void adjustToppleDirection( Object *object, struct Coord3D *direction );   /* p1w13-sonnet, batch R28: ToppleUpdate (differential/l9_topple_r28.inc) */
	void adjustToppleDirection( Object *object, struct Coord2D *direction );   /* batch R36: StructureToppleUpdate (differential/l9_structure_topple_r36.inc) */
};
extern ScriptEngine *TheScriptEngine;
#endif
