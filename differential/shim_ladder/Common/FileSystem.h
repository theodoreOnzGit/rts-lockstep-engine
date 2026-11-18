/* Stand-in for Common/FileSystem.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): `getFileListInDirectory`
** (the wrapper lists the files it was given, in its order: upstream's FilenameList is a std::set ordered without case, and the
** order is the caller's here) and `openFile`. */
#ifndef SHIM_LADDER_FILESYSTEM_H
#define SHIM_LADDER_FILESYSTEM_H
#include <list>
#include "Common/AsciiString.h"
#include "Common/File.h"
typedef std::list<AsciiString> FilenameList;
class FileSystem
{
public:
	void getFileListInDirectory(const AsciiString &directory, const AsciiString &filter, FilenameList &filenameList, Bool searchSubdirectories) const;
	File *openFile(const Char *filename, Int access = 0);
};
extern FileSystem *TheFileSystem;
#endif
