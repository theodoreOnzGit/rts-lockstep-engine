/* Differential-test reference for the fast float helpers of
** Libraries/Include/Lib/BaseType.h (upstream commit 0a05454d), lines 175-245.
**
** Upstream these are MSVC `__asm` blocks with no C++ fallback, so g++ cannot
** compile them. Each is transcribed here instruction for instruction into GNU
** inline assembly (AT&T syntax), on 32-bit registers as in the original.
** This is a hand transcription, NOT compiled upstream code: it verifies the
** Rust against the x86 semantics of the shipped instruction sequence.
**
** fast_float2long_round uses x87 `fld`/`fistp`, whose rounding follows the
** x87 control word. The game sets round-to-nearest (GameLogic.cpp setFPMode,
** _RC_NEAR); the Linux default control word is also round-to-nearest, so no
** control-word change is made here. Precision control does not affect fistp. */
#include <string.h>

#if !(defined(__x86_64__) || defined(__i386__))
#error "the BaseType.h asm references need an x86 host"
#endif

extern "C" {

/* __forceinline long fast_float2long_round(float f)
**   { long i; __asm { fld [f]  fistp [i] } return i; }
** `long` is 32 bits on Win32, so the store is fistp dword (fistpl). */
int ref_fast_float2long_round(float f)
{
	int i;
	__asm__ volatile(
		"flds %[f]\n\t"
		"fistpl %[i]\n\t"
		: [i] "=m"(i)
		: [f] "m"(f)
		: "st");
	return i;
}

/* __forceinline float fast_float_trunc(float f)
**   mov ecx,[f] / shr ecx,23 / mov eax,0xff800000 / xor ebx,ebx
**   sub cl,127 / cmovc eax,ebx / sar eax,cl / and [f],eax */
float ref_fast_float_trunc(float f)
{
	__asm__ volatile(
		"movl %[f], %%ecx\n\t"
		"shrl $23, %%ecx\n\t"
		"movl $0xff800000, %%eax\n\t"
		"xorl %%ebx, %%ebx\n\t"
		"subb $127, %%cl\n\t"
		"cmovcl %%ebx, %%eax\n\t"
		"sarl %%cl, %%eax\n\t"
		"andl %%eax, %[f]\n\t"
		: [f] "+m"(f)
		:
		: "eax", "ebx", "ecx", "cc");
	return f;
}

/* fast_float_floor / fast_float_ceil are plain C++ upstream around the asm
** trunc; copied verbatim except that the type-pun goes through memcpy. */
float ref_fast_float_floor(float f)
{
	static unsigned almost1 = (126 << 23) | 0x7fffff;
	unsigned bits; memcpy(&bits, &f, 4);
	float a; memcpy(&a, &almost1, 4);
	if (bits & 0x80000000)
		f -= a;
	return ref_fast_float_trunc(f);
}

float ref_fast_float_ceil(float f)
{
	static unsigned almost1 = (126 << 23) | 0x7fffff;
	unsigned bits; memcpy(&bits, &f, 4);
	float a; memcpy(&a, &almost1, 4);
	if ((bits & 0x80000000) == 0)
		f += a;
	return ref_fast_float_trunc(f);
}

/* The REAL_TO_* macros, exactly as upstream composes them. */
int ref_real_to_int(float x) { return (int)(ref_fast_float2long_round(ref_fast_float_trunc(x))); }
unsigned int ref_real_to_unsigned_int(float x) { return (unsigned int)(ref_fast_float2long_round(ref_fast_float_trunc(x))); }
short ref_real_to_short(float x) { return (short)(ref_fast_float2long_round(ref_fast_float_trunc(x))); }
unsigned short ref_real_to_unsigned_short(float x) { return (unsigned short)(ref_fast_float2long_round(ref_fast_float_trunc(x))); }
signed char ref_real_to_byte(float x) { return (signed char)(ref_fast_float2long_round(ref_fast_float_trunc(x))); }
unsigned char ref_real_to_unsigned_byte(float x) { return (unsigned char)(ref_fast_float2long_round(ref_fast_float_trunc(x))); }
int ref_double_to_int(double x) { return (int)(ref_fast_float2long_round(ref_fast_float_trunc((float)x))); }
int ref_real_to_int_ceil(float x) { return ref_fast_float2long_round(ref_fast_float_ceil(x)); }
int ref_real_to_int_floor(float x) { return ref_fast_float2long_round(ref_fast_float_floor(x)); }

} /* extern "C" */
