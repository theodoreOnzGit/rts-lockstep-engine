/* Differential-test wrapper for the draft src/drafts/ww3d/matinfo.rs (GitHub #215, group "ww3d7"), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/matinfo.h   the classes (w7_matinfo_h.inc) and the inline members (w7_matinfo_inl.inc)
**   matinfo.cpp        from MaterialInfoClass::MaterialInfoClass(void) to the end (w7_matinfo_cpp.inc)
** cut VERBATIM by build.rs (`drafts_ww3d7`, each anchor asserted to occur once). The real shader.h (ShaderClass renamed W7ShaderClass), vector.h
** (DynamicVectorClass), refcount.h.
**
** Stand-ins, each for something the files call outside themselves:
**   TextureClass, VertexMaterialClass: objects from fixed pools (ids 1..8; clones get ids from 100) with a name (a pool of case variants);
**       Add_Ref and Release_Ref do nothing (the draft does not model reference counts); Clone makes a new object with the next id and the
**       same name; Reset_Mappers, Make_Unique log an event; Are_Mappers_Time_Variant answers from a flag per material.
**   MeshMatDescClass, MeshModelClass: a scripted material description (passes, per-vertex and single vertex materials, per-polygon and single
**       shaders, per-stage per-polygon and single textures); the Set_* calls log an event. MeshMatDescClass::MAX_TEX_STAGES is the
**       value of meshmatdesc.h:69 (2). W3DMPO, W3DMPO_GLUE are the real ones (the prelude's). stricmp: strcasecmp.
**
** Entry point (extern "C", tests/differential_draft_ww3d7_matinfo.rs): ref_dw7_matinfo(in, nin, out, cap) returns the words written.
**   MD (mesh description): P passes, vertex count, polygon count, per pass: has material array, vertex-count handles (if so), single material,
**       has shader array, polygon-count shader bits (if so), single shader bits, per stage 0..1: has texture array, polygon-count handles (if so), single
**       texture. Handles: 0 NULL, 1..8 the pool.
**   op 0 info: [0, flags per vertex material (8 bits: time variant), n ops, per op (kind, a, b)] -> per op the result words, then the final state
**        kinds: 0 Add_Vertex_Material(a), 1 Add_Texture(a), 2 Get_Vertex_Material_Index(name a), 3 Get_Texture_Index(name a), 4 Peek_Vertex_Material(a),
**        5 Peek_Vertex_Material(name a), 6 Peek_Texture(a), 7 Peek_Texture(name a), 8 Replace_Material(a, b), 9 Replace_Texture(a, b), 10 Reset,
**        11 Clone (the clone becomes the object), 12 Reset_Texture_Mappers, 13 Make_Vertex_Materials_Unique, 14 Has_Time_Variant_Texture_Mappers
**   op 1 remapper: [1, n vertex materials, n textures, MD, n ops, per op (kind, a)]; the destination info holds the source's objects rotated by one
**        (slot k: object (k + 1) mod n) -> per op the results; kinds 0 Remap_Texture(a),
**        1 Remap_Vertex_Material(a), 2 Remap_Mesh (the MD) -> the destination's Set_* events
**   op 2 collector: [2, n ops, per op (kind, ...)] kinds 0 Collect_Materials(MD), 1 Reset, 2 Add_Texture(a), 3 Add_Shader(bits), 4 Add_Vertex_Material(a),
**        5 Find_Shader(bits), 6 Find_Texture(a), 7 Find_Vertex_Material(a); results: the three counts after each op, then the lists */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <strings.h>
#include <string>
#include <vector>

#include "always.h"
#include "wwdebug.h"
#include "refcount.h"
#include "vector.h"

#define stricmp strcasecmp
#ifndef W3DNEW
#define W3DNEW new
#endif
#ifndef W3DNEWARRAY
#define W3DNEWARRAY new
#endif

typedef uint32_t W;
static std::vector<W> g_out;
static void put(W w) { g_out.push_back(w); }

#define ShaderClass W7ShaderClass
#define TextureClass W7_TextureClass /* the forced prelude declares a TextureClass of its own */
#define VertexMaterialClass W7_VertexMaterialClass
class StringClass;
#include "shader.h"

static const char *const NAMES[8] = { "Tex", "tex", "TEX", "Wood", "wood", "", "Stone", "stone2" };
static int g_next_clone = 100;
static std::vector<W> g_events;

class TextureClass {
public:
	int id;
	const char *Get_Texture_Name(void) const { return NAMES[id - 1]; }
	void Add_Ref(void) {}
	void Release_Ref(void) {}
};
class VertexMaterialClass {
public:
	int id;
	int name;
	bool variant;
	const char *Get_Name(void) const { return NAMES[name]; }
	void Add_Ref(void) {}
	void Release_Ref(void) {}
	VertexMaterialClass *Clone(void) const
	{
		VertexMaterialClass *c = new VertexMaterialClass(*this);
		c->id = g_next_clone++;
		return c;
	}
	void Reset_Mappers(void) { g_events.push_back(1); g_events.push_back((W)id); }
	void Make_Unique(void) { g_events.push_back(2); g_events.push_back((W)id); }
	bool Are_Mappers_Time_Variant(void) { return variant; }
};

