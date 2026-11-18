/* Differential-test wrapper for the fixed-point draft (#215, upstream commit 0a05454d), src/drafts/wwlib/fixed.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/fixed.h and fixed.cpp (class fixed: the constructors, To_ASCII, As_ASCII and
**   the constants), #included UNCHANGED; the operators are fixed.h's inline members and friends.
** Stand-ins: BIG_ENDIAN is undefined after the system headers (below); TRUE_FALSE_DEFINED is defined first, as bool.h itself requires of a compiler that has a `bool` (its test is
** `_MSC_VER < 1100`; this g++ has one). (fixed.h's `Data.Raw` is the 16-bit word; every entry point gives and takes it.)
**
** Entry points (extern "C", tests/differential_draft_fixed_color.rs):
**   ref_fixed_op(op, a, b, i, u)  one operation on `a` and `b` (raw words), the int `i` and the unsigned `u`; the result is
**                                 the raw word of a `fixed` result, the int of an int result, or 0/1 for a bool. `op` numbers
**                                 are the table in the test (the same numbers).
**   ref_fixed_from_ascii(text, nul_terminated_ok)  fixed(char const *): the raw word (a NULL text when `text` is null)
**   ref_fixed_to_ascii(raw, maxlen, buf, cap)  To_ASCII: fills buf (cap bytes, which the caller has pre-filled) and returns the
**                                 count To_ASCII returns
**   ref_fixed_as_ascii(raw, buf)   As_ASCII: copies the 32-byte static buffer to buf */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stddef.h>
#define TRUE_FALSE_DEFINED
/* glibc's <endian.h> (read by stdlib.h) defines BIG_ENDIAN as a number, which upstream's `#ifdef BIG_ENDIAN` takes for a big-endian
** target; MSVC on x86 does not define it. Stand-in: undefined after the system headers. */
#undef BIG_ENDIAN
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/fixed.cpp"

static fixed mk(unsigned raw)
{
	fixed f;
	/* fixed is one unsigned short (union Data): set it through a 16-bit copy */
	unsigned short r = (unsigned short)raw;
	memcpy(&f, &r, sizeof r);
	return f;
}
static unsigned rawOf(const fixed &f)
{
	unsigned short r;
	memcpy(&r, &f, sizeof r);
	return r;
}

extern "C" {

int ref_fixed_op(int op, unsigned a, unsigned b, int i, unsigned u)
{
	fixed x = mk(a);
	const fixed y = mk(b);
	switch (op) {
		case 0: return rawOf(fixed(i, (int)u));
		case 1: return rawOf(fixed(i));
		case 2: return (int)(unsigned)x;
		case 3: x *= y; return rawOf(x);
		case 4: x *= i; return rawOf(x);
		case 5: x /= y; return rawOf(x);
		case 6: x /= i; return rawOf(x);
		case 7: x += y; return rawOf(x);
		case 8: x -= y; return rawOf(x);
		case 9: return rawOf(x * y);
		case 10: return x * i;
		case 11: return rawOf(x / y);
		case 12: return x / i;
		case 13: return rawOf(x + y);
		case 14: return x + i;
		case 15: return rawOf(x - y);
		case 16: return x - i;
		case 17: x >>= u; return rawOf(x);
		case 18: x <<= u; return rawOf(x);
		case 19: return !x;
		case 20: return x < i;
		case 21: return x > i;
		case 22: return x <= i;
		case 23: return x >= i;
		case 24: return x == i;
		case 25: return x != i;
		case 26: x.Round_Up(); return rawOf(x);
		case 27: x.Round_Down(); return rawOf(x);
		case 28: x.Round(); return rawOf(x);
		case 29: x.Saturate(u); return rawOf(x);
		case 30: x.Saturate(y); return rawOf(x);
		case 31: x.Sub_Saturate(u); return rawOf(x);
		case 32: x.Sub_Saturate(y); return rawOf(x);
		case 33: x.Inverse(); return rawOf(x);
		case 34: return i / y;
		case 35: return i - y;
		case 36: return i * y;
		case 37: return i + y;
		case 38: return u < y;
		case 39: return u > y;
		case 40: return u <= y;
		case 41: return u >= y;
		case 42: return u == y;
		case 43: return u != y;
		case 44: return x < y;
		case 45: return x > y;
		case 46: return x <= y;
		case 47: return x >= y;
		case 48: return x == y;
		case 49: return x != y;
		case 50: return rawOf(Round_Up(x));
		case 51: return rawOf(Round_Down(x));
		case 52: return rawOf(Round(x));
		case 53: return rawOf(Saturate(x, u));
		case 54: return rawOf(Sub_Saturate(x, u));
		case 55: return rawOf(Inverse(x));
		case 56: return rawOf(fixed::_1_2);
		case 57: return rawOf(fixed::_1_3);
		case 58: return rawOf(fixed::_1_4);
		case 59: return rawOf(fixed::_3_4);
		case 60: return rawOf(fixed::_2_3);
		case 61: return rawOf(x >> u);
		case 62: return rawOf(x << u);
		case 63: return rawOf(Saturate(x, y));
		case 64: return rawOf(Sub_Saturate(x, y));
	}
	return 0;
}

int ref_fixed_from_ascii(const char *text)
{
	return (int)rawOf(fixed(text));
}

int ref_fixed_to_ascii(unsigned raw, int maxlen, char *buf)
{
	return mk(raw).To_ASCII(buf, maxlen);
}

void ref_fixed_as_ascii(unsigned raw, char *buf)
{
	const char *s = mk(raw).As_ASCII();
	memcpy(buf, s, 32);
}

}
