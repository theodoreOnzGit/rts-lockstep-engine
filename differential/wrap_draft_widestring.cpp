/* Differential-test wrapper for the draft src/drafts/wwlib/widestring.rs (#215, group wwlib3), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/widestring.h and widestring.cpp, compiled UNCHANGED (#included);
**   trim.cpp's wcstrim and wwstring.cpp's StringClass::Copy_Wide, cut VERBATIM by build.rs (drafts_wwlib3).
** Built with -fshort-wchar so wchar_t is 16 bits as under MSVC. Stand-ins (what the upstream code calls outside itself):
**   - the wide C runtime: glibc's functions work on 32-bit units, so wcslen, wcscpy, wcscmp, _wcsicmp are 16-bit versions
**     (the ones of differential/shim_unicode/unicode_prelude.h's documentation): wcscmp's SIGN is what the C standard
**     defines and what the test compares (MSVC's UCRT returns -1/0/1, glibc and the shim the difference); _wcsicmp folds
**     'A'..'Z' only (the C locale);
**   - _vsnwprintf: the test gives Format a format string without % directives, so it copies the format; at 512 units or
**     more it returns -1 (a buffer without terminator, undefined upstream: those lengths are not generated);
**   - MultiByteToWideChar / WideCharToMultiByte (CP_ACP = Windows-1252): a table typed here from the code page chart,
**     the same chart the draft's table was typed from, so the table itself is NOT independently checked; unmapped units
**     become '?' and set *used (the test only converts units the code page has);
**   - win.h, wwdebug.h, wwstring.h's StringClass and FastCriticalSectionClass (differential/shim_draft_wwlib3): types and
**     no-ops (no locks, WWASSERT compiled out as in the release build).
** The temporary-string pool (the four static 256-unit buffers) is real and is exercised: the test builds some strings with
** hint_temporary = true. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdarg.h>
#include <wchar.h>
#include <wctype.h>
#include <string>

static size_t shim_wcslen16(const wchar_t *s) { size_t n = 0; while (s[n] != 0) ++n; return n; }
static wchar_t *shim_wcscpy16(wchar_t *d0, const wchar_t *s) { wchar_t *d = d0; while ((*d++ = *s++) != 0) {} return d0; }
static int shim_wcscmp16(const wchar_t *a, const wchar_t *b)
{
	while (*a != 0 && *a == *b) { ++a; ++b; }
	return (int)(unsigned short)*a - (int)(unsigned short)*b;
}
static int shim_lower(int c) { return (c >= 'A' && c <= 'Z') ? c + 32 : c; }
static int shim_wcsicmp16(const wchar_t *a, const wchar_t *b)
{
	int f, l;
	do { f = shim_lower((unsigned short)*a++); l = shim_lower((unsigned short)*b++); } while (f != 0 && f == l);
	return f - l;
}
template <class V> static int shim_vsnwprintf16(wchar_t *buf, size_t n, const wchar_t *fmt, V &&)
{
	size_t len = shim_wcslen16(fmt);
	if (len >= n) { memcpy(buf, fmt, n * sizeof(wchar_t)); return -1; }
	memcpy(buf, fmt, (len + 1) * sizeof(wchar_t));
	return (int)len;
}
#define wcslen shim_wcslen16
#define wcscpy shim_wcscpy16
#define wcscmp shim_wcscmp16
#define _wcsicmp shim_wcsicmp16
#define _vsnwprintf shim_vsnwprintf16
#define _cdecl

#include "win.h"
#include "widestring.h"
#include "wwstring.h"
#include "trim.h"

/* Windows-1252: bytes 0x80..0x9F (0x81, 0x8D, 0x8F, 0x90, 0x9D are undefined in the chart and map to themselves in
** MultiByteToWideChar); every other byte is its own code point (Latin-1) */
static const unsigned short CP[32] = {
	0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D, 0x017D, 0x008F,
	0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178 };
int MultiByteToWideChar(unsigned, unsigned long, const char *mb, int mbn, wchar_t *wc, int wcn)
{
	int n = mbn < 0 ? (int)strlen(mb) + 1 : mbn;
	if (wc == 0) return n;
	for (int i = 0; i < n && i < wcn; i++) {
		unsigned char b = (unsigned char)mb[i];
		wc[i] = (b >= 0x80 && b < 0xA0) ? CP[b - 0x80] : b;
	}
	return n;
}
int WideCharToMultiByte(unsigned, unsigned long, const wchar_t *wc, int wcn, char *mb, int mbn, const char *, BOOL *used)
{
	int n = wcn < 0 ? (int)wcslen(wc) + 1 : wcn;
	if (used) *used = 0;
	if (mb == 0) return n;
	for (int i = 0; i < n && i < mbn; i++) {
		unsigned c = (unsigned short)wc[i];
		int b = -1;
		if (c < 0x80 || (c >= 0xA0 && c <= 0xFF)) b = (int)c;
		else for (int k = 0; k < 32; k++) if (CP[k] == c) { b = 0x80 + k; break; }
		if (b < 0) { b = '?'; if (used) *used = 1; }
		mb[i] = (char)b;
	}
	return n;
}

