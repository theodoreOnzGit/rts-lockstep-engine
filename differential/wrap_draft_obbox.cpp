/* Differential-test wrapper for the draft src/drafts/wwlib/obbox.rs (#215), upstream commit 0a05454d:
**   Libraries/Source/WWVegas/WWMath/obbox.h + obbox.cpp (compiled UNCHANGED as its own translation unit).
** Stand-in: WWMath::Random_Float(void) (wwmath.cpp:78, `(float)(rand() & 0xFFF) / (float)0xFFF`; wwmath.cpp is not
** compiled) returns the next of the values the test sets with draft_obb_randoms(), so both sides draw the same sequence.
** Matrix3x3 and Matrix3D come from the unchanged matrix3.cpp (wrap_draft_matrix.cpp's library) and matrix3d.cpp.
**
** Parameter block p[] (the same offsets on the Rust side, tests/differential_draft_obbox.rs); a box is basis 9, center 3,
** extent 3 (15 floats): box0 0..15, box1 15..30, v0 30..33, v1 33..36, dt 36, axis 37..40, triangle V0 V1 V2 40..49, N
** 49..52, 8 points 52..76, params 76..79, Matrix3D tm 79..91, min extent 91, max extent 92. */
#include <string.h>
#include <stdlib.h>
#include "obbox.h"
#include "tri.h"
#include "matrix3.h"
#include "matrix3d.h"
#include "vector3.h"

static float g_rand[16];
static int g_rand_i = 0;

float WWMath::Random_Float(void)
{
	return g_rand[g_rand_i++ & 15];
}

extern "C" void draft_obb_randoms(const float *r, int n)
{
	for (int i = 0; i < n && i < 16; i++)
		g_rand[i] = r[i];
	g_rand_i = 0;
}

static OBBoxClass Box(const float *p)
{
	Matrix3x3 b(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8]);
	return OBBoxClass(Vector3(p[9], p[10], p[11]), Vector3(p[12], p[13], p[14]), b);
}
static int OutBox(const OBBoxClass &b, float *o)
{
	for (int i = 0; i < 3; i++)
		for (int j = 0; j < 3; j++)
			o[i * 3 + j] = b.Basis[i][j];
	o[9] = b.Center.X; o[10] = b.Center.Y; o[11] = b.Center.Z;
	o[12] = b.Extent.X; o[13] = b.Extent.Y; o[14] = b.Extent.Z;
	return 15;
}

extern "C" int draft_obb(int op, const float *p, float *out)
{
	OBBoxClass b0 = Box(p), b1 = Box(p + 15);
	Vector3 v0(p[30], p[31], p[32]), v1(p[33], p[34], p[35]);
	float dt = p[36];
	Vector3 axis(p[37], p[38], p[39]);
	Vector3 tv0(p[40], p[41], p[42]), tv1(p[43], p[44], p[45]), tv2(p[46], p[47], p[48]), tn(p[49], p[50], p[51]);
	switch (op) {
	case 0: { OBBoxClass b(Vector3(p[9], p[10], p[11]), Vector3(p[12], p[13], p[14])); return OutBox(b, out); }
	case 1: return OutBox(b0, out);
	case 2: out[0] = (float)(b0 == b1); return 1;
	case 3: out[0] = (float)(b0 != b1); return 1;
	case 4: {
		Vector3 pts[8];
		for (int i = 0; i < 8; i++) pts[i] = Vector3(p[52 + i * 3], p[53 + i * 3], p[54 + i * 3]);
		OBBoxClass b = b0;
		b.Init_From_Box_Points(pts, 8);
		return OutBox(b, out);
	}
	case 5: { OBBoxClass b = b0; b.Init_Random(p[91], p[92]); return OutBox(b, out); }
	case 6: out[0] = b0.Project_To_Axis(axis); return 1;
	case 7: out[0] = b0.Volume(); return 1;
	case 8: { float prm[3] = { p[76], p[77], p[78] }; Vector3 r; b0.Compute_Point(prm, &r); out[0] = r.X; out[1] = r.Y; out[2] = r.Z; return 3; }
	case 9: { Vector3 r; b0.Compute_Axis_Aligned_Extent(&r); out[0] = r.X; out[1] = r.Y; out[2] = r.Z; return 3; }
	case 10: {
		const float *m = p + 79;
		Matrix3D tm(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
		OBBoxClass r;
		OBBoxClass::Transform(tm, b0, &r);
		return OutBox(r, out);
	}
	case 11: out[0] = (float)Oriented_Boxes_Intersect(b0, b1); return 1;
	case 12: out[0] = (float)Oriented_Boxes_Collide(b0, v0, b1, v1, dt); return 1;
	case 13: {
		TriClass t;
		t.N = &tn; t.V[0] = &tv0; t.V[1] = &tv1; t.V[2] = &tv2;
		out[0] = (float)Oriented_Box_Intersects_Tri(b0, t);
		return 1;
	}
	}
	return -1;
}
