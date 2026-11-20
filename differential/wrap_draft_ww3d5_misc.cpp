/* Differential-test wrapper for the small WW3D2 drafts (GitHub #215, upstream commit 0a05454d), group "ww3d5":
** src/drafts/ww3d/{static_sort_list, shader_presets, w3dexclusionlist, matpass, rinfo}.rs.
**
** From upstream, cut VERBATIM by build.rs (`drafts_ww3d5`, each anchor asserted to occur once) into OUT_DIR/draft_ww3d5_gen:
**   static_sort_list.h    from `// Just defines the interface` to the end (w5_ssl_h.inc): StaticSortListClass, DefaultStaticSortListClass
**   static_sort_list.cpp  from the constructor to the end (w5_ssl_cpp.inc)
**   shader.cpp            the preset block, from `// Texturing, zbuffer, primary gradient, no blending` to `_PresetMultiplicativeSpriteShader`
**                         (w5_presets.inc), and ShaderClass::Enable_Fog (w5_fog.inc)
**   w3dexclusionlist.cpp  from the constructor to the end (w5_excl.inc)
**   matpass.cpp           the constructor and Set_Shader (w5_mp_ctor.inc, w5_mp_shader.inc) and the static initialiser (w5_mp_static.inc)
**   rinfo.h               from `const unsigned MAX_ADDITIONAL_MATERIAL_PASSES` to the end (w5_rinfo_h.inc)
**   rinfo.cpp             from the constructor to the end (w5_rinfo_cpp.inc)
** The real shader.h (ShaderClass renamed W5ShaderClass so that no symbol meets the other libraries' ShaderClass), matpass.h, w3d_file.h
** (MAX_SORT_LEVEL), refcount.h and hashtemplate.h are the real ones.
**
** Stand-ins, each for something the files call outside themselves:
**   RenderObjClass, RenderHookClass, RenderInfoClass (static_sort_list): a scripted object (id, has hook, Pre_Render answer) whose Render,
**       Pre_Render, Post_Render and Release_Ref log an event; RefRenderObjListClass is a FIFO (Add_Tail without Add_Ref,
**       Remove_Head); TheDX8MeshRenderer.Flush() logs an event.
**   StringClass (w3dexclusionlist): a std::string with the members the file calls (Peek_Buffer, Get_Length, ==); DynamicVectorClass is the
**       real vector.h's; PrototypeClass, HTreeClass, HAnimClass: a Get_Name.
**   MaterialPassClass (rinfo): Add_Ref and Release_Ref do nothing (the draft does not model reference counts); CameraClass is empty.
**   ShaderClass::Report_Unable_To_Fog is empty (its body is #ifdef WWDEBUG upstream). MaterialPassClass::Install_Materials is empty
**       (device only) and its destructor does nothing (the texture and material references are not modelled).
**
** Entry point (extern "C", tests/differential_draft_ww3d5_misc.rs): ref_dw5_misc(in, nin, out, cap) returns the words written.
**   op 0 sort list: [0, n objects, per object (has hook, Pre_Render answer), n ops, per op (kind, a, b)] -> events..., min, max
**        kinds: 0 Set_Min_Sort a, 1 Set_Max_Sort a, 2 Add_To_List(object a, level b), 3 Render_And_Clear.
**        events: 1 id Pre_Render, 2 id Post_Render, 3 id Render, 4 id Release_Ref, 5 Flush; each Render_And_Clear ends with 0xFFFFFFFF
**   op 1 preset: [1, index] -> [bits]  (index into the table below)
**   op 2 exclusion list: [2, n names, (length, bytes) each, n queries, (kind, length, bytes) each] -> one word per query
**        kinds: 0 Is_Excluded(PrototypeClass *), 1 (HTreeClass *), 2 (HAnimClass *), 3 (const char *)
**   op 3 material pass: [3, shader bits, translucent flag, per polygon culling flag] -> [Peek_Shader bits, translucent, per polygon,
**        default translucent, default per polygon culling]
**   op 4 render info: [4, n ops, per op (kind, a)] -> per op (count, current flags, last pass) then the passes
**        kinds: 0 Push_Material_Pass(a; 0 is NULL), 1 Pop_Material_Pass, 2 Push_Override_Flags(a), 3 Pop_Override_Flags, 4 Current_Override_Flags() = a */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <string>
#include <vector>
#include <deque>

#include "always.h"
#include "wwdebug.h"
#include "w3d_file.h"
#include "refcount.h"

#define TextureClass W5_TextureClass /* the forced prelude declares a TextureClass of its own */
class TextureClass;
class VertexMaterialClass;
class OBBoxClass;
class CameraClass {};
class LightEnvironmentClass;
class VisRasterizerClass;
class BWRenderClass;
class TexProjectClass;
class MeshModelClass;

#define ShaderClass W5ShaderClass
#include "shader.h"

typedef uint32_t W;
static std::vector<W> g_out;
static void put(W w) { g_out.push_back(w); }

