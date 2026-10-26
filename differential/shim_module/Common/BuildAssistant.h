/* Stand-in for GameEngine/Include/Common/BuildAssistant.h, Module build only (differential tests, p1w13-sonnet, batch R76): what
** ProductionUpdate.cpp asks of the build assistant, whether a unit can be made, the test's answer (differential/l9_production_r76.inc).
** CanMakeType is the header's, lines 75-84, verbatim; the other modules read only CONSTRUCTION_COMPLETE of it. */
#ifndef SHIM_MODULE_BUILDASSISTANT_H
#define SHIM_MODULE_BUILDASSISTANT_H
#include "Lib/BaseType.h"
class Object;
class ThingTemplate;
enum { CONSTRUCTION_COMPLETE = -1 };   /* line 49, verbatim (AutoDepositUpdate.cpp reads it) */
enum CanMakeType
{
	CANMAKE_OK,
	CANMAKE_NO_PREREQ,
	CANMAKE_NO_MONEY,
	CANMAKE_FACTORY_IS_DISABLED,
	CANMAKE_QUEUE_FULL,						// production bldg has full production queue
	CANMAKE_PARKING_PLACES_FULL,	// production bldg has finite slots for existing units
	CANMAKE_MAXED_OUT_FOR_PLAYER	// player has as many as they are allowed at once (eg, Black Lotus
};
class BuildAssistant
{
public:
	CanMakeType canMakeUnit( Object *builder, const ThingTemplate *whatToBuild ) const;
};
extern BuildAssistant *TheBuildAssistant;
#endif
