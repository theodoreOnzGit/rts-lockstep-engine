/* Differential-test wrapper for the collision-test drafts (GitHub #215, upstream commit 0a05454d), group "ww3dpure":
** src/drafts/ww3d/coltest.rs against WW3D2/coltest.cpp and coltest.h (the AABoxCollisionTestClass and OBBoxCollisionTestClass
** members; RayCollisionTestClass is reused from the port and not part of the draft).
**
** coltest.cpp is compiled UNCHANGED (its own translation unit, NDEBUG so Rotate's debug cross-check is out, as in the shipped
** game); coltest.h, aabox, obbox, matrix3d etc. are the real headers and objects (obbox.cpp and the colmath files are the
** unchanged upstream_draft_wwmath library, matrix3d.cpp is upstream_wwmath). No stand-ins; the CastResultStruct handed to the
** tests is a local one that is never read.
**
** Entry point (extern "C", tests/differential_draft_coltest.rs): ref_dpw_coltest(op, p, out) returns the number of words
** written to `out` (floats; booleans and ints as 0/1 floats). Parameter block p[]: AABox center 0..3, extent 3..6, move 6..9,
** second box center 9..12 extent 12..15 (Cull), min 15..18, max 18..21, Matrix3D tm (12, row major) 21..33, OBB basis
** (row major) 33..42, center 42..45, extent 45..48, translation 48..51, rotation type 51, collision type 52.
** Ops: 0 AABox ctor, 1 AABox Cull(box), 2 AABox Cull(min,max), 3 Translate, 4 Rotate, 5 Transform, 6 OBB ctor,
** 7 OBB(OBB test, tm), 8 OBB(AABox test, tm), 9 OBB Cull(box), 10 OBB Cull(min,max).
** State outputs: AABox test box 6, move 3, SweepMin 3, SweepMax 3, CollisionType (16 words);
** OBB test basis 9, center 3, extent 3, move 3, SweepMin 3, SweepMax 3, CollisionType (25 words). */
#include <string.h>
#include "coltest.h"
#include "matrix3d.h"

static int OutA(const AABoxCollisionTestClass &t, float *o)
{
	int n = 0;
	o[n++] = t.Box.Center.X; o[n++] = t.Box.Center.Y; o[n++] = t.Box.Center.Z;
	o[n++] = t.Box.Extent.X; o[n++] = t.Box.Extent.Y; o[n++] = t.Box.Extent.Z;
	o[n++] = t.Move.X; o[n++] = t.Move.Y; o[n++] = t.Move.Z;
	o[n++] = t.SweepMin.X; o[n++] = t.SweepMin.Y; o[n++] = t.SweepMin.Z;
	o[n++] = t.SweepMax.X; o[n++] = t.SweepMax.Y; o[n++] = t.SweepMax.Z;
	o[n++] = (float)t.CollisionType;
	return n;
}

static int OutO(const OBBoxCollisionTestClass &t, float *o)
{
	int n = 0;
	for (int i = 0; i < 3; i++)
		for (int j = 0; j < 3; j++)
			o[n++] = t.Box.Basis[i][j];
	o[n++] = t.Box.Center.X; o[n++] = t.Box.Center.Y; o[n++] = t.Box.Center.Z;
	o[n++] = t.Box.Extent.X; o[n++] = t.Box.Extent.Y; o[n++] = t.Box.Extent.Z;
	o[n++] = t.Move.X; o[n++] = t.Move.Y; o[n++] = t.Move.Z;
	o[n++] = t.SweepMin.X; o[n++] = t.SweepMin.Y; o[n++] = t.SweepMin.Z;
	o[n++] = t.SweepMax.X; o[n++] = t.SweepMax.Y; o[n++] = t.SweepMax.Z;
	o[n++] = (float)t.CollisionType;
	return n;
}

extern "C" int ref_dpw_coltest(int op, const float *p, float *out)
{
	CastResultStruct res;
	AABoxClass box(Vector3(p[0], p[1], p[2]), Vector3(p[3], p[4], p[5]));
	Vector3 mv(p[6], p[7], p[8]);
	AABoxClass box2(Vector3(p[9], p[10], p[11]), Vector3(p[12], p[13], p[14]));
	Vector3 mn(p[15], p[16], p[17]), mx(p[18], p[19], p[20]);
	const float *m = p + 21;
	Matrix3D tm(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
	Matrix3x3 basis(p[33], p[34], p[35], p[36], p[37], p[38], p[39], p[40], p[41]);
	OBBoxClass obb(Vector3(p[42], p[43], p[44]), Vector3(p[45], p[46], p[47]), basis);
	Vector3 tr(p[48], p[49], p[50]);
	int rot = (int)p[51];
	int type = (int)p[52];
	AABoxCollisionTestClass at(box, mv, &res, type);
	switch (op) {
	case 0: return OutA(at, out);
	case 1: out[0] = (float)at.Cull(box2); return 1;
	case 2: out[0] = (float)at.Cull(mn, mx); return 1;
	case 3: at.Translate(tr); return OutA(at, out);
	case 4: at.Rotate((AABoxCollisionTestClass::ROTATION_TYPE)rot); return OutA(at, out);
	case 5: at.Transform(tm); return OutA(at, out);
	case 6: { OBBoxCollisionTestClass t(obb, mv, &res, type); return OutO(t, out); }
	case 7: { OBBoxCollisionTestClass t(obb, mv, &res, type); OBBoxCollisionTestClass u(t, tm); return OutO(u, out); }
	case 8: { OBBoxCollisionTestClass u(at, tm); return OutO(u, out); }
	case 9: { OBBoxCollisionTestClass t(obb, mv, &res, type); out[0] = (float)t.Cull(box2); return 1; }
	case 10: { OBBoxCollisionTestClass t(obb, mv, &res, type); out[0] = (float)t.Cull(mn, mx); return 1; }
	}
	return -1;
}
