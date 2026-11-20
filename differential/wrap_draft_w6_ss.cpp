/* Differential-test wrapper for the shatter system's clipping geometry draft (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/shattersystem.rs (VertexClass, PolygonClass, BSPClass) against WW3D2/shattersystem.cpp.
**
** VERBATIM cuts by build.rs `drafts_ww3d6` (each anchor asserted to occur once), included below:
**   ss_defs.inc    the BPT_* defines
**   ss_decl.inc    the declarations of VertexClass, PolygonClass and BSPClass
**   ss_statics.inc MAX_MESH_FRAGMENTS, ShatterPatterns and ClipPools
**   ss_impl.inc    VertexClass, PolygonClass and BSPClass members (to the "ShatterSystem Implementation" banner)
** Stand-ins, each for something the cut calls outside itself:
**  - MeshMatDescClass: only the two enumerators MAX_PASSES = 4, MAX_TEX_STAGES = 2 (meshmatdesc.h, copied here; the class is the mesh's
**    material description).
**  - HTreeClass: Num_Pivots, Get_Parent_Index and Get_Transform over the scripted pivots (the BSP builder calls nothing else).
**  - DX8Wrapper::Convert_Color(unsigned) is the real transcription of wrap_w3d_mesh.cpp (via the forced-included mesh_prelude.h);
**    VertexClass::Lerp's colour lines are no-ops upstream (the Vector4::Lerp result is discarded), so the call only has to exist.
**  - W3DNEW is the real always.h macro (`new("W3D_" __FILE__, 0)`), which the force-included w3d_prelude.h routes to the plain allocator.
**
** Entry points (extern "C", tests/differential_draft_shattersystem6.rs). Words are uint32, floats by bits. A VERTEX is 31 words: position 3,
** normal 3, pass_count, dcg 4, dig 4, texcoord 16 (pass, stage, u, v). A POLYGON is: material_id, num_verts, plane (n 3, d), then 24 vertices
** (all slots; the test fills only the first num_verts and leaves the rest as the constructor does).
**   ref_w6_ss_vertex(in, out)  in = [op, plane 4, v0 (31), v1 (31), alpha]; op 0 Which_Side(v0, plane) -> [side];
**       op 1 Intersect_Plane(v0, v1, plane) -> VERTEX; op 2 Lerp(v0, v1, alpha) -> VERTEX
**   ref_w6_ss_polygon(in, out) in = [op, plane 4, POLYGON]; op 0 Which_Side -> [side]; op 1 Split -> [front POLYGON, back POLYGON];
**       op 2 Compute_Plane -> [POLYGON]; op 3 Is_Degenerate -> [ret, POLYGON]; op 4 Salvage_Degenerate -> [ret, POLYGON]
**   ref_w6_ss_bsp(in, out) in = [npivots, per pivot: parent, 12 floats (row-major 3x4), root bone, npolys, POLYGONs...] builds BSPClass(tree, root, leaf_index)
**       and clips every polygon -> [leaf_index, per pool 0..31: count, then POLYGONs (num_verts, material, plane, num_verts vertices as VERTEX)] */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>
#include "always.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "plane.h"
#include "simplevec.h"
#include "wwmath.h"
#include "dx8wrapper.h"

#define assert(x) ((void)0)
#ifndef WWASSERT_PRINT
#define WWASSERT_PRINT(x, y) ((void)0)
#endif
#ifndef WWASSERT
#define WWASSERT(x) ((void)0)
#endif
#ifndef WWDEBUG_SAY
#define WWDEBUG_SAY(x) ((void)0)
#endif

/* the prelude forward-declares the real HTreeClass: the stand-in is renamed, and the cut's `HTreeClass *` means it */
#define HTreeClass W6HTreeClass

