/* Differential-test wrapper for src/drafts/sweep_ww3d/texture.rs (GitHub #215, group ww3d4; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_ww3d4`), from WW3D2/texture.cpp: TextureBaseClass::{Invalidate_Old_Unused_Textures,
** Invalidate, Peek_D3D_Base_Texture, Set_D3D_Base_Texture, Is_Missing_Texture, Get_Reduction, Set_HSV_Shift,
** _Get_Total_Locked_Surface_Size, _Get_Total_Texture_Size, _Get_Total_Lightmap_Texture_Size, _Get_Total_Procedural_Texture_Size,
** _Get_Total_Texture_Count, _Get_Total_Lightmap_Texture_Count, _Get_Total_Procedural_Texture_Count,
** _Get_Total_Locked_Surface_Count}.
** Stand-ins: TextureBaseClass is the fields and the members those functions use (texture.h names); Get_Texture_Memory_Usage
** returns the memory the test sets; IDirect3DBaseTexture8 logs AddRef/Release (event = id | 0x10000 for AddRef, id for Release);
** WW3D::Get_Sync_Time / Get_Thumbnail_Enabled / Get_Texture_Reduction / Is_Large_Texture_Extra_Reduction_Enabled return the test's
** values; WW3DAssetManager::Texture_Hash() and HashTemplateIterator walk the test's textures; MissingTexture::_Get_Missing_Texture
** returns the test's missing texture; DX8Wrapper::Set_DX8_Texture is not reached. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#define TextureClass DW4_TextureClass /* the forced prelude declares a TextureClass of its own */
#define WW3DAssetManager DW4_WW3DAssetManager /* the forced prelude declares a WW3DAssetManager of its own */
#include "vector3.h"

