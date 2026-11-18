/* Differential-test wrapper for the Euler angles draft (#215, upstream commit 0a05454d), src/drafts/wwlib/euler.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/euler.cpp and euler.h (EulerAnglesClass: the constructor, From_Matrix,
**   To_Matrix, Get_Angle, and the static _euler_* helpers they call), #included UNCHANGED, with matrix3d.h, quat.h and wwmath.h.
**   The Matrix3D methods it calls (Make_Identity ...) are matrix3d.cpp, compiled unchanged by build.rs's upstream_wwmath library,
**   which links after this one.
** Stand-ins: <stddef.h> read first (always.h uses size_t), lower-case forwarders (shim_draft_codec) for MSVC's case-insensitive file
** names, and the include order of the other WWMath wrappers (differential/shim's always.h, wwdebug.h).
**
** Entry points (extern "C", tests/differential_draft_curves.rs):
**   ref_euler_helpers(order, out)  the static _euler_axis_i/j/k/h, _euler_unpack_order (i j k h n s f) and the macros EULER_FRAME,
**                                  EULER_REPEAT, EULER_PARITY of euler.cpp, which this translation unit sees as euler.cpp is #included
**   ref_euler_orders(out)          the 24 EulerOrder* globals, in the order of the header's declarations the matrix is 12 floats as bits, row-major 3x4 (each row's x, y, z,
** translation), as Matrix3D::Matrix3D(m11 ... m34).
**   ref_euler_from_matrix(m, order, angles)  EulerAnglesClass(m, order): the three angles as doubles (bits, low word first)
**   ref_euler_round_trip(m, order, angles, back)  the same, then To_Matrix of that object into `back` (12 floats as bits) */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/euler.cpp"

static float realOf(int i) { float f; memcpy(&f, &i, sizeof f); return f; }
static int bitsOf(float f) { int i; memcpy(&i, &f, sizeof i); return i; }

extern "C" {

void ref_euler_from_matrix(const int *m, int order, unsigned long long *angles)
{
	Matrix3D M(realOf(m[0]), realOf(m[1]), realOf(m[2]), realOf(m[3]), realOf(m[4]), realOf(m[5]), realOf(m[6]), realOf(m[7]), realOf(m[8]), realOf(m[9]), realOf(m[10]), realOf(m[11]));
	EulerAnglesClass e(M, order);
	for (int i = 0; i < 3; i++) {
		double a = e.Get_Angle(i);
		memcpy(&angles[i], &a, 8);
	}
}

void ref_euler_round_trip(const int *m, int order, unsigned long long *angles, int *back)
{
	Matrix3D M(realOf(m[0]), realOf(m[1]), realOf(m[2]), realOf(m[3]), realOf(m[4]), realOf(m[5]), realOf(m[6]), realOf(m[7]), realOf(m[8]), realOf(m[9]), realOf(m[10]), realOf(m[11]));
	EulerAnglesClass e(M, order);
	for (int i = 0; i < 3; i++) {
		double a = e.Get_Angle(i);
		memcpy(&angles[i], &a, 8);
	}
	Matrix3D R;
	e.To_Matrix(R);
	for (int r = 0; r < 3; r++)
		for (int c = 0; c < 4; c++)
			back[r * 4 + c] = bitsOf(R[r][c]);
}


void ref_euler_helpers(int order, int *out)
{
	int i, j, k, h, n, s, f;
	_euler_unpack_order(order, i, j, k, h, n, s, f);
	int v[14] = {_euler_axis_i(order), _euler_axis_j(order), _euler_axis_k(order), _euler_axis_h(order), i, j, k, h, n, s, f, (int)EULER_FRAME(order), (int)EULER_REPEAT(order), (int)EULER_PARITY(order)};
	for (int q = 0; q < 14; q++) out[q] = v[q];
}

void ref_euler_orders(int *out)
{
	int v[24] = {EulerOrderXYZs, EulerOrderXYXs, EulerOrderXZYs, EulerOrderXZXs, EulerOrderYZXs, EulerOrderYZYs, EulerOrderYXZs, EulerOrderYXYs, EulerOrderZXYs, EulerOrderZXZs, EulerOrderZYXs, EulerOrderZYZs,
	             EulerOrderXYZr, EulerOrderXYXr, EulerOrderXZYr, EulerOrderXZXr, EulerOrderYZXr, EulerOrderYZYr, EulerOrderYXZr, EulerOrderYXYr, EulerOrderZXYr, EulerOrderZXZr, EulerOrderZYXr, EulerOrderZYZr};
	for (int q = 0; q < 24; q++) out[q] = v[q];
}

}
