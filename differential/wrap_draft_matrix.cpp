/* Differential-test wrapper for the drafts src/drafts/wwlib/matrix3.rs and matrix4.rs (#215), upstream commit 0a05454d:
**   Libraries/Source/WWVegas/WWMath/matrix3.h + matrix3.cpp and matrix4.h + matrix4.cpp.
** Both .cpp files are compiled UNCHANGED as their own translation units; the headers are included unchanged. Nothing is
** cut or stood in: matrix3d.cpp and quat.cpp are the ones already linked (upstream_wwmath, the W3D readers), and
** the floating-point functions (sinf, cosf, tan, atan2f ...) are the ones the test binary interposes (tests/support).
** All matrices cross as float arrays in row-major order: Matrix3x3 9, Matrix3D 12, Matrix4x4 16.
**
** One parameter block p[] (the same offsets on the Rust side, tests/differential_draft_matrix.rs):
**   a 0..9, b 9..18, v 18..21, q 21..25 (x y z w), m3d 25..37, m4 37..53, s0 53, s1 54, w 55..59 (a Vector4), u 59..62.
** Each entry returns the number of floats written to out[]. */
#include <string.h>
#include <stdlib.h>
#include "matrix3.h"
#include "matrix4.h"
#include "matrix3d.h"
#include "quat.h"
#include "vector3.h"
#include "vector4.h"

/* matrix3.cpp's `operator * (Matrix3D, Matrix3x3)` and `operator * (Matrix3x3, Matrix3D)`, VERBATIM (m3_ops.inc, cut by
** build.rs). The friend declarations in matrix3.h say WWINLINE, which is `inline` here (and `__forceinline` on MSVC), so
** the definitions in matrix3.cpp are never emitted for another translation unit to call: they are only reachable from
** a translation unit that has the definition, as this one does. */
#include "m3_ops.inc"

static Matrix3x3 M3(const float *a)
{
	return Matrix3x3(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8]);
}
static int O3(const Matrix3x3 &m, float *o)
{
	for (int i = 0; i < 3; i++)
		for (int j = 0; j < 3; j++)
			o[i * 3 + j] = m[i][j];
	return 9;
}
static Matrix4x4 M4(const float *a)
{
	return Matrix4x4(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15]);
}
static int O4(const Matrix4x4 &m, float *o)
{
	for (int i = 0; i < 4; i++)
		for (int j = 0; j < 4; j++)
			o[i * 4 + j] = m[i][j];
	return 16;
}
static Matrix3D MD(const float *a)
{
	return Matrix3D(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]);
}
static int OV3(const Vector3 &v, float *o)
{
	o[0] = v.X; o[1] = v.Y; o[2] = v.Z;
	return 3;
}
static int OV4(const Vector4 &v, float *o)
{
	o[0] = v.X; o[1] = v.Y; o[2] = v.Z; o[3] = v.W;
	return 4;
}
static int OF(float f, float *o)
{
	o[0] = f;
	return 1;
}

/* the constants: Identity, RotateX90 .. RotateZ270 (index 0..9) */
extern "C" int draft_m3_constant(int k, float *out)
{
	const Matrix3x3 *c[10] = { &Matrix3x3::Identity, &Matrix3x3::RotateX90, &Matrix3x3::RotateX180, &Matrix3x3::RotateX270,
		&Matrix3x3::RotateY90, &Matrix3x3::RotateY180, &Matrix3x3::RotateY270, &Matrix3x3::RotateZ90, &Matrix3x3::RotateZ180,
		&Matrix3x3::RotateZ270 };
	return O3(*c[k], out);
}

