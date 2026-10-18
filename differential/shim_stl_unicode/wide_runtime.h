/* The 16-bit wide C runtime of differential/shim_unicode/unicode_prelude.h,
** copied (its lines from the guard to the last macro, which is what
** UnicodeString.cpp needs), for the STLTypedefs-with-UnicodeString build
** (differential/wrap_stl_typedefs_unicode.cpp). The unicode prelude itself
** also includes that build's AsciiString and UnicodeString stand-ins, which
** this build must not see; so this copy stops before them. Keep the two in
** step: the behaviour described in unicode_prelude.h applies here too. */
#ifndef SHIM_STL_UNICODE_WIDE_RUNTIME_H
#define SHIM_STL_UNICODE_WIDE_RUNTIME_H
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
#endif
