/* Forced include (-include) for GlobalData.cpp, INIGameData.cpp, the
** WeaponBonusSet functions and wrap_globaldata.cpp, differential tests only.
** It is the INI build's prelude (shim_ini/ini_prelude.h) without the .big
** prelude it includes there, whose stand-in `class GlobalData` (three
** members, for the archive code) would collide with the real one: only the
** MSVC string names and _MAX_PATH of that prelude are repeated. Then the
** Windows calls the constructor makes, as stand-ins that find nothing (the
** executable, My Documents, the registry) and answer 500 ms for the system's
** double-click time.
**
** The wrapper (which defines GLOBALDATA_WRAPPER) reads GlobalData's private
** members: for it every `private` and `protected` of the headers below
** means `public`, as in differential/wrap_armor.cpp (the layout does not
** depend on access); the standard headers come first, unaffected. */
#ifndef SHIM_GLOBALDATA_PRELUDE_H
#define SHIM_GLOBALDATA_PRELUDE_H
#include <strings.h>
#include <stdlib.h>
#include <stddef.h>
#include <stdio.h>
#include <ctype.h>
#include <string.h>
#include <time.h>
#include <new>
#include <string>
#include <map>
#include <set>
#include <vector>
#include <list>
#include <bitset>
#include <stdexcept>
#include <typeinfo>
#include <cxxabi.h>
#include <type_traits>
#ifdef GLOBALDATA_WRAPPER
#define private public
#define protected public
#endif
#include "PreRTS.h"
#include "Common/AsciiString.h"
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _MAX_PATH 260
#define _isnan isnan

/* GetModuleFileName(NULL, buffer, size): no executable file name */
inline unsigned GetModuleFileName(void *, char *buffer, unsigned) { buffer[0] = 0; return 0; }
/* ::SHGetSpecialFolderPath(NULL, path, CSIDL_PERSONAL, true): no My Documents */
#define CSIDL_PERSONAL 5
inline bool SHGetSpecialFolderPath(void *, char *, int, bool) { return false; }
inline bool CreateDirectory(const char *, void *) { return false; }
/* GetDoubleClickTime(): Windows' default */
inline unsigned GetDoubleClickTime() { return 500; }
#endif
