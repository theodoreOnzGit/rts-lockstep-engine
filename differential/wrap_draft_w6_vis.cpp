/* Differential-test wrapper for the visibility rasteriser draft (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/visrasterizer.rs (IDBufferClass, VisPolyClass, VisRasterizerClass) against WW3D2/visrasterizer.cpp / .h.
**
** VERBATIM cuts by build.rs `drafts_ww3d6` (each anchor asserted to occur once):
**   vis_decl.inc  visrasterizer.h from `struct GradientsStruct;` to the closing #endif: IDBufferClass and VisRasterizerClass (`private` and `protected`
**                 are made `public` around it, the wrapper reads the buffers)
**   vis_poly.inc  visrasterizer.cpp: VisPolyClass and its two statics
**   vis_impl.inc  visrasterizer.cpp from VisRasterizerClass's constructor to the end of the file (the 4 WWMath::Float_To_Long calls name
**                 Ref_W6_Fistp, the MSVC x86 `fld`/`fistp` branch the shipped game ran; the Linux wwmath.h truncates)
** visrasterizer.h itself is not included (it pulls in meshgeometry.h); its includes' needs are met by the real headers below.
**
** Stand-ins, each for something the cut calls outside itself:
**  - CameraClass (camera.h, a render object): the view matrix, a perspective Project_Camera_Space_Point (x/-z, y/-z, 1 + 1/z), the six view-space
**    frustum planes and a frustum object, all scripted by the test (the cut calls nothing else on it); Add_Ref / Release_Ref do nothing.
**  - CollisionMath::Overlap_Test(frustum, box): returns INSIDE when the test's flag is set, else OVERLAPPED (the draft takes this as VisCamera::frustum_contains_box).
**  - TriIndex (meshgeometry.h): `Vector3i16`, three shorts I, J, K, as upstream's typedef.
**  - W3DNEWARRAY allocations are not zeroed by upstream's allocator here: the wrapper zeroes both buffers after Set_Resolution (the game's allocator zero-fills).
**  - VectorProcessorClass::Transform(Vector3 *, ..., Matrix3D) is upstream's, a verbatim cut linked from another library (wrap_w3d_deform).
**
** Entry points (extern "C", tests/differential_draft_visrasterizer.rs); words are uint32, floats by bits:
**   ref_w6_vis_idbuf(in, out) -> n    in = [w, h, backface_id, frontface_id, mode, two_sided, ntris, per triangle: op, 9 floats]; op 0 Render_Triangle,
**       1 Clear, 2 Reset_Pixel_Counter, 3 Set_Resolution again (w, h in the first two floats, the buffers zeroed)
**       out = per op: the Render_Triangle result and Get_Pixel_Counter; at the end the whole ID buffer (w*h words)
**   ref_w6_vis_raster(in, out) -> n   in = [w, h, backface_id, frontface_id, mode, two_sided, inside, model tm 12, view tm 12, planes 6x4, nverts, verts 3 each,
**       ntris, tris 3 each (as 16-bit indices)]; calls Set_Camera, Set_Model_Transform, Render_Triangles; out = [result, pixel counter, id buffer] */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>
#include "always.h"
#include "matrix3d.h"
#include "matrix4.h"
#include "vector3.h"
#include "simplevec.h"
#include "plane.h"
#include "aabox.h"
#include "vp.h"
#include "wwmath.h"
#include "refcount.h"

#ifndef WWASSERT
#define WWASSERT(x) ((void)0)
#endif

/* everything below is in an anonymous namespace: other wrappers define classes of the same names (CameraClass, TriIndex, ...) */
namespace {

struct Vector3i16 { short I, J, K; };
typedef Vector3i16 TriIndex;

static long Ref_W6_Fistp(float f)
{
	int32_t i;
	__asm__ volatile("flds %1\n\tfistpl %0" : "=m"(i) : "m"(f));
	return (long)i;
}

struct W6Frustum { int dummy; };

static Matrix3D g_view(1);
static PlaneClass g_planes[6];
static bool g_inside = false;

class CameraClass {
public:
	void Add_Ref(void) {}
	void Release_Ref(void) {}
	void Get_View_Matrix(Matrix3D * set_tm) const { *set_tm = g_view; }
	void Project_Camera_Space_Point(Vector3 & dest, const Vector3 & p) const
	{
		float inv = 1.0f / -p.Z;
		dest.X = p.X * inv;
		dest.Y = p.Y * inv;
		dest.Z = 1.0f + 1.0f / p.Z;
	}
	const PlaneClass * Get_View_Space_Frustum_Planes(void) const { return g_planes; }
	const W6Frustum & Get_Frustum(void) const { static W6Frustum f; return f; }
};

/* the real colmath.h is pulled in by aabox.h: the stand-in has another name, which the cut's `CollisionMath` is mapped to */
#define CollisionMath W6CollisionMath
struct W6CollisionMath {
	enum OverlapType { OUTSIDE = 0x01, INSIDE = 0x02, OVERLAPPED = 0x04 };
	static OverlapType Overlap_Test(const W6Frustum &, const AABoxClass &) { return g_inside ? INSIDE : OVERLAPPED; }
};

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

#define private public
#define protected public
#include "vis_decl.inc"
#undef private
#undef protected
#include "vis_poly.inc"
#include "vis_impl.inc"

}

