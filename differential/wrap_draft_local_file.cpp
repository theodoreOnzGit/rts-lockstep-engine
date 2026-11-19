/* Differential-test wrapper for the LocalFile draft (#215, upstream commit 0a05454d), src/drafts/logic_common/local_file.rs:
**   GeneralsMD/Code/GameEngine/Source/Common/System/File.cpp, from `File::File()` to `File::position` (the constructor, destructor,
**   open, close and size: the base class the draft reduced to three fields), cut verbatim by build.rs into file_base.inc;
**   GeneralsMD/Code/GameEngine/Source/Common/System/LocalFile.cpp, from `LocalFile::LocalFile()` to `File* LocalFile::convertToRAMFile()`
**   (the constructor, destructor, open, close, read, write, seek, scanInt, scanReal, scanString, nextLine) and from
**   `char* LocalFile::readEntireAndClose()` to the end, cut verbatim into local_file_a.inc and local_file_b.inc. The shipped build:
**   USE_BUFFERED_IO is not defined (LocalFile.h: NO_USE_BUFFERED_IO), no _DEBUG. convertToRAMFile is not cut (RAMFile is not drafted).
**
** Stand-ins (everything outside the cuts; all in an anonymous namespace, since other wrappers define classes of these names):
**  - the class declarations of File (file.h) and LocalFile (LocalFile.h), copied member for member with the members public and
**    MEMORY_POOL_GLUE_ABC empty; MemoryPoolObject with deleteInstance() doing nothing (File::close calls it only for a file marked
**    deleteOnClose, which the test never marks);
**  - AsciiString on std::string, with concat(char) as AsciiString.h line 536 has it (a 2-char buffer through concat(const char*), so that a
**    NUL character appends nothing), str(), clear(), set();
**  - DEBUG_ASSERTCRASH / DEBUG_CRASH, empty (compiled out of the release build);
**  - the C runtime's _open, _read, _write, _lseek, _close over four in-memory files "f0".."f3" (f3 does not exist until created) and 8
**    handles (numbered from 3). _open: a missing file is created only with _O_CREAT, else -1; _O_TRUNC empties a writable file; no free
**    handle gives -1. _read: 0 on a bad or write-only handle (not MSVC's -1: the scan loops of the cut never end on -1 with an unset
**    char), no CRLF translation in text mode. _write: -1 on a bad or read-only handle; _O_APPEND writes at the end; a write past the end
**    zero-fills. _lseek: the new position, -1 for a bad handle or a negative result. Every call is logged as words.
**
** Entry point (extern "C", tests/differential_draft_local_file.rs): ref_lf_local_run(f0, n0, f1, n1, f2, n2, ops, nops, out, cap) runs the
** op stream (opcode then operands; see the test) on one LocalFile and writes the results; returns the count of words. */
#include "Lib/BaseType.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <ctype.h>
#include <string>
#include <vector>