extern "C" int draft_m3(int op, const float *p, float *out)
{
	const float *a = p + 0, *b = p + 9, *v = p + 18, *q = p + 21, *md = p + 25, *m4 = p + 37;
	float s0 = p[53], s1 = p[54];
	Vector3 axis(v[0], v[1], v[2]);
	Quaternion quat(q[0], q[1], q[2], q[3]);
	switch (op) {
	case 0: { Matrix3x3 m(true); return O3(m, out); }
	case 1: return O3(M3(a), out);
	case 2: { Matrix3x3 m(Vector3(a[0], a[1], a[2]), Vector3(a[3], a[4], a[5]), Vector3(a[6], a[7], a[8])); return O3(m, out); }
	case 3: { Matrix3x3 m(axis, s0); return O3(m, out); }
	case 4: { Matrix3x3 m(axis, s0, s1); return O3(m, out); }
	case 5: { Matrix3x3 m(quat); return O3(m, out); }
	case 6: { Matrix3D d = MD(md); Matrix3x3 m(d); return O3(m, out); }
	case 7: { Matrix4x4 d = M4(m4); Matrix3x3 m(d); return O3(m, out); }
	case 8: { Matrix3D d = MD(md); Matrix3x3 m = M3(a); m.Set(d); return O3(m, out); }
	case 9: { Matrix4x4 d = M4(m4); Matrix3x3 m = M3(a); m.Set(d); return O3(m, out); }
	case 10: { Matrix3x3 m = M3(a); m.Set(axis, s0); return O3(m, out); }
	case 11: { Matrix3x3 m = M3(a); m.Set(axis, s0, s1); return O3(m, out); }
	case 12: { Matrix3x3 m = M3(a); m.Set(quat); return O3(m, out); }
	case 13: return O3(M3(a).Transpose(), out);
	case 14: return O3(M3(a).Inverse(), out);
	case 15: return OF(M3(a).Determinant(), out);
	case 16: { Matrix3x3 m = M3(a); m += M3(b); return O3(m, out); }
	case 17: { Matrix3x3 m = M3(a); m -= M3(b); return O3(m, out); }
	case 18: { Matrix3x3 m = M3(a); m *= s0; return O3(m, out); }
	case 19: { Matrix3x3 m = M3(a); m /= s0; return O3(m, out); }
	case 20: { Matrix3x3 m = M3(a); m.Make_Identity(); return O3(m, out); }
	case 21: { Matrix3x3 m = M3(a); m.Rotate_X(s0); return O3(m, out); }
	case 22: { Matrix3x3 m = M3(a); m.Rotate_X(s0, s1); return O3(m, out); }
	case 23: { Matrix3x3 m = M3(a); m.Rotate_Y(s0); return O3(m, out); }
	case 24: { Matrix3x3 m = M3(a); m.Rotate_Y(s0, s1); return O3(m, out); }
	case 25: { Matrix3x3 m = M3(a); m.Rotate_Z(s0); return O3(m, out); }
	case 26: { Matrix3x3 m = M3(a); m.Rotate_Z(s0, s1); return O3(m, out); }
	case 27: return OF(M3(a).Get_X_Rotation(), out);
	case 28: return OF(M3(a).Get_Y_Rotation(), out);
	case 29: return OF(M3(a).Get_Z_Rotation(), out);
	case 30: return OV3(M3(a).Get_X_Vector(), out);
	case 31: return OV3(M3(a).Get_Y_Vector(), out);
	case 32: return OV3(M3(a).Get_Z_Vector(), out);
	case 33: { Vector3 r; M3(a).Get_X_Vector(&r); return OV3(r, out); }
	case 34: { Vector3 r; M3(a).Get_Y_Vector(&r); return OV3(r, out); }
	case 35: { Vector3 r; M3(a).Get_Z_Vector(&r); return OV3(r, out); }
	case 36: return O3(-M3(a), out);
	case 37: return O3(M3(a) * s0, out);
	case 38: return O3(s0 * M3(a), out);
	case 39: return O3(M3(a) / s0, out);
	case 40: return O3(M3(a) + M3(b), out);
	case 41: { Matrix3x3 r; Matrix3x3::Add(M3(a), M3(b), &r); return O3(r, out); }
	case 42: return O3(M3(a) - M3(b), out);
	case 43: { Matrix3x3 r; Matrix3x3::Subtract(M3(a), M3(b), &r); return O3(r, out); }
	case 44: return O3(M3(a) * M3(b), out);
	case 45: return O3(MD(md) * M3(b), out);
	case 46: return O3(M3(a) * MD(md), out);
	case 47: { Matrix3x3 r; Matrix3x3::Multiply(M3(a), M3(b), &r); return O3(r, out); }
	case 48: { Matrix3x3 r = M3(a); Matrix3x3::Multiply(r, M3(b), &r); return O3(r, out); }   /* result aliases A */
	case 49: { Matrix3x3 r; Matrix3x3::Multiply(MD(md), M3(b), &r); return O3(r, out); }
	case 50: { Matrix3x3 r; Matrix3x3::Multiply(M3(a), MD(md), &r); return O3(r, out); }
	case 51: return OV3(M3(a) * axis, out);
	case 52: { Vector3 r; Matrix3x3::Rotate_Vector(M3(a), axis, &r); return OV3(r, out); }
	case 53: { Vector3 r; Matrix3x3::Transpose_Rotate_Vector(M3(a), axis, &r); return OV3(r, out); }
	case 54: { Vector3 r = axis; Matrix3x3::Rotate_Vector(M3(a), r, &r); return OV3(r, out); }   /* aliased */
	case 55: { Vector3 r = axis; Matrix3x3::Transpose_Rotate_Vector(M3(a), r, &r); return OV3(r, out); }   /* aliased */
	case 56: out[0] = (float)M3(a).Is_Orthogonal(); return 1;
	case 57: { Matrix3x3 m = M3(a); m.Re_Orthogonalize(); return O3(m, out); }
	case 58: { Matrix3x3 m = M3(a); Vector3 r; m.Rotate_AABox_Extent(axis, &r); return OV3(r, out); }
	case 59: return O3(Create_X_Rotation_Matrix3(s0, s1), out);
	case 60: return O3(Create_X_Rotation_Matrix3(s0), out);
	case 61: return O3(Create_Y_Rotation_Matrix3(s0, s1), out);
	case 62: return O3(Create_Y_Rotation_Matrix3(s0), out);
	case 63: return O3(Create_Z_Rotation_Matrix3(s0, s1), out);
	case 64: return O3(Create_Z_Rotation_Matrix3(s0), out);
	case 65: out[0] = (float)(M3(a) == M3(b)); return 1;
	case 66: out[0] = (float)(M3(a) != M3(b)); return 1;
	case 67: { Matrix3x3 m = M3(a); Matrix3D d = MD(md); m = d; return O3(m, out); }   /* operator = (Matrix3D) */
	case 68: { Matrix3x3 m = M3(a); Matrix4x4 d = M4(m4); m = d; return O3(m, out); }  /* operator = (Matrix4x4) */
	}
	return -1;
}

