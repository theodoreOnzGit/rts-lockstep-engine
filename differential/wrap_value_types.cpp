/* Differential-test wrapper for the value types and small templates of
** Libraries/Include/Lib/BaseType.h (upstream commit 0a05454d).
**
** The real header cannot be compiled by g++ (MSVC __int64, __forceinline,
** __asm). build.rs therefore copies two segments of its text VERBATIM into
** OUT_DIR/basetype_derived.h, each located by its first and last line and
** asserted to occur once:
**   1. `template <typename NUM>` / `inline NUM sqr` ... `deg2rad` (lines
**      142-167: sqr, clamp, sign, rad2deg, deg2rad);
**   2. `struct RealRange` ... the end of `RGBAColorInt` (lines 258-492: the
**      derived types with their methods).
** What those segments need from the rest of the header is reproduced here
** from it: the fundamental typedefs (lines 124-135, __int64 spelled
** long long), PI (line 86) and min/max (lines 100-106). `Coord2D::toAngle`
** calls ACos from Lib/trig.h, i.e. Trig.cpp, linked from the
** upstream_gameengine library (differential/wrap_trig.cpp). The shim
** <math.h> keeps `sqrt(float)` the C double function, as in the MSVC build.
**
** This build does not see differential/shim/Lib/BaseType.h (whose
** member-only structs would clash with the real ones). */
#include <math.h>
#include <stdlib.h>
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
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
#define TRUE true
#define FALSE false

#include "Lib/trig.h"
#include "basetype_derived.h"

static Coord3D c3(const float *p) { Coord3D c; c.x = p[0]; c.y = p[1]; c.z = p[2]; return c; }
static void out3(const Coord3D &c, float *o) { o[0] = c.x; o[1] = c.y; o[2] = c.z; }

extern "C" {

/* Coord2D. op: 0 length, 1 normalize (out[0..2]), 2 toAngle */
float ref_vt_coord2d(int op, float x, float y, float *out)
{
	Coord2D c; c.x = x; c.y = y;
	switch (op) {
	case 0: return c.length();
	case 1: c.normalize(); out[0] = c.x; out[1] = c.y; return 0.0f;
	case 2: return c.toAngle();
	}
	abort();
}

/* Coord3D. op: 0 length, 1 lengthSqr, 2 normalize, 3 crossProduct(a, b),
** 4 add, 5 sub, 6 scale(k), 7 equals, 8 operator==, 9 set(b), 10 set(k,k,k),
** 11 zero. Vector results go to out. */
float ref_vt_coord3d(int op, const float *a, const float *b, float k, float *out)
{
	Coord3D c = c3(a), d = c3(b), r;
	switch (op) {
	case 0: return c.length();
	case 1: return c.lengthSqr();
	case 2: c.normalize(); out3(c, out); return 0.0f;
	case 3: Coord3D::crossProduct(&c, &d, &r); out3(r, out); return 0.0f;
	case 4: c.add(&d); out3(c, out); return 0.0f;
	case 5: c.sub(&d); out3(c, out); return 0.0f;
	case 6: c.scale(k); out3(c, out); return 0.0f;
	case 7: return c.equals(d) ? 1.0f : 0.0f;
	case 8: return (c == d) ? 1.0f : 0.0f;
	case 9: c.set(&d); out3(c, out); return 0.0f;
	case 10: c.set(k, k, k); out3(c, out); return 0.0f;
	case 11: c.zero(); out3(c, out); return 0.0f;
	}
	abort();
}

/* Region2D/Region3D (float). op: 0 R2 width, 1 R2 height, 2 R3 width,
** 3 R3 height, 4 R3 depth, 5 isInRegionNoZ(q), 6 isInRegionWithZ(q),
** 7 R3 zero (out lo, hi) */
float ref_vt_region(int op, const float *lo, const float *hi, const float *q, float *out)
{
	Region2D r2; r2.lo.x = lo[0]; r2.lo.y = lo[1]; r2.hi.x = hi[0]; r2.hi.y = hi[1];
	Region3D r3; r3.lo = c3(lo); r3.hi = c3(hi);
	Coord3D p = c3(q);
	switch (op) {
	case 0: return r2.width();
	case 1: return r2.height();
	case 2: return r3.width();
	case 3: return r3.height();
	case 4: return r3.depth();
	case 5: return r3.isInRegionNoZ(&p) ? 1.0f : 0.0f;
	case 6: return r3.isInRegionWithZ(&p) ? 1.0f : 0.0f;
	case 7: r3.zero(); out3(r3.lo, out); out3(r3.hi, out + 3); return 0.0f;
	}
	abort();
}

/* Integer types. op: 0 ICoord2D length, 1 ICoord3D length, 2 IRegion2D
** width, 3 IRegion2D height, 4 IRegion3D width, 5 height, 6 depth,
** 7 ICoord3D zero (returns x|y|z). Callers pass only inputs where upstream
** is defined (no Int overflow). */
int ref_vt_int(int op, const int *lo, const int *hi)
{
	ICoord2D a2; a2.x = lo[0]; a2.y = lo[1];
	ICoord3D a3; a3.x = lo[0]; a3.y = lo[1]; a3.z = lo[2];
	IRegion2D r2; r2.lo.x = lo[0]; r2.lo.y = lo[1]; r2.hi.x = hi[0]; r2.hi.y = hi[1];
	IRegion3D r3; r3.lo = a3; r3.hi.x = hi[0]; r3.hi.y = hi[1]; r3.hi.z = hi[2];
	switch (op) {
	case 0: return a2.length();
	case 1: return a3.length();
	case 2: return r2.width();
	case 3: return r2.height();
	case 4: return r3.width();
	case 5: return r3.height();
	case 6: return r3.depth();
	case 7: a3.zero(); return a3.x | a3.y | a3.z;
	}
	abort();
}

/* RGBColor: getAsInt (only defined inputs), setFromInt(c) into out. */
int ref_vt_rgb_get_as_int(const float *rgb)
{
	RGBColor c; c.red = rgb[0]; c.green = rgb[1]; c.blue = rgb[2];
	return c.getAsInt();
}
void ref_vt_rgb_set_from_int(int v, float *out)
{
	RGBColor c; c.setFromInt(v);
	out[0] = c.red; out[1] = c.green; out[2] = c.blue;
}

/* Templates and macros on Real. op: 0 sqr(a), 1 clamp(a, b, c), 2 sign(a),
** 3 rad2deg(a), 4 deg2rad(a), 5 min(a, b), 6 max(a, b),
** 7 RealRange{a,b}.combine({c,d}) -> lo, 8 the same -> hi */
float ref_vt_real(int op, float a, float b, float c, float d)
{
	RealRange r, o;
	switch (op) {
	case 0: return sqr(a);
	case 1: return clamp(a, b, c);
	case 2: return (float)sign(a);
	case 3: return rad2deg(a);
	case 4: return deg2rad(a);
	case 5: return min(a, b);
	case 6: return max(a, b);
	case 7: r.lo = a; r.hi = b; o.lo = c; o.hi = d; r.combine(o); return r.lo;
	case 8: r.lo = a; r.hi = b; o.lo = c; o.hi = d; r.combine(o); return r.hi;
	}
	abort();
}

/* Templates on Int. op: 0 sqr(a) (|a| <= 46340 only), 1 clamp(a, b, c),
** 2 sign(a) */
int ref_vt_intfn(int op, int a, int b, int c)
{
	switch (op) {
	case 0: return sqr(a);
	case 1: return clamp(a, b, c);
	case 2: return sign(a);
	}
	abort();
}

} /* extern "C" */
