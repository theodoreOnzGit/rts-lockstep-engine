/* Differential-test wrapper for BezierSegment and BezFwdIterator (upstream
** commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezierSegment.cpp
**   GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezFwdIterator.cpp
** (both #included below, unchanged). The headers come from shim_bezier
** (Coord3D's methods, the D3DX math transcription, STLTypeDefs.h), then the
** shim and the engine headers; see build.rs.
**
** Entry points (extern "C", tests/differential_bezier.rs):
**   ref_bez_eval    evaluateBezSegmentAtT(t) -> out[3]
**   ref_bez_points  getSegmentPoints(n) -> out[3n]; returns 0, or 1 if upstream threw
**   ref_bez_length  getApproximateLength(tol) -> *out
**   ref_bez_split   splitSegmentAtT(t) -> out[24] (segment 1, then segment 2)
**   ref_bez_iter    BezFwdIterator(n, seg): start, then done/getCurrent/next, n points
** A segment is given as 12 floats: x, y, z of each control point in turn. */
#include <stdlib.h>
#include <string.h>
#include <stdexcept>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezierSegment.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezFwdIterator.cpp"

/* m_controlPoints is protected: a derived class, copy-constructed from the
** segment, reads it (no cast of an object to a type it is not) */
struct BezierPeek : public BezierSegment
{
	explicit BezierPeek(const BezierSegment &s) : BezierSegment(s) {}
	void copyTo(float *out) const
	{
		for (int i = 0; i < 4; ++i) {
			out[3 * i] = m_controlPoints[i].x;
			out[3 * i + 1] = m_controlPoints[i].y;
			out[3 * i + 2] = m_controlPoints[i].z;
		}
	}
};

/* the constructor takes a `Real*` (it does not modify it) */
static BezierSegment segmentOf(const float *cp)
{
	return BezierSegment((Real *)cp);
}

extern "C" {

void ref_bez_eval(const float *cp, float t, float *out)
{
	BezierSegment seg = segmentOf(cp);
	Coord3D r;
	seg.evaluateBezSegmentAtT(t, &r);
	out[0] = r.x;
	out[1] = r.y;
	out[2] = r.z;
}

/* returns 0, or 1 when upstream threw (std::length_error for a negative count) */
int ref_bez_points(const float *cp, int n, float *out, int cap, int *count)
{
	BezierSegment seg = segmentOf(cp);
	VecCoord3D v;
	try {
		seg.getSegmentPoints(n, &v);
	} catch (...) {
		*count = -1;
		return 1;
	}
	*count = (int)v.size();
	for (size_t i = 0; i < v.size() && (int)(3 * i + 2) < cap; ++i) {
		out[3 * i] = v[i].x;
		out[3 * i + 1] = v[i].y;
		out[3 * i + 2] = v[i].z;
	}
	return 0;
}

float ref_bez_length(const float *cp, float tol)
{
	BezierSegment seg = segmentOf(cp);
	return seg.getApproximateLength(tol);
}

void ref_bez_split(const float *cp, float t, float *out)
{
	BezierSegment seg = segmentOf(cp);
	BezierSegment a, b;
	seg.splitSegmentAtT(t, a, b);
	BezierPeek(a).copyTo(out);
	BezierPeek(b).copyTo(out + 12);
}

void ref_bez_iter(const float *cp, int n, float *out)
{
	BezierSegment seg = segmentOf(cp);
	BezFwdIterator it(n, &seg);
	it.start();
	int i = 0;
	while (!it.done()) {
		const Coord3D &c = it.getCurrent();
		out[3 * i] = c.x;
		out[3 * i + 1] = c.y;
		out[3 * i + 2] = c.z;
		++i;
		it.next();
	}
}

} /* extern "C" */
