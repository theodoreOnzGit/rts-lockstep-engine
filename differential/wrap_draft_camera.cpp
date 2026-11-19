/* Differential-test wrapper for src/drafts/sweep_ww3d/camera.rs (GitHub #215, group sweepww3d; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_sweepww3d`, anchors asserted to occur once), from WW3D2/camera.cpp:
**   CameraClass::Transform_To_View_Space, Rotate_To_View_Space, Get_Near_Clip_Bounding_Box, Get_Horizontal_FOV,
**   Get_Vertical_FOV, Get_Aspect_Ratio, Get_D3D_Projection_Matrix, Convert_Old, Compute_Projected_Sphere_Radius.
** Stand-in: CameraClass has only the fields these read (mutable CameraInvTransform, NearClipBBox, ProjectionTransform as in
** camera.h; ViewPlane is two Vector2, upstream's ViewportClass Min and Max) and Update_Frustum, which counts its calls.
** Entry point: ref_dsww3d_cam_run(in, nin, out, cap), the state words then the ops are listed in tests/differential_draft_camera.rs. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "matrix3.h"
#include "matrix4.h"
#include "obbox.h"
#include "wwmath.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static int g_update_calls = 0;

struct DraftViewPlane { Vector2 Min; Vector2 Max; };

class CameraClass
{
public:
	enum ProjectionType { PERSPECTIVE, ORTHO };
	void Transform_To_View_Space(Vector3 & dest,const Vector3 & ws_point) const;
	void Rotate_To_View_Space(Vector3 & dest,const Vector3 & ws_vector) const;
	const OBBoxClass & Get_Near_Clip_Bounding_Box(void) const;
	float Get_Horizontal_FOV(void) const;
	float Get_Vertical_FOV(void) const;
	float Get_Aspect_Ratio(void) const;
	void Get_D3D_Projection_Matrix(Matrix4x4 * set_tm);
	void Convert_Old(Vector3 &pos);
	float Compute_Projected_Sphere_Radius(float dist,float radius);

	void Update_Frustum(void) const { g_update_calls++; }

	ProjectionType Projection;
	DraftViewPlane ViewPlane;
	float AspectRatio;
	float ZNear;
	float ZFar;
	mutable OBBoxClass NearClipBBox;
	mutable Matrix4x4 ProjectionTransform;
	mutable Matrix3D CameraInvTransform;
};

#include "ca_tvs.inc"
#include "ca_rvs.inc"
#include "ca_nclip.inc"
#include "ca_hfov.inc"
#include "ca_vfov.inc"
#include "ca_asp.inc"
#include "ca_d3d.inc"
#include "ca_conv.inc"
#include "ca_cpsr.inc"

static Vector3 v3(const uint32_t * p) { return Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2])); }
static void put3(std::vector<uint32_t> & o, const Vector3 & v) { o.push_back(w_of(v.X)); o.push_back(w_of(v.Y)); o.push_back(w_of(v.Z)); }

}

/* in: inv transform 12, obb centre 3, extent 3, basis 9 (row-major), view min x y, max x y, aspect, znear, zfar,
** projection (0 perspective, 1 ortho), projection matrix 16, nops, ops:
**  0 Transform_To_View_Space(vec3) -> 3 words + update calls   1 Rotate_To_View_Space(vec3) -> 3 + calls
**  2 Get_Near_Clip_Bounding_Box -> 15 words (centre, extent, basis) + calls
**  3 Get_Horizontal_FOV  4 Get_Vertical_FOV  5 Get_Aspect_Ratio -> 1 word each
**  6 Get_D3D_Projection_Matrix -> 16 words + calls   7 Convert_Old(vec3) -> 3 words
**  8 Compute_Projected_Sphere_Radius(dist, radius) -> 1 word */
extern "C" int ref_dsww3d_cam_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	CameraClass c;
	float m[12];
	for (int i = 0; i < 12; i++) m[i] = f_of(*p++);
	c.CameraInvTransform = Matrix3D(m);
	c.NearClipBBox.Center = v3(p); p += 3;
	c.NearClipBBox.Extent = v3(p); p += 3;
	for (int r = 0; r < 3; r++) { c.NearClipBBox.Basis[r] = v3(p); p += 3; }
	c.ViewPlane.Min = Vector2(f_of(p[0]), f_of(p[1])); p += 2;
	c.ViewPlane.Max = Vector2(f_of(p[0]), f_of(p[1])); p += 2;
	c.AspectRatio = f_of(*p++);
	c.ZNear = f_of(*p++);
	c.ZFar = f_of(*p++);
	c.Projection = (*p++ == 0) ? CameraClass::PERSPECTIVE : CameraClass::ORTHO;
	for (int r = 0; r < 4; r++) for (int k = 0; k < 4; k++) c.ProjectionTransform[r][k] = f_of(*p++);
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		g_update_calls = 0;
		switch (op) {
		case 0: { Vector3 d; c.Transform_To_View_Space(d, v3(p)); p += 3; put3(o, d); o.push_back(g_update_calls); break; }
		case 1: { Vector3 d; c.Rotate_To_View_Space(d, v3(p)); p += 3; put3(o, d); o.push_back(g_update_calls); break; }
		case 2: { const OBBoxClass & b = c.Get_Near_Clip_Bounding_Box(); put3(o, b.Center); put3(o, b.Extent); for (int r = 0; r < 3; r++) put3(o, b.Basis[r]); o.push_back(g_update_calls); break; }
		case 3: o.push_back(w_of(c.Get_Horizontal_FOV())); break;
		case 4: o.push_back(w_of(c.Get_Vertical_FOV())); break;
		case 5: o.push_back(w_of(c.Get_Aspect_Ratio())); break;
		case 6: { Matrix4x4 t; c.Get_D3D_Projection_Matrix(&t); for (int r = 0; r < 4; r++) for (int q = 0; q < 4; q++) o.push_back(w_of(t[r][q])); o.push_back(g_update_calls); break; }
		case 7: { Vector3 v = v3(p); p += 3; c.Convert_Old(v); put3(o, v); break; }
		case 8: { float dist = f_of(p[0]), radius = f_of(p[1]); p += 2; o.push_back(w_of(c.Compute_Projected_Sphere_Radius(dist, radius))); break; }
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
