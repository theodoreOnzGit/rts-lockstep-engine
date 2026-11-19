/* Forced include (-include) for the guilogic3 chat build (#215, group guilogic3, upstream commit 0a05454d): the logicx text prelude
** (shim_draft_text/text_prelude.h: the 16-bit wide C runtime, `iswspace` as ASCII white space) and a `_vsnwprintf` that also reads
** MSVC's `%ls` (a wide string), which Chat.cpp's `fullMsg.format( L"%ls %ls", ... )` uses and the logicx one does not. A stand-in for the
** C runtime's wide printf, for the conversions the chat cut uses (%ls, %s with a wide argument as MSVC reads it, %d, %c, %%); never
** upstream code. It returns -1 when the text does not fit, as `_vsnwprintf` does. */
#ifndef SHIM_DRAFT_G3_PRELUDE_H
#define SHIM_DRAFT_G3_PRELUDE_H
#include "../shim_draft_text/text_prelude.h"

static inline int shim_g3_vsnwprintf(WideChar *buf, size_t n, const WideChar *fmt, va_list ap)
{
	size_t at = 0;
	bool full = false;
	auto put = [&](WideChar c) { if (at < n) buf[at++] = c; else full = true; };
	for (const WideChar *f = fmt; *f != 0; ++f) {
		if (*f != '%') { put(*f); continue; }
		++f;
		if (*f == '%') { put('%'); }
		else if (*f == 'l' && f[1] == 's') {
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
#define _vsnwprintf shim_g3_vsnwprintf
#endif