/* ---------------- shader presets, Enable_Fog ---------------- */
void W5ShaderClass::Report_Unable_To_Fog(const char *) {}
#include "w5_presets.inc"
#include "w5_fog.inc"

/* ---------------- static_sort_list ---------------- */
namespace sortlist {

struct RenderInfoClass {};
struct RenderObjClass;
struct RenderHookClass {
	bool pre;
	bool Pre_Render(RenderObjClass *, RenderInfoClass &);
	void Post_Render(RenderObjClass *, RenderInfoClass &);
};
struct RenderObjClass {
	int id;
	RenderHookClass hook;
	bool has_hook;
	RenderHookClass *Get_Render_Hook(void) { return has_hook ? &hook : NULL; }
	void Render(RenderInfoClass &) { put(3); put((W)id); }
	void Release_Ref(void) { put(4); put((W)id); }
};
bool RenderHookClass::Pre_Render(RenderObjClass *o, RenderInfoClass &) { put(1); put((W)o->id); return pre; }
void RenderHookClass::Post_Render(RenderObjClass *o, RenderInfoClass &) { put(2); put((W)o->id); }

struct RefRenderObjListClass {
	std::deque<RenderObjClass *> q;
	void Add_Tail(RenderObjClass *o, bool) { q.push_back(o); }
	RenderObjClass *Remove_Head(void)
	{
		if (q.empty()) return NULL;
		RenderObjClass *o = q.front();
		q.pop_front();
		return o;
	}
};
struct { void Flush(void) { put(5); } } TheDX8MeshRenderer;

#include "w5_ssl_h.inc"
#include "w5_ssl_cpp.inc"

} // namespace sortlist

/* ---------------- w3dexclusionlist ---------------- */
/* stand-in: StringClass (global, completing shader.h's forward declaration) */
class StringClass {
public:
	std::string s;
	StringClass() {}
	StringClass(const char *c) : s(c) {}
	char *Peek_Buffer(void) { return &s[0]; }
	const char *Peek_Buffer(void) const { return s.c_str(); }
	unsigned int Get_Length(void) const { return (unsigned int)s.size(); }
	bool operator==(const StringClass &o) const { return s == o.s; }
	bool operator!=(const StringClass &o) const { return s != o.s; }
	operator const char *() const { return s.c_str(); }
};
#define __WWSTRING_H /* hashtemplate.h would otherwise include the real wwstring.h, which declares StringClass again */
#include "hashtemplate.h"

namespace excl {

struct PrototypeClass { StringClass name; const char *Get_Name(void) const { return name.Peek_Buffer(); } };
struct HTreeClass { StringClass name; const char *Get_Name(void) const { return name.Peek_Buffer(); } };
struct HAnimClass { StringClass name; const char *Get_Name(void) const { return name.Peek_Buffer(); } };

class W3DExclusionListClass
{
public:
	W3DExclusionListClass(const DynamicVectorClass<StringClass> & names);
	~W3DExclusionListClass(void);
	bool	Is_Excluded(PrototypeClass * proto) const;
	bool	Is_Excluded(HTreeClass * htree) const;
	bool	Is_Excluded(HAnimClass * hanim) const;
	bool	Is_Excluded(const char * root_name) const;
protected:
	const DynamicVectorClass<StringClass> &	Names;
	HashTemplateClass<StringClass,int>			NameHash;
};

#include "w5_excl.inc"

} // namespace excl

/* ---------------- matpass ---------------- */
#include "matpass.h"
MaterialPassClass::~MaterialPassClass(void) {}
void MaterialPassClass::Install_Materials(void) const {}
#include "w5_mp_static.inc"
#include "w5_mp_ctor.inc"
#include "w5_mp_shader.inc"

/* ---------------- rinfo ---------------- */
namespace rinfo {

struct MaterialPassClass {
	void Add_Ref(void) {}
	void Release_Ref(void) {}
};
#include "w5_rinfo_h.inc"
#include "w5_rinfo_cpp.inc"

} // namespace rinfo

static void rd_str(const W *in, int &i, std::string &s)
{
	W len = in[i++];
	s.clear();
	for (W k = 0; k < len; ++k) s.push_back((char)in[i++]);
}

