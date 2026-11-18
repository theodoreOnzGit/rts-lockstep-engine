/* Differential-test wrapper for the MD5 draft (#215, upstream commit 0a05454d), src/drafts/wwlib/md5.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/md5.cpp (MD5Init, MD5Update, MD5Final, and the static MD5Transform, Encode,
**   Decode and MD5_memcpy / MD5_memset they call), #included UNCHANGED, with md5.h and global.h.
**
** One stand-in: global.h says `typedef unsigned long int UINT4;` ("a four byte word"), which is 4 bytes on the Windows the game
** was built for and 8 on this Linux. `long` is defined as nothing while md5.cpp is read, so that line is `unsigned int`, the
** shipped build's type (nothing else in md5.cpp, md5.h or global.h says `long`).
**
** Entry point (extern "C", tests/differential_draft_codec.rs):
**   ref_md5_case(ctx_in, init, data, data_len, cuts, ncuts, ctx_mid, digest, ctx_final)
**     ctx_in    88 bytes: state[4], count[2] (u32, native order), buffer[64]; the context to start from (a real MD5Init's, or
**               a random one when `init` is 0: Update and Final are defined on any context whose count's low 6 bits index
**               the buffer)
**     init      1: MD5Init(&ctx) first (only state and count are written)
**     data      the message, fed to MD5Update in `ncuts` pieces of the lengths in `cuts`
**     ctx_mid   the 88 bytes of the context after the updates; digest 16 bytes from MD5Final; ctx_final the context after
**               MD5Final (zeroized) */
#include <stdlib.h>
#include <string.h>
#define long
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/md5.cpp"
#undef long

extern "C" {

void ref_md5_case(const unsigned char *ctx_in, int init, const unsigned char *data, unsigned data_len, const unsigned *cuts, unsigned ncuts,
                  unsigned char *ctx_mid, unsigned char *digest, unsigned char *ctx_final)
{
	MD5_CTX ctx;
	memcpy(&ctx, ctx_in, sizeof ctx);
	if (init) MD5Init(&ctx);
	unsigned char *m = (unsigned char *)malloc(data_len + 1);
	memcpy(m, data, data_len);
	unsigned at = 0;
	for (unsigned k = 0; k < ncuts; ++k) {
		MD5Update(&ctx, m + at, cuts[k]);
		at += cuts[k];
	}
	memcpy(ctx_mid, &ctx, sizeof ctx);
	MD5Final(digest, &ctx);
	memcpy(ctx_final, &ctx, sizeof ctx);
	free(m);
}

}
