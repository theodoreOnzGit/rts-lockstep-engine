/* Differential-test wrapper for the WWMath collision maths drafts (GitHub #215, upstream commit 0a05454d).
**
** NO CUT: upstream's own files are compiled whole, unchanged (build.rs, `drafts_colmath`), as the upstream build does:
**   WWMath/colmath.cpp, colmathaabox.cpp, colmathaabtri.cpp, colmathfrustum.cpp, colmathline.cpp, colmathobbobb.cpp,
**   colmathobbox.cpp, colmathobbtri.cpp, colmathplane.cpp, colmathsphere.cpp  (CollisionMath's overloads)
**   WWMath/aabox.cpp, obbox.cpp, lineseg.cpp, tri.cpp                          (the shapes' out-of-line members)
** with the headers (colmath*.h, colmathinlines.h, aabox.h, ...) the files include, on differential/shim (always.h and
** wwdebug.h are the release-build stand-ins). The wrapper holds only the decoding of the words into the shapes and the
** switch below. Stand-ins for what the files call outside themselves:
**   WWMath::Random_Float()   (lineseg.cpp's LineSegClass::Set_Random; never called here): returns 0.
**
** Entry point (extern "C", tests/differential_draft_colmath.rs):
**   ref_dcm_run(in, nin, out, cap): `in` is [op, then the operands in order], each a run of 32-bit words (floats as bits):
**     point/vector 3 words (x, y, z); AABox 6 (center, extent); OBBox 15 (basis rows 3x3, center, extent);
**     sphere 4 (center, radius); line 6 (p0, p1; LineSegClass(p0, p1) computes DP, Dir and Length);
**     tri 12 (normal, v0, v1, v2); plane 4 (normal, d); AAPlane 2 (axis 0..2, dist); frustum 24 (six planes);
**     min-max box 6; int 1; result prefill 10 (start_bad, fraction, normal, surface_type, compute_contact_point,
**     contact_point), so a field the function leaves alone is compared too.
**   `out` is [the return value (bool 0/1, or the OverlapType value), then for the Collide ops the result's 10 words],
**   and for the two `planes_passed` ops the int after the return value. Returns the number of words written.
**   An op the wrapper does not know writes nothing and returns -1. See OPS below for the op numbers. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "always.h"
#include "wwmath.h"
#include "vector3.h"
#include "matrix3d.h"
#include "matrix3.h"
#include "plane.h"
#include "aaplane.h"
#include "lineseg.h"
#include "tri.h"
#include "sphere.h"
#include "aabox.h"
#include "obbox.h"
#include "frustum.h"
#include "colmath.h"
#include "colmathinlines.h"

/* lineseg.cpp's Set_Random calls this; it is not compiled in the other WWMath libraries of the differential build. */
float WWMath::Random_Float(void) { return 0.0f; }

