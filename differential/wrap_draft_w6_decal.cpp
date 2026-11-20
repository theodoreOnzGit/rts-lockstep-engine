/* Differential-test wrapper for the decal mesh's polygon clipper (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/decalmsh.rs `DecalPolyClass` (reset, add_vertex, clip) against WW3D2/decalmsh.cpp's DecalPolyClass (declaration and its three members), cut VERBATIM
** by build.rs `drafts_ww3d6` (decal_poly.inc, from `class DecalPolyClass` to the two statics; one anchor pair, each asserted to occur once). PlaneClass (In_Front,
** Compute_Intersection), Vector3 (Lerp) and SimpleDynVecClass are the real headers; `DISABLE_CLIPPING` is 0 as in decalmsh.cpp. No stand-ins.
** (DecalMeshClass::Create_Decal, Render, Delete_Decal need MeshClass, MeshModelClass and the DX8 buffers: not covered.)
**
** Entry point (extern "C", tests/differential_draft_decalpoly.rs), uint32 words, floats by bits:
**   ref_w6_decal_clip(in, out) -> n   in = [nplanes, planes (n 3, d) x nplanes, nverts, per vertex: position 3, normal 3]
**       clips the polygon by plane 0 into a second polygon, that by plane 1 back into the first, and so on (as Create_Decal ping-pongs _DecalPoly0/1)
**       out = [per clip: count, per vertex: position 3, normal 3] */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include "always.h"
#include "vector3.h"
#include "plane.h"
#include "simplevec.h"

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

namespace {

#define DISABLE_CLIPPING 0

#include "decal_poly.inc"

}

extern "C" int ref_w6_decal_clip(const W *in, W *out)
{
	const W *p = in;
	int np = (int)*p++;
	PlaneClass planes[8];
	for (int i = 0; i < np; i++, p += 4) planes[i].Set(F(p[0]), F(p[1]), F(p[2]), F(p[3]));
	int nv = (int)*p++;
	DecalPolyClass a, b;
	for (int i = 0; i < nv; i++, p += 6) a.Add_Vertex(Vector3(F(p[0]), F(p[1]), F(p[2])), Vector3(F(p[3]), F(p[4]), F(p[5])));
	W *o = out;
	DecalPolyClass * src = &a, * dst = &b;
	for (int k = 0; k < np; k++) {
		src->Clip(planes[k], *dst);
		*o++ = (W)dst->Verts.Count();
		for (int i = 0; i < dst->Verts.Count(); i++) {
			*o++ = B(dst->Verts[i].X); *o++ = B(dst->Verts[i].Y); *o++ = B(dst->Verts[i].Z);
			*o++ = B(dst->VertNorms[i].X); *o++ = B(dst->VertNorms[i].Y); *o++ = B(dst->VertNorms[i].Z);
		}
		DecalPolyClass * t = src; src = dst; dst = t;
	}
	return (int)(o - out);
}
