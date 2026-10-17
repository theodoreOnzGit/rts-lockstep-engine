/* Forced include (-include) for the .big archive sources, differential tests
** only. Win32BIGFileSystem.cpp and its headers rely on things the real
** PreRTS.h / windows.h provide: the MSVC string functions stricmp/_stricmp,
** _MAX_PATH (260 on Win32), the DEBUG_* macros, and TheGlobalData (for
** ArchiveFileSystem::loadMods and DataChunkOutput's temporary file, not
** exercised). */
#ifndef SHIM_ARCHIVE_PRELUDE_H
#define SHIM_ARCHIVE_PRELUDE_H
#include <strings.h>
#include "PreRTS.h"
#include "Common/AsciiString.h"
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _MAX_PATH 260
#include <string>
#include <map>
#include <set>
#include <vector>
#include <list>
class GlobalData
{
public:
	AsciiString m_modBIG;
	AsciiString m_modDir;
	AsciiString getPath_UserData() const;
};
extern GlobalData *TheGlobalData;
#endif
