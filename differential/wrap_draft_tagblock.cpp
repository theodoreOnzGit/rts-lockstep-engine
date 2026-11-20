/* Differential-test wrapper for the draft src/drafts/wwlib/tagblock.rs (#215, group wwlib4), upstream commit 0a05454d:
** WWLib/TagBlock.h and tagblock.cpp, compiled UNCHANGED (build.rs copies them, with the lower-case names of the headers they include,
** into OUT_DIR/draft_wwlib4_gen). Force-included: shim_w3d/w3d_prelude.h (32-bit uint32 etc.).
** Stand-ins: RawFileClass (shim_draft_wwlib4/rawfile.h: an in-memory file system with the Win32 open/seek/read/write semantics, see
** its header), and assert is compiled out (NDEBUG, the release build); strcmpi is strcasecmp and _lrotl (crc.h, never called) a 32-bit rotate. TagBlockFile's own virtual Get_Date_Time is not exercised
** (the file's time stamp is the OS's).
**
** Entry point ref_tb_run(image, ilen, names, nnames, prog, nwords, out, cap): the file "tb.dat" starts as `image` (ilen bytes, ilen<0:
** absent); `names` is nnames NUL-terminated names back to back; `prog` a stream of words:
**   0 n          Does_Tag_Exist(name n)                      -> out: 0/1
**   1 n s        Open_Tag(name n) into slot s (if free)      -> out: 1 + data size, or 0; -5 if the slot is taken
**   2 n s        Create_Tag(name n) into slot s (if free)    -> out: 1, or 0; -5
**   3 s          Close_Tag(slot s) (if taken)                -> out: -7 / -6 (slot empty)
**   4 s n seed   Write(n bytes b_i = seed*31+i*7) on slot    -> out: result, Tell, Get_Data_Size
**   5 s n        Read(n) on slot                              -> out: result, Tell, then the bytes read
**   6 s pos dir  Seek(pos, dir) (0 CUR, 1 SET, 2 END); a negative result is put back to 0 with Seek(0, SEEK_SET) -> out: result
**   7 s          End_Write_Access                             -> out: result
**   8            Reset_File (only if no slot is taken)        -> out: -8
**   9            destroy the file object and open it again (only if no slot is taken) -> out: -9
** At the end the remaining handles are closed in slot order, and out gets the final image length and bytes. */
#define NDEBUG
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <string>
#include <strings.h>
#define strcmpi strcasecmp      /* the MSVC CRT name (case-insensitive compare; C locale) */
static inline unsigned long _lrotl(unsigned long v, int n) { unsigned int x = (unsigned int)v; return (x << n) | (x >> (32 - n)); } /* crc.h, never called here */
#include "always.h"
#include "tagblock.h"
#include "tagblock.cpp"

extern "C" int ref_tb_run(const unsigned char *image, int ilen, const char *names, int nnames, const int *prog, int nwords, int *out, int cap)
{
	mem_fs().clear();
	if (ilen >= 0) mem_fs()["tb.dat"] = std::vector<unsigned char>(image, image + ilen);
	std::vector<const char *> nm;
	const char *q = names;
	for (int i = 0; i < nnames; i++) { nm.push_back(q); q += strlen(q) + 1; }
	int n = 0;
	auto put = [&](int w) { if (n < cap) out[n] = w; n++; };
	TagBlockFile *f = new TagBlockFile("tb.dat");
	TagBlockHandle *h[4] = {0, 0, 0, 0};
	auto any = [&]() { for (int i = 0; i < 4; i++) if (h[i]) return true; return false; };
	const int *p = prog;
	const int *end = prog + nwords;
	while (p < end) {
		int op = *p++;
		switch (op) {
		case 0: put(f->Does_Tag_Exist(nm[*p++]) ? 1 : 0); break;
		case 1: { int a = *p++, s = *p++; if (h[s]) { put(-5); break; } h[s] = f->Open_Tag(nm[a]); put(h[s] ? 1 + h[s]->Get_Data_Size() : 0); break; }
		case 2: { int a = *p++, s = *p++; if (h[s]) { put(-5); break; } h[s] = f->Create_Tag(nm[a]); put(h[s] ? 1 : 0); break; }
		case 3: { int s = *p++; if (h[s]) { f->Close_Tag(h[s]); h[s] = 0; put(-7); } else put(-6); break; }
		case 4: { int s = *p++, len = *p++, seed = *p++; if (!h[s]) { put(-6); break; }
			std::vector<unsigned char> b(len > 0 ? len : 0); for (int i = 0; i < len; i++) b[i] = (unsigned char)(seed * 31 + i * 7);
			int r = h[s]->Write(b.data(), len); put(r); put(h[s]->Tell()); put(h[s]->Get_Data_Size()); break; }
		case 5: { int s = *p++, len = *p++; if (!h[s]) { put(-6); break; }
			std::vector<unsigned char> b(len > 0 ? len : 0);
			int r = h[s]->Read(b.data(), len); put(r); put(h[s]->Tell()); for (int i = 0; i < r; i++) put(b[i]); break; }
		case 6: { int s = *p++, pos = *p++, d = *p++; if (!h[s]) { put(-6); break; }
			int r = h[s]->Seek(pos, d == 0 ? SEEK_CUR : (d == 1 ? SEEK_SET : SEEK_END));
			if (r < 0) h[s]->Seek(0, SEEK_SET);
			put(r); break; }
		case 7: { int s = *p++; if (!h[s]) { put(-6); break; } put(h[s]->End_Write_Access()); break; }
		case 8: if (!any()) f->Reset_File(); put(-8); break;
		case 9: if (!any()) { delete f; f = new TagBlockFile("tb.dat"); } put(-9); break;
		}
	}
	for (int i = 0; i < 4; i++) if (h[i]) { f->Close_Tag(h[i]); h[i] = 0; }
	delete f;
	std::vector<unsigned char> &img = mem_fs()["tb.dat"];
	put((int)img.size());
	for (size_t i = 0; i < img.size(); i++) put(img[i]);
	return n;
}