extern "C" int ref_dw5_misc(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	g_out.clear();
	int i = 0;
	W op = in[i++];
	switch (op) {
	case 0: {
		using namespace sortlist;
		W n = in[i++];
		std::vector<RenderObjClass> objs(n);
		for (W k = 0; k < n; ++k) {
			objs[k].id = (int)k;
			objs[k].has_hook = in[i++] != 0;
			objs[k].hook.pre = in[i++] != 0;
		}
		DefaultStaticSortListClass list;
		RenderInfoClass ri;
		W nops = in[i++];
		for (W k = 0; k < nops; ++k) {
			W kind = in[i++], a = in[i++], b = in[i++];
			switch (kind) {
			case 0: list.Set_Min_Sort(a); break;
			case 1: list.Set_Max_Sort(a); break;
			case 2: list.Add_To_List(&objs[a], b); break;
			default: list.Render_And_Clear(ri); put(0xFFFFFFFFu); break;
			}
		}
		put(list.Get_Min_Sort());
		put(list.Get_Max_Sort());
		break;
	}
	case 1: {
		W idx = in[i++];
		static const W5ShaderClass *table[] = {
			&W5ShaderClass::_PresetOpaqueShader, &W5ShaderClass::_PresetAdditiveShader, &W5ShaderClass::_PresetBumpenvmapShader,
			&W5ShaderClass::_PresetAlphaShader, &W5ShaderClass::_PresetMultiplicativeShader, &W5ShaderClass::_PresetOpaque2DShader,
			&W5ShaderClass::_PresetOpaqueSpriteShader, &W5ShaderClass::_PresetAdditive2DShader, &W5ShaderClass::_PresetAlpha2DShader,
			&W5ShaderClass::_PresetAdditiveSpriteShader, &W5ShaderClass::_PresetAlphaSpriteShader, &W5ShaderClass::_PresetOpaqueSolidShader,
			&W5ShaderClass::_PresetAdditiveSolidShader, &W5ShaderClass::_PresetAlphaSolidShader, &W5ShaderClass::_PresetATest2DShader,
			&W5ShaderClass::_PresetATestSpriteShader, &W5ShaderClass::_PresetATestBlend2DShader, &W5ShaderClass::_PresetATestBlendSpriteShader,
			&W5ShaderClass::_PresetScreen2DShader, &W5ShaderClass::_PresetScreenSpriteShader, &W5ShaderClass::_PresetMultiplicative2DShader,
			&W5ShaderClass::_PresetMultiplicativeSpriteShader,
		};
		if (idx >= sizeof(table) / sizeof(table[0])) return -1;
		put(table[idx]->Get_Bits());
		break;
	}
	case 2: {
		DynamicVectorClass<StringClass> names;
		W n = in[i++];
		for (W k = 0; k < n; ++k) {
			std::string s;
			rd_str(in, i, s);
			names.Add(StringClass(s.c_str()));
		}
		excl::W3DExclusionListClass list(names);
		W nq = in[i++];
		for (W k = 0; k < nq; ++k) {
			W kind = in[i++];
			std::string s;
			rd_str(in, i, s);
			bool r;
			if (kind == 0) { excl::PrototypeClass p; p.name = StringClass(s.c_str()); r = list.Is_Excluded(&p); }
			else if (kind == 1) { excl::HTreeClass p; p.name = StringClass(s.c_str()); r = list.Is_Excluded(&p); }
			else if (kind == 2) { excl::HAnimClass p; p.name = StringClass(s.c_str()); r = list.Is_Excluded(&p); }
			else r = list.Is_Excluded(s.c_str());
			put((W)r);
		}
		break;
	}
	case 3: {
		MaterialPassClass mp;
		put((W)mp.Is_Enabled_On_Translucent_Meshes()); /* the defaults, first */
		put((W)MaterialPassClass::Is_Per_Polygon_Culling_Enabled());
		mp.Set_Shader(W5ShaderClass(in[i++]));
		mp.Enable_On_Translucent_Meshes(in[i++] != 0);
		MaterialPassClass::Enable_Per_Polygon_Culling(in[i++] != 0);
		put(mp.Peek_Shader().Get_Bits());
		put((W)mp.Is_Enabled_On_Translucent_Meshes());
		put((W)MaterialPassClass::Is_Per_Polygon_Culling_Enabled());
		MaterialPassClass::Enable_Per_Polygon_Culling(true);
		break;
	}
	case 4: {
		CameraClass cam;
		rinfo::RenderInfoClass ri(cam);
		W nops = in[i++];
		for (W k = 0; k < nops; ++k) {
			W kind = in[i++], a = in[i++];
			switch (kind) {
			case 0: ri.Push_Material_Pass(a == 0 ? NULL : (rinfo::MaterialPassClass *)(uintptr_t)a); break;
			case 1: ri.Pop_Material_Pass(); break;
			case 2: ri.Push_Override_Flags((rinfo::RenderInfoClass::RINFO_OVERRIDE_FLAGS)a); break;
			case 3: ri.Pop_Override_Flags(); break;
			default: ri.Current_Override_Flags() = (rinfo::RenderInfoClass::RINFO_OVERRIDE_FLAGS)a; break;
			}
			put((W)ri.Additional_Pass_Count());
			put((W)ri.Current_Override_Flags());
			put(ri.Additional_Pass_Count() > 0 ? (W)(uintptr_t)ri.Peek_Additional_Pass(ri.Additional_Pass_Count() - 1) : 0xFFFFFFFFu);
		}
		for (int k = 0; k < ri.Additional_Pass_Count(); ++k) put((W)(uintptr_t)ri.Peek_Additional_Pass(k));
		break;
	}
	default:
		return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	if (!g_out.empty()) memcpy(out, &g_out[0], g_out.size() * sizeof(W));
	return (int)g_out.size();
}
