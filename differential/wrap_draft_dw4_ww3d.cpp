/* Differential-test wrapper for src/drafts/sweep_ww3d/ww3d.rs (GitHub #215, group ww3d4; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_ww3d4`), from WW3D2/ww3d.cpp: WW3D::Set_NPatches_Gap_Filling_Mode, Set_NPatches_Level,
** _Invalidate_Textures, Set_Texture_Filter, Normalize_Coordinates, Enable_Large_Texture_Extra_Reduction.
** Stand-ins: class WW3D is its statics these functions touch (ww3d.h names and enums); TheDX8MeshRenderer.Invalidate() counts
** calls (g_mesh_inval); DX8Wrapper::Get_Device_Resolution_Width/Height return the resolution the test sets; Bound is the real
** WWLib/bound.h; TextureFilterClass::_Init_Filters records its argument; WW3DAssetManager::Get_Instance() is NULL or an object
** whose Texture_Hash() is g_ntex textures; HashTemplateIterator walks that count; TextureClass::Invalidate and
** TextureLoader::Flush_Pending_Load_Tasks count their calls.
** Entry point: ref_dw4_ww3d_run(in, nin, out, cap): in = [res_w, res_h, ntex, has_mgr, nops, ops...];
**   ops: 0 Set_NPatches_Gap_Filling_Mode(mode)  1 Set_NPatches_Level(level)  2 Set_Texture_Filter(f)
**        3 Normalize_Coordinates(x, y) -> fx, fy bits   4 Enable_Large_Texture_Extra_Reduction(onoff)
**   every op is followed by the state: GapMode, NPatchesLevel, TextureFilter, last _Init_Filters arg (or -1), LargeEnabled,
**   mesh Invalidate count, flush count, texture Invalidate count. */
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <vector>
#include "always.h"
#define TextureClass DW4_TextureClass /* the forced prelude declares a TextureClass of its own */
#define WW3DAssetManager DW4_WW3DAssetManager /* the forced prelude declares a WW3DAssetManager of its own */
#include "bound.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

static int g_res_w = 0, g_res_h = 0, g_ntex = 0, g_has_mgr = 0;
static int g_mesh_inval = 0, g_flush = 0, g_tex_inval = 0, g_init_arg = -1;

struct DX8MeshRenderer { void Invalidate() { g_mesh_inval++; } };
static DX8MeshRenderer TheDX8MeshRenderer;

struct DX8Wrapper {
	static int Get_Device_Resolution_Width() { return g_res_w; }
	static int Get_Device_Resolution_Height() { return g_res_h; }
};

struct TextureFilterClass {
	enum TextureFilterMode { TEXTURE_FILTER_BILINEAR, TEXTURE_FILTER_TRILINEAR, TEXTURE_FILTER_ANISOTROPIC };
	static void _Init_Filters(TextureFilterMode m) { g_init_arg = (int)m; }
};

struct TextureClass { void Invalidate() { g_tex_inval++; } };
struct StringClass {};
struct TextureLoader { static void Flush_Pending_Load_Tasks() { g_flush++; } };

template <class K, class V> struct HashTemplateIterator {
	int i; TextureClass t;
	HashTemplateIterator(int) : i(0) {}
	void First() { i = 0; }
	bool Is_Done() { return i >= g_ntex; }
	void Next() { i++; }
	TextureClass * Peek_Value() { return &t; }
};
struct WW3DAssetManager {
	static WW3DAssetManager * Get_Instance() { static WW3DAssetManager m; return g_has_mgr ? &m : 0; }
	int Texture_Hash() { return 0; }
};

class WW3D {
public:
	enum NPatchesGapFillingModeEnum { NPATCHES_GAP_FILLING_DISABLED, NPATCHES_GAP_FILLING_ENABLED, NPATCHES_GAP_FILLING_FORCE };
	static void Set_NPatches_Gap_Filling_Mode(NPatchesGapFillingModeEnum mode);
	static void Set_NPatches_Level(unsigned level);
	static void _Invalidate_Textures();
	static void Set_Texture_Filter(int texture_filter);
	static void Normalize_Coordinates(int x, int y, float &fx, float &fy);
	static void Enable_Large_Texture_Extra_Reduction(bool onoff);
	static NPatchesGapFillingModeEnum NPatchesGapFillingMode;
	static unsigned NPatchesLevel;
	static int TextureFilter;
	static bool _LargeTextureExtraReductionEnabled;
};
WW3D::NPatchesGapFillingModeEnum WW3D::NPatchesGapFillingMode = WW3D::NPATCHES_GAP_FILLING_DISABLED;
unsigned WW3D::NPatchesLevel = 1;
int WW3D::TextureFilter = 0;
bool WW3D::_LargeTextureExtraReductionEnabled = false;

#include "w4_gap.inc"
#include "w4_level.inc"
#include "w4_invtex.inc"
#include "w4_filter.inc"
#include "w4_norm.inc"
#include "w4_lte.inc"

static void state(std::vector<uint32_t> & o)
{
	o.push_back((uint32_t)WW3D::NPatchesGapFillingMode); o.push_back(WW3D::NPatchesLevel); o.push_back((uint32_t)WW3D::TextureFilter);
	o.push_back((uint32_t)g_init_arg); o.push_back(WW3D::_LargeTextureExtraReductionEnabled ? 1 : 0);
	o.push_back(g_mesh_inval); o.push_back(g_flush); o.push_back(g_tex_inval);
}

}

extern "C" int ref_dw4_ww3d_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	g_res_w = (int)*p++; g_res_h = (int)*p++; g_ntex = (int)*p++; g_has_mgr = (int)*p++;
	g_mesh_inval = g_flush = g_tex_inval = 0; g_init_arg = -1;
	WW3D::NPatchesGapFillingMode = WW3D::NPATCHES_GAP_FILLING_DISABLED; WW3D::NPatchesLevel = 1; WW3D::TextureFilter = 0;
	WW3D::_LargeTextureExtraReductionEnabled = false;
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		switch (op) {
		case 0: WW3D::Set_NPatches_Gap_Filling_Mode((WW3D::NPatchesGapFillingModeEnum)*p++); break;
		case 1: WW3D::Set_NPatches_Level(*p++); break;
		case 2: WW3D::Set_Texture_Filter((int)*p++); break;
		case 3: { int x = (int)*p++; int y = (int)*p++; float fx, fy; WW3D::Normalize_Coordinates(x, y, fx, fy); o.push_back(w_of(fx)); o.push_back(w_of(fy)); break; }
		case 4: WW3D::Enable_Large_Texture_Extra_Reduction(*p++ != 0); break;
		}
		state(o);
	}
	if ((int)o.size() > cap) return -1;
	memcpy(out, o.data(), o.size() * 4);
	return (int)o.size();
}
