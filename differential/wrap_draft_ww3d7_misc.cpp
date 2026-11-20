/* Differential-test wrapper for the small WW3D2 drafts (GitHub #215, upstream commit 0a05454d), group "ww3d7":
** src/drafts/ww3d/{polyinfo, assetstatus}.rs (and later more; the renderobjectrecycler is in wrap_draft_ww3d7_recycler.cpp).
**
** From upstream, cut VERBATIM by build.rs (`drafts_ww3d7`, each anchor asserted to occur once) into OUT_DIR/draft_ww3d7_gen:
**   polyinfo.h     the class (w7_poly_h.inc);  polyinfo.cpp  from Set_Texture to the end (w7_poly_cpp.inc)
**   assetstatus.h  the class (w7_asset_h.inc); assetstatus.cpp from `AssetStatusClass AssetStatusClass::Instance;` to the end (w7_asset_cpp.inc)
** The real shader.h (ShaderClass renamed W7ShaderClass so that no symbol meets the other libraries' ShaderClass), refcount.h and hashtemplate.h.
**
** Stand-ins, each for something the files call outside themselves:
**   TextureClass, VertexMaterialClass (polyinfo): empty classes whose Add_Ref and Release_Ref do nothing (the draft does not model reference counts).
**   StringClass (assetstatus): a std::string with the members the file and hashtemplate.h call (Peek_Buffer, Get_Length, ==, +=, Format).
**   RawFileClass (assetstatus): captures what `~AssetStatusClass` writes to asset_report.txt (the dtor body is `#ifdef WWDEBUG`: WWDEBUG is
**       defined for this cut, as in a debug build, which is the only build that writes the report); _strlwr: ASCII lower-casing (the CRT's).
**
**   matpass.cpp    the constructor and destructor (w7_mp_ctor.inc), Set_Texture (w7_mp_settex.inc), and from Set_Material to the end (w7_mp_rest.inc: Set_Material,
**                  Get_Texture, Get_Material); matpass.h is the real one (MaterialPassClass renamed W7MaterialPassClass); Install_Materials is device only (an empty stand-in, not cut)
**   rinfo.h / rinfo.cpp  the classes (w7_rinfo_h.inc) and everything from the RenderInfoClass constructor (w7_rinfo_cpp.inc), in namespace rinfo7; the
**                  classes are renamed W7* so that ww3d5's do not meet them; CameraClass is an empty stand-in, the pass is the real MaterialPassClass
** Entry point (extern "C", tests/differential_draft_ww3d7_misc.rs): ref_dw7_misc(in, nin, out, cap) returns the words written.
**   op 0 polyinfo: [0, n ops, per op (kind, a, b, c)] -> per op (texture handle, vertex material handle, shader bits); handle 0 is NULL
**        kinds: 0 construct anew (a, b handles, c shader bits), 1 Set_Texture(a), 2 Set_Vertex_Material(a), 3 Set_Shader(bits a),
**        4 Set(a, b, c bits)
**   op 1 asset status: [1, flags (bit 0 Reporting, bit 1 LoadOnDemandReporting, bit 2: do not set them, keep the constructor's), n ops, per op (kind, length, bytes)] -> the report text as words
**        kinds: 0..5 Report_Load_On_Demand_RObj, _HAnim, _HTree, Report_Missing_RObj, _HAnim, _HTree; 6 Enable_Reporting(length is the flag),
**        7 Enable_Load_On_Demand_Reporting(length is the flag). Output: the number of bytes, then the bytes ( 0 bytes: no report written ).
**   op 2 material pass: [2, n ops, per op (kind, a, b)]; kinds 0 Set_Texture(handle b, stage a), 1 Set_Material(a), 2 Set_Cull_Volume(a),
**        3 Enable_On_Translucent_Meshes(a) -> after each op: Peek_Texture and Get_Texture of the 8 stages, Peek_Material, Get_Material, Get_Cull_Volume,
**        Is_Enabled_On_Translucent_Meshes, Peek_Shader bits (handles: 0 NULL, 1..8)
**   op 3 render info defaults: [3, render type] -> RenderInfoClass fields (fog_scale, fog_start, fog_end, alphaOverride, materialPassAlphaOverride,
**        materialPassEmissiveOverride, light_environment null, Texture_Projector null, Additional_Pass_Count, Current_Override_Flags, &Camera == the camera),
**        then SpecialRenderInfoClass's (the same, RenderType, VisRasterizer null, BWRenderer null), RENDER_VIS, RENDER_SHADOW, the two limits */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <string>
#include <vector>
#include <stdarg.h>

#include "always.h"
#include "wwdebug.h"
#include "w3d_file.h"
#include "refcount.h"

#ifndef W3DNEW
#define W3DNEW new
#endif

typedef uint32_t W;
static std::vector<W> g_out;
static void put(W w) { g_out.push_back(w); }

