/* Differential-test wrapper for the draft src/drafts/wwlib/mixfile.rs (#215, group wwlib4), upstream commit 0a05454d:
** WWLib/mixfile.h and mixfile.cpp, copied by build.rs (drafts_wwlib4) with these edits, each asserted: (1) `long` / `unsigned long` spelled
** `int` / `unsigned int` (the Win32 data model: the file's header and tables are 4-byte fields, `long` is 8 bytes on this Linux target);
** (2) MSVC 6's `for` scope: two loops reuse `index` / `i` declared by an earlier `for` (now declared in their own `for`); (3) Add_Files and Setup_Mix_File (the Windows FindFirstFile tool that builds MAKEMIX.MIX, not part of the archive format) cut off the end.
** Force-included: shim_w3d/w3d_prelude.h (32-bit uint32, the CRT names).
** Stand-ins: RawFileClass (an in-memory file system, shim_draft_wwlib4/rawfile.h), FileFactoryClass / SimpleFileFactoryClass
** (ffactory.h), StringClass (wwstring.h), win.h (GetFileAttributes, DeleteFile, MoveFile, _splitpath over the same file system),
** WWASSERT and WWDEBUG_SAY compiled out. The test's factory (TestFactory) hands out RawFileClass objects of the requested name.
**
** ref_mx_run(dir, sizes, nsizes, names, nnames, prog, nwords, out, cap). All paths are `dir` + a name; the archive is dir+"pkg.mix". Source file
** `dir`+"s<i>" holds sizes[i] bytes b_k = (i*13 + k*7 + (k>>8)) & 0xFF (i = 7 does not exist). `names` are the NUL separated names.
** Ops (words): 0 k (src name form)*k   MixFileCreator over dir+"pkg.mix": form 0 Add_File(dir+"s<src>", names[name]), 1 Add_File(dir+"s<src>")
**                  (saved under the source's own path), 2 Add_File(names[name], FileClass * holding source src's bytes); destroyed at the end -> out -10
**              1   construct MixFileFactoryClass(dir+"pkg.mix", factory)                                   -> out: Is_Valid
**              2   for each name: Get_File -> out found, Size, then the bytes read after Seek(0) (up to 64)
**              3   Build_Filename_List, Build_Ordered_Filename_List, Build_Internal_Filename_List + Get_Filename_List -> out: ok, count, names
**              (Flush_Changes of an archive that was put in place as a raw image is skipped: it writes `Size()` bytes per sub-file from a buffer
**               that a short read leaves uninitialised.) (Make, Flush_Changes and writing an image destroy the factory: upstream keeps the old table and name offset, which no longer describe the file)
**              4 n Delete_File(names[n])    5 src n  Add_File(dir+"s<src>", names[n])    6 Flush_Changes (then out -12)
**              7 k create the files dir+"_tmpmix01.dat" .. k (the temporary names Flush_Changes tries)
**              8 len bytes...  write the archive file as the given bytes
** At the end: the archive's length (-1 if absent) and bytes. */
#define NDEBUG
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <string>
#include "always.h"
#include "mixfile.h"
#include "mixfile.cpp"

namespace {
class TestFactory : public FileFactoryClass {
public:
	virtual FileClass *Get_File(char const *filename) { return new RawFileClass(filename); }
	virtual void Return_File(FileClass *file) { delete file; }
};
std::vector<unsigned char> source_bytes(int i, int len)
{
	std::vector<unsigned char> b(len);
	for (int k = 0; k < len; k++) b[k] = (unsigned char)(i * 13 + k * 7 + (k >> 8));
	return b;
}
}

