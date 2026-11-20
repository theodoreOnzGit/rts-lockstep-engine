/* (#215, group wwlib4) stand-in for WWLib/win.h as mixfile.cpp (Flush_Changes) uses it: GetFileAttributes, DeleteFile, MoveFile and
** _splitpath over the in-memory file system of rawfile.h, with the Win32 results (GetFileAttributes: 0xFFFFFFFF if absent; MoveFile fails
** if the destination exists or the source does not; _splitpath: drive "X:" if the second character is ':', the directory up to and
** including the last '\\' or '/'). */
#ifndef WWLIB4_WIN_H
#define WWLIB4_WIN_H
#include "rawfile.h"
#define _MAX_DRIVE 3
#define _MAX_DIR 256
#define _MAX_FNAME 256
#define _MAX_EXT 256
inline unsigned int GetFileAttributes(const char *n) { return mem_fs().count(n) ? 0x80u : 0xFFFFFFFFu; }
inline int DeleteFile(const char *n) { return mem_fs().erase(n) ? 1 : 0; }
inline int MoveFile(const char *a, const char *b)
{
	MemFs &fs = mem_fs();
	if (!fs.count(a) || fs.count(b)) return 0;
	fs[b] = fs[a];
	fs.erase(a);
	return 1;
}
inline void _splitpath(const char *path, char *drive, char *dir, char *fname, char *ext)
{
	std::string p = path;
	size_t i = 0;
	if (drive) drive[0] = 0;
	if (p.size() >= 2 && p[1] == ':') { if (drive) { drive[0] = p[0]; drive[1] = ':'; drive[2] = 0; } i = 2; }
	size_t last = p.find_last_of("\\/");
	std::string d, rest;
	if (last == std::string::npos || last < i) { rest = p.substr(i); }
	else { d = p.substr(i, last + 1 - i); rest = p.substr(last + 1); }
	if (dir) strcpy(dir, d.c_str());
	if (fname) strcpy(fname, rest.c_str());
	if (ext) ext[0] = 0;
}
#endif
