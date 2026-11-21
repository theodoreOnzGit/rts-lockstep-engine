/* differential/shim_ui4 (#215, group ui4): the shim's Common/UnicodeString.h (differential/shim/Common/UnicodeString.h) COPIED, with the members the menu
** cuts call and the shim lacks added at the end of the class (marked `ui4`): set(const UnicodeString &), translate(const char *), trim() (UnicodeString.cpp:231: leading and trailing
** iswspace removed; the C locale's white space: 0x20 and 0x09..0x0D). The menus' wrapper (wrap_draft_ui4.cpp) puts this directory ahead of differential/shim. */
/* Shim for GameEngine/Include/Common/UnicodeString.h, differential tests only:
** TheEmptyString, str(), getLength(), isEmpty(), set(const WideChar*), getCharAt(),
** construction and assignment from a WideChar string, operator==, on a
** std::basic_string of the 16-bit WideChar (see the shim Lib/BaseType.h).
** WideChar is 16 bits here, as on Win32, so upstream's L"..." literals
** (32-bit wchar_t on Linux) get their own constructor and format(), which
** narrow each unit to 16 bits; format() goes through glibc's vswprintf, so
** it is NOT upstream's _vsnwprintf (the tests only use the "MISSING" text
** GameText::fetch formats as an existence flag, never its content). */
#ifndef SHIM_UI4_UNICODESTRING_H
#define SHIM_UI4_UNICODESTRING_H
#include <string>
#include <stdarg.h>
#include <wchar.h>
#include "Lib/BaseType.h"
class UnicodeString
{
public:
	static const UnicodeString TheEmptyString;
	UnicodeString() {}
	UnicodeString(const WideChar *s) { if (s) m_s = s; }
	UnicodeString(const wchar_t *s) { setWide(s); }
	UnicodeString &operator=(const WideChar *s) { if (s) m_s = s; else m_s.clear(); return *this; }
	WideChar getCharAt(int i) const { return m_s[i]; }
	void format(const wchar_t *fmt, ...)
	{
		wchar_t buf[2048];
		va_list args;
		const wchar_t *use = fmt;
#ifdef SHIM_UNICODE_MSVC_FORMAT
		/* #78, for the TUs that define SHIM_UNICODE_MSVC_FORMAT (differential/wrap_save_list.cpp): MSVC's wide printf
		** reads `%s` as a wide string and `%S` as a narrow one, glibc's the other way round (`%s` narrow, through the C
		** locale: ASCII only; `%S` and `%ls` wide). The format is rewritten to glibc's spelling first. */
		std::wstring msvc;
		for (const wchar_t *c = fmt; *c; ++c) {
			msvc += *c;
			if (*c != L'%') continue;
			++c;
			while (*c && wcschr(L"-+ #0123456789.*", *c)) msvc += *c++;
			if (!*c) break;
			if (*c == L's') msvc += L"ls";
			else if (*c == L'S') msvc += L's';
			else msvc += *c;
		}
		use = msvc.c_str();
#endif
		va_start(args, fmt);
		int n = vswprintf(buf, sizeof(buf) / sizeof(buf[0]) - 1, use, args);
		va_end(args);
		if (n < 0) buf[0] = 0;
		setWide(buf);
	}
	bool operator==(const UnicodeString &o) const { return m_s == o.m_s; }
	const WideChar *str() const { return m_s.c_str(); }
	int getLength() const { return (int)m_s.size(); }
	bool isEmpty() const { return m_s.empty(); }
	void clear() { m_s.clear(); }
	/* test helper (not upstream): set from code units that may contain NUL */
	void setUnits(const WideChar *p, int n) { m_s.assign(p, n); }
	/* getBufferForRead(len) (UnicodeString.cpp): room for len units and a NUL,
	** zero-filled as the engine's allocator fills it; the length is then
	** that of the C string the caller leaves (see fixLength) */
	WideChar *getBufferForRead(int len) { m_s.assign(len, 0); return &m_s[0]; }
	void fixLength() { size_t n = 0; while (n < m_s.size() && m_s[n]) ++n; m_s.resize(n); }
	/* --- p1w4-sides --- */
	/* translate(const AsciiString&) (UnicodeString.cpp line 200): each char, sign-extended as
	** the signed char it is, becomes a WideChar */
	template <class A> void translate(const A &src)
	{
		clear();
		for (int i = 0; i < src.getLength(); i++) m_s += (WideChar)src.getCharAt(i);
	}
	/* --- end p1w4-sides --- */
	/* --- p1w10-sonnet --- */
	/* format(UnicodeString format, ...) (UnicodeString.h line 243): the format is this string's units widened back to wchar_t */
	void format(UnicodeString fmt, ...)
	{
		wchar_t w[1024];
		size_t n = 0;
		for (; n + 1 < sizeof(w) / sizeof(w[0]) && n < fmt.m_s.size(); ++n) w[n] = (wchar_t)fmt.m_s[n];
		w[n] = 0;
		wchar_t buf[2048];
		va_list args;
		va_start(args, fmt);
		int r = vswprintf(buf, sizeof(buf) / sizeof(buf[0]) - 1, w, args);
		va_end(args);
		if (r < 0) buf[0] = 0;
		setWide(buf);
	}
	/* --- end p1w10-sonnet --- */
	/* --- layer 12 batch G1 --- */
	/* compare (UnicodeString.h: wcscmp of str() and s): the 16-bit units compared in order (batch L3b) */
	int compare(const WideChar *s) const
	{
		const WideChar *a = str();
		while (*a && *a == *s) { ++a; ++s; }
		return (int)*a - (int)*s;
	}
	/* compare(const UnicodeString&) (UnicodeString.h: wcscmp of the two strings), for Mouse::setCursorTooltip (#124) */
	int compare(const UnicodeString &s) const { return compare(s.str()); }
	/* compareNoCase (UnicodeString.h: _wcsicmp): ASCII letters folded to lower case, then the units compared */
	int compareNoCase(const UnicodeString &o) const
	{
		size_t i = 0;
		for (;; ++i) {
			WideChar a = i < m_s.size() ? m_s[i] : 0, b = i < o.m_s.size() ? o.m_s[i] : 0;
			if (a >= 'A' && a <= 'Z') a += 32;
			if (b >= 'A' && b <= 'Z') b += 32;
			if (a != b) return a < b ? -1 : 1;
			if (a == 0) return 0;
		}
	}
	/* operator!= (UnicodeString.h: wcscmp of the two strings), for Recorder.cpp's version checks (batch R1) */
	bool operator!=(const UnicodeString &o) const { return m_s != o.m_s; }
	/* concat(WideChar) (UnicodeString.h line 380: concat of { c, 0 }, so a 0 appends nothing), for the dispatcher's
	** beacon text (batch D1) */
	void concat(WideChar c) { if (c) m_s += c; }
	/* --- end layer 12 batch G1 --- */
	/* removeLastChar (UnicodeString.cpp: the unit before the terminator dropped, nothing for an empty string), for the
	** transitions' typed text (#77) */
	void removeLastChar() { if (!m_s.empty()) m_s.erase(m_s.size() - 1); }
	/* ui4: UnicodeString::set(const UnicodeString &) */
	void set(const UnicodeString &o) { m_s = o.m_s; }
	void set(const WideChar *s2) { if (s2) m_s = s2; else m_s.clear(); }
	/* ui4: UnicodeString::translate(const AsciiString &) called with a C string (the AsciiString is made from it): each char, sign-extended as the signed char it is,
	** becomes a WideChar, a 0 appended as nothing (concat(WideChar)) */
	void translate(const char *c) { m_s.clear(); for (; *c; ++c) { WideChar w = (WideChar)*c; if (w) m_s += w; } }
	/* ui4: UnicodeString::trim() */
	void trim()
	{
		size_t a = 0;
		while (a < m_s.size() && (m_s[a] == 0x20 || (m_s[a] >= 9 && m_s[a] <= 13))) ++a;
		m_s.erase(0, a);
		while (!m_s.empty() && (m_s[m_s.size() - 1] == 0x20 || (m_s[m_s.size() - 1] >= 9 && m_s[m_s.size() - 1] <= 13))) m_s.erase(m_s.size() - 1);
	}
private:
	void setWide(const wchar_t *s) { m_s.clear(); for (; s && *s; ++s) m_s += (WideChar)*s; }
	std::basic_string<WideChar> m_s;
};
#endif
