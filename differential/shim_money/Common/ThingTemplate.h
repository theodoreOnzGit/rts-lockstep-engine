/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5), handicap
** tests only: Handicap::getBestThingType asks whether the template is a
** structure. The wrapper answers (see wrap_handicap.cpp). */
#ifndef SHIM_MONEY_THINGTEMPLATE_H
#define SHIM_MONEY_THINGTEMPLATE_H
#include "Common/KindOf.h"
class ThingTemplate
{
public:
	Bool isKindOf(KindOfType t) const;
};
#endif
