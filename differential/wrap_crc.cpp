/* Differential-test wrapper for GameEngine/Include/Common/crc.h and
** GameEngine/Source/Common/crc.cpp (upstream commit 0a05454d).
**
** Two references are exposed:
**  1. ref_crc_debug_*: the C++ CRC class compiled from the unchanged upstream
**     header and crc.cpp with _DEBUG defined (the out-of-line C++ path).
**  2. ref_crc_asm_*: the release-build path of crc.h is an MSVC `_asm` block
**     that g++ cannot compile. It is transcribed here instruction for
**     instruction into GNU inline assembly (AT&T syntax). This is a
**     hand transcription, not compiled upstream code; it checks that the
**     Rust matches the x86 semantics of the shipped instruction sequence. */
#include "Common/CRC.h"   /* compiled with -D_DEBUG: the C++ class */

extern "C" {

unsigned int ref_crc_debug_buffer(const unsigned char *buf, int len)
{
	CRC c;
	c.computeCRC(buf, len);
	return c.get();
}

/* Feed `n` consecutive chunks of `buf` (lengths in lens[]) into one CRC. */
unsigned int ref_crc_debug_chunks(const unsigned char *buf, const int *lens, int n)
{
	CRC c;
	for (int i = 0; i < n; ++i) {
		c.computeCRC(buf, lens[i]);
		if (lens[i] > 0) buf += lens[i];
	}
	return c.get();
}

/* Release-build CRC::computeCRC, transcribed from crc.h lines 77-103:
**   mov esi,[buf] / mov ecx,[len] / dec ecx / mov edi,[crcPtr]
**   mov ebx,dword ptr [edi] / xor eax,eax
** lp: mov al,byte ptr [esi] / shl ebx,1 / inc esi / adc ebx,eax
**   dec ecx / jns lp / mov dword ptr [edi],ebx
** The guard `if (!buf||len<1) return;` precedes the asm upstream. */
unsigned int ref_crc_asm_update(unsigned int crc, const unsigned char *buf, int len)
{
#if defined(__x86_64__) || defined(__i386__)
	if (!buf || len < 1)
		return crc;
	const unsigned char *p = buf;
	int n = len;
	__asm__ volatile(
		"decl %%ecx\n\t"
		"xorl %%eax, %%eax\n\t"
		"1:\n\t"
		"movb (%[p]), %%al\n\t"
		"shll $1, %%ebx\n\t"
		"inc %[p]\n\t"
		"adcl %%eax, %%ebx\n\t"
		"decl %%ecx\n\t"
		"jns 1b\n\t"
		: "+b"(crc), [p] "+S"(p), "+c"(n)
		:
		: "eax", "cc", "memory");
	return crc;
#else
#error "the asm CRC reference needs an x86 host"
#endif
}

} /* extern "C" */
