/* Stand-in for GameEngine/Include/Common/Player.h, ThingTemplate build only
** (differential tests, p1w4-thingtemplate): the members of Player that
** ThingTemplate.cpp's calcCostToBuild, calcTimeToBuild and
** getBuildFacilityTemplate and ProductionPrerequisite.cpp call. They are
** layer 7; the Rust does not translate those three methods (a cut point), so
** the wrapper defines these to return constants and never calls them. */
#ifndef SHIM_TT_PLAYER_H
#define SHIM_TT_PLAYER_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/KindOf.h"
#include "Common/Science.h"
class ThingTemplate;
class Handicap
{
public:
	enum HandicapType { BUILDCOST, BUILDTIME };
	Real getHandicap(HandicapType t, const ThingTemplate *tmpl) const;
};
class Energy
{
public:
	Real getEnergySupplyRatio() const;
};
class Player
{
public:
	Real getProductionCostChangePercent(AsciiString buildTemplateName) const;
	Real getProductionCostChangeBasedOnKindOf(KindOfMaskType kindOf) const;
	Real getProductionTimeChangePercent(AsciiString buildTemplateName) const;
	const Handicap *getHandicap() const;
	Energy *getEnergy() const;
	Bool buildsInstantly() const;
	void countObjectsByThingTemplate(Int numTemplates, const ThingTemplate *const *things, Bool ignoreDead, Int *counts) const;
	Bool hasScience(ScienceType t) const;
};
#endif
