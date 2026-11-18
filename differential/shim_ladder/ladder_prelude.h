/* Forced include (-include) for the ladder build (GitHub #215), differential tests only: as shim_honors/honors_prelude.h, with this directory's forwarders (the real AsciiString.h and the richer 16-bit UnicodeString of differential/shim). */
#ifndef SHIM_LADDER_PRELUDE_H
#define SHIM_LADDER_PRELUDE_H
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
#include "Common/GameMemory.h"
#include "Common/UnicodeString.h"
#include "Common/AsciiString.h"
#endif
