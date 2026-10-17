/* Differential-test wrapper for the .big archive reader (upstream commit
** 0a05454d), all compiled unchanged as their own translation units:
**   GameEngine/Source/Common/System/ArchiveFile.cpp
**   GameEngine/Source/Common/System/ArchiveFileSystem.cpp
**   GameEngine/Source/Common/System/File.cpp
**   GameEngine/Source/Common/System/RAMFile.cpp
**   GameEngine/Source/Common/System/StreamingArchiveFile.cpp
**   GameEngineDevice/Source/Win32Device/Common/Win32BIGFile.cpp
**   GameEngineDevice/Source/Win32Device/Common/Win32BIGFileSystem.cpp
** with differential/shim/archive_prelude.h force-included and the shims in
** differential/shim standing in for the engine headers they need.
**
** The engine reads archives through TheLocalFileSystem. Here that is a
** stand-in, ShimLocalFileSystem, holding named byte buffers; the File it
** hands out (MemFile) reads and seeks with the semantics of upstream's
** unbuffered LocalFile (_read: returns the bytes that exist; _lseek: allowed
** past the end, -1 before the start). Archives are loaded one by one through
** the real Win32BIGFileSystem::loadBigFilesFromDirectory, whose file listing
** the stand-in answers with exactly the archive named by the mask. */
#include <stdlib.h>
#include <string.h>
#include <string>
#include <map>
#include <vector>
#include "Common/File.h"
#include "Common/LocalFileSystem.h"
#include "Common/FileSystem.h"
#include "Common/ArchiveFile.h"
#include "Common/ArchiveFileSystem.h"
#include "Common/GameAudio.h"
#include "Common/registry.h"
#include "Win32Device/Common/Win32BIGFileSystem.h"

const AsciiString AsciiString::TheEmptyString;

/* ---- stand-ins for engine singletons that the compiled code references
** but the tests never reach (they abort if reached). FileSystem::openFile
** is reached by INI::load in the INI tests: it forwards to the hook that
** differential/wrap_ini.cpp installs, and aborts without one. */
GlobalData *TheGlobalData = 0;
AudioManager *TheAudio = 0;
FileSystem *TheFileSystem = 0;
File *(*shim_file_system_open_hook)(const Char *, Int) = 0;
File *FileSystem::openFile(const Char *name, Int access)
{
	if (shim_file_system_open_hook) return shim_file_system_open_hook(name, access);
	abort();
}
Bool GetStringFromGeneralsRegistry(AsciiString, AsciiString, AsciiString &) { return FALSE; }

namespace {

class MemFile : public File
{
public:
	MemFile(const std::vector<char> &data) : m_bytes(data), m_data(&m_bytes), m_pos(0) {}
	virtual ~MemFile() {}
	virtual Int read(void *buffer, Int bytes)
	{
		if (!m_open) return -1;
		Int size = (Int)m_data->size();
		if (buffer == NULL) { m_pos += bytes; return bytes; }
		Int avail = m_pos < size ? size - m_pos : 0;
		Int n = bytes < avail ? bytes : avail;
		if (n < 0) n = 0;
		if (n > 0) memcpy(buffer, &(*m_data)[m_pos], n);
		m_pos += n;
		return n;
	}
	virtual Int write(const void *, Int) { return -1; }
	virtual Int seek(Int pos, seekMode mode = CURRENT)
	{
		long long np;
		switch (mode) {
		case START: np = pos; break;
		case CURRENT: np = (long long)m_pos + pos; break;
		case END: np = (long long)m_data->size() + pos; break;
		default: return -1;
		}
		if (np < 0 || np > 0x7fffffffLL) return -1;
		m_pos = (Int)np;
		return m_pos;
	}
	virtual void nextLine(Char *, Int) { abort(); }
	virtual Bool scanInt(Int &) { abort(); }
	virtual Bool scanReal(Real &) { abort(); }
	virtual Bool scanString(AsciiString &) { abort(); }
	virtual char *readEntireAndClose() { abort(); }
	virtual File *convertToRAMFile() { abort(); }
private:
	std::vector<char> m_bytes; /* own copy: like an open handle, unaffected if the name is re-registered */
	const std::vector<char> *m_data;
	Int m_pos;
};

class ShimLocalFileSystem : public LocalFileSystem
{
public:
	std::map<std::string, std::vector<char> > m_files;
	virtual void init() {}
	virtual void reset() {}
	virtual void update() {}
	virtual File *openFile(const Char *filename, Int access = 0)
	{
		std::map<std::string, std::vector<char> >::iterator it = m_files.find(filename);
		if (it == m_files.end()) return NULL;
		MemFile *f = new MemFile(it->second);
		if (!f->open(filename, access)) { delete f; return NULL; }
		return f;
	}
	virtual Bool doesFileExist(const Char *filename) const { return m_files.count(filename) != 0; }
	/* answers loadBigFilesFromDirectory("", mask): exactly the file named mask */
	virtual void getFileListInDirectory(const AsciiString &, const AsciiString &, const AsciiString &searchName,
		FilenameList &filenameList, Bool) const
	{
		if (m_files.count(searchName.str())) filenameList.insert(searchName);
	}
	virtual Bool getFileInfo(const AsciiString &, FileInfo *fileInfo) const
	{
		fileInfo->sizeHigh = fileInfo->sizeLow = fileInfo->timestampHigh = fileInfo->timestampLow = 0;
		return TRUE;
	}
	virtual Bool createDirectory(AsciiString) { return FALSE; }
};

ShimLocalFileSystem *theLocal = 0;
Win32BIGFileSystem *theBig = 0;

static int putList(const FilenameList &list, char *out, int cap)
{
	int n = 0;
	for (FilenameList::const_iterator it = list.begin(); it != list.end(); ++it) {
		int len = it->getLength();
		for (int k = 0; k <= len; ++k) { if (n < cap) out[n] = it->str()[k]; ++n; }
	}
	return n;
}

} /* namespace */

