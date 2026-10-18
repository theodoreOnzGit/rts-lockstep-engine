/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5), academy
** tests only: isKindOf (the kind-of bits the wrapper sets) and calcCostToBuild
** (the dozer's command set, which upstream never searches, see the module
** documentation of logic::world::academy_stats). */
#ifndef SHIM_ACADEMY_THINGTEMPLATE_H
#define SHIM_ACADEMY_THINGTEMPLATE_H
#include "Lib/BaseType.h"
#include "Common/KindOf.h"
class Player;
class ThingTemplate
{
public:
	Bool isKindOf(KindOfType t) const { return m_kindOf.test(t); }
	Int calcCostToBuild(const Player *p) const;
	KindOfMaskType m_kindOf;
};
#endif
