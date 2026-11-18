/* Differential-test wrapper for the Base64 draft (#215, upstream commit 0a05454d), src/drafts/wwlib/base64.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/base64.cpp (Base64_Encode, Base64_Decode), #included UNCHANGED.
** Stand-in: BIG_ENDIAN is undefined after the system headers (see below).
**
** Entry points (extern "C", tests/differential_draft_codec.rs): the upstream functions themselves, over buffers the caller
** gives (the test fills `dest` with a pattern first and compares the whole buffer, so a stray or missing write shows):
**   ref_base64_encode(source, slen, dest, dlen, null_source, null_dest)  Base64_Encode; a `null_*` flag passes NULL
**   ref_base64_decode(...)                                              Base64_Decode, the same arguments */
#include <stdlib.h>
#include <string.h>
/* glibc's <endian.h> (read by stdlib.h) defines BIG_ENDIAN as a number, which upstream's `#ifdef BIG_ENDIAN` takes for a big-endian
** target; MSVC on x86 does not define it. Stand-in: undefined after the system headers. */
#undef BIG_ENDIAN
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/base64.cpp"

extern "C" {

int ref_base64_encode(const unsigned char *source, int slen, unsigned char *dest, int dlen, int null_source, int null_dest)
{
	return Base64_Encode(null_source ? NULL : source, slen, null_dest ? NULL : dest, dlen);
}

int ref_base64_decode(const unsigned char *source, int slen, unsigned char *dest, int dlen, int null_source, int null_dest)
{
	return Base64_Decode(null_source ? NULL : source, slen, null_dest ? NULL : dest, dlen);
}

}
