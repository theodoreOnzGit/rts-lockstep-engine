/* Forced include (-include) for the battle honours build (GitHub #157), differential tests only: as
** shim_ascii/ascii_prelude.h (the MSVC string-function names, the forward strcpy, the allocator, the REAL
** Common/AsciiString.h), but with this directory's UnicodeString (Common/UnicodeString.h here), which has the
** `format(L"%d", ...)` and `TheEmptyString` the honours code uses. The class names are renamed by -D in build.rs. */
#ifndef SHIM_HONORS_PRELUDE_H
#define SHIM_HONORS_PRELUDE_H
#include <strings.h>
#include <stdio.h>
#include <ctype.h>
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _vsnprintf vsnprintf
#include <string.h>
static inline char *shim_forward_strcpy(char *dst, const char *src)
{
	char *d = dst;
	while ((*d++ = *src++) != 0) {}
	return dst;
}
#define strcpy shim_forward_strcpy
#include "PreRTS.h"
#include "../shim_ascii/Common/GameMemory.h"
#include "Common/UnicodeString.h"
#include "../shim_ascii/Common/AsciiString.h"
#endif