#define ShaderClass W7ShaderClass
#define TextureClass W7_TextureClass /* the forced prelude declares a TextureClass of its own */
#define VertexMaterialClass W7_VertexMaterialClass
class StringClass;
#include "shader.h"

/* ---------------- polyinfo ---------------- */
struct TextureClass { void Add_Ref(void) {} void Release_Ref(void) {} };
struct VertexMaterialClass { void Add_Ref(void) {} void Release_Ref(void) {} };

namespace poly {

#include "w7_poly_h.inc"
#include "w7_poly_cpp.inc"

} // namespace poly

/* ---------------- assetstatus ---------------- */
class StringClass {
public:
	std::string s;
	StringClass() {}
	StringClass(const char *c) : s(c) {}
	StringClass(const char *c, bool) : s(c) {}
	StringClass(int, bool) {}
	char *Peek_Buffer(void) { return &s[0]; }
	const char *Peek_Buffer(void) const { return s.c_str(); }
	unsigned int Get_Length(void) const { return (unsigned int)s.size(); }
	bool operator==(const StringClass &o) const { return s == o.s; }
	bool operator!=(const StringClass &o) const { return s != o.s; }
	operator const char *() const { return s.c_str(); }
	StringClass &operator+=(const char *c) { s += c; return *this; }
	StringClass &operator+=(const StringClass &o) { s += o.s; return *this; }
	int Format(const char *fmt, ...)
	{
		char buf[256];
		va_list ap;
		va_start(ap, fmt);
		int n = vsnprintf(buf, sizeof buf, fmt, ap);
		va_end(ap);
		s = buf;
		return n;
	}
};
#define __WWSTRING_H /* hashtemplate.h would otherwise include the real wwstring.h, which declares StringClass again */
#define WW_STRING_H
#include "hashtemplate.h"

static std::string *g_file = 0;
static char *_strlwr(char *s)
{
	for (char *p = s; *p; ++p)
		if (*p >= 'A' && *p <= 'Z') *p = (char)(*p + 32);
	return s;
}
class RawFileClass {
public:
	enum { WRITE = 2 };
	RawFileClass(const char *) {}
	int Create(void) { return 1; }
	int Open(int) { return 1; }
	int Write(const void *p, int n) { if (g_file) g_file->append((const char *)p, (size_t)n); return n; }
	void Close(void) {}
};

#define WWDEBUG 1
#include "w7_asset_h.inc"
#include "w7_asset_cpp.inc"
#undef WWDEBUG

/* ---------------- matpass and rinfo ---------------- */
class OBBoxClass { public: int id; };
class CameraClass {};
class LightEnvironmentClass;
class VisRasterizerClass;
class BWRenderClass;
class TexProjectClass;
#include "matpass.h"
void MaterialPassClass::Install_Materials(void) const {} /* stand-in: device only (DX8Wrapper::Set_Material, Set_Shader, Set_Texture) */
#include "w7_mp_ctor.inc"
#include "w7_mp_settex.inc"
#include "w7_mp_rest.inc"
namespace rinfo7 {
#include "w7_rinfo_h.inc"
#include "w7_rinfo_cpp.inc"
}

static void rd_str(const W *in, int &i, std::string &s)
{
	W len = in[i++];
	s.clear();
	for (W k = 0; k < len; ++k) s.push_back((char)in[i++]);
}