namespace dcm {

struct Reader
{
	const unsigned * p;
	int n;
	unsigned u() { return p[n++]; }
	float f() { unsigned b = p[n++]; float x; memcpy(&x, &b, sizeof x); return x; }
	Vector3 v() { float x = f(); float y = f(); float z = f(); return Vector3(x, y, z); }
};

struct Writer
{
	unsigned * p;
	int cap;
	int n;
	void u(unsigned x) { if (n < cap) p[n] = x; n++; }
	void f(float x) { unsigned b; memcpy(&b, &x, sizeof b); u(b); }
	void v(const Vector3 & x) { f(x.X); f(x.Y); f(x.Z); }
};

struct TriHolder
{
	Vector3 n;
	Vector3 vs[3];
	TriClass t;
	void set(Reader & r)
	{
		n = r.v();
		vs[0] = r.v();
		vs[1] = r.v();
		vs[2] = r.v();
		t.N = &n;
		t.V[0] = &vs[0];
		t.V[1] = &vs[1];
		t.V[2] = &vs[2];
	}
};

static AABoxClass rd_aab(Reader & r) { Vector3 c = r.v(); Vector3 e = r.v(); return AABoxClass(c, e); }
static OBBoxClass rd_obb(Reader & r)
{
	OBBoxClass b;
	for (int i = 0; i < 3; i++) {
		b.Basis[i] = r.v();
	}
	b.Center = r.v();
	b.Extent = r.v();
	return b;
}
static SphereClass rd_sph(Reader & r) { Vector3 c = r.v(); float rad = r.f(); return SphereClass(c, rad); }
static LineSegClass rd_line(Reader & r) { Vector3 a = r.v(); Vector3 b = r.v(); return LineSegClass(a, b); }
static PlaneClass rd_plane(Reader & r) { PlaneClass p; p.N = r.v(); p.D = r.f(); return p; }
static AAPlaneClass rd_aap(Reader & r)
{
	unsigned axis = r.u();
	float dist = r.f();
	return AAPlaneClass((AAPlaneClass::AxisEnum)axis, dist);
}
static void rd_frustum(Reader & r, FrustumClass & fr)
{
	for (int i = 0; i < 6; i++) {
		fr.Planes[i] = rd_plane(r);
	}
}
static void rd_cr(Reader & r, CastResultStruct & c)
{
	c.StartBad = r.u() != 0;
	c.Fraction = r.f();
	c.Normal = r.v();
	c.SurfaceType = r.u();
	c.ComputeContactPoint = r.u() != 0;
	c.ContactPoint = r.v();
}
static void wr_cr(Writer & w, const CastResultStruct & c)
{
	w.u(c.StartBad ? 1 : 0);
	w.f(c.Fraction);
	w.v(c.Normal);
	w.u(c.SurfaceType);
	w.u(c.ComputeContactPoint ? 1 : 0);
	w.v(c.ContactPoint);
}

} /* namespace dcm */

using namespace dcm;

