/* Forced include (-include) for the network3 LANAPI build (#215, group network3, upstream commit 0a05454d): the guilogic3 chat prelude
** (shim_draft_g3/g3_prelude.h: the 16-bit wide C runtime and a `_vsnwprintf` that reads %ls, %s (a wide string, as MSVC's wide printf does),
** %d, %c, %%), a `_vsnwprintf` that also reads MSVC's `%hs` (a narrow string), which RequestHasMap's `mapDisplayName.format(L"%hs", ...)`
** uses, and `wcsncpy` over 16-bit units (the C library's: copy at most n units, stop at the terminator and zero-fill the rest of n).
** Stand-ins for C-runtime functions, never upstream code. */
#ifndef SHIM_DRAFT_N3_PRELUDE_H
#define SHIM_DRAFT_N3_PRELUDE_H
#include "../shim_draft_g3/g3_prelude.h"

static inline int shim_n3_vsnwprintf(WideChar *buf, size_t n, const WideChar *fmt, va_list ap)
{
	size_t at = 0;
	bool full = false;
	auto put = [&](WideChar c) { if (at < n) buf[at++] = c; else full = true; };
	for (const WideChar *f = fmt; *f != 0; ++f) {
		if (*f != '%') { put(*f); continue; }
		++f;
		if (*f == '%') { put('%'); }
		else if (*f == 'h' && f[1] == 's') {
			++f;
			const char *s = va_arg(ap, const char *);
			for (; *s != 0; ++s) put((WideChar)(unsigned char)*s);
		} else if (*f == 'l' && f[1] == 's') {
			++f;
			const WideChar *s = va_arg(ap, const WideChar *);
			for (; *s != 0; ++s) put(*s);
		} else if (*f == 's') {
			const WideChar *s = va_arg(ap, const WideChar *);
			for (; *s != 0; ++s) put(*s);
		} else if (*f == 'd') {
			char t[16];
			snprintf(t, sizeof(t), "%d", va_arg(ap, int));
			for (char *p = t; *p; ++p) put((WideChar)*p);
		} else if (*f == 'c') {
			put((WideChar)va_arg(ap, int));
		} else {
			abort();
		}
		if (*f == 0) break;
	}
	if (full) return -1;
	if (at < n) buf[at] = 0;
	return (int)at;
}
#undef _vsnwprintf
#define _vsnwprintf shim_n3_vsnwprintf

static inline WideChar *shim_n3_wcsncpy(WideChar *dst, const WideChar *src, size_t n)
{
	size_t i = 0;
	for (; i < n && src[i] != 0; ++i) dst[i] = src[i];
	for (; i < n; ++i) dst[i] = 0;
	return dst;
}
#define wcsncpy shim_n3_wcsncpy
#endif
