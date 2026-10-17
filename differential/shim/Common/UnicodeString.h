/* Shim for GameEngine/Include/Common/UnicodeString.h, differential tests only:
** str(), getLength(), isEmpty(), set(const WideChar*), getCharAt(),
** construction and assignment from a WideChar string, operator==, on a
** std::basic_string of the 16-bit WideChar (see the shim Lib/BaseType.h).
** WideChar is 16 bits here, as on Win32, so upstream's L"..." literals
** (32-bit wchar_t on Linux) get their own constructor and format(), which
** narrow each unit to 16 bits; format() goes through glibc's vswprintf, so
** it is NOT upstream's _vsnwprintf (the tests only use the "MISSING" text
** GameText::fetch formats as an existence flag, never its content). */
#ifndef SHIM_UNICODESTRING_H
#define SHIM_UNICODESTRING_H
#include <string>
#include <stdarg.h>
#include <wchar.h>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	UnicodeString() {}
	UnicodeString(const WideChar *s) { if (s) m_s = s; }
	UnicodeString(const wchar_t *s) { setWide(s); }
	UnicodeString &operator=(const WideChar *s) { if (s) m_s = s; else m_s.clear(); return *this; }
	WideChar getCharAt(int i) const { return m_s[i]; }
	void format(const wchar_t *fmt, ...)
	{
		wchar_t buf[2048];
		va_list args;
		va_start(args, fmt);
		int n = vswprintf(buf, sizeof(buf) / sizeof(buf[0]) - 1, fmt, args);
		va_end(args);
		if (n < 0) buf[0] = 0;
		setWide(buf);
	}
	bool operator==(const UnicodeString &o) const { return m_s == o.m_s; }
	const WideChar *str() const { return m_s.c_str(); }
	int getLength() const { return (int)m_s.size(); }
	bool isEmpty() const { return m_s.empty(); }
	void set(const WideChar *s) { m_s = s; }
	void clear() { m_s.clear(); }
	/* test helper (not upstream): set from code units that may contain NUL */
	void setUnits(const WideChar *p, int n) { m_s.assign(p, n); }
private:
	void setWide(const wchar_t *s) { m_s.clear(); for (; s && *s; ++s) m_s += (WideChar)*s; }
	std::basic_string<WideChar> m_s;
};
#endif
