/* Differential-test wrapper for the LCW draft (#215, upstream commit 0a05454d), src/drafts/wwlib/lcw.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lcw.cpp  (LCW_Uncomp), #included UNCHANGED. lcw.h is the upstream LCW.H
**   through shim_draft_codec/lcw.h (the file name's case). LCW_Comp is MSVC inline assembly (`#if defined(_MSC_VER)`) and is
**   not compiled here, as in any non-MSVC build.
** No stand-ins; compiled with -fpermissive, because lcw.cpp:127 casts a pointer to `unsigned` (`(unsigned) dest_ptr & 0x3`, the
** address's low bits), which 32-bit MSVC accepts and a 64-bit compiler rejects; the low two bits are the same.
**
** Entry point (extern "C", tests/differential_draft_codec.rs):
**   ref_lcw_uncomp(src, dest, cap, align) decodes src into a buffer of cap + 64 bytes that the wrapper allocates, whose
**   start is at an address congruent to `align` (0 to 3) modulo 4 (the 0xFE long run aligns the destination by its address),
**   copies the returned count of bytes to `dest` (cap bytes at most) and returns that count. */
#include <stdlib.h>
#include <string.h>
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lcw.cpp"

extern "C" {

int ref_lcw_uncomp(const unsigned char *src, unsigned char *dest, unsigned cap, unsigned align)
{
	unsigned char *raw = (unsigned char *)malloc(cap + 64 + 8);
	unsigned char *buf = raw + 4 + ((4 - ((unsigned long)(raw + 4) & 3)) & 3) + (align & 3);
	memset(raw, 0xCD, cap + 64 + 8);
	int n = LCW_Uncomp(src, buf, 0);
	memcpy(dest, buf, (unsigned)n < cap ? (unsigned)n : cap);
	free(raw);
	return n;
}

}
