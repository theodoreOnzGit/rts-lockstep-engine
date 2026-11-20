/* Differential-test wrapper for src/drafts/sweep_ww3d/meshmdlio.rs (GitHub #215, group ww3d4; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_ww3d4`): WW3D2/meshmdlio.cpp MeshModelClass::{post_process_fog, get_sort_flags(int),
** get_sort_flags(), compute_static_sort_levels, modify_for_overbright} and WW3D2/shader.cpp ShaderClass::{Enable_Fog,
** Get_SS_Category}.
** Stand-ins: ShaderClass is the real shader.h, renamed DW4ShaderClass so that no symbol meets the other libraries' ShaderClass
** (the mesh library's Get_SS_Category aborts); ShaderClass::Report_Unable_To_Fog is empty (its body is #ifdef WWDEBUG upstream);
** MeshModelClass / MeshMatDescClass are in an anonymous namespace and hold only what the cuts touch (PassCount, Shader, ShaderArray,
** Texture, TextureArray, Material, MaterialArray, PolyCount, VertexCount, SortLevel, and the accessors of meshmdl.h /
** meshmatdesc.h, written as those headers write them); CurMatDesc == DefMatDesc; ShareBufferClass, TexBufferClass, MatBufferClass
** are arrays with Get_Count/Get_Element/Peek_Element/Set_Element; TextureClass pointers are opaque ids; VertexMaterialClass holds a
** UV source; REF_PTR_SET is a plain assignment (reference counts are not under test); NEW_REF is `new`.
** Entry point: ref_dw4_mmio_run(in, nin, out, cap): in = [op, pass_count, poly_count, vertex_count, sort_level, arg, then per pass
**  0..3: shader, has_array, array words...; then for passes 0 and 1: tex0, has_texarray, ids..., mat_uv, has_matarray, uv words...];
**  op 0 get_sort_flags(arg) 1 get_sort_flags() 2 compute_static_sort_levels 3 modify_for_overbright 4 post_process_fog.
**  out = [result, sort_level, then the whole state in the input's order]. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#define TextureClass DW4_TextureClass /* the forced prelude declares a TextureClass of its own */
#include "w3d_file.h"

#define ShaderClass DW4ShaderClass
#include "shader.h"

#define REF_PTR_SET(dst, src) ((dst) = (src))
#define NEW_REF(T, args) (new T args)

namespace {

class TextureClass;
class VertexMaterialClass { public: int uv; int Get_UV_Source(int) { return uv; } void Set_UV_Source(int, int v) { uv = v; } };

template <class T> class ShareBufferClass {
public:
	std::vector<T> a;
	int Get_Count() const { return (int)a.size(); }
	T & Get_Element(int i) { return a[i]; }
};
class TexBufferClass {
public:
	std::vector<TextureClass*> a;
	TexBufferClass(int n, const char *) : a(n, (TextureClass*)0) {}
	void Set_Element(int i, TextureClass * t) { a[i] = t; }
	TextureClass * Peek_Element(int i) { return a[i]; }
};
class MatBufferClass {
public:
	std::vector<VertexMaterialClass*> a;
	VertexMaterialClass * Peek_Element(int i) { return a[i]; }
};

#define MAX_PASSES 4
#define MAX_TEX_STAGES 2

class MeshMatDescClass {
public:
	int Get_Pass_Count() const { return PassCount; }
	int PassCount, PolyCount, VertexCount;
	ShaderClass Shader[MAX_PASSES];
	ShareBufferClass<ShaderClass> * ShaderArray[MAX_PASSES];
	TextureClass * Texture[MAX_PASSES][MAX_TEX_STAGES];
	TexBufferClass * TextureArray[MAX_PASSES][MAX_TEX_STAGES];
	VertexMaterialClass * Material[MAX_PASSES];
	MatBufferClass * MaterialArray[MAX_PASSES];
	ShaderClass Get_Single_Shader(int pass) const { return Shader[pass]; }
	void Set_Single_Shader(ShaderClass s, int pass) { Shader[pass] = s; }
	bool Has_Shader_Array(int pass) const { return ShaderArray[pass] != NULL; }
	ShaderClass Get_Shader(int pidx, int pass) const { if (ShaderArray[pass]) return ShaderArray[pass]->Get_Element(pidx); return Shader[pass]; }
	void Set_Shader(int pidx, ShaderClass s, int pass) { ShaderArray[pass]->Get_Element(pidx) = s; }
	int Get_Polygon_Count() const { return PolyCount; }
};

class MeshModelClass {
public:
	MeshMatDescClass * DefMatDesc;
	MeshMatDescClass * CurMatDesc;
	int PolyCount, VertexCount, SortLevel;
	const char * Get_Name() const { return "ref"; }
	int Get_Pass_Count() const { return CurMatDesc->Get_Pass_Count(); }
	void Set_Single_Shader(ShaderClass shader, int pass = 0) { CurMatDesc->Set_Single_Shader(shader, pass); }
	ShaderClass Get_Single_Shader(int pass = 0) const { return CurMatDesc->Get_Single_Shader(pass); }
	void Set_Shader(int pidx, ShaderClass shader, int pass = 0) { CurMatDesc->Set_Shader(pidx, shader, pass); }
	bool Has_Shader_Array(int pass) const { return CurMatDesc->Has_Shader_Array(pass); }
	ShaderClass Get_Shader(int pidx, int pass = 0) const { return CurMatDesc->Get_Shader(pidx, pass); }
	int Get_Polygon_Count() const { return CurMatDesc->Get_Polygon_Count(); }
	void post_process_fog(void);
	unsigned int get_sort_flags(int pass) const;
	unsigned int get_sort_flags(void) const;
	void compute_static_sort_levels(void);
	void modify_for_overbright(void);
};

}