extern "C" int ref_mx_run(const char *dir, const int *sizes, int nsizes, const char *names, int nnames, const int *prog, int nwords, int *out, int cap)
{
	mem_fs().clear();
	std::string D = dir;
	for (int i = 0; i < nsizes; i++) {
		if (i == 7) continue;
		char nm[16]; sprintf(nm, "s%d", i);
		mem_fs()[D + nm] = source_bytes(i, sizes[i]);
	}
	std::vector<const char *> nm;
	const char *q = names;
	for (int i = 0; i < nnames; i++) { nm.push_back(q); q += strlen(q) + 1; }
	int n = 0;
	auto put = [&](int w) { if (n < cap) out[n] = w; n++; };
	std::string mix = D + "pkg.mix";
	TestFactory tf;
	MixFileFactoryClass *mf = 0;
	bool raw = false;   /* the archive was put in place as a raw image: Flush_Changes is skipped (see the header) */
	const int *p = prog, *end = prog + nwords;
	while (p < end) {
		int op = *p++;
		switch (op) {
		case 0: {
			int k = *p++;
			MixFileCreator *c = new MixFileCreator(mix.c_str());
			for (int j = 0; j < k; j++) {
				int src = *p++, name = *p++, form = *p++;
				char sn[16]; sprintf(sn, "s%d", src);
				std::string sp = D + sn;
				if (form == 0) c->Add_File(sp.c_str(), nm[name]);
				else if (form == 1) c->Add_File(sp.c_str());
				else {
					RawFileClass f(sp.c_str());
					f.Open();
					c->Add_File(nm[name], &f);
					f.Close();
				}
			}
			delete c;
			delete mf; mf = 0; raw = false;
			put(-10);
			break; }
		case 1:
			delete mf;
			mf = new MixFileFactoryClass(mix.c_str(), &tf);
			put(mf->Is_Valid() ? 1 : 0);
			break;
		case 2:
			if (!mf) { put(-6); break; }
			for (int j = 0; j < nnames; j++) {
				FileClass *f = mf->Get_File(nm[j]);
				if (!f) { put(0); continue; }
				put(1);
				put(f->Size());
				f->Open();
				f->Seek(0, SEEK_SET);
				std::vector<unsigned char> b(100000);
				int r = f->Read(b.data(), 100000);
				put(r);
				for (int k = 0; k < r && k < 64; k++) put(b[k]);
				mf->Return_File(f);
			}
			break;
		case 3: {
			if (!mf) { put(-6); break; }
			DynamicVectorClass<StringClass> l1, l2, l3;
			put(mf->Build_Filename_List(l1) ? 1 : 0);
			put(l1.Count());
			for (int j = 0; j < l1.Count(); j++) { const char *s = l1[j]; put((int)strlen(s)); for (const char *c = s; *c; c++) put((unsigned char)*c); }
			put(mf->Build_Ordered_Filename_List(l2) ? 1 : 0);
			put(l2.Count());
			for (int j = 0; j < l2.Count(); j++) { const char *s = l2[j]; put((int)strlen(s)); for (const char *c = s; *c; c++) put((unsigned char)*c); }
			put(mf->Build_Internal_Filename_List() ? 1 : 0);
			mf->Get_Filename_List(l3);
			put(l3.Count());
			for (int j = 0; j < l3.Count(); j++) { const char *s = l3[j]; put((int)strlen(s)); for (const char *c = s; *c; c++) put((unsigned char)*c); }
			break; }
		case 4: { int a = *p++; if (mf) mf->Delete_File(nm[a]); break; }
		case 5: { int src = *p++, a = *p++; char sn[16]; sprintf(sn, "s%d", src); std::string sp = D + sn; if (mf) mf->Add_File(sp.c_str(), nm[a]); break; }
		case 6: if (raw) { put(-13); break; } if (mf) mf->Flush_Changes(); delete mf; mf = 0; put(-12); break;
		case 7: { int k = *p++; for (int j = 1; j <= k; j++) { char t[32]; sprintf(t, "_tmpmix%.2d.dat", j); mem_fs()[D + t] = std::vector<unsigned char>(3, 7); } break; }
		case 8: { int len = *p++; std::vector<unsigned char> b(len); for (int j = 0; j < len; j++) b[j] = (unsigned char)*p++; mem_fs()[mix] = b; delete mf; mf = 0; raw = true; break; }
		}
	}
	delete mf;
	if (mem_fs().count(mix)) {
		std::vector<unsigned char> &img = mem_fs()[mix];
		put((int)img.size());
		for (size_t i = 0; i < img.size(); i++) put(img[i]);
	} else put(-1);
	return n;
}
