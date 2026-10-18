/* Forced include (-include) for the UnicodeString build, differential tests
** only: what the real PreRTS.h brings before UnicodeString.cpp's own
** includes (the allocator, AsciiString, UnicodeString, the wide-character
** C runtime), made 16-bit the way MSVC's is.
**
** WideChar is `wchar_t` upstream, which MSVC makes a 16-bit unsigned type.
** This build compiles with -fshort-wchar (build.rs), so `wchar_t` and the
** `L"..."` literals upstream uses are 16-bit and unsigned here too, and
** WideChar is `wchar_t` itself (the other builds' shim BaseType.h spells it
** `unsigned short`, which the `L" \t\n\r"` of nextToken would not convert to).
** glibc's wide functions work on 32-bit units, so each one UnicodeString
** calls is replaced below by a 16-bit version that reproduces MSVC's
** behaviour in the C locale:
**
**  wcslen, wcscmp, wcsspn, wcscspn: the standard definitions on 16-bit
**    units; wcscmp orders the units as UNSIGNED (wchar_t is unsigned
**    short in MSVC), and only the sign of its result is used.
**  wcscpy, wcscat: copy FORWARD, one unit at a time, stopping after the
**    terminator. UnicodeString::set(const WideChar*) copies into its own
**    buffer when it has room, and trim() and nextToken() set a string from
**    its own tail, so source and destination overlap (destination first),
**    undefined in C. MSVC's forward copy gives the intended result (a
**    forward copy with dst < src never reads a unit it has overwritten),
**    which the game relies on; the translation does the same (see
**    src/common/unicode_string.rs). The reverse overlap, wcscat of a
**    string onto itself, runs away under a forward copy (the terminator is
**    overwritten before it is read) and is not exercised.
**  _wcsicmp: folds each unit to lower case with towlower, which in the C
**    locale changes only 'A'..'Z' (as for _stricmp in AsciiString), and
**    returns the difference of the folded units. Folding to LOWER case
**    matters: '_' (0x5f) sorts after 'A' that way, before it if folded up.
**  iswspace: in the C locale, true for 0x09..0x0d and 0x20. MSVC's answer
**    for units from 0x80 (0x85, 0xa0, 0x2000...) in the C locale is NOT
**    established here (it needs the MSVC runtime), so this stand-in says
**    false for them and the differential test never evaluates iswspace on
**    such a unit: the translation returns an error for them.
**  _vsnwprintf: format_va is not translated; this stand-in lets it compile
**    and aborts if it is ever called. */
#ifndef SHIM_UNICODE_PRELUDE_H
#define SHIM_UNICODE_PRELUDE_H
#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>
#include <string>
#include <vector>
#define WideChar shim_unicode_unused_widechar
#include "Lib/BaseType.h"
#undef WideChar
typedef wchar_t WideChar;
static_assert(sizeof(WideChar) == 2, "build with -fshort-wchar");

static inline size_t shim_wcslen16(const WideChar *s)
{
	size_t n = 0;
	while (s[n] != 0) ++n;
	return n;
}
static inline WideChar *shim_wcscpy16(WideChar *dst, const WideChar *src)
{
	WideChar *d = dst;
	while ((*d++ = *src++) != 0) {}
	return dst;
}
static inline WideChar *shim_wcscat16(WideChar *dst, const WideChar *src)
{
	WideChar *d = dst;
	while (*d != 0) ++d;
	while ((*d++ = *src++) != 0) {}
	return dst;
}
static inline int shim_wcscmp16(const WideChar *a, const WideChar *b)
{
	while (*a != 0 && *a == *b) { ++a; ++b; }
	return (int)(unsigned short)*a - (int)(unsigned short)*b;
}
static inline int shim_towlower16(int c) { return (c >= 'A' && c <= 'Z') ? c + ('a' - 'A') : c; }
static inline int shim_wcsicmp16(const WideChar *a, const WideChar *b)
{
	int f, l;
	do {
		f = shim_towlower16((unsigned short)*a++);
		l = shim_towlower16((unsigned short)*b++);
	} while (f != 0 && f == l);
	return f - l;
}
static inline size_t shim_wcsspn16(const WideChar *s, const WideChar *set)
{
	size_t n = 0;
	for (; s[n] != 0; ++n) {
		const WideChar *p = set;
		while (*p != 0 && *p != s[n]) ++p;
		if (*p == 0) break;
	}
	return n;
}
static inline size_t shim_wcscspn16(const WideChar *s, const WideChar *set)
{
	size_t n = 0;
	for (; s[n] != 0; ++n) {
		const WideChar *p = set;
		while (*p != 0 && *p != s[n]) ++p;
		if (*p != 0) break;
	}
	return n;
}
static inline int shim_iswspace16(unsigned int c) { return (c >= 9 && c <= 13) || c == 32; }
static inline int shim_vsnwprintf_unreachable(WideChar *, size_t, const WideChar *, va_list) { abort(); }
#define wcslen shim_wcslen16
#define wcscpy shim_wcscpy16
#define wcscat shim_wcscat16
#define wcscmp shim_wcscmp16
#define _wcsicmp shim_wcsicmp16
#define wcsspn shim_wcsspn16
#define wcscspn shim_wcscspn16
#define iswspace shim_iswspace16
#define _vsnwprintf shim_vsnwprintf_unreachable
#include "PreRTS.h"
#include "Common/GameMemory.h"
#include "Common/CriticalSection.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#endif