#include "trim_wcstrim.inc"
#include "wwstring_copy_wide.inc"
#include "widestring.cpp"

/* The program (an int stream): ops on a string A. A string argument is its length (-1 for NULL) then its units. The output is,
** after every op, [Get_Length(), c-string length, the units of the c-string] and the op's own result words. */
static int g_cap, *g_out, g_n;
static void put(int w) { if (g_n < g_cap) g_out[g_n] = w; g_n++; }
static const wchar_t *arg(const int *&p, std::basic_string<wchar_t> &store)
{
	int len = *p++;
	if (len < 0) return 0;
	store.clear();
	for (int i = 0; i < len; i++) store.push_back((wchar_t)*p++);
	return store.c_str();
}
static void dump(const WideStringClass &s)
{
	const wchar_t *c = (const wchar_t *)s;
	put(s.Get_Length());
	int l = (int)wcslen(c);
	put(l);
	for (int i = 0; i < l; i++) put((unsigned short)c[i]);
}

extern "C" int ref_ws_run(const int *prog, int nops, int *out, int cap)
{
	g_out = out; g_cap = cap; g_n = 0;
	const int *p = prog;
	WideStringClass *a = new WideStringClass();
	for (int k = 0; k < nops; k++) {
		int op = *p++;
		std::basic_string<wchar_t> st;
		switch (op) {
		case 0: { const wchar_t *s = arg(p, st); *a = s; break; }
		case 1: { *a = (wchar_t)*p++; break; }
		case 2: { const wchar_t *s = arg(p, st); *a += s; break; }
		case 3: { *a += (wchar_t)*p++; break; }
		case 4: { /* A = A + B(arg) via operator+ of two classes; the result's length and units are the output */
			const wchar_t *s = arg(p, st); bool tmp = *p++ != 0; WideStringClass b(s, tmp); WideStringClass c = (*a + b);
			dump(c); break; }
		case 5: { const wchar_t *s = arg(p, st); int r = a->Compare(s); put(r < 0 ? -1 : r > 0 ? 1 : 0); break; }
		case 6: { const wchar_t *s = arg(p, st); int r = a->Compare_No_Case(s); put(r < 0 ? -1 : r > 0 ? 1 : 0); break; }
		case 7: { const wchar_t *s = arg(p, st);
			put(*a == s); put(*a != s); put(*a < s); put(*a <= s); put(*a > s); put(*a >= s); break; }
		case 8: { int i = *p++; put((unsigned short)(*(const WideStringClass *)a)[i]); break; }
		case 9: { int i = *p++; int n = *p++; a->Erase(i, n); break; }
		case 10: { a->Trim(); break; }
		case 11: { const wchar_t *s = arg(p, st); put(a->Format(s)); break; }
		case 12: { std::string t; int len = *p++; if (len < 0) { put(a->Convert_From((const char *)0)); break; }
			for (int i = 0; i < len; i++) t.push_back((char)*p++);
			put(a->Convert_From(t.c_str())); break; }
		case 13: { StringClass sc; bool r = a->Convert_To(sc); put(r); put((int)sc.s.size()); for (size_t i = 0; i < sc.s.size(); i++) put((unsigned char)sc.s[i]); break; }
		case 14: { put(a->Is_ANSI()); break; }
		case 15: { WideStringClass *b = new WideStringClass(*a); dump(*b); *a = *b; delete b; break; } /* copy constructor and operator= */
		case 16: { int n = *p++; int fill = *p++; wchar_t *b = a->Get_Buffer(n);
			for (int i = 0; i < n - 1; i++) b[i] = (wchar_t)fill; b[n - 1] = 0; break; }
		case 17: { put(a->Is_Empty()); break; }
		case 18: { /* constructors: kind 0 from WCHAR*, 1 from WCHAR, 2 from char*, then the string replaces A */
			int kind = *p++; WideStringClass *n;
			if (kind == 0) { const wchar_t *s = arg(p, st); bool tmp = *p++ != 0; n = new WideStringClass(s, tmp); }
			else if (kind == 1) { wchar_t ch = (wchar_t)*p++; bool tmp = *p++ != 0; n = new WideStringClass(ch, tmp); }
			else { std::string t; int len = *p++; for (int i = 0; i < len; i++) t.push_back((char)*p++); bool tmp = *p++ != 0; n = new WideStringClass(t.c_str(), tmp); }
			delete a; a = n; break; }
		case 19: { int i = *p++; int v = *p++; (*a)[i] = (wchar_t)v; break; }
		}
		dump(*a);
	}
	delete a;
	return g_n;
}
