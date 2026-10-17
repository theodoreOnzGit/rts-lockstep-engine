/* Differential-test wrapper for WWMath (upstream commit 0a05454d):
**   Libraries/Source/WWVegas/WWMath/wwmath.h, wwmath.cpp (table init only),
**   vector2.h, vector3.h, vector4.h, matrix3d.h, matrix3d.cpp.
** The upstream headers are included unchanged; matrix3d.cpp is compiled
** unchanged as its own translation unit. g++ is not _MSC_VER, so wwmath.h
** takes its portable branches (Sin=sinf, Cos=cosf, Sqrt=sqrt,
** Inv_Sqrt=1/sqrt, Float_To_Long=(long)f) -- the same branches the Rust
** translation follows. See src/ww/wwmath.rs for what the shipped Win32 build
** did instead. Vectors cross the boundary as float[2|3|4], Matrix3D as
** float[12] in row-major order (Row[0].X .. Row[2].W). */
#include <string.h>
#include <stdlib.h>
#include "wwmath.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "D3dx8math.h"  /* shim: declares D3DXMatrixInverse for the stub below */

/* ---- wwmath.cpp ---------------------------------------------------------
** wwmath.cpp cannot be compiled as is: it pulls in lookuptable.h,
** wwprofile.h and wwhack.h, and its second loop reuses the first loop's
** `a` outside its scope (pre-standard MSVC for-scoping). The table
** definitions and the two loops of WWMath::Init (wwmath.cpp lines 44-71) are
** reproduced here verbatim, the only change being `int a` in the second
** loop. LookupTableMgrClass::Init() is not called (unrelated tables). */
float _FastAcosTable[ARC_TABLE_SIZE];
float _FastAsinTable[ARC_TABLE_SIZE];
float _FastSinTable[SIN_TABLE_SIZE];
float _FastInvSinTable[SIN_TABLE_SIZE];

static void shim_wwmath_init_tables(void)
{
	for (int a=0;a<ARC_TABLE_SIZE;++a) {
		float cv=float(a-ARC_TABLE_SIZE/2)*(1.0f/(ARC_TABLE_SIZE/2));
		_FastAcosTable[a]=acos(cv);
		_FastAsinTable[a]=asin(cv);
	}

	for (int a=0;a<SIN_TABLE_SIZE;++a) {
		float cv= (float)a * 2.0f * WWMATH_PI / SIN_TABLE_SIZE; //float(a-SIN_TABLE_SIZE/2)*(1.0f/(SIN_TABLE_SIZE/2));
		_FastSinTable[a]=sin(cv);

		if (a>0) {
			_FastInvSinTable[a]=1.0f/_FastSinTable[a];
		} else {
			_FastInvSinTable[a]=WWMATH_FLOAT_MAX;
		}
	}
}

/* ---- link stubs for matrix3d.cpp functions that are NOT translated ------ */
/* Get_Inverse calls the DirectX SDK (not in the upstream release). */
D3DXMATRIX* D3DXMatrixInverse(D3DXMATRIX*, float*, const D3DXMATRIX*) { abort(); }

static inline Vector3 v3(const float *p) { return Vector3(p[0], p[1], p[2]); }
static inline void put3(const Vector3 &v, float *p) { p[0] = v.X; p[1] = v.Y; p[2] = v.Z; }
static inline Vector2 v2(const float *p) { return Vector2(p[0], p[1]); }
static inline void put2(const Vector2 &v, float *p) { p[0] = v.X; p[1] = v.Y; }
static inline Vector4 v4(const float *p) { return Vector4(p[0], p[1], p[2], p[3]); }
static inline void put4(const Vector4 &v, float *p) { p[0] = v.X; p[1] = v.Y; p[2] = v.Z; p[3] = v.W; }
static inline Matrix3D m3(const float *p)
{
	return Matrix3D(p[0],p[1],p[2],p[3], p[4],p[5],p[6],p[7], p[8],p[9],p[10],p[11]);
}
static inline void putm(const Matrix3D &m, float *p)
{
	for (int i = 0; i < 3; ++i) for (int j = 0; j < 4; ++j) p[i*4+j] = m[i][j];
}