namespace {

std::vector<int> theLog;

#include "lc_file_standins.h"

/* ---- the C runtime stand-ins ---- */
enum { lc_O_RDONLY = 0, lc_O_WRONLY = 1, lc_O_RDWR = 2, lc_O_APPEND = 8, lc_O_CREAT = 0x100, lc_O_TRUNC = 0x200, lc_O_TEXT = 0x4000, lc_O_BINARY = 0x8000 };
#define _O_RDONLY lc_O_RDONLY
#define _O_WRONLY lc_O_WRONLY
#define _O_RDWR lc_O_RDWR
#define _O_APPEND lc_O_APPEND
#define _O_CREAT lc_O_CREAT
#define _O_TRUNC lc_O_TRUNC
#define _O_TEXT lc_O_TEXT
#define _O_BINARY lc_O_BINARY
#define _S_IREAD 0x100
#define _S_IWRITE 0x80
#define _open lc_open
#define _read lc_read
#define _write lc_write
#define _lseek lc_lseek
#define _close lc_close

struct DevFile { std::string data; bool exists; };
struct Slot { bool used; int file; int pos; int flags; };
DevFile devFiles[4];
Slot slots[8];

bool readable(int fl) { return (fl & 3) != lc_O_WRONLY; }
bool writable(int fl) { return (fl & 3) != lc_O_RDONLY; }
Slot *slot(int h) { return (h >= 3 && h < 11 && slots[h - 3].used) ? &slots[h - 3] : 0; }

int lc_open(const char *name, int flags, int mode)
{
	(void)mode;
	int r = -1;
	int f = (name[0] == 'f' && name[1] >= '0' && name[1] <= '3' && name[2] == 0) ? name[1] - '0' : -1;
	if (f >= 0)
	{
		bool ok = true;
		if (!devFiles[f].exists)
		{
			if (flags & lc_O_CREAT) { devFiles[f].exists = true; devFiles[f].data.clear(); }
			else ok = false;
		}
		else if ((flags & lc_O_TRUNC) && writable(flags)) devFiles[f].data.clear();
		if (ok)
		{
			for (int i = 0; i < 8 && r < 0; ++i)
				if (!slots[i].used) { slots[i].used = true; slots[i].file = f; slots[i].pos = 0; slots[i].flags = flags; r = i + 3; }
		}
	}
	theLog.push_back(1); theLog.push_back(f); theLog.push_back(flags); theLog.push_back(r);
	return r;
}
int lc_read(int h, void *buf, int n)
{
	Slot *s = slot(h);
	int r = 0;
	if (s && readable(s->flags))
	{
		int size = (int)devFiles[s->file].data.size();
		int k = size - s->pos;
		if (k > n) k = n;
		if (k < 0) k = 0;
		if (k > 0) memcpy(buf, devFiles[s->file].data.data() + s->pos, k);
		s->pos += k;
		r = k;
	}
	theLog.push_back(2); theLog.push_back(h); theLog.push_back(n); theLog.push_back(r);
	return r;
}
int lc_write(int h, const void *buf, int n)
{
	Slot *s = slot(h);
	int r = -1;
	if (s && writable(s->flags))
	{
		std::string &d = devFiles[s->file].data;
		if (s->flags & lc_O_APPEND) s->pos = (int)d.size();
		if (s->pos + n > (int)d.size()) d.resize(s->pos + n, '\0');
		memcpy(&d[s->pos], buf, n);
		s->pos += n;
		r = n;
	}
	theLog.push_back(3); theLog.push_back(h); theLog.push_back(n); theLog.push_back(r);
	return r;
}
int lc_lseek(int h, int off, int mode)
{
	Slot *s = slot(h);
	int r = -1;
	if (s)
	{
		int base = mode == SEEK_SET ? 0 : mode == SEEK_CUR ? s->pos : (int)devFiles[s->file].data.size();
		long long np = (long long)base + off;
		if (np >= 0) { s->pos = (int)np; r = s->pos; }
	}
	theLog.push_back(4); theLog.push_back(h); theLog.push_back(off); theLog.push_back(mode); theLog.push_back(r);
	return r;
}
int lc_close(int h)
{
	Slot *s = slot(h);
	if (s) s->used = false;
	theLog.push_back(5); theLog.push_back(h);
	return 0;
}

/* ---- LocalFile.h's class LocalFile (the unbuffered branch) ---- */
Int s_totalOpen = 0;
class LocalFile : public File
{
	MEMORY_POOL_GLUE_ABC(LocalFile)
public:
	int m_handle;
	LocalFile();
	virtual ~LocalFile();
	virtual Bool open(const Char *filename, Int access = 0);
	virtual void close(void);
	virtual Int read(void *buffer, Int bytes);
	virtual Int write(const void *buffer, Int bytes);
	virtual Int seek(Int new_pos, seekMode mode = CURRENT);
	virtual void nextLine(Char *buf = NULL, Int bufSize = 0);
	virtual Bool scanInt(Int &newInt);
	virtual Bool scanReal(Real &newReal);
	virtual Bool scanString(AsciiString &newString);
	virtual char *readEntireAndClose();
	virtual File *convertToRAMFile();
};

#include "file_base.inc"
/* the file's own static is declared above (the cut starts at the constructor) */
#include "local_file_a.inc"
File *LocalFile::convertToRAMFile() { return this; } /* not cut: RAMFile is not drafted; never called */
#include "local_file_b.inc"

void putState(const LocalFile *f)
{
	theLog.push_back(f->m_access); theLog.push_back(f->m_open); theLog.push_back(f->m_handle); theLog.push_back(s_totalOpen);
}
void putBytes(const unsigned char *b, int n)
{
	for (int i = 0; i < n; ++i) theLog.push_back(b[i]);
}
unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }

} // namespace

