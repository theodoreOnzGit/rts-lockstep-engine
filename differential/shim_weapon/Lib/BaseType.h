/* The Weapon firing build's BaseType.h (p1w8-sonnet): the root shim's (differential/shim/Lib/BaseType.h) with Coord3D copied VERBATIM from
** the real header (Weapon.cpp calls its set, length, sub, normalize ...), PI, and the small templates sqr/clamp/sign/rad2deg of the real header.
** Original comment of the root shim follows.
**
** Shim for Libraries/Include/Lib/BaseType.h (upstream commit 0a05454d),
** differential tests only. The real header uses MSVC-only constructs
** (__int64, __forceinline, __asm blocks) that g++ cannot compile. Only the
** fundamental typedefs are reproduced here, copied from the real header with
** __int64 spelled as long long, plus (for the Xfer files) the BitSet/BitClear/
** BitTest macros and the data members of the derived types (lines 172-175 and
** 258-492; their methods are omitted, nothing compiled here calls them,
** except Coord3D::zero and ICoord3D::zero, lines 357-362 and 419-425, which
** GameLogic/Damage.h calls (INI tests)).
** WideChar is `wchar_t` upstream, which is 16 bits on Win32 and 32 bits on
** Linux; it is spelled `unsigned short` here so the bytes Xfer moves are the
** Win32 ones. The asm helpers of the real header are transcribed separately
** in differential/wrap_base_type.cpp. */
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


#ifndef PI
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#endif
template <typename NUM>
inline NUM sqr(NUM x)
{
	return x*x;
}
template <typename NUM>
inline NUM clamp(NUM lo, NUM val, NUM hi)
{
	if (val < lo) return lo;
	else if (val > hi) return hi;
	else return val;
}
template <typename NUM>
inline int sign(NUM x)
{
	if (x > 0) return 1;
	else if (x < 0) return -1;
	else return 0;
}
inline Real rad2deg(Real rad) { return rad * (180/PI); }
inline Real deg2rad(Real rad) { return rad * (PI/180); }

#define BitTest( x, i ) ( ( (x) & (i) ) != 0 )
#define BitSet( x, i ) ( (x) |= (i) )
#define BitClear( x, i ) ( (x ) &= ~(i) )
#define BitToggle( x, i ) ( (x) ^= (i) )

struct RealRange { Real lo, hi; };
struct Coord2D { Real x, y; };
struct ICoord2D { Int x, y; };
struct Region2D { Coord2D lo, hi; };
struct IRegion2D { ICoord2D lo, hi; };
/* struct Coord3D, verbatim from Libraries/Include/Lib/BaseType.h (build.rs writes weapon_gen/basetype_coord3d.h) */
#include "basetype_coord3d.h"
struct ICoord3D { Int x, y, z; void zero( void ) { x = 0; y = 0; z = 0; } };
struct Region3D { Coord3D lo, hi; };
struct IRegion3D { ICoord3D lo, hi; };
struct RGBColor { Real red, green, blue;
	/* p1w2-globaldata: setFromInt (BaseType.h lines 472-477), called by GlobalData's constructor */
	void setFromInt( Int c ) { red = ((c >> 16) & 0xff) / 255.0f; green = ((c >> 8) & 0xff) / 255.0f; blue = ((c >> 0) & 0xff) / 255.0f; }
};
struct RGBAColorReal { Real red, green, blue, alpha; };
struct RGBAColorInt { UnsignedInt red, green, blue, alpha; };

#ifndef TRUE
#define TRUE true
#endif
#ifndef FALSE
#define FALSE false
#endif
#endif
