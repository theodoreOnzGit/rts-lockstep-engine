/* Stand-in for GameEngine/Include/Common/Money.h (layer 4), academy tests only:
** countMoney(), the amount the wrapper sets. */
#ifndef SHIM_ACADEMY_MONEY_H
#define SHIM_ACADEMY_MONEY_H
#include "Lib/BaseType.h"
class Money
{
public:
	Money() : m_amount(0) {}
	UnsignedInt countMoney() const { return m_amount; }
	UnsignedInt m_amount;
};
#endif
