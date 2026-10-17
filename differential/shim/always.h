/* Shim for WWLib/always.h (upstream commit 0a05454d), differential tests only.
** The real header declares MSVC-specific operator new overloads (and a placement
** operator new[] that collides with the standard <new>). Only the macros that
** WWMath actually uses are reproduced here, with the upstream definitions. */
#ifndef ALWAYS_H
#define ALWAYS_H
#include <assert.h>
#include <stddef.h>
#define WWINLINE inline
#define NOMINMAX
#ifndef MAX
#define MAX(a,b)            (((a) > (b)) ? (a) : (b))
#endif
#ifndef MIN
#define MIN(a,b)            (((a) < (b)) ? (a) : (b))
#endif
#define W3DMPO_GLUE(ARGCLASS)
class W3DMPO { };
#ifndef ARRAY_SIZE
#define ARRAY_SIZE(x) int(sizeof(x)/sizeof(x[0]))
#endif
#endif