static void zero_buffers(IDBufferClass & b)
{
	size_t n = (size_t)(b.ResWidth > 0 && b.ResHeight > 0 ? b.ResWidth * b.ResHeight : 0);
	if (n) { memset(b.IDBuffer, 0, n * 4); memset(b.ZBuffer, 0, n * 4); }
}

extern "C" {

int ref_w6_vis_idbuf(const W *in, W *out)
{
	IDBufferClass b;
	int w = (int)in[0], h = (int)in[1];
	b.Set_Resolution(w, h);
	zero_buffers(b);
	b.Set_Backface_ID(in[2]);
	b.Set_Frontface_ID(in[3]);
	b.Set_Render_Mode(in[4] ? IDBufferClass::NON_OCCLUDER_MODE : IDBufferClass::OCCLUDER_MODE);
	b.Enable_Two_Sided_Rendering(in[5] != 0);
	int n = (int)in[6];
	const W *p = in + 7;
	W *o = out;
	for (int i = 0; i < n; i++) {
		int op = (int)*p++;
		const W *f = p;
		p += 9;
		if (op == 0) {
			Vector3 v0(F(f[0]), F(f[1]), F(f[2])), v1(F(f[3]), F(f[4]), F(f[5])), v2(F(f[6]), F(f[7]), F(f[8]));
			*o++ = (W)b.Render_Triangle(v0, v1, v2);
			*o++ = (W)b.Get_Pixel_Counter();
		} else if (op == 1) { b.Clear(); }
		else if (op == 2) { b.Reset_Pixel_Counter(); }
		else {
			int nw = (int)f[0], nh = (int)f[1];
			bool same = nw == b.ResWidth && nh == b.ResHeight; /* Set_Resolution returns early for the same size: the buffers keep their contents */
			b.Set_Resolution(nw, nh);
			if (!same) zero_buffers(b);
		}
	}
	int rw, rh;
	b.Get_Resolution(&rw, &rh);
	*o++ = (W)rw; *o++ = (W)rh;
	for (int y = 0; y < rh; y++) {
		const uint32 *row = b.Get_Pixel_Row(y, 0, rw);
		for (int x = 0; x < rw; x++) *o++ = row[x];
	}
	return (int)(o - out);
}

int ref_w6_vis_raster(const W *in, W *out)
{
	VisRasterizerClass r;
	CameraClass cam;
	int w = (int)in[0], h = (int)in[1];
	r.Set_Resolution(w, h);
	zero_buffers(r.IDBuffer);
	r.Set_Backface_ID(in[2]);
	r.Set_Frontface_ID(in[3]);
	r.Set_Render_Mode(in[4] ? IDBufferClass::NON_OCCLUDER_MODE : IDBufferClass::OCCLUDER_MODE);
	r.Enable_Two_Sided_Rendering(in[5] != 0);
	g_inside = in[6] != 0;
	float m[12], v[12];
	for (int i = 0; i < 12; i++) { m[i] = F(in[7 + i]); v[i] = F(in[19 + i]); }
	g_view = Matrix3D(v);
	for (int i = 0; i < 6; i++) g_planes[i].Set(F(in[31 + 4 * i]), F(in[32 + 4 * i]), F(in[33 + 4 * i]), F(in[34 + 4 * i]));
	const W *p = in + 55;
	int nv = (int)*p++;
	std::vector<Vector3> verts(nv > 0 ? nv : 1);
	for (int i = 0; i < nv; i++) { verts[i].X = F(p[0]); verts[i].Y = F(p[1]); verts[i].Z = F(p[2]); p += 3; }
	int nt = (int)*p++;
	std::vector<TriIndex> tris(nt > 0 ? nt : 1);
	for (int i = 0; i < nt; i++) { tris[i].I = (short)p[0]; tris[i].J = (short)p[1]; tris[i].K = (short)p[2]; p += 3; }
	r.Set_Camera(&cam);
	r.Set_Model_Transform(Matrix3D(m));
	AABoxClass bounds;
	bool res = r.Render_Triangles(verts.data(), nv, tris.data(), nt, bounds);
	W *o = out;
	*o++ = (W)res;
	*o++ = (W)r.Get_Pixel_Counter();
	for (int y = 0; y < h; y++) {
		const uint32 *row = r.Get_Pixel_Row(y, 0, w);
		for (int x = 0; x < w; x++) *o++ = row[x];
	}
	return (int)(o - out);
}

}