extern "C" {

/* ---- wwmath.h ---- */
void  ref_wwmath_init_tables(void) { shim_wwmath_init_tables(); }
float ref_fast_sin_table(int i) { return _FastSinTable[i]; }
float ref_fast_inv_sin_table(int i) { return _FastInvSinTable[i]; }
float ref_fast_acos_table(int i) { return _FastAcosTable[i]; }
float ref_fast_asin_table(int i) { return _FastAsinTable[i]; }

float ref_fabs(float v) { return WWMath::Fabs(v); }
int   ref_float_to_int_chop(float f) { return WWMath::Float_To_Int_Chop(f); }
int   ref_float_to_int_floor(float f) { return WWMath::Float_To_Int_Floor(f); }
float ref_cos(float v) { return WWMath::Cos(v); }
float ref_sin(float v) { return WWMath::Sin(v); }
float ref_sqrt(float v) { return WWMath::Sqrt(v); }
float ref_inv_sqrt(float v) { return WWMath::Inv_Sqrt(v); }
long long ref_float_to_long(float f) { return (long long)WWMath::Float_To_Long(f); }
long long ref_float_to_long_d(double f) { return (long long)WWMath::Float_To_Long(f); }
float ref_fast_sin(float v) { return WWMath::Fast_Sin(v); }
float ref_fast_inv_sin(float v) { return WWMath::Fast_Inv_Sin(v); }
float ref_fast_cos(float v) { return WWMath::Fast_Cos(v); }
float ref_fast_inv_cos(float v) { return WWMath::Fast_Inv_Cos(v); }
float ref_fast_acos(float v) { return WWMath::Fast_Acos(v); }
float ref_acos(float v) { return WWMath::Acos(v); }
float ref_fast_asin(float v) { return WWMath::Fast_Asin(v); }
float ref_asin(float v) { return WWMath::Asin(v); }
float ref_atan(float v) { return WWMath::Atan(v); }
float ref_atan2(float y, float x) { return WWMath::Atan2(y, x); }
float ref_sign(float v) { return WWMath::Sign(v); }
float ref_ceil(float v) { return WWMath::Ceil(v); }
float ref_floor(float v) { return WWMath::Floor(v); }
int   ref_fast_is_float_positive(float v) { return WWMath::Fast_Is_Float_Positive(v); }
int   ref_is_power_of_2(unsigned int v) { return WWMath::Is_Power_Of_2(v); }
float ref_clamp(float v, float lo, float hi) { return WWMath::Clamp(v, lo, hi); }
double ref_clamp_d(double v, double lo, double hi) { return WWMath::Clamp(v, lo, hi); }
int   ref_clamp_int(int v, int lo, int hi) { return WWMath::Clamp_Int(v, lo, hi); }
float ref_wrap(float v, float lo, float hi) { return WWMath::Wrap(v, lo, hi); }
double ref_wrap_d(double v, double lo, double hi) { return WWMath::Wrap(v, lo, hi); }
float ref_min(float a, float b) { return WWMath::Min(a, b); }
float ref_max(float a, float b) { return WWMath::Max(a, b); }
int   ref_float_as_int(float f) { return WWMath::Float_As_Int(f); }
float ref_lerp(float a, float b, float t) { return WWMath::Lerp(a, b, t); }
double ref_lerp_d(double a, double b, float t) { return WWMath::Lerp(a, b, t); }
unsigned char ref_unit_float_to_byte(float f) { return WWMath::Unit_Float_To_Byte(f); }
float ref_byte_to_unit_float(unsigned char b) { return WWMath::Byte_To_Unit_Float(b); }
int   ref_is_valid_float(float x) { return WWMath::Is_Valid_Float(x); }

/* ---- vector2.h ---- */
/* op: 0 +, 1 -, 2 v*k, 3 k*v, 4 v/k, 5 +=, 6 -=, 7 *=k, 8 /=k, 9 neg,
**     10 Scale(a.X,a.Y), 11 Scale(b), 12 Update_Min, 13 Update_Max,
**     14 Normalize(), 15 Rotate(theta=k), 16 Rotate(s=b.X,c=b.Y),
**     17 Lerp(a,b,k), 18 ::Normalize(a) */
void ref_v2_op(int op, const float *a, const float *b, float k, float *out)
{
	Vector2 A = v2(a), B = v2(b), R(0.0f, 0.0f);
	switch (op) {
	case 0: R = A + B; break;
	case 1: R = A - B; break;
	case 2: R = A * k; break;
	case 3: R = k * A; break;
	case 4: R = A / k; break;
	case 5: R = A; R += B; break;
	case 6: R = A; R -= B; break;
	case 7: R = A; R *= k; break;
	case 8: R = A; R /= k; break;
	case 9: R = -A; break;
	case 10: R = A; R.Scale(B.X, B.Y); break;
	case 11: R = A; R.Scale(B); break;
	case 12: R = A; R.Update_Min(B); break;
	case 13: R = A; R.Update_Max(B); break;
	case 14: R = A; R.Normalize(); break;
	case 15: R = A; R.Rotate(k); break;
	case 16: R = A; R.Rotate(B.X, B.Y); break;
	case 17: Vector2::Lerp(A, B, k, &R); break;
	case 18: R = Normalize(A); break;
	}
	put2(R, out);
}
/* op: 0 Dot_Product, 1 operator*, 2 Perp_Dot_Product, 3 Length, 4 Length2,
**     5 Distance, 6 Quick_Distance, 7 ::Distance(x1,y1,x2,y2),
**     8 ::Quick_Distance(x1,y1,x2,y2) */
float ref_v2_scalar(int op, const float *a, const float *b)
{
	Vector2 A = v2(a), B = v2(b);
	switch (op) {
	case 0: return Vector2::Dot_Product(A, B);
	case 1: return A * B;
	case 2: return Vector2::Perp_Dot_Product(A, B);
	case 3: return A.Length();
	case 4: return A.Length2();
	case 5: return Vector2::Distance(A, B);
	case 6: return Vector2::Quick_Distance(A, B);
	case 7: return Distance(A.X, A.Y, B.X, B.Y);
	case 8: return Quick_Distance(A.X, A.Y, B.X, B.Y);
	}
	return 0.0f;
}
/* op: 0 ==, 1 !=, 2 Equal_Within_Epsilon(k), 3 Is_Valid(a) */
int ref_v2_pred(int op, const float *a, const float *b, float k)
{
	Vector2 A = v2(a), B = v2(b);
	switch (op) {
	case 0: return A == B;
	case 1: return A != B;
	case 2: return Equal_Within_Epsilon(A, B, k);
	case 3: return A.Is_Valid();
	}
	return -1;
}
/* Rotate_Towards_Vector(target=b, max_s, max_c, positive_turn); returns
** (return_value | positive_turn << 1); `inout` holds this vector. */
int ref_v2_rotate_towards(float *inout, const float *target, float max_s, float max_c)
{
	Vector2 A = v2(inout), T = v2(target);
	bool positive = false;
	bool r = A.Rotate_Towards_Vector(T, max_s, max_c, positive);
	put2(A, inout);
	return (r ? 1 : 0) | (positive ? 2 : 0);
}
int ref_v2_rotate_towards_theta(float *inout, const float *target, float max_theta)
{
	Vector2 A = v2(inout), T = v2(target);
	bool positive = false;
	bool r = A.Rotate_Towards_Vector(T, max_theta, positive);
	put2(A, inout);
	return (r ? 1 : 0) | (positive ? 2 : 0);
}

/* ---- vector3.h ---- */
/* op: 0 +, 1 -, 2 v*k, 3 k*v, 4 v/k, 5 +=, 6 -=, 7 *=k, 8 /=k, 9 neg,
**     10 Normalize, 11 Scale(b), 12 Rotate_X(k), 13 Rotate_Y(k),
**     14 Rotate_Z(k), 15 Rotate_X(s=b.X,c=b.Y), 16 Rotate_Y(s,c),
**     17 Rotate_Z(s,c), 18 Cross_Product, 19 Normalized_Cross_Product,
**     20 Add, 21 Subtract, 22 Update_Min, 23 Update_Max,
**     24 Cap_Absolute_To, 25 Lerp(a,b,k) */
void ref_v3_op(int op, const float *a, const float *b, float k, float *out)
{
	Vector3 A = v3(a), B = v3(b), R(0.0f, 0.0f, 0.0f);
	switch (op) {
	case 0: R = A + B; break;
	case 1: R = A - B; break;
	case 2: R = A * k; break;
	case 3: R = k * A; break;
	case 4: R = A / k; break;
	case 5: R = A; R += B; break;
	case 6: R = A; R -= B; break;
	case 7: R = A; R *= k; break;
	case 8: R = A; R /= k; break;
	case 9: R = -A; break;
	case 10: R = A; R.Normalize(); break;
	case 11: R = A; R.Scale(B); break;
	case 12: R = A; R.Rotate_X(k); break;
	case 13: R = A; R.Rotate_Y(k); break;
	case 14: R = A; R.Rotate_Z(k); break;
	case 15: R = A; R.Rotate_X(B.X, B.Y); break;
	case 16: R = A; R.Rotate_Y(B.X, B.Y); break;
	case 17: R = A; R.Rotate_Z(B.X, B.Y); break;
	case 18: Vector3::Cross_Product(A, B, &R); break;
	case 19: Vector3::Normalized_Cross_Product(A, B, &R); break;
	case 20: Vector3::Add(A, B, &R); break;
	case 21: Vector3::Subtract(A, B, &R); break;
	case 22: R = A; R.Update_Min(B); break;
	case 23: R = A; R.Update_Max(B); break;
	case 24: R = A; R.Cap_Absolute_To(B); break;
	case 25: Vector3::Lerp(A, B, k, &R); break;
	}
	put3(R, out);
}
/* op: 0 Dot_Product, 1 Length, 2 Length2, 3 Quick_Length, 4 Distance,
**     5 Quick_Distance, 6 Cross_Product_X, 7 Cross_Product_Y,
**     8 Cross_Product_Z */
float ref_v3_scalar(int op, const float *a, const float *b)
{
	Vector3 A = v3(a), B = v3(b);
	switch (op) {
	case 0: return Vector3::Dot_Product(A, B);
	case 1: return A.Length();
	case 2: return A.Length2();
	case 3: return A.Quick_Length();
	case 4: return Vector3::Distance(A, B);
	case 5: return Vector3::Quick_Distance(A, B);
	case 6: return Vector3::Cross_Product_X(A, B);
	case 7: return Vector3::Cross_Product_Y(A, B);
	case 8: return Vector3::Cross_Product_Z(A, B);
	}
	return 0.0f;
}
/* Find_*_At_*: op 0 X_At_Y, 1 X_At_Z, 2 Y_At_X, 3 Y_At_Z, 4 Z_At_X, 5 Z_At_Y */
float ref_v3_find(int op, float v, const float *p1, const float *p2)
{
	Vector3 A = v3(p1), B = v3(p2);
	switch (op) {
	case 0: return Vector3::Find_X_At_Y(v, A, B);
	case 1: return Vector3::Find_X_At_Z(v, A, B);
	case 2: return Vector3::Find_Y_At_X(v, A, B);
	case 3: return Vector3::Find_Y_At_Z(v, A, B);
	case 4: return Vector3::Find_Z_At_X(v, A, B);
	case 5: return Vector3::Find_Z_At_Y(v, A, B);
	}
	return 0.0f;
}
/* op: 0 ==, 1 !=, 2 Equal_Within_Epsilon(k), 3 Is_Valid(a) */
int ref_v3_pred(int op, const float *a, const float *b, float k)
{
	Vector3 A = v3(a), B = v3(b);
	switch (op) {
	case 0: return A == B;
	case 1: return A != B;
	case 2: return Equal_Within_Epsilon(A, B, k);
	case 3: return A.Is_Valid();
	}
	return -1;
}

/* ---- vector4.h (the subset Matrix3D uses) ---- */
/* op: 0 +, 1 -, 2 v*k, 3 k*v, 4 v/k, 5 +=, 6 -=, 7 *=k, 8 /=k, 9 neg */
void ref_v4_op(int op, const float *a, const float *b, float k, float *out)
{
	Vector4 A = v4(a), B = v4(b), R(0.0f, 0.0f, 0.0f, 0.0f);
	switch (op) {
	case 0: R = A + B; break;
	case 1: R = A - B; break;
	case 2: R = A * k; break;
	case 3: R = k * A; break;
	case 4: R = A / k; break;
	case 5: R = A; R += B; break;
	case 6: R = A; R -= B; break;
	case 7: R = A; R *= k; break;
	case 8: R = A; R /= k; break;
	case 9: R = -A; break;
	}
	put4(R, out);
}
float ref_v4_dot(const float *a, const float *b) { return v4(a) * v4(b); }
int ref_v4_eq(const float *a, const float *b) { return (v4(a) == v4(b)) | ((v4(a) != v4(b)) << 1); }

/* ---- matrix3d.h / matrix3d.cpp ---- */
/* Unary in-place ops on m (12 floats), parameters in p[]:
** 0 Make_Identity, 1 Translate(p0,p1,p2), 2 Translate(Vector3 p), 3 Translate_X,
** 4 Translate_Y, 5 Translate_Z, 6 Rotate_X(theta), 7 Rotate_Y(theta),
** 8 Rotate_Z(theta), 9 Rotate_X(s,c), 10 Rotate_Y(s,c), 11 Rotate_Z(s,c),
** 12 Scale(k), 13 Scale(x,y,z), 14 Scale(Vector3&), 15-17 Pre_Rotate_X/Y/Z(theta),
** 18-20 Pre_Rotate_X/Y/Z(s,c), 21-23 In_Place_Pre_Rotate_X/Y/Z(theta),
** 24-26 In_Place_Pre_Rotate_X/Y/Z(s,c), 27 Re_Orthogonalize,
** 28 Set(axis=p0..2, angle=p3), 29 Set(axis=p0..2, s=p3, c=p4),
** 30 Set(position p0..2), 31 Set(x,y,z,pos = p0..11),
** 32 Look_At(p=p0..2, t=p3..5, roll=p6), 33 Obj_Look_At(same),
** 34 buildTransformMatrix(pos=p0..2, dir=p3..5), 35 Set_Translation(p0..2),
** 36 Adjust_Translation(p0..2), 37 Adjust_X/Y/Z_Translation(p0,p1,p2),
** 38 Copy_3x3_Matrix(p0..8), 39 Get_Orthogonal_Inverse (m := inverse(m)),
** 40 postMul(that = p0..11), 41 preMul(that = p0..11) */
void ref_m3_op(int op, float *m, const float *p)
{
	Matrix3D M = m3(m);
	Vector3 a(p[0], p[1], p[2]);
	switch (op) {
	case 0: M.Make_Identity(); break;
	case 1: M.Translate(p[0], p[1], p[2]); break;
	case 2: M.Translate(a); break;
	case 3: M.Translate_X(p[0]); break;
	case 4: M.Translate_Y(p[0]); break;
	case 5: M.Translate_Z(p[0]); break;
	case 6: M.Rotate_X(p[0]); break;
	case 7: M.Rotate_Y(p[0]); break;
	case 8: M.Rotate_Z(p[0]); break;
	case 9: M.Rotate_X(p[0], p[1]); break;
	case 10: M.Rotate_Y(p[0], p[1]); break;
	case 11: M.Rotate_Z(p[0], p[1]); break;
	case 12: M.Scale(p[0]); break;
	case 13: M.Scale(p[0], p[1], p[2]); break;
	case 14: M.Scale(a); break;
	case 15: M.Pre_Rotate_X(p[0]); break;
	case 16: M.Pre_Rotate_Y(p[0]); break;
	case 17: M.Pre_Rotate_Z(p[0]); break;
	case 18: M.Pre_Rotate_X(p[0], p[1]); break;
	case 19: M.Pre_Rotate_Y(p[0], p[1]); break;
	case 20: M.Pre_Rotate_Z(p[0], p[1]); break;
	case 21: M.In_Place_Pre_Rotate_X(p[0]); break;
	case 22: M.In_Place_Pre_Rotate_Y(p[0]); break;
	case 23: M.In_Place_Pre_Rotate_Z(p[0]); break;
	case 24: M.In_Place_Pre_Rotate_X(p[0], p[1]); break;
	case 25: M.In_Place_Pre_Rotate_Y(p[0], p[1]); break;
	case 26: M.In_Place_Pre_Rotate_Z(p[0], p[1]); break;
	case 27: M.Re_Orthogonalize(); break;
	case 28: M.Set(a, p[3]); break;
	case 29: M.Set(a, p[3], p[4]); break;
	case 30: M.Set(a); break;
	case 31: M.Set(Vector3(p[0],p[1],p[2]), Vector3(p[3],p[4],p[5]), Vector3(p[6],p[7],p[8]), Vector3(p[9],p[10],p[11])); break;
	case 32: M.Look_At(a, Vector3(p[3],p[4],p[5]), p[6]); break;
	case 33: M.Obj_Look_At(a, Vector3(p[3],p[4],p[5]), p[6]); break;
	case 34: M.buildTransformMatrix(a, Vector3(p[3],p[4],p[5])); break;
	case 35: M.Set_Translation(a); break;
	case 36: M.Adjust_Translation(a); break;
	case 37: M.Adjust_X_Translation(p[0]); M.Adjust_Y_Translation(p[1]); M.Adjust_Z_Translation(p[2]); break;
	case 38: { float t[3][3]; for (int i=0;i<3;++i) for (int j=0;j<3;++j) t[i][j]=p[i*3+j]; M.Copy_3x3_Matrix(t); } break;
	case 39: { Matrix3D inv(m3(m)); M.Get_Orthogonal_Inverse(inv); M = inv; } break;
	case 40: M.postMul(m3(p)); break;
	case 41: M.preMul(m3(p)); break;
	}
	putm(M, m);
}
/* Matrix3D::Multiply(A,B,&R); alias: 0 none, 1 R==&A, 2 R==&B */
void ref_m3_multiply(const float *a, const float *b, int alias, float *out)
{
	Matrix3D A = m3(a), B = m3(b), R;
	if (alias == 1) { Matrix3D::Multiply(A, B, &A); R = A; }
	else if (alias == 2) { Matrix3D::Multiply(A, B, &B); R = B; }
	else Matrix3D::Multiply(A, B, &R);
	putm(R, out);
}
/* Vector ops: 0 Transform_Vector, 1 static Rotate_Vector, 2 Inverse_Transform_Vector,
** 3 static Inverse_Rotate_Vector, 4 member Rotate_Vector, 5 member Inverse_Rotate_Vector,
** 6 mulVector3(in,out), 7 mulVector3(inout), 8 mulVector3Array(in,out,1),
** 9 mulVector3Array(inout,1), 10 Transform_Vector aliased (out == &in),
** 11 Get_X_Vector, 12 Get_Y_Vector, 13 Get_Z_Vector, 14 Get_Translation */
void ref_m3_vec(int op, const float *m, const float *v, float *out)
{
	Matrix3D M = m3(m);
	Vector3 V = v3(v), R(0.0f, 0.0f, 0.0f);
	switch (op) {
	case 0: Matrix3D::Transform_Vector(M, V, &R); break;
	case 1: Matrix3D::Rotate_Vector(M, V, &R); break;
	case 2: Matrix3D::Inverse_Transform_Vector(M, V, &R); break;
	case 3: Matrix3D::Inverse_Rotate_Vector(M, V, &R); break;
	case 4: R = M.Rotate_Vector(V); break;
	case 5: R = M.Inverse_Rotate_Vector(V); break;
	case 6: M.mulVector3(V, R); break;
	case 7: R = V; M.mulVector3(R); break;
	case 8: M.mulVector3Array(&V, &R, 1); break;
	case 9: R = V; M.mulVector3Array(&R, 1); break;
	case 10: R = V; Matrix3D::Transform_Vector(M, R, &R); break;
	case 11: R = M.Get_X_Vector(); break;
	case 12: R = M.Get_Y_Vector(); break;
	case 13: R = M.Get_Z_Vector(); break;
	case 14: R = M.Get_Translation(); break;
	}
	put3(R, out);
}
/* 0 Get_X_Rotation, 1 Get_Y_Rotation, 2 Get_Z_Rotation, 3 Is_Orthogonal */
float ref_m3_scalar(int op, const float *m)
{
	Matrix3D M = m3(m);
	switch (op) {
	case 0: return M.Get_X_Rotation();
	case 1: return M.Get_Y_Rotation();
	case 2: return M.Get_Z_Rotation();
	case 3: return (float)M.Is_Orthogonal();
	}
	return 0.0f;
}
/* Transform_Min_Max_AABox (op 0) / Transform_Center_Extent_AABox (op 1):
** inputs a, b; outputs out[0..2], out[3..5]. */
void ref_m3_aabox(int op, const float *m, const float *a, const float *b, float *out)
{
	Matrix3D M = m3(m);
	Vector3 r0(0.0f, 0.0f, 0.0f), r1(0.0f, 0.0f, 0.0f);
	if (op == 0) M.Transform_Min_Max_AABox(v3(a), v3(b), &r0, &r1);
	else M.Transform_Center_Extent_AABox(v3(a), v3(b), &r0, &r1);
	put3(r0, out); put3(r1, out + 3);
}
int ref_m3_solve(float *m)
{
	Matrix3D M = m3(m);
	bool ok = Matrix3D::Solve_Linear_System(M);
	putm(M, m);
	return ok;
}
int ref_m3_eq(const float *a, const float *b) { return (m3(a) == m3(b)) | ((m3(a) != m3(b)) << 1); }
/* 0 Identity, 1 RotateX90, 2 RotateX180, 3 RotateX270, 4 RotateY90, 5 RotateY180,
** 6 RotateY270, 7 RotateZ90, 8 RotateZ180, 9 RotateZ270 */
void ref_m3_static(int which, float *out)
{
	const Matrix3D *t[10] = { &Matrix3D::Identity, &Matrix3D::RotateX90, &Matrix3D::RotateX180,
		&Matrix3D::RotateX270, &Matrix3D::RotateY90, &Matrix3D::RotateY180, &Matrix3D::RotateY270,
		&Matrix3D::RotateZ90, &Matrix3D::RotateZ180, &Matrix3D::RotateZ270 };
	putm(*t[which], out);
}

} /* extern "C" */

