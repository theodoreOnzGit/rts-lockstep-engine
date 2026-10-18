/* Stand-in for GameEngine/Include/Common/UnicodeString.h, academy tests only:
** the members AcademyStats.cpp uses (format, concat), over the WideChar string
** of the shared shim. The advice's text is GameText's; the wrapper's
** TheGameText->fetch returns the label, so the test reads the labels. */
#ifndef SHIM_ACADEMY_UNICODESTRING_H
#define SHIM_ACADEMY_UNICODESTRING_H
#include <string>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	UnicodeString() {}
	UnicodeString(const WideChar *s) { if (s) m_s = s; }
	UnicodeString(const wchar_t *s) { for (; s && *s; ++s) m_s += (WideChar)*s; }
	void format(const UnicodeString &s) { m_s = s.m_s; }
	void concat(const UnicodeString &s) { m_s += s.m_s; }
	const std::basic_string<WideChar> &units() const { return m_s; }
private:
	std::basic_string<WideChar> m_s;
};
#endif
