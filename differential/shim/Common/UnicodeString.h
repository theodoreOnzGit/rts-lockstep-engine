/* Shim for GameEngine/Include/Common/UnicodeString.h, differential tests only:
** str(), getLength(), set(const WideChar*) on a std::basic_string of the
** 16-bit WideChar (see the shim Lib/BaseType.h). */
#ifndef SHIM_UNICODESTRING_H
#define SHIM_UNICODESTRING_H
#include <string>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	UnicodeString() {}
	const WideChar *str() const { return m_s.c_str(); }
	int getLength() const { return (int)m_s.size(); }
	void set(const WideChar *s) { m_s = s; }
	void clear() { m_s.clear(); }
	/* test helper (not upstream): set from code units that may contain NUL */
	void setUnits(const WideChar *p, int n) { m_s.assign(p, n); }
private:
	std::basic_string<WideChar> m_s;
};
#endif
