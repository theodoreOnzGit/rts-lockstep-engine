/* Stand-in for GameEngine/Include/Common/ThingTemplate.h (layer 5), score
** keeper tests only (p1w6-energy): the name, the kind-of mask (isKindOf,
** isKindOfMulti: the real BitFlags test), and isEquivalentTo, which the
** wrapper answers by the templates' equivalence groups. */
#ifndef SHIM_SCORE_THINGTEMPLATE_H
#define SHIM_SCORE_THINGTEMPLATE_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/KindOf.h"
class ThingTemplate
{
public:
	ThingTemplate() : m_group(0) {}
	AsciiString getName() const;
	Bool isKindOf(KindOfType t) const { return m_kindOf.test(t); }
	Bool isKindOfMulti(const KindOfMaskType &mustBeSet, const KindOfMaskType &mustBeClear) const
	{
		return m_kindOf.testSetAndClear(mustBeSet, mustBeClear);
	}
	Bool isEquivalentTo(const ThingTemplate *tt) const;
	AsciiString m_name;
	KindOfMaskType m_kindOf;
	Int m_group;
};
#endif
