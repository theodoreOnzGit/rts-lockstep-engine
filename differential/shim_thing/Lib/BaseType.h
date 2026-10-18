/* Stand-in for Libraries/Include/Lib/BaseType.h, Thing and ObjectIter tests
** only. The real header cannot be compiled by g++ (MSVC __int64, __forceinline,
** __asm), and the root shim's BaseType.h declares Coord3D with zero() alone,
** so Thing.cpp's `crossProduct` would not compile against it. This header has
** the root shim's typedefs and small structs, and Coord3D itself copied
** VERBATIM from the real header by build.rs (basetype_coord3d.h, the struct's
** own lines). The wrappers rename Coord3D (wrap_thing.cpp, wrap_object_iter.cpp)
** so that this struct is not the root shim's Coord3D to the linker. */
#ifndef _BASE_TYPE_H_
#define _BASE_TYPE_H_
#include <math.h>
#include <string.h>
typedef float              Real;
typedef int                Int;
typedef unsigned int       UnsignedInt;
typedef unsigned short     UnsignedShort;
typedef short              Short;
typedef unsigned char      UnsignedByte;
typedef char               Byte;
typedef char               Char;
typedef bool               Bool;
typedef long long          Int64;
typedef unsigned long long UnsignedInt64;
typedef unsigned short     WideChar;

#define PI 3.14159265359f

#define BitTest( x, i ) ( ( (x) & (i) ) != 0 )
#define BitSet( x, i ) ( (x) |= (i) )
#define BitClear( x, i ) ( (x ) &= ~(i) )
#define BitToggle( x, i ) ( (x) ^= (i) )

struct Coord2D { Real x, y; };
struct ICoord2D { Int x, y; };
struct Region2D { Coord2D lo, hi; };
struct IRegion2D { ICoord2D lo, hi; };

/* struct Coord3D, verbatim from Libraries/Include/Lib/BaseType.h (see build.rs) */
#include "basetype_coord3d.h"

#ifndef TRUE
#define TRUE true
#endif
#ifndef FALSE
#define FALSE false
#endif
#endif
