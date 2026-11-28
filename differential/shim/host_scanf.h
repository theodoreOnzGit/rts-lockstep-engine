/* Differential tests only (#242): a host-independent `sscanf` for the
** upstream files that read numbers with it, INI.cpp (INI::scanInt,
** scanUnsignedInt, scanReal, scanPercentToReal) and CrateSystem.cpp
** (parseCrateCreationEntry). Each force-included prelude that needs it
** (shim_ini/ini_prelude.h, shim_terrain/terrain_prelude.h) includes this
** header after <stdio.h> and then `#define sscanf Host_independent_sscanf`.
** The upstream files are not edited.
**
** Why. Upstream's reference answers came from the host's C library. glibc
** after 2.39 reads `%f` by the C standard's rule, so "1e" is a matching
** failure, where earlier glibc gave 1.0. `src/common/ini.rs` reproduces the
** earlier reading. On a newer glibc, 18 test binaries then failed with the
** C++ side moved and the Rust side unchanged (#242). This stand-in fixes the
** C++ side to glibc 2.39's reading on every host.
**
** `%f` is a translation of glibc 2.39's float conversion
** (stdio-common/vfscanf-internal.c, the `case L_('f')` path), covering the
** cases a single "%f" on a string reaches:
** - C white space is skipped; nothing after it is EOF.
** - An optional sign is read and applied after the conversion.
** - "nan" (any case) is NaN, with the sign and the default payload. Any
**   "(chars)" after it is left unread.
** - "inf" is infinity; if an 'i' follows, the whole "inity" must follow,
**   else the match fails.
** - Otherwise characters are collected into a buffer: a "0x"/"0X" prefix
**   (hexadecimal), then digits (hex digits after "0x"), one '.', and one
**   exponent letter ('e', or 'p' after "0x") once a digit has been seen,
**   with an optional sign right after it. An empty buffer, or just "0x",
**   fails. The value is `strtof` of the buffer, which must take at least one
**   character ("1e" collects "1e", and strtof takes the "1": 1.0).
** `strtof` itself did not change across these glibc versions.
** `%d` and `%u` go to the host's `vsscanf` unchanged (the C23 change does
** not touch them). Any other format aborts, so a new caller cannot slip
** past unnoticed.
**
** Measured 2026-11-28 against glibc 2.39's own sscanf("%f"), in the
** container: 6,000,054 tokens (2,000,027 from a 32-character alphabet of
** digits, signs, '.', e/p/x, the letters of inf/nan, white space and
** parentheses; 4,000,027 digit-heavy ones up to 15 characters; each set
** plus 27 fixed edge cases), 0 with a different result or a different bit
** pattern. */
#ifndef SHIM_HOST_SCANF_H
#define SHIM_HOST_SCANF_H
#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <string.h>
#include <ctype.h>

static inline int Host_scanf_lower(int c) { return c >= 'A' && c <= 'Z' ? c + 32 : c; }

/* Match the lower-case word `w` at *p, ignoring case; advance *p past it on success. */
static inline int Host_scanf_word(const char **p, const char *w)
{
	const char *q = *p;
	for (; *w; ++w, ++q)
		if (Host_scanf_lower((unsigned char)*q) != *w)
			return 0;
	*p = q;
	return 1;
}

/* glibc 2.39's `%f` on a string (see the header). */
static inline int Host_scanf_real(const char *s, float *out)
{
	const char *p = s;
	while (*p && isspace((unsigned char)*p))
		++p;
	if (!*p)
		return EOF;
	int neg = 0;
	if (*p == '+' || *p == '-') {
		neg = *p == '-';
		++p;
	}
	if (Host_scanf_lower((unsigned char)*p) == 'n') {
		if (!Host_scanf_word(&p, "nan"))
			return 0;
		*out = neg ? -__builtin_nanf("") : __builtin_nanf("");
		return 1;
	}
	if (Host_scanf_lower((unsigned char)*p) == 'i') {
		if (!Host_scanf_word(&p, "inf"))
			return 0;
		if (Host_scanf_lower((unsigned char)*p) == 'i' && !Host_scanf_word(&p, "inity"))
			return 0;
		*out = neg ? -__builtin_inff() : __builtin_inff();
		return 1;
	}
	char b[512];
	size_t n = 0;
	int hex = 0, got_digit = 0, got_dot = 0, got_e = 0;
	if (p[0] == '0' && Host_scanf_lower((unsigned char)p[1]) == 'x') {
		b[n++] = p[0];
		b[n++] = p[1];
		p += 2;
		hex = 1;
	}
	while (*p && n < sizeof b - 2) {
		int c = (unsigned char)*p;
		if (hex ? isxdigit(c) : isdigit(c)) {
			/* a binary exponent takes decimal digits only */
			if (got_e && hex && !isdigit(c))
				break;
			b[n++] = (char)c;
			got_digit = 1;
			++p;
		} else if (c == '.' && !got_dot && !got_e) {
			b[n++] = (char)c;
			got_dot = 1;
			++p;
		} else if (Host_scanf_lower(c) == (hex ? 'p' : 'e') && got_digit && !got_e) {
			b[n++] = (char)c;
			got_e = 1;
			++p;
			if (*p == '+' || *p == '-')
				b[n++] = *p++;
		} else {
			break;
		}
	}
	b[n] = 0;
	if (n == 0 || (hex && n == 2))
		return 0;
	char *end;
	float v = strtof(b, &end);
	if (end == b)
		return 0;
	*out = neg ? -v : v;
	return 1;
}

static inline int Host_independent_sscanf(const char *s, const char *format, ...)
{
	va_list ap;
	va_start(ap, format);
	int r;
	if (strcmp(format, "%f") == 0) {
		r = Host_scanf_real(s, va_arg(ap, float *));
	} else if (strcmp(format, "%d") == 0 || strcmp(format, "%u") == 0) {
		r = vsscanf(s, format, ap);
	} else {
		fprintf(stderr, "host_scanf.h: format \"%s\" is not covered (#242)\n", format);
		abort();
	}
	va_end(ap);
	return r;
}
#endif