static TextureClass g_tex[8];
static VertexMaterialClass g_vm[8];
static void init_pools(W variant_flags)
{
	for (int k = 0; k < 8; ++k) {
		g_tex[k].id = k + 1;
		g_vm[k].id = k + 1;
		g_vm[k].name = k;
		g_vm[k].variant = ((variant_flags >> k) & 1) != 0;
	}
	g_next_clone = 100;
}
static TextureClass *tp(W h) { return h == 0 ? 0 : &g_tex[h - 1]; }
static VertexMaterialClass *vp(W h) { return h == 0 ? 0 : &g_vm[h - 1]; }
static W th(TextureClass *t) { return t == 0 ? 0 : (W)t->id; }
static W vh(VertexMaterialClass *v) { return v == 0 ? 0 : (W)v->id; }

class MeshMatDescClass {
public:
	enum { MAX_TEX_STAGES = 2 };
	struct Pass {
		bool has_mat_array;
		std::vector<W> mats;
		W single_mat;
		bool has_shader_array;
		std::vector<W> shaders;
		W single_shader;
		bool has_tex_array[2];
		std::vector<W> texs[2];
		W single_tex[2];
	};
	std::vector<Pass> passes;
	int vertex_count, poly_count;
	int Get_Pass_Count(void) const { return (int)passes.size(); }
	int Get_Vertex_Count(void) const { return vertex_count; }
	int Get_Polygon_Count(void) const { return poly_count; }
	bool Has_Material_Array(int pass) const { return passes[pass].has_mat_array; }
	VertexMaterialClass *Peek_Material(int v, int pass) const { return vp(passes[pass].mats[v]); }
	VertexMaterialClass *Peek_Single_Material(int pass) const { return vp(passes[pass].single_mat); }
	bool Has_Shader_Array(int pass) const { return passes[pass].has_shader_array; }
	ShaderClass Get_Shader(int poly, int pass) const { return ShaderClass(passes[pass].shaders[poly]); }
	ShaderClass Get_Single_Shader(int pass) const { return ShaderClass(passes[pass].single_shader); }
	bool Has_Texture_Array(int pass, int stage) const { return passes[pass].has_tex_array[stage]; }
	TextureClass *Peek_Texture(int poly, int pass, int stage) const { return tp(passes[pass].texs[stage][poly]); }
	TextureClass *Peek_Single_Texture(int pass, int stage) const { return tp(passes[pass].single_tex[stage]); }
	void Set_Material(int v, VertexMaterialClass *m, int pass) { g_events.push_back(10); g_events.push_back((W)v); g_events.push_back(vh(m)); g_events.push_back((W)pass); }
	void Set_Single_Material(VertexMaterialClass *m, int pass) { g_events.push_back(11); g_events.push_back(vh(m)); g_events.push_back((W)pass); }
	void Set_Texture(int poly, TextureClass *t, int pass, int stage) { g_events.push_back(12); g_events.push_back((W)poly); g_events.push_back(th(t)); g_events.push_back((W)pass); g_events.push_back((W)stage); }
	void Set_Single_Texture(TextureClass *t, int pass, int stage) { g_events.push_back(13); g_events.push_back(th(t)); g_events.push_back((W)pass); g_events.push_back((W)stage); }
	/* MeshModelClass's (derived) members the collector calls */
	VertexMaterialClass *Get_Single_Material(int pass) const { return vp(passes[pass].single_mat); }
};
class MeshModelClass : public MeshMatDescClass {};

#include "w7_matinfo_h.inc"
#include "w7_matinfo_inl.inc"
#include "w7_matinfo_cpp.inc"

struct Rd {
	const W *p;
	W u(void) { return *p++; }
};

static void read_md(Rd &r, MeshModelClass &m)
{
	W np = r.u();
	m.vertex_count = (int)r.u();
	m.poly_count = (int)r.u();
	m.passes.resize(np);
	for (W k = 0; k < np; ++k) {
		MeshMatDescClass::Pass &p = m.passes[k];
		p.has_mat_array = r.u() != 0;
		p.mats.clear();
		if (p.has_mat_array) for (int j = 0; j < m.vertex_count; ++j) p.mats.push_back(r.u());
		p.single_mat = r.u();
		p.has_shader_array = r.u() != 0;
		p.shaders.clear();
		if (p.has_shader_array) for (int j = 0; j < m.poly_count; ++j) p.shaders.push_back(r.u());
		p.single_shader = r.u();
		for (int s = 0; s < 2; ++s) {
			p.has_tex_array[s] = r.u() != 0;
			p.texs[s].clear();
			if (p.has_tex_array[s]) for (int j = 0; j < m.poly_count; ++j) p.texs[s].push_back(r.u());
			p.single_tex[s] = r.u();
		}
	}
}

