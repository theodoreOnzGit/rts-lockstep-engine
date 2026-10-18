/* Forced include (-include) for the STLTypedefs build, differential tests
** only. The same stand-ins as the AsciiString build (differential/shim_ascii/
** ascii_prelude.h) without its PreRTS.h, which is the shim tree's and would
** shadow the real STLTypedefs.h: the MSVC string-function names, the
** pointer-sized allocator and UnicodeString of shim_dict, and the REAL
** Common/AsciiString.h (through shim_ascii's forwarder). The classes are
** renamed by -D in build.rs, as in the AsciiString build, so that this
** build's AsciiString cannot be linked against another build's. */
#ifndef SHIM_STL_PRELUDE_H
#define SHIM_STL_PRELUDE_H
#include <strings.h>
#include <string.h>
#include <stdio.h>
#include <ctype.h>
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _vsnprintf vsnprintf
/* strcpy as a forward byte copy, as in shim_ascii/ascii_prelude.h: AsciiString's
** set() and trim() copy overlapping strings forward (the MSVC behaviour). See
** src/common/ascii_string.rs. AsciiString.cpp is compiled in this build. */
static inline char *shim_stl_forward_strcpy(char *dst, const char *src)
{
	char *d = dst;
	while ((*d++ = *src++) != 0) {}
	return dst;
}
#define strcpy shim_stl_forward_strcpy
/* BaseType.h's PI (real header line 86), which GameCommon.h uses. */
#ifndef PI
#define PI 3.14159265359f
#endif
/* STLTypedefs.h forward-declares enums without a base type (MSVC accepts
** that; g++ does not). These opaque declarations, each with the int base
** the real enums have, stand in for the real definitions, which this
** translation unit does not need. */
enum NameKeyType : int;
enum ObjectID : int;
enum DrawableID : int;
#include "Common/GameMemory.h"
#include "Common/UnicodeString.h"
#include "Common/AsciiString.h"
#endif
