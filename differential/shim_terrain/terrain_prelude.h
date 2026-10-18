/* Forced include (-include) for the Terrain, Multiplayer and Crate build,
** differential tests only: what the real PreRTS.h brings to every GameEngine
** .cpp, without the shared shim's PreRTS.h (which includes the stand-in
** BaseType.h by a relative path, and so would bring the stand-in in ahead of
** the real one this build needs for RGBColor).
** 1. <bitset>, the C library headers INI.h's code relies on, and the error
**    codes and ScienceType of Errors.h and Science.h (see shim_ini's prelude);
** 2. the REAL Lib/BaseType.h (through the forwarder beside this file);
** 3. the BitFlags build's bit-name machinery: the real BitFlagsIO.h through
**    shim_flags (which includes KindOf.h first). */
#ifndef SHIM_TERRAIN_PRELUDE_H
#define SHIM_TERRAIN_PRELUDE_H
#include <bitset>
#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>
#include <string.h>
#include <strings.h>
#include "Lib/BaseType.h"
#include "Common/Errors.h"
#include "Common/GameMemory.h"
#include "Common/Debug.h"
#include "Common/Science.h"
#define _isnan isnan
/* the MSVC string names (archive_prelude.h of the shared shim) */
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#include "Common/BitFlagsIO.h"
#endif