static void dump_info(MaterialInfoClass &mi)
{
	put((W)mi.Vertex_Material_Count());
	for (int k = 0; k < mi.Vertex_Material_Count(); ++k) put(vh(mi.Peek_Vertex_Material(k)));
	put((W)mi.Texture_Count());
	for (int k = 0; k < mi.Texture_Count(); ++k) put(th(mi.Peek_Texture(k)));
}

extern "C" int ref_dw7_matinfo(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	g_out.clear();
	g_events.clear();
	Rd r = { in };
	W op = r.u();
	switch (op) {
	case 0: {
		init_pools(r.u());
		MaterialInfoClass *mi = new MaterialInfoClass;
		W n = r.u();
		for (W k = 0; k < n; ++k) {
			W kind = r.u(), a = r.u(), b = r.u();
			switch (kind) {
			case 0: put((W)mi->Add_Vertex_Material(vp(a))); break;
			case 1: put((W)mi->Add_Texture(tp(a))); break;
			case 2: put((W)mi->Get_Vertex_Material_Index(NAMES[a])); break;
			case 3: put((W)mi->Get_Texture_Index(NAMES[a])); break;
			case 4: put(vh(mi->Peek_Vertex_Material((int)a))); break;
			case 5: put(vh(mi->Peek_Vertex_Material(NAMES[a]))); break;
			case 6: put(th(mi->Peek_Texture((int)a))); break;
			case 7: break; /* Peek_Texture(const char *) is declared (matinfo.h:104) and never defined upstream: calling it does not link */
			case 8: mi->Replace_Material((int)a, vp(b)); break;
			case 9: mi->Replace_Texture((int)a, tp(b)); break;
			case 10: mi->Reset(); break;
			case 11: { MaterialInfoClass *c = mi->Clone(); delete mi; mi = c; break; }
			case 12: g_events.clear(); mi->Reset_Texture_Mappers(); for (size_t j = 0; j < g_events.size(); ++j) put(g_events[j]); break;
			case 13: g_events.clear(); mi->Make_Vertex_Materials_Unique(); for (size_t j = 0; j < g_events.size(); ++j) put(g_events[j]); break;
			default: put((W)mi->Has_Time_Variant_Texture_Mappers()); break;
			}
			dump_info(*mi);
		}
		delete mi;
		break;
	}
	case 1: {
		init_pools(0);
		W nv = r.u(), nt = r.u();
		MaterialInfoClass *src = new MaterialInfoClass, *dest = new MaterialInfoClass;
		/* the destination is the source rotated by one (a handle of the source can be a handle of the destination, which a cache keyed on the wrong side confuses) */
		for (W k = 0; k < nv; ++k) { src->Add_Vertex_Material(&g_vm[k]); dest->Add_Vertex_Material(&g_vm[(k + 1) % nv]); }
		for (W k = 0; k < nt; ++k) { src->Add_Texture(&g_tex[k]); dest->Add_Texture(&g_tex[(k + 1) % nt]); }
		MeshModelClass md;
		read_md(r, md);
		MaterialRemapperClass *rm = new MaterialRemapperClass(src, dest);
		W n = r.u();
		for (W k = 0; k < n; ++k) {
			W kind = r.u(), a = r.u();
			if (kind == 0) put(th(rm->Remap_Texture(tp(a))));
			else if (kind == 1) put(vh(rm->Remap_Vertex_Material(vp(a))));
			else {
				MeshMatDescClass dd;
				g_events.clear();
				rm->Remap_Mesh(&md, &dd);
				put((W)g_events.size());
				for (size_t j = 0; j < g_events.size(); ++j) put(g_events[j]);
			}
		}
		delete rm;
		delete src;
		delete dest;
		break;
	}
	case 2: {
		init_pools(0);
		MaterialCollectorClass *mc = new MaterialCollectorClass;
		W n = r.u();
		for (W k = 0; k < n; ++k) {
			W kind = r.u();
			switch (kind) {
			case 0: { MeshModelClass md; read_md(r, md); mc->Collect_Materials(&md); break; }
			case 1: mc->Reset(); break;
			case 2: mc->Add_Texture(tp(r.u())); break;
			case 3: mc->Add_Shader(ShaderClass(r.u())); break;
			case 4: mc->Add_Vertex_Material(vp(r.u())); break;
			case 5: put((W)mc->Find_Shader(ShaderClass(r.u()))); break;
			case 6: put((W)mc->Find_Texture(tp(r.u()))); break;
			default: put((W)mc->Find_Vertex_Material(vp(r.u()))); break;
			}
			put((W)mc->Get_Shader_Count());
			put((W)mc->Get_Vertex_Material_Count());
			put((W)mc->Get_Texture_Count());
		}
		for (int k = 0; k < mc->Get_Shader_Count(); ++k) put(mc->Peek_Shader(k).Get_Bits());
		for (int k = 0; k < mc->Get_Vertex_Material_Count(); ++k) put(vh(mc->Peek_Vertex_Material(k)));
		for (int k = 0; k < mc->Get_Texture_Count(); ++k) put(th(mc->Peek_Texture(k)));
		delete mc;
		break;
	}
	default: return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