namespace {

struct MeshMatDescClass { enum { MAX_PASSES = 4, MAX_TEX_STAGES = 2 }; };

class HTreeClass {
public:
	int Num_Pivots(void) const { return (int)tm.size(); }
	int Get_Parent_Index(int i) const { return parent[i]; }
	const Matrix3D & Get_Transform(int i) const { return tm[i]; }
	std::vector<int> parent;
	std::vector<Matrix3D> tm;
};

#include "ss_defs.inc"
#include "ss_decl.inc"
#include "ss_statics.inc"
#include "ss_impl.inc"

typedef uint32_t W;
float F(W w) { float f; memcpy(&f, &w, 4); return f; }
W B(float f) { W w; memcpy(&w, &f, 4); return w; }

void read_vertex(const W *&p, VertexClass &v)
{
	v.Position.X = F(p[0]); v.Position.Y = F(p[1]); v.Position.Z = F(p[2]);
	v.Normal.X = F(p[3]); v.Normal.Y = F(p[4]); v.Normal.Z = F(p[5]);
	v.PassCount = (int)p[6];
	for (int i = 0; i < 4; i++) { v.DCG[i] = p[7 + i]; v.DIG[i] = p[11 + i]; }
	for (int ip = 0; ip < 4; ip++) for (int is = 0; is < 2; is++) {
		v.TexCoord[ip][is].X = F(p[15 + (ip * 2 + is) * 2]);
		v.TexCoord[ip][is].Y = F(p[16 + (ip * 2 + is) * 2]);
	}
	p += 31;
}

void write_vertex(W *&o, const VertexClass &v)
{
	*o++ = B(v.Position.X); *o++ = B(v.Position.Y); *o++ = B(v.Position.Z);
	*o++ = B(v.Normal.X); *o++ = B(v.Normal.Y); *o++ = B(v.Normal.Z);
	*o++ = (W)v.PassCount;
	for (int i = 0; i < 4; i++) *o++ = v.DCG[i];
	for (int i = 0; i < 4; i++) *o++ = v.DIG[i];
	for (int ip = 0; ip < 4; ip++) for (int is = 0; is < 2; is++) { *o++ = B(v.TexCoord[ip][is].X); *o++ = B(v.TexCoord[ip][is].Y); }
}

void read_polygon(const W *&p, PolygonClass &g)
{
	g.MaterialId = (int)p[0];
	g.NumVerts = (int)p[1];
	g.Plane.Set(F(p[2]), F(p[3]), F(p[4]), F(p[5]));
	p += 6;
	for (int i = 0; i < PolygonClass::BPT_POLY_MAX_VERTS; i++) read_vertex(p, g.Verts[i]);
}

void write_polygon(W *&o, const PolygonClass &g)
{
	*o++ = (W)g.MaterialId; *o++ = (W)g.NumVerts;
	*o++ = B(g.Plane.N.X); *o++ = B(g.Plane.N.Y); *o++ = B(g.Plane.N.Z); *o++ = B(g.Plane.D);
	for (int i = 0; i < PolygonClass::BPT_POLY_MAX_VERTS; i++) write_vertex(o, g.Verts[i]);
}

}

extern "C" {

int ref_w6_ss_vertex(const W *in, W *out)
{
	W *o = out;
	int op = (int)in[0];
	PlaneClass plane(F(in[1]), F(in[2]), F(in[3]), F(in[4]));
	const W *p = in + 5;
	VertexClass v0, v1, res;
	read_vertex(p, v0);
	read_vertex(p, v1);
	float alpha = F(*p);
	if (op == 0) { *o++ = (W)v0.Which_Side(plane); }
	else {
		if (op == 1) VertexClass::Intersect_Plane(v0, v1, plane, &res);
		else VertexClass::Lerp(v0, v1, alpha, &res);
		write_vertex(o, res);
	}
	return (int)(o - out);
}

int ref_w6_ss_polygon(const W *in, W *out)
{
	W *o = out;
	int op = (int)in[0];
	PlaneClass plane(F(in[1]), F(in[2]), F(in[3]), F(in[4]));
	const W *p = in + 5;
	PolygonClass *g = new PolygonClass;
	read_polygon(p, *g);
	if (op == 0) { *o++ = (W)g->Which_Side(plane); }
	else if (op == 1) {
		PolygonClass *front = new PolygonClass, *back = new PolygonClass;
		front->MaterialId = back->MaterialId = 0;
		g->Split(plane, *front, *back);
		write_polygon(o, *front);
		write_polygon(o, *back);
		delete front; delete back;
	}
	else if (op == 2) { g->Compute_Plane(); write_polygon(o, *g); }
	else if (op == 3) { *o++ = (W)g->Is_Degenerate(); write_polygon(o, *g); }
	else { *o++ = (W)g->Salvage_Degenerate(); write_polygon(o, *g); }
	delete g;
	return (int)(o - out);
}

int ref_w6_ss_bsp(const W *in, W *out)
{
	W *o = out;
	const W *p = in;
	HTreeClass tree;
	int np = (int)*p++;
	for (int i = 0; i < np; i++) {
		tree.parent.push_back((int)*p++);
		float m[12];
		for (int k = 0; k < 12; k++) m[k] = F(*p++);
		tree.tm.push_back(Matrix3D(m));
	}
	int root = (int)*p++;
	int leaf = 0;
	BSPClass *bsp = new BSPClass(&tree, root, leaf);
	for (int i = 0; i < 32; i++) ClipPools[i].Delete_All();
	int npolys = (int)*p++;
	for (int i = 0; i < npolys; i++) {
		PolygonClass *g = new PolygonClass;
		read_polygon(p, *g);
		bsp->Clip_Polygon(*g);
		delete g;
	}
	*o++ = (W)leaf;
	for (int i = 0; i < 32; i++) {
		*o++ = (W)ClipPools[i].Count();
		for (int k = 0; k < ClipPools[i].Count(); k++) write_polygon(o, ClipPools[i][k]);
	}
	delete bsp;
	return (int)(o - out);
}

}
