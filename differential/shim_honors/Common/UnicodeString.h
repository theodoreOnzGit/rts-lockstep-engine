/* Stand-in for GameEngine/Include/Common/UnicodeString.h, the battle honours build only (GitHub #157, differential
** tests): a vector of 16-bit WideChar (as on Win32) with what the cut code and AsciiString.cpp's translate() use:
** TheEmptyString, isEmpty, getLength, getCharAt, and format(L"...") through the C library's vswprintf. */
#ifndef SHIM_HONORS_UNICODESTRING_H
#define SHIM_HONORS_UNICODESTRING_H
#include <stdarg.h>
#include <wchar.h>
#include <vector>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	static const UnicodeString TheEmptyString;
	UnicodeString() {}
	UnicodeString(const WideChar *s) { while (s && *s) m_v.push_back(*s++); }
	Bool isEmpty() const { return m_v.empty(); }
	int getLength() const { return (int)m_v.size(); }
	WideChar getCharAt(int i) const { return m_v[i]; }
	void push(WideChar c) { m_v.push_back(c); }
	void format(const wchar_t *fmt, ...)
	{
		wchar_t buf[512];
		va_list a;
		va_start(a, fmt);
		vswprintf(buf, 512, fmt, a);
		va_end(a);
		m_v.clear();
		for (const wchar_t *p = buf; *p; ++p) m_v.push_back((WideChar)*p);
	}
private:
	std::vector<WideChar> m_v;
};
#endif
