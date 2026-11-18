/* Forced include (-include) for the logicx text builds (differential/wrap_draft_language_filter.cpp, wrap_draft_version.cpp;
** #215, upstream commit 0a05454d): the STLTypedefs-with-UnicodeString prelude (shim_stl_unicode/stl_unicode_prelude.h: the 16-bit
** wide C runtime, the MSVC string-function names), then what the two cuts need beyond it. Each stand-in replaces a C-runtime
** function (never upstream code):
**  _vsnwprintf: the 16-bit wide printf of MSVC for the conversions version.cpp uses (%d, %c, %s with a WIDE string argument,
**    as MSVC's wide printf reads %s, and %%). It returns -1 when the text does not fit, as _vsnwprintf does.
**  wcsstr, wcsrchr: the standard definitions on 16-bit units (wcsrchr finds the terminating NUL when asked for 0).
**  WEOF: MSVC's ((wint_t)0xFFFF) with wint_t unsigned short, so `c == WEOF` for an UnsignedShort c compares 0xFFFF with 0xFFFF
**    (glibc's WEOF is 0xffffffff, which an UnsignedShort could never equal). */
#ifndef SHIM_DRAFT_TEXT_PRELUDE_H
#define SHIM_DRAFT_TEXT_PRELUDE_H
#include "stl_unicode_prelude.h"

static inline int shim_logicx_vsnwprintf(WideChar *buf, size_t n, const WideChar *fmt, va_list ap)
{
	size_t at = 0;
	bool full = false;
	auto put = [&](WideChar c) { if (at < n) buf[at++] = c; else full = true; };
	for (const WideChar *f = fmt; *f != 0; ++f) {
		if (*f != '%') { put(*f); continue; }
		++f;
		if (*f == '%') { put('%'); }
		else if (*f == 'd') {
			char t[16];
			snprintf(t, sizeof(t), "%d", va_arg(ap, int));
			for (char *p = t; *p; ++p) put((WideChar)*p);
		} else if (*f == 'c') {
			put((WideChar)va_arg(ap, int));
		} else if (*f == 's') {
			const WideChar *s = va_arg(ap, const WideChar *);
			for (; *s != 0; ++s) put(*s);
		} else {
			abort(); /* a conversion version.cpp does not use */
		}
		if (*f == 0) break;
	}
	if (full) return -1;
	if (at < n) buf[at] = 0;
	return (int)at;
}
#undef _vsnwprintf
#define _vsnwprintf shim_logicx_vsnwprintf

static inline const WideChar *shim_logicx_wcsstr(const WideChar *h, const WideChar *n)
{
	if (*n == 0) return h;
	for (; *h != 0; ++h) {
		const WideChar *a = h, *b = n;
		while (*a != 0 && *b != 0 && *a == *b) { ++a; ++b; }
		if (*b == 0) return h;
	}
	return 0;
}
static inline WideChar *shim_logicx_wcsstr(WideChar *h, const WideChar *n) { return (WideChar *)shim_logicx_wcsstr((const WideChar *)h, n); }
static inline const WideChar *shim_logicx_wcsrchr(const WideChar *s, int c)
{
	const WideChar *last = 0;
	for (;; ++s) {
		if (*s == (WideChar)c) last = s;
		if (*s == 0) break;
	}
	return last;
}
static inline WideChar *shim_logicx_wcsrchr(WideChar *s, int c) { return (WideChar *)shim_logicx_wcsrchr((const WideChar *)s, c); }
#define wcsstr shim_logicx_wcsstr
#define wcsrchr shim_logicx_wcsrchr
#undef WEOF
#define WEOF ((unsigned short)0xFFFF)
#endif
