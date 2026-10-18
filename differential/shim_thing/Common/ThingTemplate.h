/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5, not
** translated), Thing and ObjectIter tests only. A template is its kind mask
** and its build cost; the three kind queries and the cost are the only
** members the two units call (and the Overridable base, which OVERRIDE<T>
** needs). The wrappers rename the class (wrap_thing.cpp),
** so it cannot clash with the other fixtures' ThingTemplate stand-ins. */
#ifndef SHIM_THING_THINGTEMPLATE_H
#define SHIM_THING_THINGTEMPLATE_H
#include "Common/KindOf.h"
#include "Common/Overridable.h"
/* Overridable, as upstream: OVERRIDE<ThingTemplate> reaches the template through it. */
class ThingTemplate : public Overridable
{
public:
	ThingTemplate(const KindOfMaskType &kinds, Int buildCost) : m_kinds(kinds), m_buildCost(buildCost) {}
	Bool isKindOf(KindOfType t) const { return m_kinds.test(t); }
	Bool isKindOfMulti(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) const
	{
		return (m_kinds & mustBeSet) == mustBeSet && (m_kinds & mustBeClear).none();
	}
	Bool isAnyKindOf(const KindOfMaskType& anyKindOf) const { return (m_kinds & anyKindOf).any(); }
	Int friend_getBuildCost() const { return m_buildCost; }
private:
	KindOfMaskType m_kinds;
	Int m_buildCost;
};
#endif
