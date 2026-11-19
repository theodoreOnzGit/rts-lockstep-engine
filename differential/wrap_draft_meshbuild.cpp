/* Differential-test wrapper for the mesh builder draft (GitHub #215, upstream commit 0a05454d), group "ww3dpure":
** src/drafts/ww3d/meshbuild.rs against WW3D2/meshbuild.cpp and meshbuild.h.
**
** meshbuild.cpp is compiled WHOLE (build.rs `drafts_ww3dpure` copies it with ONE mechanical edit, `for (pass=0; ...)` ->
** `for (int pass=0; ...)`, the loop that reads `pass` declared by an earlier `for` in the same function: VC6's pre-standard
** for-scope), as its own translation unit with the real headers (meshbuild.h, uarray.h, hashcalc.h, vector2/3.h) and the real
** libc qsort (glibc 2.39: a stable merge sort; the Rust test probes this before it trusts any tie order, see
** ref_dpw_qsort_is_stable). This file includes meshbuild.h with `private` made `public` so that the private members can be
** called individually. Stand-ins: a WorldInfoClass subclass (an abstract class upstream, supplied by the application) whose
** Get_Shared_Vertex_Normal is a fixed arithmetic function of (position, smoothing group), the same on the Rust side.
**
** Words are uint32: floats by bits, ints and bools as themselves. Layouts (the Rust side builds and reads the same ones):
**  VERT in (118): position 3, normal 3, SmGroup, Id, BoneIndex, MaxVertColIndex, TexCoord[4][8][2] 64, DiffuseColor[4][3] 12,
**    SpecularColor 12, DiffuseIllumination 12, Alpha[4], VertexMaterialIndex[4], Attribute0, Attribute1.
**  VERT out (121): the 118, then SharedSmGroup, UniqueIndex, ShadeIndex.
**  FACE in (394): 3 VERT in, SmGroup, Index, Attributes, TextureIndex[4][8] 32, ShaderIndex[4], SurfaceType.
**  FACE out (411): 3 VERT out (SharedSmGroup written as 0: Reset never sets it, so a face's own verts hold the heap garbage of
**    the allocation), SmGroup, Index, Attributes, TextureIndex 32, ShaderIndex 4, SurfaceType, AddIndex, VertIdx 3, Normal 3, Dist.
**  STATS out (132): HasTexture 32, HasShader 4, HasVertexMaterial 4, HasPerPolyTexture 32, HasPerPolyShader 4,
**    HasPerVertexMaterial 4, HasDiffuseColor 4, HasSpecularColor 4, HasDiffuseIllumination 4, HasTexCoords 32, UVSplitCount,
**    StripCount, MaxStripLength, AvgStripLength.
** Entry point: ref_dpw_meshbuild(in, out) returns the number of words written. in[0] = op, then:
**  op 0 build: pass_count, face_count_guess, growth, order_pass, order_stage, compute_normals, world_mode (0 none, 1 smoothed,
**     2 not smoothed), n, n FACE in -> vert_count, face_count, STATS, verts (VERT out), faces (FACE out), Compute_Bounding_Box
**     (6), Compute_Bounding_Sphere (center 3, radius).
**  op 1 build, then Verify_Face_Normals (bool) with face 0's Normal.x increased by in[...] (delta) first; same header as op 0
**     plus delta after the faces -> [verify before the change, verify after].
**  op 2 build, then Verts[0].Position.x increased by delta, Compute_Face_Normals, Verify_Face_Normals -> bool, then face
**     Normal 3 and Dist for every face.
**  op 3 FaceClass::Compute_Plane + Is_Degenerate on one FACE in whose VertIdx are the next 3 words -> Normal 3, Dist, bool.
**  op 4 default-constructed VertClass and FaceClass (Reset): VERT out, FACE out.
**  op 5 Add_Face only: header as op 0 with Reset(pass_count, guess, growth), Add_Face for each face; -> n returned indices, then
**     for each face (in the builder's array before processing) Normal 3, Dist, AddIndex, the three Verts' SmGroup.
*/
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#define private public
#define protected public
#include "meshbuild.h"
#undef private
#undef protected

typedef MeshBuilderClass::VertClass VC;
typedef MeshBuilderClass::FaceClass FC;
typedef uint32_t W;

static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

struct TestWorld : public WorldInfoClass
{
	bool Smoothed;
	TestWorld(bool s) : Smoothed(s) {}
	virtual Vector3 Get_Shared_Vertex_Normal(Vector3 pos, int smgroup)
	{
		return Vector3(pos.X * 0.5f + smgroup, -pos.Y, pos.Z + (float)smgroup * 0.25f);
	}
	virtual bool Are_Meshes_Smoothed(void) const { return Smoothed; }
};