void DW4ShaderClass::Report_Unable_To_Fog (const char *source) {}

#include "mm_fog.inc"
#include "mm_sf1.inc"
#include "mm_sf0.inc"
#include "mm_css.inc"
#include "mm_ofb.inc"
#include "sh_fog.inc"
#include "sh_ssc.inc"

namespace {

static void put(std::vector<uint32_t> & o, uint32_t w) { o.push_back(w); }

static void dump(std::vector<uint32_t> & o, MeshMatDescClass & d, int sort_level, uint32_t result)
{
	put(o, result); put(o, (uint32_t)sort_level);
	for (int p = 0; p < MAX_PASSES; p++) {
		put(o, d.Shader[p].Get_Bits());
		if (d.ShaderArray[p]) { put(o, 1); for (int i = 0; i < d.ShaderArray[p]->Get_Count(); i++) put(o, d.ShaderArray[p]->Get_Element(i).Get_Bits()); } else put(o, 0);
	}
	for (int p = 0; p < 2; p++) {
		put(o, (uint32_t)(uintptr_t)d.Texture[p][0]);
		if (d.TextureArray[p][0]) { put(o, 1); for (size_t i = 0; i < d.TextureArray[p][0]->a.size(); i++) put(o, (uint32_t)(uintptr_t)d.TextureArray[p][0]->a[i]); } else put(o, 0);
		put(o, (uint32_t)d.Material[p]->uv);
		if (d.MaterialArray[p]) { put(o, 1); for (size_t i = 0; i < d.MaterialArray[p]->a.size(); i++) put(o, (uint32_t)d.MaterialArray[p]->a[i]->uv); } else put(o, 0);
	}
}

}

extern "C" int ref_dw4_mmio_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	const uint32_t * p = in;
	int op = (int)*p++;
	MeshMatDescClass d; memset(&d, 0, sizeof(d));
	d.PassCount = (int)*p++; d.PolyCount = (int)*p++; d.VertexCount = (int)*p++;
	MeshModelClass m;
	m.DefMatDesc = m.CurMatDesc = &d;
	m.PolyCount = d.PolyCount; m.VertexCount = d.VertexCount; m.SortLevel = (int)*p++;
	int arg = (int)*p++;
	for (int pass = 0; pass < MAX_PASSES; pass++) {
		d.Shader[pass] = ShaderClass(*p++);
		if (*p++) { d.ShaderArray[pass] = new ShareBufferClass<ShaderClass>(); for (int i = 0; i < d.PolyCount; i++) d.ShaderArray[pass]->a.push_back(ShaderClass(*p++)); }
	}
	for (int pass = 0; pass < 2; pass++) {
		d.Texture[pass][0] = (TextureClass*)(uintptr_t)*p++;
		if (*p++) { d.TextureArray[pass][0] = new TexBufferClass(d.PolyCount, "x"); for (int i = 0; i < d.PolyCount; i++) d.TextureArray[pass][0]->a[i] = (TextureClass*)(uintptr_t)*p++; }
		d.Material[pass] = new VertexMaterialClass(); d.Material[pass]->uv = (int)*p++;
		if (*p++) { d.MaterialArray[pass] = new MatBufferClass(); for (int i = 0; i < d.VertexCount; i++) { VertexMaterialClass * v = new VertexMaterialClass(); v->uv = (int)*p++; d.MaterialArray[pass]->a.push_back(v); } }
	}
	uint32_t result = 0;
	switch (op) {
	case 0: result = m.get_sort_flags(arg); break;
	case 1: result = m.get_sort_flags(); break;
	case 2: m.compute_static_sort_levels(); break;
	case 3: m.modify_for_overbright(); break;
	case 4: m.post_process_fog(); break;
	}
	std::vector<uint32_t> o;
	dump(o, d, m.SortLevel, result);
	if ((int)o.size() > cap) return -1;
	memcpy(out, o.data(), o.size() * 4);
	return (int)o.size();
}