/* ---- glibc's own libm, for MEASURING (not gating) libm-crate differences.
** The differential test binaries interpose sinf/cosf/sincosf/acosf/asinf/
** atanf/atan2f with the Rust `libm` crate so that the gated tests verify the
** translation with identical elementary functions on both sides (see
** tests/support/mod.rs). These look the real glibc functions up by handle,
** which bypasses that interposition. */
#include <dlfcn.h>
static void *glibc_libm(void)
{
	static void *h = 0;
	if (!h) h = dlopen("libm.so.6", RTLD_NOW);
	if (!h) abort();
	return h;
}
extern "C" float ref_glibc_f1(int which, float x)
{
	typedef float (*F)(float);
	static F f[5] = { 0, 0, 0, 0, 0 };
	static const char *names[5] = { "sinf", "cosf", "acosf", "asinf", "atanf" };
	if (which < 0 || which > 4) abort();
	if (!f[which]) f[which] = (F)dlsym(glibc_libm(), names[which]);
	if (!f[which]) abort();
	return f[which](x);
}
extern "C" float ref_glibc_atan2f(float y, float x)
{
	typedef float (*F)(float, float);
	static F f = 0;
	if (!f) f = (F)dlsym(glibc_libm(), "atan2f");
	if (!f) abort();
	return f(y, x);
}
