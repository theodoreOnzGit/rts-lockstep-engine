/* Forced include (-include) for the AsciiString build, differential tests
** only: what the real PreRTS.h brings before AsciiString.cpp's own includes:
** the MSVC string-function names (stricmp, _stricmp, strnicmp as the C
** library's ASCII-folding strcasecmp/strncasecmp; _vsnprintf, for the
** untested format_va), <ctype.h>, the allocator (shim_dict's GameMemory.h),
** and Common/AsciiString.h (the REAL one,
** through shim_ascii/Common) with the pointer-sized UnicodeString. The class
** names are renamed by -D in build.rs (see there). Not the .big prelude: it
** includes the std::string AsciiString stand-in by a path relative to
** itself, which no include order can override. */
#ifndef SHIM_ASCII_PRELUDE_H
#define SHIM_ASCII_PRELUDE_H
#include <strings.h>
#include <stdio.h>
#include <ctype.h>
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _vsnprintf vsnprintf
/* strcpy as a forward byte copy. AsciiString::set(const char*) copies with
** strcpy into its own buffer when it has room, and trim() and nextToken()
** set a string from its own tail, so source and destination overlap
** (destination first). That is undefined in C; MSVC's strcpy copies forward
** and gives the intended result, while glibc's vectorised strcpy does not
** (the first differential run showed a corrupted string). The translation
** follows the forward copy (see src/common/ascii_string.rs). */
#include <string.h>
static inline char *shim_forward_strcpy(char *dst, const char *src)
{
	char *d = dst;
	while ((*d++ = *src++) != 0) {}
	return dst;
}
#define strcpy shim_forward_strcpy
#include "PreRTS.h"
#include "Common/GameMemory.h"
#include "Common/UnicodeString.h"
#include "Common/AsciiString.h"
#endif