extern "C" {

/* ops: [0 open, name, access] [1 close] [2 destroy] [3 read, null, n] [4 write, null, n, bytes...] [5 seek, pos, mode]
**      [6 scanInt] [7 scanReal] [8 scanString] [9 nextLine, null, bufSize] [10 readEntireAndClose]; each result is followed by the state
**      (m_access, m_open, m_handle, s_totalOpen); the device's calls are logged inline (codes 1 to 5) */
int ref_lf_local_run(const unsigned char *f0, int n0, const unsigned char *f1, int n1, const unsigned char *f2, int n2, const int *ops, int nops, int *out, int cap)
{
	theLog.clear();
	s_totalOpen = 0;
	for (int i = 0; i < 8; ++i) slots[i].used = false;
	devFiles[0].data.assign((const char *)f0, n0); devFiles[0].exists = true;
	devFiles[1].data.assign((const char *)f1, n1); devFiles[1].exists = true;
	devFiles[2].data.assign((const char *)f2, n2); devFiles[2].exists = true;
	devFiles[3].data.clear(); devFiles[3].exists = false;
	LocalFile *lf = new LocalFile;
	int p = 0;
	while (p < nops)
	{
		int op = ops[p++];
		theLog.push_back(1000 + op);
		switch (op)
		{
		case 0: {
			char name[3] = { 'f', (char)('0' + ops[p]), 0 };
			int access = ops[p + 1]; p += 2;
			theLog.push_back(lf->open(name, access));
			break; }
		case 1: lf->close(); break;
		case 2: delete lf; lf = new LocalFile; break;
		case 3: {
			int isnull = ops[p], n = ops[p + 1]; p += 2;
			unsigned char buf[64];
			memset(buf, 0xAA, sizeof(buf));
			int r = lf->read(isnull ? 0 : buf, n);
			theLog.push_back(r);
			if (!isnull && r > 0) putBytes(buf, r < n ? r : n);
			break; }
		case 4: {
			int isnull = ops[p], n = ops[p + 1]; p += 2;
			unsigned char buf[64];
			for (int i = 0; i < n; ++i) buf[i] = (unsigned char)ops[p + i];
			p += n;
			theLog.push_back(lf->write(isnull ? 0 : buf, n));
			break; }
		case 5: {
			int pos = ops[p], mode = ops[p + 1]; p += 2;
			theLog.push_back(lf->seek(pos, (File::seekMode)mode));
			break; }
		case 6: { int v; Bool ok = lf->scanInt(v); theLog.push_back(ok); theLog.push_back(v); break; }
		case 7: { Real v; Bool ok = lf->scanReal(v); theLog.push_back(ok); theLog.push_back((int)bitsOf(v)); break; }
		case 8: {
			AsciiString s;
			Bool ok = lf->scanString(s);
			theLog.push_back(ok);
			int n = (int)strlen(s.str());
			theLog.push_back(n);
			putBytes((const unsigned char *)s.str(), n);
			break; }
		case 9: {
			int isnull = ops[p], bufSize = ops[p + 1]; p += 2;
			unsigned char buf[64];
			memset(buf, 0x55, sizeof(buf));
			lf->nextLine(isnull ? 0 : (Char *)buf, bufSize);
			if (!isnull) putBytes(buf, bufSize + 1);
			break; }
		case 10: {
			/* the bytes the read will return: size minus position, from the device, before the call */
			Slot *s = slot(lf->m_handle);
			/* nothing is read from a closed file (read returns -1) or a write-only handle */
			int k = (s && lf->m_open && readable(s->flags)) ? (int)devFiles[s->file].data.size() - s->pos : 0;
			if (k < 0) k = 0;
			char *b = lf->readEntireAndClose();
			theLog.push_back(k);
			putBytes((const unsigned char *)b, k);
			delete[] b;
			break; }
		}
		putState(lf);
	}
	/* the final contents of the four files */
	for (int i = 0; i < 4; ++i)
	{
		theLog.push_back(devFiles[i].exists);
		theLog.push_back((int)devFiles[i].data.size());
		putBytes((const unsigned char *)devFiles[i].data.data(), (int)devFiles[i].data.size());
	}
	delete lf;
	theLog.push_back(s_totalOpen);
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}

}