static const W *ReadVert(const W *p, VC &v)
{
	v.Position.Set(F(p[0]), F(p[1]), F(p[2]));
	v.Normal.Set(F(p[3]), F(p[4]), F(p[5]));
	v.SmGroup = (int)p[6]; v.Id = (int)p[7]; v.BoneIndex = (int)p[8]; v.MaxVertColIndex = (int)p[9];
	p += 10;
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) { v.TexCoord[a][b].Set(F(p[0]), F(p[1])); p += 2; }
	for (int a = 0; a < 4; a++) { v.DiffuseColor[a].Set(F(p[0]), F(p[1]), F(p[2])); p += 3; }
	for (int a = 0; a < 4; a++) { v.SpecularColor[a].Set(F(p[0]), F(p[1]), F(p[2])); p += 3; }
	for (int a = 0; a < 4; a++) { v.DiffuseIllumination[a].Set(F(p[0]), F(p[1]), F(p[2])); p += 3; }
	for (int a = 0; a < 4; a++) v.Alpha[a] = F(*p++);
	for (int a = 0; a < 4; a++) v.VertexMaterialIndex[a] = (int)*p++;
	v.Attribute0 = (int)*p++; v.Attribute1 = (int)*p++;
	return p;
}

static const W *ReadFace(const W *p, FC &f)
{
	for (int i = 0; i < 3; i++) p = ReadVert(p, f.Verts[i]);
	f.SmGroup = (int)p[0]; f.Index = (int)p[1]; f.Attributes = (int)p[2];
	p += 3;
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) f.TextureIndex[a][b] = (int)*p++;
	for (int a = 0; a < 4; a++) f.ShaderIndex[a] = (int)*p++;
	f.SurfaceType = *p++;
	return p;
}

static W *WriteVert(const VC &v, W *o, bool shared)
{
	*o++ = B(v.Position.X); *o++ = B(v.Position.Y); *o++ = B(v.Position.Z);
	*o++ = B(v.Normal.X); *o++ = B(v.Normal.Y); *o++ = B(v.Normal.Z);
	*o++ = (W)v.SmGroup; *o++ = (W)v.Id; *o++ = (W)v.BoneIndex; *o++ = (W)v.MaxVertColIndex;
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) { *o++ = B(v.TexCoord[a][b].U); *o++ = B(v.TexCoord[a][b].V); }
	for (int a = 0; a < 4; a++) { *o++ = B(v.DiffuseColor[a].X); *o++ = B(v.DiffuseColor[a].Y); *o++ = B(v.DiffuseColor[a].Z); }
	for (int a = 0; a < 4; a++) { *o++ = B(v.SpecularColor[a].X); *o++ = B(v.SpecularColor[a].Y); *o++ = B(v.SpecularColor[a].Z); }
	for (int a = 0; a < 4; a++) { *o++ = B(v.DiffuseIllumination[a].X); *o++ = B(v.DiffuseIllumination[a].Y); *o++ = B(v.DiffuseIllumination[a].Z); }
	for (int a = 0; a < 4; a++) *o++ = B(v.Alpha[a]);
	for (int a = 0; a < 4; a++) *o++ = (W)v.VertexMaterialIndex[a];
	*o++ = (W)v.Attribute0; *o++ = (W)v.Attribute1;
	*o++ = shared ? (W)v.SharedSmGroup : 0; *o++ = (W)v.UniqueIndex; *o++ = (W)v.ShadeIndex;
	return o;
}

static W *WriteFace(const FC &f, W *o, bool shared)
{
	for (int i = 0; i < 3; i++) o = WriteVert(f.Verts[i], o, shared);
	*o++ = (W)f.SmGroup; *o++ = (W)f.Index; *o++ = (W)f.Attributes;
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) *o++ = (W)f.TextureIndex[a][b];
	for (int a = 0; a < 4; a++) *o++ = (W)f.ShaderIndex[a];
	*o++ = f.SurfaceType;
	*o++ = (W)f.AddIndex;
	for (int i = 0; i < 3; i++) *o++ = (W)f.VertIdx[i];
	*o++ = B(f.Normal.X); *o++ = B(f.Normal.Y); *o++ = B(f.Normal.Z);
	*o++ = B(f.Dist);
	return o;
}

static W *WriteStats(const MeshBuilderClass::MeshStatsStruct &s, W *o)
{
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) *o++ = s.HasTexture[a][b];
	for (int a = 0; a < 4; a++) *o++ = s.HasShader[a];
	for (int a = 0; a < 4; a++) *o++ = s.HasVertexMaterial[a];
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) *o++ = s.HasPerPolyTexture[a][b];
	for (int a = 0; a < 4; a++) *o++ = s.HasPerPolyShader[a];
	for (int a = 0; a < 4; a++) *o++ = s.HasPerVertexMaterial[a];
	for (int a = 0; a < 4; a++) *o++ = s.HasDiffuseColor[a];
	for (int a = 0; a < 4; a++) *o++ = s.HasSpecularColor[a];
	for (int a = 0; a < 4; a++) *o++ = s.HasDiffuseIllumination[a];
	for (int a = 0; a < 4; a++) for (int b = 0; b < 8; b++) *o++ = s.HasTexCoords[a][b];
	*o++ = (W)s.UVSplitCount; *o++ = (W)s.StripCount; *o++ = (W)s.MaxStripLength; *o++ = B(s.AvgStripLength);
	return o;
}