extern "C" int ref_dw7_misc(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	g_out.clear();
	int i = 0;
	W op = in[i++];
	switch (op) {
	case 0: {
		using namespace poly;
		static TextureClass tex[16];
		static VertexMaterialClass vm[16];
		auto hid = [&](W h) -> int { return h; };
		(void)hid;
		auto tp = [&](W h) -> TextureClass * { return h == 0 ? 0 : &tex[h - 1]; };
		auto vp = [&](W h) -> VertexMaterialClass * { return h == 0 ? 0 : &vm[h - 1]; };
		auto th = [&](TextureClass *t) -> W { return t == 0 ? 0 : (W)(t - tex) + 1; };
		auto vh = [&](VertexMaterialClass *v) -> W { return v == 0 ? 0 : (W)(v - vm) + 1; };
		PolygonInfoClass *pi = 0;
		W n = in[i++];
		for (W k = 0; k < n; ++k) {
			W kind = in[i++], a = in[i++], b = in[i++], c = in[i++];
			W7ShaderClass sh(c);
			switch (kind) {
			case 0: delete pi; pi = new PolygonInfoClass(tp(a), vp(b), &sh); break;
			case 1: pi->Set_Texture(tp(a)); break;
			case 2: pi->Set_Vertex_Material(vp(a)); break;
			case 3: { W7ShaderClass s2(a); pi->Set_Shader(&s2); break; }
			default: pi->Set(tp(a), vp(b), &sh); break;
			}
			put(th(pi->Peek_Texture()));
			put(vh(pi->Peek_Vertex_Material()));
			put(pi->Peek_Shader()->Get_Bits());
		}
		delete pi;
		break;
	}
	case 1: {
		W flags = in[i++];
		std::string captured;
		g_file = &captured;
		{
			AssetStatusClass st;
			if (!(flags & 4)) { /* bit 2: leave the constructor's defaults */
				st.Enable_Reporting((flags & 1) != 0);
				st.Enable_Load_On_Demand_Reporting((flags & 2) != 0);
			}
			W n = in[i++];
			for (W k = 0; k < n; ++k) {
				W kind = in[i++];
				if (kind >= 6) { W flag = in[i++]; if (kind == 6) st.Enable_Reporting(flag != 0); else st.Enable_Load_On_Demand_Reporting(flag != 0); continue; }
				std::string s;
				rd_str(in, i, s);
				switch (kind) {
				case 0: st.Report_Load_On_Demand_RObj(s.c_str()); break;
				case 1: st.Report_Load_On_Demand_HAnim(s.c_str()); break;
				case 2: st.Report_Load_On_Demand_HTree(s.c_str()); break;
				case 3: st.Report_Missing_RObj(s.c_str()); break;
				case 4: st.Report_Missing_HAnim(s.c_str()); break;
				default: st.Report_Missing_HTree(s.c_str()); break;
				}
			}
		}
		g_file = 0;
		put((W)captured.size());
		for (size_t k = 0; k < captured.size(); ++k) put((unsigned char)captured[k]);
		break;
	}
	case 2: {
		using namespace poly;
		static TextureClass tex[8];
		static VertexMaterialClass vm[8];
		static OBBoxClass obb[8];
		auto tp = [&](W h) -> TextureClass * { return h == 0 ? 0 : &tex[h - 1]; };
		auto vp = [&](W h) -> VertexMaterialClass * { return h == 0 ? 0 : &vm[h - 1]; };
		auto op_ = [&](W h) -> OBBoxClass * { return h == 0 ? 0 : &obb[h - 1]; };
		auto th = [&](TextureClass *t) -> W { return t == 0 ? 0 : (W)(t - tex) + 1; };
		auto vh = [&](VertexMaterialClass *v) -> W { return v == 0 ? 0 : (W)(v - vm) + 1; };
		auto oh = [&](OBBoxClass *o) -> W { return o == 0 ? 0 : (W)(o - obb) + 1; };
		MaterialPassClass *mp = new MaterialPassClass;
		W n = in[i++];
		for (W k = 0; k < n; ++k) {
			W kind = in[i++], a = in[i++], b = in[i++];
			switch (kind) {
			case 0: mp->Set_Texture(tp(b), (int)a); break;
			case 1: mp->Set_Material(vp(a)); break;
			case 2: mp->Set_Cull_Volume(op_(a)); break;
			default: mp->Enable_On_Translucent_Meshes(a != 0); break;
			}
			for (int s2 = 0; s2 < 8; ++s2) put(th(mp->Peek_Texture(s2)));
			for (int s2 = 0; s2 < 8; ++s2) put(th(mp->Get_Texture(s2)));
			put(vh(mp->Peek_Material()));
			put(vh(mp->Get_Material()));
			put(oh(mp->Get_Cull_Volume()));
			put((W)mp->Is_Enabled_On_Translucent_Meshes());
			put(mp->Peek_Shader().Get_Bits());
		}
		delete mp;
		break;
	}
	case 3: {
		using namespace rinfo7;
		static CameraClass cam;
		W type = in[i++];
		auto fb = [](float f) -> W { W w; memcpy(&w, &f, 4); return w; };
		RenderInfoClass ri(cam);
		auto dump = [&](RenderInfoClass &r) {
			put(fb(r.fog_scale)); put(fb(r.fog_start)); put(fb(r.fog_end));
			put(fb(r.alphaOverride)); put(fb(r.materialPassAlphaOverride)); put(fb(r.materialPassEmissiveOverride));
			put(r.light_environment == 0); put(r.Texture_Projector == 0);
			put((W)r.Additional_Pass_Count()); put((W)r.Current_Override_Flags()); put(&r.Camera == &cam);
		};
		dump(ri);
		SpecialRenderInfoClass sp(cam, (int)type);
		dump(sp);
		put((W)sp.RenderType); put(sp.VisRasterizer == 0); put(sp.BWRenderer == 0);
		put((W)SpecialRenderInfoClass::RENDER_VIS); put((W)SpecialRenderInfoClass::RENDER_SHADOW);
		put((W)MAX_ADDITIONAL_MATERIAL_PASSES); put((W)MAX_OVERRIDE_FLAG_LEVEL);
		put((W)RenderInfoClass::RINFO_OVERRIDE_DEFAULT); put((W)RenderInfoClass::RINFO_OVERRIDE_FORCE_TWO_SIDED); put((W)RenderInfoClass::RINFO_OVERRIDE_FORCE_SORTING);
		put((W)RenderInfoClass::RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY); put((W)RenderInfoClass::RINFO_OVERRIDE_SHADOW_RENDERING);
		break;
	}
	default: return -1;
	}
	if ((int)g_out.size() > cap) return -2;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
