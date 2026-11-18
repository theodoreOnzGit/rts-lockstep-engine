/* Differential-test wrapper for the draft src/drafts/wwlib/frustum.rs (#215), upstream commit 0a05454d:
**   Libraries/Source/WWVegas/WWMath/frustum.h + frustum.cpp (compiled UNCHANGED as its own translation unit).
** No cut, no stand-in. Entry point draft_frustum_init(p, out): p = camera Matrix3D 12 floats (row-major), viewport min
** (2), viewport max (2), znear, zfar (18 floats); out = CameraTransform 12, Planes 6 x (N.x N.y N.z D) = 24, Corners 24,
** BoundMin 3, BoundMax 3 (66 floats). */
#include "frustum.h"
#include "matrix3d.h"
#include "vector2.h"

extern "C" int draft_frustum_init(const float *p, float *out)
{
	Matrix3D cam(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[11]);
	Vector2 vmin(p[12], p[13]), vmax(p[14], p[15]);
	FrustumClass f;
	f.Init(cam, vmin, vmax, p[16], p[17]);
	int k = 0;
	for (int i = 0; i < 3; i++)
		for (int j = 0; j < 4; j++)
			out[k++] = f.CameraTransform[i][j];
	for (int i = 0; i < 6; i++) {
		out[k++] = f.Planes[i].N.X;
		out[k++] = f.Planes[i].N.Y;
		out[k++] = f.Planes[i].N.Z;
		out[k++] = f.Planes[i].D;
	}
	for (int i = 0; i < 8; i++) {
		out[k++] = f.Corners[i].X;
		out[k++] = f.Corners[i].Y;
		out[k++] = f.Corners[i].Z;
	}
	out[k++] = f.Get_Bound_Min().X; out[k++] = f.Get_Bound_Min().Y; out[k++] = f.Get_Bound_Min().Z;
	out[k++] = f.Get_Bound_Max().X; out[k++] = f.Get_Bound_Max().Y; out[k++] = f.Get_Bound_Max().Z;
	return k;
}