namespace {

static std::vector<uint32_t> g_events;
struct IDirect3DBaseTexture8 {
	int id;
	void Release() { g_events.push_back((uint32_t)id); }
	void AddRef() { g_events.push_back((uint32_t)id | 0x10000); }
};
static IDirect3DBaseTexture8 g_d3d[64];

static unsigned g_sync = 0, g_reduction = 0; static int g_thumb = 0, g_large = 0, g_missing = 0;
struct WW3D {
	static unsigned Get_Sync_Time() { return g_sync; }
	static bool Get_Thumbnail_Enabled() { return g_thumb != 0; }
	static int Get_Texture_Reduction() { return (int)g_reduction; }
	static bool Is_Large_Texture_Extra_Reduction_Enabled() { return g_large != 0; }
};
struct MissingTexture {
	static IDirect3DBaseTexture8 * _Get_Missing_Texture() { if (!g_missing) return 0; g_d3d[g_missing].AddRef(); return &g_d3d[g_missing]; }
};

#define MIP_LEVELS_1 1
class TextureLoadTaskClass {};
class TextureClass;
class StringClass {};

class TextureBaseClass {
public:
	bool Initialized;
	unsigned InactivationTime, ExtendedInactivationTime, LastInactivationSyncTime;
	mutable unsigned LastAccessed;
	unsigned Width, Height, MipLevelCount;
	IDirect3DBaseTexture8 * D3DTexture;
	TextureLoadTaskClass * TextureLoadTask;
	TextureLoadTaskClass * ThumbnailLoadTask;
	bool IsProcedural, IsLightmap;
	Vector3 HSVShift;
	unsigned Mem;
	virtual unsigned Get_Texture_Memory_Usage() const { return Mem; }
	bool Is_Lightmap() const { return IsLightmap; }
	bool Is_Procedural() const { return IsProcedural; }
	static unsigned TexturesAppliedPerFrame;
	static void Invalidate_Old_Unused_Textures(unsigned invalidation_time_override);
	void Invalidate();
	IDirect3DBaseTexture8 * Peek_D3D_Base_Texture() const;
	void Set_D3D_Base_Texture(IDirect3DBaseTexture8* tex);
	bool Is_Missing_Texture();
	unsigned Get_Reduction() const;
	void Set_HSV_Shift(const Vector3 &hsv_shift);
	static int _Get_Total_Locked_Surface_Size();
	static int _Get_Total_Texture_Size();
	static int _Get_Total_Lightmap_Texture_Size();
	static int _Get_Total_Procedural_Texture_Size();
	static int _Get_Total_Texture_Count();
	static int _Get_Total_Lightmap_Texture_Count();
	static int _Get_Total_Procedural_Texture_Count();
	static int _Get_Total_Locked_Surface_Count();
};
unsigned TextureBaseClass::TexturesAppliedPerFrame = 0;
class TextureClass : public TextureBaseClass {};

static std::vector<TextureClass> g_tex;
template <class K, class V> struct HashTemplateIterator {
	size_t i;
	HashTemplateIterator(int) : i(0) {}
	void First() { i = 0; }
	bool Is_Done() { return i >= g_tex.size(); }
	void Next() { i++; }
	TextureClass * Peek_Value() { return &g_tex[i]; }
};
struct WW3DAssetManager {
	static WW3DAssetManager * Get_Instance() { static WW3DAssetManager m; return &m; }
	int Texture_Hash() { return 0; }
};

#include "tx_old.inc"
#include "tx_inv.inc"
#include "tx_peek.inc"
#include "tx_set.inc"
#include "tx_miss.inc"
#include "tx_red.inc"
#include "tx_hsv.inc"
#include "tx_t1.inc"
#include "tx_t2.inc"
#include "tx_t3.inc"
#include "tx_t4.inc"
#include "tx_t5.inc"
#include "tx_t6.inc"
#include "tx_t7.inc"
#include "tx_t8.inc"

static uint32_t wf(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static float ff(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }

}

/* in = [sync, thumb_enabled, reduction, large, missing_id, ntex, per texture: initialized, inact, ext_inact, last_inact, last_accessed,
**       width, height, mips, d3d_id, load_task, thumb_task, procedural, lightmap, mem, hsv x3 (bits), nops; ops: (op, tex, arg0, arg1, arg2)]
** out per op: the number of words that follow, then result words, then per texture (initialized, d3d id, last_accessed, last_inact, hsv bits x3), TexturesAppliedPerFrame,
** number of events and the events. */
extern "C" int ref_dw4_tex_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	const uint32_t * p = in;
	g_events.clear();
	for (int i = 0; i < 64; i++) g_d3d[i].id = i;
	g_sync = *p++; g_thumb = (int)*p++; g_reduction = *p++; g_large = (int)*p++; g_missing = (int)*p++;
	int n = (int)*p++;
	g_tex.assign(n, TextureClass());
	for (int i = 0; i < n; i++) {
		TextureClass & t = g_tex[i];
		t.Initialized = *p++ != 0; t.InactivationTime = *p++; t.ExtendedInactivationTime = *p++; t.LastInactivationSyncTime = *p++; t.LastAccessed = *p++;
		t.Width = *p++; t.Height = *p++; t.MipLevelCount = *p++;
		unsigned id = *p++; t.D3DTexture = id ? &g_d3d[id] : 0;
		static TextureLoadTaskClass task;
		t.TextureLoadTask = *p++ ? &task : 0; t.ThumbnailLoadTask = *p++ ? &task : 0;
		t.IsProcedural = *p++ != 0; t.IsLightmap = *p++ != 0; t.Mem = *p++;
		t.HSVShift.X = ff(*p++); t.HSVShift.Y = ff(*p++); t.HSVShift.Z = ff(*p++);
	}
	TextureBaseClass::TexturesAppliedPerFrame = 7;
	std::vector<uint32_t> o;
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++; int ti = (int)*p++; uint32_t a0 = *p++, a1 = *p++, a2 = *p++;
		g_events.clear();
		size_t start = o.size();
		switch (op) {
		case 0: TextureBaseClass::Invalidate_Old_Unused_Textures(a0); break;
		case 1: g_tex[ti].Invalidate(); break;
		case 2: { IDirect3DBaseTexture8 * r = g_tex[ti].Peek_D3D_Base_Texture(); o.push_back(r ? r->id : 0); break; }
		case 3: g_tex[ti].Set_D3D_Base_Texture(a0 ? &g_d3d[a0] : 0); break;
		case 4: o.push_back(g_tex[ti].Get_Reduction()); break;
		case 5: { Vector3 v(ff(a0), ff(a1), ff(a2)); g_tex[ti].Set_HSV_Shift(v); break; }
		case 6:
			o.push_back((uint32_t)TextureBaseClass::_Get_Total_Locked_Surface_Size()); o.push_back((uint32_t)TextureBaseClass::_Get_Total_Texture_Size());
			o.push_back((uint32_t)TextureBaseClass::_Get_Total_Lightmap_Texture_Size()); o.push_back((uint32_t)TextureBaseClass::_Get_Total_Procedural_Texture_Size());
			o.push_back((uint32_t)TextureBaseClass::_Get_Total_Texture_Count()); o.push_back((uint32_t)TextureBaseClass::_Get_Total_Lightmap_Texture_Count());
			o.push_back((uint32_t)TextureBaseClass::_Get_Total_Procedural_Texture_Count()); o.push_back((uint32_t)TextureBaseClass::_Get_Total_Locked_Surface_Count());
			break;
		case 7: { bool f = g_tex[ti].Is_Missing_Texture(); o.push_back(f ? 1 : 0); g_events.clear(); break; }
		}
		for (int i = 0; i < n; i++) {
			TextureClass & t = g_tex[i];
			o.push_back(t.Initialized ? 1 : 0); o.push_back(t.D3DTexture ? t.D3DTexture->id : 0); o.push_back(t.LastAccessed); o.push_back(t.LastInactivationSyncTime);
			o.push_back(wf(t.HSVShift.X)); o.push_back(wf(t.HSVShift.Y)); o.push_back(wf(t.HSVShift.Z));
		}
		o.push_back(TextureBaseClass::TexturesAppliedPerFrame);
		o.push_back((uint32_t)g_events.size());
		for (size_t i = 0; i < g_events.size(); i++) o.push_back(g_events[i]);
		o.insert(o.begin() + start, (uint32_t)(o.size() - start));
	}
	if ((int)o.size() > cap) return -1;
	memcpy(out, o.data(), o.size() * 4);
	return (int)o.size();
}
