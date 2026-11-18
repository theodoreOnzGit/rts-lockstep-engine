/* Differential-test wrapper for the RLE draft (#215, upstream commit 0a05454d), src/drafts/wwlib/rle.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/rle.cpp  (RLEEngine::Compress, Line_Compress, Decompress, Line_Decompress),
**   #included UNCHANGED; rle.h is the upstream RLE.H through shim_draft_codec/rle.h (the file name's case). The shim always.h
**   supplies MIN.
** No stand-ins.
**
** Entry points (extern "C", tests/differential_draft_codec.rs). Compress reads one byte past a run of zeros at the end of its
** input (`sptr[runcount] == 0 && runcount <= length`), so the wrapper copies the input into a buffer of its own with `after`
** as the byte that follows it (the draft treats a read past the slice as a non-zero byte; the test passes 0xAA):
**   ref_rle_compress(src, src_len, length, after, dest, dest_cap, measure)  Compress(src, measure ? NULL : dest, length)
**   ref_rle_line_compress(...)                                              Line_Compress, the same arguments
**   ref_rle_decompress(src, src_len, length, dest, dest_cap)                Decompress
**   ref_rle_line_decompress(src, src_len, dest, dest_cap)                   Line_Decompress
** The compress calls return the count and, unless `measure`, copy it to `dest`; the decompress calls copy the bytes they wrote
** (dest_cap at most). */
#include <stdlib.h>
#include <string.h>
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/rle.cpp"

extern "C" {

int ref_rle_compress(const unsigned char *src, unsigned src_len, int length, unsigned char after, unsigned char *dest, unsigned dest_cap, int measure)
{
	unsigned char *in = (unsigned char *)malloc(src_len + 1);
	memcpy(in, src, src_len);
	in[src_len] = after;
	unsigned char *out = (unsigned char *)malloc(dest_cap + 8);
	RLEEngine e;
	int n = e.Compress(in, measure ? NULL : out, length);
	if (!measure) memcpy(dest, out, (unsigned)n < dest_cap ? (unsigned)n : dest_cap);
	free(in);
	free(out);
	return n;
}

int ref_rle_line_compress(const unsigned char *src, unsigned src_len, int length, unsigned char after, unsigned char *dest, unsigned dest_cap, int measure)
{
	unsigned char *in = (unsigned char *)malloc(src_len + 1);
	memcpy(in, src, src_len);
	in[src_len] = after;
	unsigned char *out = (unsigned char *)malloc(dest_cap + 8);
	RLEEngine e;
	int n = e.Line_Compress(in, measure ? NULL : out, length);
	if (!measure) memcpy(dest, out, (unsigned)n < dest_cap ? (unsigned)n : dest_cap);
	free(in);
	free(out);
	return n;
}

int ref_rle_decompress(const unsigned char *src, unsigned src_len, int length, unsigned char *dest, unsigned dest_cap)
{
	unsigned char *in = (unsigned char *)malloc(src_len + 4);
	memcpy(in, src, src_len);
	memset(in + src_len, 0xAA, 4);
	unsigned char *out = (unsigned char *)malloc(dest_cap + 8);
	RLEEngine e;
	int n = e.Decompress(in, out, length);
	memcpy(dest, out, (unsigned)n < dest_cap ? (unsigned)n : dest_cap);
	free(in);
	free(out);
	return n;
}

int ref_rle_line_decompress(const unsigned char *src, unsigned src_len, unsigned char *dest, unsigned dest_cap)
{
	unsigned char *in = (unsigned char *)malloc(src_len + 4);
	memcpy(in, src, src_len);
	memset(in + src_len, 0xAA, 4);
	unsigned char *out = (unsigned char *)malloc(dest_cap + 8);
	RLEEngine e;
	int n = e.Line_Decompress(in, out);
	memcpy(dest, out, (unsigned)n < dest_cap ? (unsigned)n : dest_cap);
	free(in);
	free(out);
	return n;
}

}
