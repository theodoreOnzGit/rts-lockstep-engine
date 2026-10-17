/* Forced include (-include) for INI.cpp, differential tests only. The real
** precompiled header PreRTS.h brings in, before INI.cpp's own includes, the
** things INI.h and its headers rely on: <bitset> (BitFlags.h), the error
** codes of Common/Errors.h (INI.h's status codes are ERROR_BAD_INI), the
** ScienceType enum (INI.h names it before declaring it), and BaseType.h's
** `#define PI 3.14159265359f` (real Lib/BaseType.h line 86; GameCommon.h
** uses it). The .big prelude is reused for the MSVC string names, and
** GameCommon.cpp's `_isnan` is the MSVC name of C99's isnan. */
#ifndef SHIM_INI_PRELUDE_H
#define SHIM_INI_PRELUDE_H
#include "../shim/archive_prelude.h"
#include <bitset>
#include <stdio.h>
#include <ctype.h>
#include "Common/Errors.h"
#include "Common/Science.h"
#define _isnan isnan
#ifndef PI
#define PI 3.14159265359f
#endif
#endif