LocalFileSystem *TheLocalFileSystem = 0;

extern "C" {

void ref_big_reset(void)
{
	delete theBig;
	theBig = 0;
	delete theLocal;
	theLocal = new ShimLocalFileSystem;
	TheLocalFileSystem = theLocal;
	theBig = new Win32BIGFileSystem;
	TheArchiveFileSystem = theBig;
}

void ref_big_add_local(const char *name, const char *data, int len)
{
	theLocal->m_files[name] = std::vector<char>(data, data + len);
}

/* Win32BIGFileSystem::loadBigFilesFromDirectory("", name, overwrite) */
int ref_big_load(const char *name, int overwrite)
{
	return theBig->loadBigFilesFromDirectory(AsciiString(""), AsciiString(name), overwrite != 0) ? 1 : 0;
}

int ref_big_exists(const char *name) { return theBig->doesFileExist(name) ? 1 : 0; }

/* getArchiveFilenameForFile, NUL-terminated into out; returns its length */
int ref_big_archive_for(const char *name, char *out, int cap)
{
	AsciiString a = theBig->getArchiveFilenameForFile(AsciiString(name));
	int len = a.getLength();
	if (len + 1 <= cap) memcpy(out, a.str(), len + 1);
	return len;
}

/* ArchiveFileSystem::openFile(name) for reading, as a RAMFile (streaming 0)
** or a StreamingArchiveFile (streaming 1). Returns -1 if NULL, -3 if it
** threw; else the number of bytes a read of min(size(), cap) bytes into out
** returned, with size() in *size_out. */
int ref_big_open(const char *name, char *out, int cap, int streaming, int *size_out)
{
	File *f;
	try { f = theBig->openFile(name, streaming ? File::STREAMING : 0); } catch (...) { return -3; }
	if (f == NULL) return -1;
	Int size = f->size();
	*size_out = size;
	Int want = size < cap ? size : cap;
	Int got = want > 0 ? f->read(out, want) : 0;
	f->close(); /* deleteOnClose: deletes the file object */
	return got;
}

/* ArchiveFileSystem::getFileInfo(name); returns -1 if FALSE, else sizeLow */
long long ref_big_file_info(const char *name)
{
	FileInfo fi;
	if (!theBig->getFileInfo(AsciiString(name), &fi)) return -1;
	return (unsigned int)fi.sizeLow;
}

/* ArchiveFileSystem::getFileListInDirectory(current, original, search, list,
** TRUE): the names, each NUL-terminated, in list order; returns total bytes. */
int ref_big_list(const char *current, const char *original, const char *search, char *out, int cap)
{
	FilenameList list;
	theBig->getFileListInDirectory(AsciiString(current), AsciiString(original), AsciiString(search), list, TRUE);
	return putList(list, out, cap);
}

} /* extern "C" */
