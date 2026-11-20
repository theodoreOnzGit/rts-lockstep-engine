/* Differential-test wrapper for the black-and-white triangle rasteriser draft (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/bwrender.rs (BWBuffer, BWRenderClass, BW_Render) against WW3D2/bwrender.cpp.
**
** The REAL bwrender.h and bwrender.cpp are compiled (build.rs `drafts_ww3d6` copies both; the one edit is that the 10 calls of
** WWMath::Float_To_Long name Ref_W6_Fistp, defined below as the MSVC x86 branch the shipped game ran: x87 `fld`/`fistp`, round
** to nearest even, out of range or NaN -> 0x80000000; the g++ build of wwmath.h takes the portable `(long) f` branch). WW3D2/bw_render.cpp
** (class BW_Render) is the same code under the old name: `diff` against bwrender.cpp shows only the class name, the include and header
** comments, so only bwrender.cpp is run. `VectorProcessorClass::MulAdd` is cut VERBATIM from WWMath/vp.cpp (vp_muladd.inc).
** bwrender.cpp is #included here (not compiled alone) so that its `::memset` can be a stand-in, see below.
**
** Stand-in: memset (bwrender.cpp's Set_H_Line clears `w = end_x - start_x` pixels, and Fill clears scale*scale). A call whose range is
** not inside the pixel buffer would be a wild write upstream (a negative w is a huge size_t); the stand-in records "wild" and writes
** nothing, and the test drops a program for which wild is set (the draft clamps a negative width to nothing, a documented deviation).
**
** Entry point (extern "C", tests/differential_draft_bwrender.rs):
**   ref_w6_bw_run(in, out_bytes, out_words) -> number of words written to out_words
**   in (uint32): [scale, initial fill byte, nops, ops...]; op 0 Fill(c); op 1 Set_Vertex_Locations(count, 2*count float bits)
**       (the vertices are then scaled in place); op 2 Render_Triangles(n, n indices); op 3 Render_Triangle_Strip(n, n indices);
**       op 4 Buffer::Set_H_Line(start_x, end_x, y) (signed words)
**   out_bytes: scale*scale pixels. out_words: [wild, vertex count, 2*count float bits of the (scaled) vertices]. */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>

static unsigned char *g_base = 0;
static size_t g_len = 0;
static int g_wild = 0;
static void *ref_w6_memset(void *ptr, int c, size_t n)
{
	unsigned char *p = (unsigned char *)ptr;
	if (p < g_base || p > g_base + g_len || n > (size_t)(g_base + g_len - p)) { g_wild = 1; return ptr; }
	return memset(ptr, c, n);
}

static long Ref_W6_Fistp(float f)
{
	int32_t i;
	__asm__ volatile("flds %1\n\tfistpl %0" : "=m"(i) : "m"(f));
	return (long)i;
}

#include "always.h"
#include "vector2.h"
#include "vector3.h"
#include "vector3i.h"
#include "vp.h"
#include "wwmath.h"
/* the class's members default to private and the cut touches pixel_buffer: `class` is `struct` for this one header */
#define class struct
#include "bwrender.h"
#undef class

#include "vp_muladd.inc"

#define memset ref_w6_memset
#include "bwrender.cpp"
#undef memset

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

extern "C" int ref_w6_bw_run(const W *in, unsigned char *out_bytes, W *out_words)
{
	int scale = (int)in[0];
	std::vector<unsigned char> buf((size_t)scale * (size_t)scale);
	g_base = buf.data(); g_len = buf.size(); g_wild = 0;
	memset(buf.data(), (int)in[1], buf.size());
	BWRenderClass r(buf.data(), scale);
	std::vector<Vector2> verts;
	int nops = (int)in[2];
	const W *p = in + 3;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		if (op == 0) { r.Fill((unsigned char)*p++); }
		else if (op == 1) {
			int count = (int)*p++;
			verts.assign(count > 0 ? count : 1, Vector2(0, 0));
			for (int i = 0; i < count; i++) { verts[i].X = F(p[2 * i]); verts[i].Y = F(p[2 * i + 1]); }
			p += 2 * count;
			r.Set_Vertex_Locations(verts.data(), count);
		}
		else if (op == 2 || op == 3) {
			int n = (int)*p++;
			std::vector<unsigned long> idx(n);
			for (int i = 0; i < n; i++) idx[i] = p[i];
			p += n;
			if (op == 2) r.Render_Triangles(idx.data(), n); else r.Render_Triangle_Strip(idx.data(), n);
		}
		else { int sx = (int)p[0], ex = (int)p[1], y = (int)p[2]; p += 3; r.pixel_buffer.Set_H_Line(sx, ex, y); }
	}
	memcpy(out_bytes, buf.data(), buf.size());
	int n = 0;
	out_words[n++] = (W)g_wild;
	out_words[n++] = (W)verts.size();
	for (size_t i = 0; i < verts.size(); i++) { out_words[n++] = B(verts[i].X); out_words[n++] = B(verts[i].Y); }
	return n;
}