/* 100 equal-key elements through qsort: true when the order of equal keys is kept (see the header) */
extern "C" int ref_dpw_qsort_is_stable(void)
{
	struct E { int key; int pos; } a[200];
	for (int i = 0; i < 200; i++) { a[i].key = (i * 7) % 5; a[i].pos = i; }
	qsort(a, 200, sizeof(E), [](const void *x, const void *y) -> int { return ((const E *)x)->key - ((const E *)y)->key; });
	for (int i = 1; i < 200; i++)
		if (a[i - 1].key == a[i].key && a[i - 1].pos > a[i].pos) return 0;
	return 1;
}

extern "C" int ref_dpw_meshbuild(const W *in, W *out)
{
	W *o = out;
	int op = (int)in[0];
	if (op == 4) {
		VC v; FC f;
		o = WriteVert(v, o, false);
		o = WriteFace(f, o, false);
		return (int)(o - out);
	}
	if (op == 3) {
		FC f;
		const W *p = ReadFace(in + 1, f);
		for (int i = 0; i < 3; i++) f.VertIdx[i] = (int)p[i];
		f.Compute_Plane();
		bool d = f.Is_Degenerate();
		*o++ = B(f.Normal.X); *o++ = B(f.Normal.Y); *o++ = B(f.Normal.Z); *o++ = B(f.Dist); *o++ = d;
		return (int)(o - out);
	}
	int pass_count = (int)in[1], guess = (int)in[2], growth = (int)in[3], opass = (int)in[4], ostage = (int)in[5];
	bool compute_normals = in[6] != 0;
	int world_mode = (int)in[7], n = (int)in[8];
	const W *p = in + 9;
	TestWorld world(world_mode == 1);
	MeshBuilderClass mb(pass_count, guess, growth);
	mb.Set_Polygon_Ordering_Channel(opass, ostage);
	if (world_mode != 0) mb.Set_World_Info(&world);
	static FC *tmp = NULL;
	if (!tmp) tmp = new FC;
	if (op == 5) {
		for (int i = 0; i < n; i++) {
			p = ReadFace(p, *tmp);
			*o++ = (W)mb.Add_Face(*tmp);
		}
		for (int i = 0; i < n; i++) {
			const FC &f = mb.Faces[i];
			*o++ = B(f.Normal.X); *o++ = B(f.Normal.Y); *o++ = B(f.Normal.Z); *o++ = B(f.Dist); *o++ = (W)f.AddIndex;
			for (int k = 0; k < 3; k++) *o++ = (W)f.Verts[k].SmGroup;
		}
		mb.Set_World_Info(NULL);
		return (int)(o - out);
	}
	for (int i = 0; i < n; i++) {
		p = ReadFace(p, *tmp);
		mb.Add_Face(*tmp);
	}
	mb.Build_Mesh(compute_normals);
	if (op == 0) {
		*o++ = (W)mb.Get_Vertex_Count();
		*o++ = (W)mb.Get_Face_Count();
		o = WriteStats(mb.Get_Mesh_Stats(), o);
		for (int i = 0; i < mb.Get_Vertex_Count(); i++) o = WriteVert(mb.Get_Vertex(i), o, true);
		for (int i = 0; i < mb.Get_Face_Count(); i++) o = WriteFace(mb.Get_Face(i), o, false);
		Vector3 mn, mx, c; float r;
		mb.Compute_Bounding_Box(&mn, &mx);
		*o++ = B(mn.X); *o++ = B(mn.Y); *o++ = B(mn.Z); *o++ = B(mx.X); *o++ = B(mx.Y); *o++ = B(mx.Z);
		mb.Compute_Bounding_Sphere(&c, &r);
		*o++ = B(c.X); *o++ = B(c.Y); *o++ = B(c.Z); *o++ = B(r);
	} else if (op == 1) {
		float delta = F(*p);
		*o++ = mb.Verify_Face_Normals();
		if (mb.FaceCount > 0) mb.Faces[0].Normal.X += delta;
		*o++ = mb.Verify_Face_Normals();
	} else if (op == 2) {
		float delta = F(*p);
		mb.Verts[0].Position.X += delta;
		mb.Compute_Face_Normals();
		*o++ = mb.Verify_Face_Normals();
		for (int i = 0; i < mb.FaceCount; i++) {
			const FC &f = mb.Faces[i];
			*o++ = B(f.Normal.X); *o++ = B(f.Normal.Y); *o++ = B(f.Normal.Z); *o++ = B(f.Dist);
		}
	}
	mb.Set_World_Info(NULL);
	return (int)(o - out);
}
