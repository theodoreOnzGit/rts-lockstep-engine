/* Shim for Libraries/Include/Lib/BaseType.h, Bezier tests only (see
** differential/wrap_bezier.cpp): differential/shim/Lib/BaseType.h with the
** Coord3D methods that BezierSegment.cpp and BezFwdIterator.cpp call. */
/* Shim for Libraries/Include/Lib/BaseType.h (upstream commit 0a05454d),
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

#define BitTest( x, i ) ( ( (x) & (i) ) != 0 )
#define BitSet( x, i ) ( (x) |= (i) )
#define BitClear( x, i ) ( (x ) &= ~(i) )
#define BitToggle( x, i ) ( (x) ^= (i) )

struct RealRange { Real lo, hi; };
struct Coord2D { Real x, y; };
struct ICoord2D { Int x, y; };
struct Region2D { Coord2D lo, hi; };
struct IRegion2D { ICoord2D lo, hi; };
struct Coord3D
{
	Real x, y, z;

	/* Lib/BaseType.h lines 331-398 (the methods Bezier code calls), verbatim
	** except that `sqrt` is the C library's, as in the real header */
	Real length( void ) const { return (Real)sqrt( x*x + y*y + z*z ); }
	void zero( void ) { x = 0.0f; y = 0.0f; z = 0.0f; }
	void add( const Coord3D *a ) { x += a->x; y += a->y; z += a->z; }
	void scale( Real scale ) { x *= scale; y *= scale; z *= scale; }
};
struct ICoord3D { Int x, y, z; void zero( void ) { x = 0; y = 0; z = 0; } };
struct Region3D { Coord3D lo, hi; };
struct IRegion3D { ICoord3D lo, hi; };
struct RGBColor { Real red, green, blue; };
struct RGBAColorReal { Real red, green, blue, alpha; };
struct RGBAColorInt { UnsignedInt red, green, blue, alpha; };

#ifndef TRUE
#define TRUE true
#endif
#ifndef FALSE
#define FALSE false
#endif
/* Lib/BaseType.h line 240 */
#ifndef INT_TO_REAL
#define INT_TO_REAL(x) ((Real) (x))
#endif

#endif
