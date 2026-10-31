/* Stand-in for Common/FileSystem.h, differential tests only (differential/wrap_game_info.cpp): openFile, answered from the wrapper's table of files that exist (a file is closed at once). */
#ifndef SHIM_GINFO_FILESYSTEM_H
#define SHIM_GINFO_FILESYSTEM_H
#include "Common/AsciiString.h"
class File { public: void close(); };
class FileSystem { public: File *openFile(const char *path); };
extern FileSystem *TheFileSystem;
#endif
