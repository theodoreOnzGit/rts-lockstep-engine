/* Stand-in for GameEngine/Include/GameLogic/ScriptEngine.h, Module build only (differential tests, p1w12-sonnet, die batch D): the one call
** SpecialPowerCompletionDie makes, answered by differential/l9_dies_d.inc. Renamed ModTestScriptEngine by module_prelude.h. */
#ifndef SHIM_MODULE_SCRIPTENGINE_H
#define SHIM_MODULE_SCRIPTENGINE_H
#include "Common/AsciiString.h"
#include "Common/GameType.h"
class Object;
class ThingTemplate;
/* batch R69: what CommandButtonHuntUpdate reads of an attack priority info; getPriority is ScriptEngine.cpp's on the test's table
** (differential/l9_command_button_hunt_r69.inc) */
class AttackPriorityInfo { public: Int getPriority(const ThingTemplate *tThing) const; Int m_defaultPriority; const ThingTemplate *m_tmpl[6]; Int m_priority[6]; };
class ScriptEngine
{
public:
	void notifyOfCompletedSpecialPower(Int playerIndex, const AsciiString &specialPowerName, ObjectID sourceObject);
	void notifyOfCompletedUpgrade(Int playerIndex, const AsciiString &upgrade, ObjectID sourceObject);   /* p1w13-sonnet, batch R76: ProductionUpdate */
	void notifyOfTriggeredSpecialPower(Int playerIndex, const AsciiString &completedPower, ObjectID sourceObj);   /* special power batch H */
	void transferObjectName(const AsciiString &unitName, Object *pNewObject);   /* p1w13-sonnet, batch R13b: VeterancyCrateCollide's pilot */
	void adjustToppleDirection( Object *object, struct Coord3D *direction );   /* p1w13-sonnet, batch R28: ToppleUpdate (differential/l9_topple_r28.inc) */
	void adjustToppleDirection( Object *object, struct Coord2D *direction );   /* batch R36: StructureToppleUpdate (differential/l9_structure_topple_r36.inc) */
};
extern ScriptEngine *TheScriptEngine;
#endif
