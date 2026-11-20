/* Differential-test wrapper for the vertex-stream copy functions in src/drafts/ww3d/shdchunk.rs (GitHub #215, group "ww3d7"), upstream commit 0a05454d:
** the nine `Copy_Vertex_Stream` members of the wwshade shaders, cut VERBATIM by build.rs (`drafts_ww3d7`, each anchor asserted to occur once):
**   Shd6SimpleClass (shdsimple.cpp), Shd6GlossMaskClass, Shd6BumpDiffClass, Shd6BumpSpecClass, Shd7BumpDiffClass, Shd7BumpSpecClass, Shd8BumpDiffClass,
**   Shd8BumpSpecClass, Shd6CubeMapClass  (w7_cvs_*.inc)
** and the vertex layouts of dx8fvf.h (VertexFormatXYZNDUV1, ...TG3, VertexFormatXYZNDCUBEMAP: w7_vf_*.inc), VertexStreamStruct (shdinterface.h: w7_vss.inc)
** and MAX_TEXTURE_STAGES (dx8wrapper.h:76: w7_maxtexstages.inc). Vector2/3/4 are WWMath's.
** Stand-ins: the nine classes (the real ones, in shd*.h, hold device types): each is a class with the one member the cut defines; none of the cut functions
** touches a member.
**
** Entry point (extern "C", tests/differential_draft_ww3d7_shdcopy.rs): ref_dw7_shdcopy(in, nin, out, cap) returns the words written (-1 if too small).
**   in = [kind 0..8 (the order above), vertex count, mask (bit 0 Locations, 1 Normals, 2 UV[0], 3 DiffuseInt, 4 S, 5 T, 6 SxT, 7 UV[1] only (UV[0] NULL)),
**   then per present stream its words: 3 or 2 floats or 1 word per vertex, in the mask's order]
**   out = the vertex array's words: 9 per vertex (kinds 0..3), 18 (kinds 4..7), 7 (kind 8); the buffer is pre-filled with 0xA5A5A5A5 so that an
**   unwritten word shows. */
#include <stdint.h>
#include <string.h>
#include <vector>

#include "always.h"
#include "wwdebug.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"

#include "w7_maxtexstages.inc"
#include "w7_vss.inc"
#include "w7_vf_uv1.inc"
#include "w7_vf_tg3.inc"
#include "w7_vf_cube.inc"

#define CVS_CLASS(c) class c { public: void Copy_Vertex_Stream(unsigned stream, void *dest_buffer, const VertexStreamStruct &vss, unsigned vertex_count); }
CVS_CLASS(Shd6SimpleClass);
CVS_CLASS(Shd6GlossMaskClass);
CVS_CLASS(Shd6BumpDiffClass);
CVS_CLASS(Shd6BumpSpecClass);
CVS_CLASS(Shd7BumpDiffClass);
CVS_CLASS(Shd7BumpSpecClass);
CVS_CLASS(Shd8BumpDiffClass);
CVS_CLASS(Shd8BumpSpecClass);
CVS_CLASS(Shd6CubeMapClass);

#include "w7_cvs_simple.inc"
#include "w7_cvs_glossmask.inc"
#include "w7_cvs_6bumpdiff.inc"
#include "w7_cvs_6bumpspec.inc"
#include "w7_cvs_7bumpdiff.inc"
#include "w7_cvs_7bumpspec.inc"
#include "w7_cvs_8bumpdiff.inc"
#include "w7_cvs_8bumpspec.inc"
#include "w7_cvs_cubemap.inc"

typedef uint32_t W;
static float flt(W w) { float f; memcpy(&f, &w, 4); return f; }

extern "C" int ref_dw7_shdcopy(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	int i = 0;
	W kind = in[i++];
	W n = in[i++];
	W mask = in[i++];
	std::vector<Vector3> loc(n), nrm(n), s(n), t(n), sxt(n);
	std::vector<Vector2> uv(n);
	std::vector<W> dif(n);
	if (mask & 1) for (W k = 0; k < n; ++k) { loc[k].X = flt(in[i]); loc[k].Y = flt(in[i + 1]); loc[k].Z = flt(in[i + 2]); i += 3; }
	if (mask & 2) for (W k = 0; k < n; ++k) { nrm[k].X = flt(in[i]); nrm[k].Y = flt(in[i + 1]); nrm[k].Z = flt(in[i + 2]); i += 3; }
	if (mask & (4 | 128)) for (W k = 0; k < n; ++k) { uv[k].U = flt(in[i]); uv[k].V = flt(in[i + 1]); i += 2; }
	if (mask & 8) for (W k = 0; k < n; ++k) dif[k] = in[i++];
	if (mask & 16) for (W k = 0; k < n; ++k) { s[k].X = flt(in[i]); s[k].Y = flt(in[i + 1]); s[k].Z = flt(in[i + 2]); i += 3; }
	if (mask & 32) for (W k = 0; k < n; ++k) { t[k].X = flt(in[i]); t[k].Y = flt(in[i + 1]); t[k].Z = flt(in[i + 2]); i += 3; }
	if (mask & 64) for (W k = 0; k < n; ++k) { sxt[k].X = flt(in[i]); sxt[k].Y = flt(in[i + 1]); sxt[k].Z = flt(in[i + 2]); i += 3; }
	VertexStreamStruct vss;
	vss.Locations = (mask & 1) ? &loc[0] : 0;
	vss.Normals = (mask & 2) ? &nrm[0] : 0;
	if (mask & 4) vss.UV[0] = &uv[0];
	if (mask & 128) vss.UV[1] = &uv[0];
	vss.DiffuseInt = (mask & 8) ? &dif[0] : 0;
	vss.S = (mask & 16) ? &s[0] : 0;
	vss.T = (mask & 32) ? &t[0] : 0;
	vss.SxT = (mask & 64) ? &sxt[0] : 0;
	int per = kind <= 3 ? 9 : (kind <= 7 ? 18 : 7);
	int total = per * (int)n;
	if (total > cap) return -1;
	for (int k = 0; k < total; ++k) out[k] = 0xA5A5A5A5u;
	void *dest = out;
	switch (kind) {
	case 0: Shd6SimpleClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 1: Shd6GlossMaskClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 2: Shd6BumpDiffClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 3: Shd6BumpSpecClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 4: Shd7BumpDiffClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 5: Shd7BumpSpecClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 6: Shd8BumpDiffClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	case 7: Shd8BumpSpecClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	default: Shd6CubeMapClass().Copy_Vertex_Stream(0, dest, vss, n); break;
	}
	return total;
}
