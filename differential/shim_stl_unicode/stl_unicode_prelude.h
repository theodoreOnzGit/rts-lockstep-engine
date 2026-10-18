/* Forced include (-include) for the STLTypedefs build with the REAL
** UnicodeString (differential/wrap_stl_typedefs_unicode.cpp only): the 16-bit
** wide C runtime that UnicodeString.cpp calls (wide_runtime.h, copied from
** differential/shim_unicode/unicode_prelude.h), then what stl_prelude.h adds
** for STLTypedefs.h: the opaque enums it forward-declares, and BaseType.h's
** PI, which GameCommon.h uses. */
#ifndef SHIM_STL_UNICODE_PRELUDE_H
#define SHIM_STL_UNICODE_PRELUDE_H
#include "wide_runtime.h"
/* The MSVC C-library names that the real AsciiString.h calls (as in
** differential/shim_stl/stl_prelude.h). */
#include <strings.h>
#include <string.h>
#include <stdio.h>
#define stricmp strcasecmp
#define _stricmp strcasecmp
#define strnicmp strncasecmp
#define _vsnprintf vsnprintf
/* STLTypedefs.h forward-declares enums without a base type (MSVC accepts
** that; g++ does not). See differential/shim_stl/stl_prelude.h. */
enum NameKeyType : int;
enum ObjectID : int;
enum DrawableID : int;
#ifndef PI
#define PI 3.14159265359f
#endif
#endif
