/* Stand-in for GameEngine/Include/Common/UnicodeString.h, Module build only (differential tests, p1w12-sonnet, update batch L): what AutoDepositUpdate.cpp
** does with a UnicodeString -- `moneyString.format(TheGameText->fetch("GUI:AddCash"), amount)` and hand it to the in-game UI -- is a number the UI is told. The
** class keeps the label and the first integer argument of `format`; differential/l9_updates_l.inc logs them. The real class is the unicode build's
** (wrap_unicode_string.cpp); this one is renamed ModTestUnicodeString by module_prelude.h so that the two never meet at link time. */
#ifndef SHIM_MODULE_UNICODESTRING_H
#define SHIM_MODULE_UNICODESTRING_H
#include <stdarg.h>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	UnicodeString() : m_label(0), m_arg(0) {}
	void format(const UnicodeString &fmt, ...) { va_list args; va_start(args, fmt); m_label = fmt.m_label; m_arg = va_arg(args, int); va_end(args); }
	/* batch R76: ProductionUpdate's `msg.format(format.str(), upgradeName.str())`: the format's label and the upgrade's name */
	const char *str() const { return m_label; }
	void format(const char *fmt, const char *arg) { m_label = fmt; m_arg = 0; m_argLabel = arg; }
	const char *m_argLabel = 0;
	const char *m_label;
	int m_arg;
};
#endif
