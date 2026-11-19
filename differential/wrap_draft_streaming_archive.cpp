/* Differential-test wrapper for the StreamingArchiveFile draft (#215, upstream commit 0a05454d), src/drafts/logic_common/streaming_archive_file.rs:
**   GeneralsMD/Code/GameEngine/Source/Common/System/StreamingArchiveFile.cpp from `StreamingArchiveFile::StreamingArchiveFile()` to the
**   end of the file (constructor, destructor, open(filename, access), open(File*), openFromArchive, close, read, write, seek), and
**   File.cpp from `File::File()` to `File::print` (the base class: constructor, destructor, open, close, size, position), each cut
**   verbatim by build.rs (streaming_archive.inc, file_base.inc).
**
** Stand-ins (outside the cuts; in an anonymous namespace): lc_file_standins.h (File's declaration, AsciiString, MemoryPoolObject, the
** DEBUG macros); StreamingArchiveFile's declaration (StreamingArchiveFile.h) on File directly, where upstream derives from RAMFile
** (RAMFile's own members and functions are not used by the cut); TheFileSystem, whose openFile finds no file (so open(filename, access)
** answers FALSE, which the draft does not have); and MemFile, the archive's File over a byte block: seek clamps to [0, size], read
** copies at most what remains (a NULL buffer only advances), both log their calls as words (RAMFile does the same clamping).
**
** Entry point (extern "C", tests/differential_draft_streaming_archive.rs): ref_saf_run(data, n, ops, nops, out, cap), op stream:
**   [0 openFromArchive has_archive name offset size] [1 close] [2 read isnull n] [3 write n] [4 seek pos mode] [5 destroy] [6 open(File*)];
**   after each op the state (m_open, m_file != NULL, m_startingPos, m_size, m_curPos, name length, name bytes). Archive calls log
**   [200 1 pos mode result] for seek and [200 2 bytes isnull result] for read. Returns the words, or -1 beyond cap. */
#include "Lib/BaseType.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <string>
#include <vector>

namespace {

std::vector<int> theLog;

#include "lc_file_standins.h"

#include "file_base.inc"

class StreamingArchiveFile : public File
{
public:
	File *m_file;
	Int m_startingPos;
	Int m_size;
	Int m_curPos;
	StreamingArchiveFile();
	virtual ~StreamingArchiveFile();
	virtual Bool open(const Char *filename, Int access = 0);
	virtual void close(void);
	virtual Int read(void *buffer, Int bytes);
	virtual Int write(const void *buffer, Int bytes);
	virtual Int seek(Int new_pos, seekMode mode = CURRENT);
	virtual void nextLine(Char *buf = NULL, Int bufSize = 0) {}
	virtual Bool scanInt(Int &newInt) { return FALSE; }
	virtual Bool scanReal(Real &newReal) { return FALSE; }
	virtual Bool scanString(AsciiString &newString) { return FALSE; }
	virtual Bool open(File *file);
	virtual Bool openFromArchive(File *archiveFile, const AsciiString &filename, Int offset, Int size);
	virtual char *readEntireAndClose() { return NULL; }
	virtual File *convertToRAMFile() { return this; }
};

struct FileSystem { File *openFile(const char *, int) { return 0; } };
FileSystem theFS;
FileSystem *TheFileSystem = &theFS;

#include "streaming_archive.inc"

class MemFile : public File
{
public:
	std::vector<unsigned char> data;
	int pos;
	MemFile() : pos(0) {}
	virtual Int read(void *buffer, Int bytes)
	{
		int k = (int)data.size() - pos;
		if (k > bytes) k = bytes;
		if (k < 0) k = 0;
		if (buffer && k > 0) memcpy(buffer, &data[pos], k);
		pos += k;
		theLog.push_back(200); theLog.push_back(2); theLog.push_back(bytes); theLog.push_back(buffer == 0); theLog.push_back(k);
		return k;
	}
	virtual Int write(const void *, Int) { return -1; }
	virtual Int seek(Int bytes, seekMode mode)
	{
		int np = mode == START ? bytes : mode == CURRENT ? pos + bytes : (int)data.size() + bytes;
		if (np < 0) np = 0;
		if (np > (int)data.size()) np = (int)data.size();
		pos = np;
		theLog.push_back(200); theLog.push_back(1); theLog.push_back(bytes); theLog.push_back(mode); theLog.push_back(pos);
		return pos;
	}
	virtual void nextLine(Char *, Int) {}
	virtual Bool scanInt(Int &) { return FALSE; }
	virtual Bool scanReal(Real &) { return FALSE; }
	virtual Bool scanString(AsciiString &) { return FALSE; }
	virtual char *readEntireAndClose() { return NULL; }
	virtual File *convertToRAMFile() { return this; }
};

void putState(const StreamingArchiveFile *f)
{
	theLog.push_back(f->m_open); theLog.push_back(f->m_file != 0); theLog.push_back(f->m_startingPos); theLog.push_back(f->m_size); theLog.push_back(f->m_curPos);
	int n = (int)strlen(f->m_nameStr.str());
	theLog.push_back(n);
	for (int i = 0; i < n; ++i) theLog.push_back((unsigned char)f->m_nameStr.str()[i]);
}

} // namespace

extern "C" {

int ref_saf_run(const unsigned char *data, int n, const int *ops, int nops, int *out, int cap)
{
	theLog.clear();
	MemFile archive;
	archive.data.assign(data, data + n);
	StreamingArchiveFile *f = new StreamingArchiveFile;
	putState(f);
	int p = 0;
	while (p < nops)
	{
		int op = ops[p++];
		theLog.push_back(1000 + op);
		switch (op)
		{
		case 0: {
			int has = ops[p], name = ops[p + 1], offset = ops[p + 2], size = ops[p + 3]; p += 4;
			AsciiString nm;
			nm.set(name == 0 ? "a.big" : name == 1 ? "b" : name == 2 ? "" : "dir/c.dat");
			theLog.push_back(f->openFromArchive(has ? &archive : 0, nm, offset, size));
			break; }
		case 1: f->close(); break;
		case 2: {
			int isnull = ops[p], k = ops[p + 1]; p += 2;
			unsigned char buf[64];
			memset(buf, 0xAA, sizeof(buf));
			int r = f->read(isnull ? 0 : buf, k);
			theLog.push_back(r);
			if (!isnull) for (int i = 0; i < r && i < k; ++i) theLog.push_back(buf[i]);
			break; }
		case 3: { int k = ops[p++]; unsigned char b[4] = {1, 2, 3, 4}; theLog.push_back(f->write(b, k)); break; }
		case 4: { int pos = ops[p], mode = ops[p + 1]; p += 2; theLog.push_back(f->seek(pos, (File::seekMode)mode)); break; }
		case 5: delete f; f = new StreamingArchiveFile; break;
		default: theLog.push_back(f->open((File *)&archive)); break;
		}
		putState(f);
	}
	delete f;
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}

}
