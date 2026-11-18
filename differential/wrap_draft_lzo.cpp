/* Differential-test wrapper for the LZO draft (#215, upstream commit 0a05454d), src/drafts/wwlib/lzo.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo1x_c.cpp   (lzo1x_1_compress)
**   WWLib/lzo1x_d.cpp                                            (lzo1x_decompress)
**   WWLib/lzo.cpp                                                (LZOCompressor::Compress, ::Decompress)
** all #included below UNCHANGED, so the object code is the upstream text compiled as the shipped build compiles it
** (NDEBUG, so the asserts are out). lzo.h, lzo1x.h, lzoconf.h and lzo_conf.h come from upstream too.
**
** Two stand-ins. (1) `private` is `public` while lzo.h is read, so ref_lzo_work_buffer_clear can zero WorkBuffer (no code changes).
** (2) lzo.cpp includes mutex.h (the Windows CRITICAL_SECTION wrapper, mutex.cpp, thread.h); `MUTEX_H` is
** defined first so that header is skipped, and CriticalSectionClass and its LockClass are declared here as empty classes. They
** only guard LZOCompressor::WorkBuffer against threads, and the test calls from one thread at a time (a mutex of its own).
**
** Entry points (extern "C", tests/differential_draft_codec.rs):
**   ref_lzo_compress_fresh(in, n, out, &out_len)   lzo1x_1_compress with a work buffer zeroed by this call (the draft's
**                                                  "fresh dictionary per call")
**   ref_lzo_compressor_compress(in, n, out, &out_len)  LZOCompressor::Compress: the static WorkBuffer, kept between calls
**   ref_lzo_work_buffer_clear()                    zero LZOCompressor::WorkBuffer (a process start's state)
**   ref_lzo_decompress(in, n, out, &out_len)       lzo1x_decompress
** `out` must hold LZO_BUFFER_SIZE(n) bytes (compress); the caller sizes decompression's. */
#include <stdlib.h>
#include <string.h>

#define MUTEX_H
class CriticalSectionClass
{
public:
	class LockClass
	{
	public:
		LockClass(CriticalSectionClass &) {}
		~LockClass() {}
	};
};

/* the test clears LZOCompressor::WorkBuffer (private) to reproduce a process start: lzo.h is read once with private public */
#define private public
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo.h"
#undef private

#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo1x_c.cpp"
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo1x_d.cpp"
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo.cpp"

extern "C" {

int ref_lzo_compress_fresh(const unsigned char *in, unsigned n, unsigned char *out, unsigned *out_len)
{
	lzo_uint ol = 0;
	static unsigned char *wrk = 0;
	if (!wrk) wrk = (unsigned char *)malloc(LZO1X_MEM_COMPRESS);
	memset(wrk, 0, LZO1X_MEM_COMPRESS);
	int r = lzo1x_1_compress(in, n, out, &ol, wrk);
	*out_len = (unsigned)ol;
	return r;
}

int ref_lzo_compressor_compress(const unsigned char *in, unsigned n, unsigned char *out, unsigned *out_len)
{
	lzo_uint ol = 0;
	int r = LZOCompressor::Compress(in, n, out, &ol);
	*out_len = (unsigned)ol;
	return r;
}

void ref_lzo_work_buffer_clear(void)
{
	memset(LZOCompressor::WorkBuffer, 0, sizeof(LZOCompressor::WorkBuffer));
}

int ref_lzo_decompress(const unsigned char *in, unsigned n, unsigned char *out, unsigned *out_len)
{
	lzo_uint ol = 0;
	int r = lzo1x_decompress(in, n, out, &ol, NULL);
	*out_len = (unsigned)ol;
	return r;
}

}