extern "C" int ref_dcm_run(const unsigned * in, int nin, unsigned * out, int cap)
{
	(void)nin;
	Reader r = { in, 0 };
	Writer w = { out, cap, 0 };
	unsigned op = r.u();
	typedef CollisionMath CM;
	switch (op) {

	/* ---- Overlap_Test, AAPlane first ---- */
	case 0: { AAPlaneClass a = rd_aap(r); Vector3 b = r.v(); w.u(CM::Overlap_Test(a, b)); break; }
	case 1: { AAPlaneClass a = rd_aap(r); LineSegClass b = rd_line(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 2: { AAPlaneClass a = rd_aap(r); TriHolder b; b.set(r); w.u(CM::Overlap_Test(a, b.t)); break; }
	case 3: { AAPlaneClass a = rd_aap(r); SphereClass b = rd_sph(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 4: { AAPlaneClass a = rd_aap(r); AABoxClass b = rd_aab(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 5: { AAPlaneClass a = rd_aap(r); OBBoxClass b = rd_obb(r); w.u(CM::Overlap_Test(a, b)); break; }
	/* ---- Plane first ---- */
	case 6: { PlaneClass a = rd_plane(r); Vector3 b = r.v(); w.u(CM::Overlap_Test(a, b)); break; }
	case 7: { PlaneClass a = rd_plane(r); LineSegClass b = rd_line(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 8: { PlaneClass a = rd_plane(r); TriHolder b; b.set(r); w.u(CM::Overlap_Test(a, b.t)); break; }
	case 9: { PlaneClass a = rd_plane(r); SphereClass b = rd_sph(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 11: { PlaneClass a = rd_plane(r); AABoxClass b = rd_aab(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 12: { PlaneClass a = rd_plane(r); OBBoxClass b = rd_obb(r); w.u(CM::Overlap_Test(a, b)); break; }
	/* ---- Sphere first ---- */
	case 13: { SphereClass a = rd_sph(r); Vector3 b = r.v(); w.u(CM::Overlap_Test(a, b)); break; }
	case 14: { SphereClass a = rd_sph(r); LineSegClass b = rd_line(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 15: { SphereClass a = rd_sph(r); TriHolder b; b.set(r); w.u(CM::Overlap_Test(a, b.t)); break; }
	case 16: { SphereClass a = rd_sph(r); SphereClass b = rd_sph(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 17: { SphereClass a = rd_sph(r); AABoxClass b = rd_aab(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 18: { SphereClass a = rd_sph(r); OBBoxClass b = rd_obb(r); w.u(CM::Overlap_Test(a, b)); break; }
	/* ---- AABox first ---- */
	case 19: { AABoxClass a = rd_aab(r); Vector3 b = r.v(); w.u(CM::Overlap_Test(a, b)); break; }
	case 20: { AABoxClass a = rd_aab(r); LineSegClass b = rd_line(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 21: { AABoxClass a = rd_aab(r); TriHolder b; b.set(r); w.u(CM::Overlap_Test(a, b.t)); break; }
	case 22: { AABoxClass a = rd_aab(r); AABoxClass b = rd_aab(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 23: { AABoxClass a = rd_aab(r); OBBoxClass b = rd_obb(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 24: { AABoxClass a = rd_aab(r); SphereClass b = rd_sph(r); w.u(CM::Overlap_Test(a, b)); break; }
	/* ---- OBBox first ---- */
	case 25: { OBBoxClass a = rd_obb(r); Vector3 b = r.v(); w.u(CM::Overlap_Test(a, b)); break; }
	case 26: { OBBoxClass a = rd_obb(r); LineSegClass b = rd_line(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 27: { OBBoxClass a = rd_obb(r); TriHolder b; b.set(r); w.u(CM::Overlap_Test(a, b.t)); break; }
	case 28: { OBBoxClass a = rd_obb(r); AABoxClass b = rd_aab(r); w.u(CM::Overlap_Test(a, b)); break; }
	case 29: { OBBoxClass a = rd_obb(r); OBBoxClass b = rd_obb(r); w.u(CM::Overlap_Test(a, b)); break; }
	/* ---- Frustum first ---- */
	case 30: { FrustumClass fr; rd_frustum(r, fr); Vector3 b = r.v(); w.u(CM::Overlap_Test(fr, b)); break; }
	case 31: { FrustumClass fr; rd_frustum(r, fr); TriHolder b; b.set(r); w.u(CM::Overlap_Test(fr, b.t)); break; }
	case 32: { FrustumClass fr; rd_frustum(r, fr); SphereClass b = rd_sph(r); w.u(CM::Overlap_Test(fr, b)); break; }
	case 33: { FrustumClass fr; rd_frustum(r, fr); AABoxClass b = rd_aab(r); w.u(CM::Overlap_Test(fr, b)); break; }
	case 34: { FrustumClass fr; rd_frustum(r, fr); OBBoxClass b = rd_obb(r); w.u(CM::Overlap_Test(fr, b)); break; }
	case 35: { FrustumClass fr; rd_frustum(r, fr); AABoxClass b = rd_aab(r); int pp = (int)r.u(); unsigned res = CM::Overlap_Test(fr, b, pp); w.u(res); w.u((unsigned)pp); break; }
	case 36: { FrustumClass fr; rd_frustum(r, fr); OBBoxClass b = rd_obb(r); int pp = (int)r.u(); unsigned res = CM::Overlap_Test(fr, b, pp); w.u(res); w.u((unsigned)pp); break; }
	/* ---- min, max, line ---- */
	case 37: { Vector3 mn = r.v(); Vector3 mx = r.v(); LineSegClass b = rd_line(r); w.u(CM::Overlap_Test(mn, mx, b)); break; }

	/* ---- Intersection_Test ---- */
	case 40: { AABoxClass a = rd_aab(r); TriHolder b; b.set(r); w.u(CM::Intersection_Test(a, b.t)); break; }
	case 41: { AABoxClass a = rd_aab(r); AABoxClass b = rd_aab(r); w.u(CM::Intersection_Test(a, b)); break; }
	case 42: { AABoxClass a = rd_aab(r); OBBoxClass b = rd_obb(r); w.u(CM::Intersection_Test(a, b)); break; }
	case 43: { OBBoxClass a = rd_obb(r); TriHolder b; b.set(r); w.u(CM::Intersection_Test(a, b.t)); break; }
	case 44: { OBBoxClass a = rd_obb(r); AABoxClass b = rd_aab(r); w.u(CM::Intersection_Test(a, b)); break; }
	case 45: { OBBoxClass a = rd_obb(r); OBBoxClass b = rd_obb(r); w.u(CM::Intersection_Test(a, b)); break; }
	case 46: { SphereClass a = rd_sph(r); AABoxClass b = rd_aab(r); w.u(CM::Intersection_Test(a, b)); break; }
	case 47: { SphereClass a = rd_sph(r); OBBoxClass b = rd_obb(r); w.u(CM::Intersection_Test(a, b)); break; }

	/* ---- Collide, line first ---- */
	case 60: { LineSegClass a = rd_line(r); AAPlaneClass b = rd_aap(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, b, &c); w.u(k); wr_cr(w, c); break; }
	case 61: { LineSegClass a = rd_line(r); PlaneClass b = rd_plane(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, b, &c); w.u(k); wr_cr(w, c); break; }
	case 62: { LineSegClass a = rd_line(r); TriHolder b; b.set(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, b.t, &c); w.u(k); wr_cr(w, c); break; }
	case 63: { LineSegClass a = rd_line(r); SphereClass b = rd_sph(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, b, &c); w.u(k); wr_cr(w, c); break; }
	case 64: { LineSegClass a = rd_line(r); AABoxClass b = rd_aab(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, b, &c); w.u(k); wr_cr(w, c); break; }
	case 65: { LineSegClass a = rd_line(r); OBBoxClass b = rd_obb(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, b, &c); w.u(k); wr_cr(w, c); break; }
	/* ---- Collide, swept AABox ---- */
	case 66: { AABoxClass a = rd_aab(r); Vector3 m = r.v(); PlaneClass b = rd_plane(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b, &c); w.u(k); wr_cr(w, c); break; }
	case 67: { AABoxClass a = rd_aab(r); Vector3 m = r.v(); TriHolder b; b.set(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b.t, &c); w.u(k); wr_cr(w, c); break; }
	case 68: { AABoxClass a = rd_aab(r); Vector3 m = r.v(); AABoxClass b = rd_aab(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b, &c); w.u(k); wr_cr(w, c); break; }
	case 69: { AABoxClass a = rd_aab(r); Vector3 m = r.v(); OBBoxClass b = rd_obb(r); Vector3 m2 = r.v(); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b, m2, &c); w.u(k); wr_cr(w, c); break; }
	/* ---- Collide, swept OBBox ---- */
	case 70: { OBBoxClass a = rd_obb(r); Vector3 m = r.v(); PlaneClass b = rd_plane(r); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b, &c); w.u(k); wr_cr(w, c); break; }
	case 71: { OBBoxClass a = rd_obb(r); Vector3 m = r.v(); TriHolder b; b.set(r); Vector3 m2 = r.v(); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b.t, m2, &c); w.u(k); wr_cr(w, c); break; }
	case 72: { OBBoxClass a = rd_obb(r); Vector3 m = r.v(); AABoxClass b = rd_aab(r); Vector3 m2 = r.v(); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b, m2, &c); w.u(k); wr_cr(w, c); break; }
	case 73: { OBBoxClass a = rd_obb(r); Vector3 m = r.v(); OBBoxClass b = rd_obb(r); Vector3 m2 = r.v(); CastResultStruct c; rd_cr(r, c); bool k = CM::Collide(a, m, b, m2, &c); w.u(k); wr_cr(w, c); break; }

	default:
		return -1;
	}
	return w.n;
}
