/* (#215, group wwlib4) force-included MSVC names for the WWSaveLoad build: TCHAR (chunkio.h's WRITE_MICRO_CHUNK_WWSTRING), _strlwr
** (definitionmgr.cpp), strcmpi. */
#ifndef WWLIB4_COMPAT_H
#define WWLIB4_COMPAT_H
#include <stdint.h>
#include <string.h>
#include <strings.h>
#include <ctype.h>
typedef char TCHAR;
inline char *_strlwr(char *s) { for (char *p = s; *p; ++p) *p = (char)tolower((unsigned char)*p); return s; }
#ifndef strcmpi
#define strcmpi strcasecmp
#endif
#endif
