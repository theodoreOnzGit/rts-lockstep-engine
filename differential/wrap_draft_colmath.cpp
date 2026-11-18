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

static Matrix3D rd_tm(Reader & r)
{
	float f[12];
	for (int i = 0; i < 12; i++) {
		f[i] = r.f();
	}
	return Matrix3D(f);
}
static MinMaxAABoxClass rd_mm(Reader & r) { Vector3 a = r.v(); Vector3 b = r.v(); return MinMaxAABoxClass(a, b); }
static void wr_aab(Writer & w, const AABoxClass & b) { w.v(b.Center); w.v(b.Extent); }
static void wr_mm(Writer & w, const MinMaxAABoxClass & b) { w.v(b.MinCorner); w.v(b.MaxCorner); }
static void wr_obb(Writer & w, const OBBoxClass & b) { for (int i = 0; i < 3; i++) w.v(b.Basis[i]); w.v(b.Center); w.v(b.Extent); }
static void wr_sph(Writer & w, const SphereClass & s) { w.v(s.Center); w.f(s.Radius); }
static void wr_line(Writer & w, const LineSegClass & l) { w.v(l.Get_P0()); w.v(l.Get_P1()); w.v(l.Get_DP()); w.v(l.Get_Dir()); w.f(l.Get_Length()); }

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

	/* ---- AABoxClass / MinMaxAABoxClass (aabox.h, aabox.cpp) ---- */
	case 100: { AABoxClass a = rd_aab(r); Vector3 p = r.v(); w.f(a.Project_To_Axis(p)); break; }
	case 101: { AABoxClass a = rd_aab(r); w.f(a.Volume()); break; }
	case 102: { AABoxClass a = rd_aab(r); Vector3 p = r.v(); w.u(a.Contains(p)); break; }
	case 103: { AABoxClass a = rd_aab(r); AABoxClass b = rd_aab(r); w.u(a.Contains(b)); break; }
	case 104: { AABoxClass a = rd_aab(r); MinMaxAABoxClass b = rd_mm(r); w.u(a.Contains(b)); break; }
	case 105: { AABoxClass a = rd_aab(r); Matrix3D m = rd_tm(r); a.Transform(m); wr_aab(w, a); break; }
	case 106: { Matrix3D m = rd_tm(r); AABoxClass a = rd_aab(r); AABoxClass o; AABoxClass::Transform(m, a, &o); wr_aab(w, o); break; }
	case 107: { AABoxClass a = rd_aab(r); Vector3 p = r.v(); a.Add_Point(p); wr_aab(w, a); break; }
	case 108: { AABoxClass a = rd_aab(r); AABoxClass b = rd_aab(r); a.Add_Box(b); wr_aab(w, a); break; }
	case 109: { AABoxClass a = rd_aab(r); MinMaxAABoxClass b = rd_mm(r); a.Add_Box(b); wr_aab(w, a); break; }
	case 110: { int n = (int)r.u(); Vector3 pts[8]; for (int i = 0; i < n; i++) pts[i] = r.v(); AABoxClass a(pts, n); wr_aab(w, a); break; }
	case 111: { MinMaxAABoxClass b = rd_mm(r); AABoxClass a(b); wr_aab(w, a); break; }
	case 112: { LineSegClass l = rd_line(r); AABoxClass a; a.Init(l); wr_aab(w, a); break; }
	case 113: { Vector3 mn = r.v(); Vector3 mx = r.v(); AABoxClass a; a.Init_Min_Max(mn, mx); wr_aab(w, a); break; }
	case 114: { AABoxClass a = rd_aab(r); Vector3 p = r.v(); a.Translate(p); wr_aab(w, a); break; }
	case 115: { int n = (int)r.u(); Vector3 pts[8]; for (int i = 0; i < n; i++) pts[i] = r.v(); MinMaxAABoxClass b(pts, n); wr_mm(w, b); break; }
	case 116: { AABoxClass a = rd_aab(r); MinMaxAABoxClass b(a); wr_mm(w, b); break; }
	case 117: { MinMaxAABoxClass b = rd_mm(r); Vector3 p = r.v(); b.Add_Point(p); wr_mm(w, b); break; }
	case 118: { MinMaxAABoxClass b = rd_mm(r); MinMaxAABoxClass c = rd_mm(r); b.Add_Box(c); wr_mm(w, b); break; }
	case 119: { MinMaxAABoxClass b = rd_mm(r); AABoxClass c = rd_aab(r); b.Add_Box(c); wr_mm(w, b); break; }
	case 120: { MinMaxAABoxClass b = rd_mm(r); Vector3 mn = r.v(); Vector3 mx = r.v(); b.Add_Box(mn, mx); wr_mm(w, b); break; }
	case 121: { MinMaxAABoxClass b = rd_mm(r); Matrix3D m = rd_tm(r); b.Transform(m); wr_mm(w, b); break; }
	case 122: { MinMaxAABoxClass b = rd_mm(r); Vector3 p = r.v(); b.Translate(p); wr_mm(w, b); break; }
	case 123: { MinMaxAABoxClass b = rd_mm(r); w.f(b.Volume()); break; }
	case 124: { MinMaxAABoxClass b = rd_mm(r); b.Init_Empty(); wr_mm(w, b); break; }
	/* ---- OBBoxClass (obbox.h, obbox.cpp) ---- */
	case 130: { OBBoxClass a = rd_obb(r); Vector3 p = r.v(); w.f(a.Project_To_Axis(p)); break; }
	case 131: { OBBoxClass a = rd_obb(r); w.f(a.Volume()); break; }
	case 132: { OBBoxClass a = rd_obb(r); float prm[3]; prm[0] = r.f(); prm[1] = r.f(); prm[2] = r.f(); Vector3 o; a.Compute_Point(prm, &o); w.v(o); break; }
	case 133: { OBBoxClass a = rd_obb(r); Vector3 o; a.Compute_Axis_Aligned_Extent(&o); w.v(o); break; }
	case 134: { Matrix3D m = rd_tm(r); OBBoxClass a = rd_obb(r); OBBoxClass o; OBBoxClass::Transform(m, a, &o); wr_obb(w, o); break; }
	case 135: { OBBoxClass a = rd_obb(r); OBBoxClass b = rd_obb(r); w.u(a == b); break; }
	case 137: { OBBoxClass a = rd_obb(r); OBBoxClass b = rd_obb(r); w.u(Oriented_Boxes_Intersect(a, b)); break; }
	case 138: { OBBoxClass a = rd_obb(r); Vector3 v0 = r.v(); OBBoxClass b = rd_obb(r); Vector3 v1 = r.v(); float dt = r.f(); w.u(Oriented_Boxes_Collide(a, v0, b, v1, dt)); break; }
	case 139: { OBBoxClass a = rd_obb(r); TriHolder t; t.set(r); w.u(Oriented_Box_Intersects_Tri(a, t.t)); break; }
	/* ---- SphereClass (sphere.h) ---- */
	case 140: { Matrix3D m = rd_tm(r); Vector3 p = r.v(); float rad = r.f(); SphereClass s(m, p, rad); wr_sph(w, s); break; }
	case 141: { SphereClass s = rd_sph(r); Vector3 p = r.v(); s.Re_Center(p); wr_sph(w, s); break; }
	case 142: { SphereClass s = rd_sph(r); SphereClass t = rd_sph(r); s.Add_Sphere(t); wr_sph(w, s); break; }
	case 143: { SphereClass s = rd_sph(r); Matrix3D m = rd_tm(r); s.Transform(m); wr_sph(w, s); break; }
	case 144: { SphereClass s = rd_sph(r); w.f(s.Volume()); break; }
	case 145: { int n = (int)r.u(); Vector3 pts[8]; for (int i = 0; i < n; i++) pts[i] = r.v(); SphereClass s(pts, n); wr_sph(w, s); break; }
	case 146: { Vector3 p = r.v(); SphereClass t = rd_sph(r); SphereClass s(p, t); wr_sph(w, s); break; }
	case 147: { SphereClass s = rd_sph(r); Matrix3D m = rd_tm(r); s *= m; wr_sph(w, s); break; }
	case 148: { SphereClass s = rd_sph(r); SphereClass t = rd_sph(r); w.u(Spheres_Intersect(s, t)); break; }
	case 149: { SphereClass s = rd_sph(r); SphereClass t = rd_sph(r); SphereClass o = Add_Spheres(s, t); wr_sph(w, o); break; }
	case 150: { Matrix3D m = rd_tm(r); SphereClass s = rd_sph(r); SphereClass o = Transform_Sphere(m, s); wr_sph(w, o); break; }
	/* ---- LineSegClass (lineseg.h, lineseg.cpp), AAPlaneClass ---- */
	case 160: { LineSegClass l = rd_line(r); Matrix3D m = rd_tm(r); LineSegClass o(l, m); wr_line(w, o); break; }
	case 161: { LineSegClass l = rd_line(r); float t = r.f(); Vector3 o; l.Compute_Point(t, &o); w.v(o); break; }
	case 162: { LineSegClass l = rd_line(r); Vector3 p = r.v(); w.v(l.Find_Point_Closest_To(p)); break; }
	case 163: { LineSegClass l = rd_line(r); LineSegClass m = rd_line(r); Vector3 p1 = r.v(); float f1 = r.f(); Vector3 p2 = r.v(); float f2 = r.f(); bool k = l.Find_Intersection(m, &p1, &f1, &p2, &f2); w.u(k); w.v(p1); w.f(f1); w.v(p2); w.f(f2); break; }
	case 170: { AAPlaneClass a = rd_aap(r); Vector3 o(9, 8, 7); a.Get_Normal(&o); w.v(o); break; }

	default:
		return -1;
	}
	return w.n;
}
