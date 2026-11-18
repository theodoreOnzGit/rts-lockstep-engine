/* Differential-test wrapper for the language filter draft (#215, upstream commit 0a05454d), src/drafts/logic_common/language_filter.rs:
**   GeneralsMD/Code/GameEngine/Include/GameClient/LanguageFilter.h (read unchanged, `protected`/`private` read as `public`) and
**   GameEngine/Source/GameClient/LanguageFilter.cpp from `LanguageFilter *TheLanguageFilter = NULL;` to the end of the file
**   (the constructor and destructor, init, reset, update, filterLine, unHaxor, readWord, createLanguageFilter), cut verbatim by
**   build.rs into language_filter.inc.
**
** Stand-ins (all outside the cut): UnicodeString and AsciiString are upstream's own (UnicodeString.cpp unchanged, -fshort-wchar,
** shim_draft_text/text_prelude.h; classes renamed by build.rs); SubsystemInterface (the base class: virtual init, reset, update);
** File, with READ and BINARY, read(void*, Int) and close(), over a memory block, with RAMFile's read (the bytes that remain
** are copied and their count returned, 0 at the end); TheFileSystem->openFile(name, access), returning that File or NULL;
** NEW is `new`.
**
** Entry points (extern "C", tests/differential_draft_language_filter.rs):
**   ref_lf_unhaxor(in, n, out)                     unHaxor of the n units; returns the length written to out
**   ref_lf_read_word(data, len, pos, out, newpos)  one readWord over the bytes from byte offset pos; out is 128 units;
**                                                  returns the Bool, writes the new offset
**   ref_lf_init(data, len, has_file)               TheLanguageFilter->init() with the file (or none)
**   ref_lf_count()                                 m_wordList.size()
**   ref_lf_word(i, out)                            the i-th key in map order, units to out; returns its length
**   ref_lf_filter(in, n, out)                      filterLine of the n units; returns the length of the result */
/* renamed: other differential builds define classes of these names, with other bodies */
#define File LogicxFile
#define FileSystem LogicxFileSystem
#define TheFileSystem LogicxTheFileSystem
#define SubsystemInterface LogicxSubsystemInterface
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdlib.h>
#include <vector>

namespace {
DynamicMemoryAllocator theShimAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theShimAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init() {}
	virtual void reset() {}
	virtual void update() {}
};

class File
{
public:
	enum { READ = 1, BINARY = 2 };
	const unsigned char *data; int size; int pos;
	File(const unsigned char *d, int n) : data(d), size(n), pos(0) {}
	int read(void *buffer, int bytes)
	{
		if (pos + bytes > size) bytes = size - pos;
		if (bytes < 0) bytes = 0;
		memcpy(buffer, data + pos, bytes);
		pos += bytes;
		return bytes;
	}
	void close() {}
};
struct FileSystem
{
	File *file;
	File *openFile(const char *, int) { return file; }
};
FileSystem theFS;
FileSystem *TheFileSystem = &theFS;

#define private public
#define protected public
#include "GameClient/LanguageFilter.h"
#undef private
#undef protected
#define NEW new
#include "language_filter.inc"

namespace {
UnicodeString make(const unsigned short *u, int n)
{
	UnicodeString s;
	for (int i = 0; i < n; ++i) s.concat((WideChar)u[i]);
	return s;
}
int emit(const UnicodeString &s, unsigned short *out)
{
	int n = s.getLength();
	for (int i = 0; i < n; ++i) out[i] = (unsigned short)s.getCharAt(i);
	return n;
}
LanguageFilter *theFilter = 0;
File *theFile = 0;
unsigned char *theData = 0;
}

extern "C" {

int ref_lf_unhaxor(const unsigned short *in, int n, unsigned short *out)
{
	LanguageFilter f;
	UnicodeString w = make(in, n);
	f.unHaxor(w);
	return emit(w, out);
}

int ref_lf_read_word(const unsigned char *data, int len, int pos, unsigned short *out, int *newpos)
{
	LanguageFilter f;
	File file(data, len);
	file.pos = pos;
	int r = f.readWord(&file, out);
	*newpos = file.pos;
	return r;
}

void ref_lf_init(const unsigned char *data, int len, int has_file)
{
	delete theFilter;
	delete theFile;
	free(theData);
	theData = (unsigned char *)malloc(len + 1);
	memcpy(theData, data, len);
	theFile = has_file ? new File(theData, len) : 0;
	theFS.file = theFile;
	theFilter = new LanguageFilter;
	theFilter->init();
}

int ref_lf_count(void) { return (int)theFilter->m_wordList.size(); }

int ref_lf_word(int i, unsigned short *out)
{
	LangMapIter it = theFilter->m_wordList.begin();
	while (i-- > 0) ++it;
	return emit(it->first, out);
}

int ref_lf_filter(const unsigned short *in, int n, unsigned short *out)
{
	UnicodeString line = make(in, n);
	theFilter->filterLine(line);
	return emit(line, out);
}

}