extern "C" int draft_m4(int op, const float *p, float *out)
{
	/* a, b are the 4x4s at m4 (37..53) and p[0..16] (a overlaps the Matrix3x3 slots on purpose: the Rust side lays them out) */
	const float *a = p + 0, *b = p + 16, *v = p + 32, *md = p + 36, *w = p + 48;
	float s0 = p[52], s1 = p[53], s2 = p[54], s3 = p[55], s4 = p[56], s5 = p[57];
	Vector3 v3(v[0], v[1], v[2]);
	Vector4 v4(w[0], w[1], w[2], w[3]);
	switch (op) {
	case 0: { Matrix4x4 m(true); return O4(m, out); }
	case 1: return O4(M4(a), out);
	case 2: { Matrix4x4 m(Vector4(a[0], a[1], a[2], a[3]), Vector4(a[4], a[5], a[6], a[7]), Vector4(a[8], a[9], a[10], a[11]), Vector4(a[12], a[13], a[14], a[15])); return O4(m, out); }
	case 3: { Matrix3D d = MD(md); Matrix4x4 m(d); return O4(m, out); }
	case 4: { Matrix4x4 m = M4(a); m.Make_Identity(); return O4(m, out); }
	case 5: { Matrix4x4 m = M4(a); Matrix3D d = MD(md); m.Init(d); return O4(m, out); }
	case 6: { Matrix4x4 m = M4(a); m.Init(Vector4(b[0], b[1], b[2], b[3]), Vector4(b[4], b[5], b[6], b[7]), Vector4(b[8], b[9], b[10], b[11]), Vector4(b[12], b[13], b[14], b[15])); return O4(m, out); }
	case 7: { Matrix4x4 m = M4(a); m.Init(b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]); return O4(m, out); }
	case 8: { Matrix4x4 m = M4(a); m.Init_Ortho(s0, s1, s2, s3, s4, s5); return O4(m, out); }
	case 9: { Matrix4x4 m = M4(a); m.Init_Perspective(s0, s1, s2, s3); return O4(m, out); }
	case 10: { Matrix4x4 m = M4(a); m.Init_Perspective(s0, s1, s2, s3, s4, s5); return O4(m, out); }
	case 11: return O4(M4(a).Transpose(), out);
	case 12: return O4(M4(a).Inverse(), out);
	case 13: { Matrix4x4 m = M4(a); m += M4(b); return O4(m, out); }
	case 14: { Matrix4x4 m = M4(a); m -= M4(b); return O4(m, out); }
	case 15: { Matrix4x4 m = M4(a); m *= s0; return O4(m, out); }
	case 16: { Matrix4x4 m = M4(a); m /= s0; return O4(m, out); }
	case 17: return O4(-M4(a), out);
	case 18: return O4(M4(a) * s0, out);
	case 19: return O4(s0 * M4(a), out);
	case 20: return O4(M4(a) / s0, out);
	case 21: return O4(M4(a) + M4(b), out);
	case 22: return O4(Add(M4(a), M4(b)), out);
	case 23: return O4(M4(a) - M4(b), out);
	case 24: return O4(Subtract(M4(a), M4(b)), out);
	case 25: return O4(M4(a) * M4(b), out);
	case 26: return O4(Multiply(M4(a), M4(b)), out);
	case 27: return O4(M4(a) * MD(md), out);
	case 28: return O4(MD(md) * M4(b), out);
	case 29: { Matrix4x4 r; Matrix4x4::Multiply(M4(a), M4(b), &r); return O4(r, out); }
	case 30: { Matrix4x4 r; Matrix4x4::Multiply(MD(md), M4(b), &r); return O4(r, out); }
	case 31: { Matrix4x4 r; Matrix4x4::Multiply(M4(a), MD(md), &r); return O4(r, out); }
	case 32: return OV4(M4(a) * v4, out);
	case 33: return OV4(M4(a) * v3, out);
	case 34: { Vector3 r; Matrix4x4::Transform_Vector(M4(a), v3, &r); return OV3(r, out); }
	case 35: { Vector4 r; Matrix4x4::Transform_Vector(M4(a), v3, &r); return OV4(r, out); }
	case 36: { Vector4 r; Matrix4x4::Transform_Vector(M4(a), v4, &r); return OV4(r, out); }
	case 37: { Vector3 r = v3; Matrix4x4::Transform_Vector(M4(a), r, &r); return OV3(r, out); }
	case 38: { Vector4 r = v4; Matrix4x4::Transform_Vector(M4(a), r, &r); return OV4(r, out); }
	case 39: out[0] = (float)(M4(a) == M4(b)); return 1;
	case 40: out[0] = (float)(M4(a) != M4(b)); return 1;
	}
	return -1;
}
