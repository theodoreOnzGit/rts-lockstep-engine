/* Differential-test wrapper for the SHA-1 draft (#215, upstream commit 0a05454d), src/drafts/wwlib/sha.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sha.h and sha.cpp (SHAEngine::Hash, Process_Partial, Process_Block, Result,
**   Init), #included UNCHANGED.
**
** Stand-ins. (1) `long`: sha.h sizes everything by `sizeof(long)` (SRC_BLOCK_SIZE = 16*sizeof(long)) and the algorithm by
** `long` being 32 bits, as on the Windows the game was built for; on this Linux it is 64. `long` is defined as `int` after
** the system headers are read, while sha.h and sha.cpp are read. (2) shim_draft_codec/new.h and iostream.h stand in for the
** pre-standard <new.h> and <iostream.h> (nothing from the latter is used). (3) always.h is read first from differential/shim (its ALWAYS_H guard then skips WWLib/always.h, whose placement operator new[] collides with <new>'s; sha.h uses nothing from it). (4) TRUE_FALSE_DEFINED is defined, as bool.h
** itself requires of a compiler with a bool type (`_MSC_VER < 1100` is its test).
**
** Entry point (extern "C", tests/differential_draft_codec.rs):
**   ref_sha_case(data, data_len, ops, nops, out) runs one engine through `nops` operations of two words [kind, n]:
**     kind 0  Hash(next n bytes of data)        kind 1  Result(): appends the 20 digest bytes to out
**     kind 2  Init()                            kind 3  Result() twice in a row (the IsCached path): appends both
**   and returns the count of bytes written to out. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <new>
#include "always.h"
#define TRUE_FALSE_DEFINED
#define long int
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sha.h"
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sha.cpp"
#undef long

extern "C" {

int ref_sha_case(const unsigned char *data, unsigned data_len, const int *ops, unsigned nops, unsigned char *out)
{
	SHAEngine e;
	unsigned char *m = (unsigned char *)malloc(data_len + 1);
	memcpy(m, data, data_len);
	unsigned at = 0;
	int w = 0;
	for (unsigned k = 0; k < nops; ++k) {
		int kind = ops[2 * k], n = ops[2 * k + 1];
		if (kind == 0) {
			e.Hash(m + at, n);
			at += n;
		} else if (kind == 1) {
			w += e.Result(out + w);
		} else if (kind == 3) {
			w += e.Result(out + w);
			w += e.Result(out + w);
		} else {
			e.Init();
		}
	}
	free(m);
	return w;
}

}
