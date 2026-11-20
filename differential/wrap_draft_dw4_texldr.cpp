/* Differential-test wrapper for src/drafts/sweep_ww3d/textureloader.rs (GitHub #215, group ww3d4; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_ww3d4`), from WW3D2/textureloader.cpp: TextureLoadTaskListClass::{ctor, Push_Front, Push_Back,
** Pop_Front, Pop_Back, Remove} and the file-static Get_Texture_Information.
** Stand-ins: TextureLoadTaskListNodeClass / TextureLoadTaskListClass are textureloader.h's declarations (copied unchanged, Root made
** public so the test can walk the list); TextureLoadTaskClass is the node plus nothing; WWASSERT is compiled out as in the release
** build. For Get_Texture_Information: ThumbnailManagerClass::Peek_Thumbnail_Instance_From_Any_Manager returns the thumbnail the
** test configures (or NULL); DDSFileClass, Targa (Open, Header.Width/Height), TARGA_ERROR_HANDLER (the bare call result, as
** Targa_Error_Handler returns it for the success/failure code), Get_WW3D_Format (sets the format the test configures) and
** WW3D::Get_Texture_Reduction / Get_Texture_Min_Dimension are configured by the test; WW3DFormat is ww3dformat.h's enum.
** Entry points:
**  ref_dw4_tl_list(in, nin, out, cap): in = [nlists, ntasks, nops, ops...]; ops 0 Push_Front(l,t) 1 Push_Back(l,t) 2 Pop_Front(l)
**    3 Pop_Back(l) 4 Remove(l,t); after each op: result task (or 0xFFFFFFFF), then for every list its tasks front to back, 0xFFFFFFFE
**    ending it, then for every task the index of its List (0xFFFFFFFF none).
**  ref_dw4_tl_info(in, nin, out, cap): see the test. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"

namespace {

#define WWASSERT(x) ((void)0)

enum WW3DFormat { WW3D_FORMAT_UNKNOWN=0, WW3D_FORMAT_R8G8B8, WW3D_FORMAT_A8R8G8B8, WW3D_FORMAT_X8R8G8B8, WW3D_FORMAT_R5G6B5, WW3D_FORMAT_X1R5G5B5,
	WW3D_FORMAT_A1R5G5B5, WW3D_FORMAT_A4R4G4B4, WW3D_FORMAT_R3G3B2, WW3D_FORMAT_A8, WW3D_FORMAT_A8R3G3B2, WW3D_FORMAT_X4R4G4B4, WW3D_FORMAT_A8P8,
	WW3D_FORMAT_P8, WW3D_FORMAT_L8, WW3D_FORMAT_A8L8, WW3D_FORMAT_A4L4, WW3D_FORMAT_U8V8, WW3D_FORMAT_L6V5U5, WW3D_FORMAT_X8L8V8U8,
	WW3D_FORMAT_DXT1, WW3D_FORMAT_DXT2, WW3D_FORMAT_DXT3, WW3D_FORMAT_DXT4, WW3D_FORMAT_DXT5, WW3D_FORMAT_COUNT };

class TextureLoadTaskListClass;

class TextureLoadTaskListNodeClass
{
	friend class TextureLoadTaskListClass;
	public:
		TextureLoadTaskListNodeClass(void) : Next(0), Prev(0) { }
		TextureLoadTaskListClass *Get_List(void)		{ return List; }
		TextureLoadTaskListNodeClass *Next;
		TextureLoadTaskListNodeClass *Prev;
		TextureLoadTaskListClass *		List;
};

class TextureLoadTaskClass : public TextureLoadTaskListNodeClass { public: int id; };

class TextureLoadTaskListClass
{
	public:
		TextureLoadTaskListClass(void);
		bool									Is_Empty		(void) const		{ return (Root.Next == &Root); }
		void									Push_Front	(TextureLoadTaskClass *task);
		void									Push_Back	(TextureLoadTaskClass *task);
		TextureLoadTaskClass *			Pop_Front	(void);
		TextureLoadTaskClass *			Pop_Back		(void);
		void									Remove		(TextureLoadTaskClass *task);
		TextureLoadTaskListNodeClass	Root;
};

#include "tl_ctor.inc"
#include "tl_pf.inc"
#include "tl_pb.inc"
#include "tl_popf.inc"
#include "tl_popb.inc"
#include "tl_rem.inc"

// ---- Get_Texture_Information stand-ins
struct ThumbnailClass {
	WW3DFormat fmt; unsigned w, h, mips;
	WW3DFormat Get_Original_Texture_Format() { return fmt; }
	unsigned Get_Original_Texture_Width() { return w; }
	unsigned Get_Original_Texture_Height() { return h; }
	unsigned Get_Original_Texture_Mip_Level_Count() { return mips; }
};
static ThumbnailClass g_thumb; static int g_thumb_present = 0;
struct ThumbnailManagerClass { static ThumbnailClass * Peek_Thumbnail_Instance_From_Any_Manager(const char *) { return g_thumb_present ? &g_thumb : 0; } };

static int g_dds_ok = 0; static unsigned g_dds_w, g_dds_h, g_dds_d, g_dds_mips; static WW3DFormat g_dds_fmt;
struct DDSFileClass {
	DDSFileClass(const char *, unsigned) {}
	bool Is_Available() { return g_dds_ok != 0; }
	unsigned Get_Width(unsigned) { return g_dds_w; }
	unsigned Get_Height(unsigned) { return g_dds_h; }
	unsigned Get_Depth(unsigned) { return g_dds_d; }
	WW3DFormat Get_Format() { return g_dds_fmt; }
	unsigned Get_Mip_Level_Count() { return g_dds_mips; }
};

static int g_tga_ok = 0; static short g_tga_w, g_tga_h; static WW3DFormat g_tga_fmt;
#define TGA_READMODE 0
#define TARGA_ERROR_HANDLER(call, filename) (call)
struct Targa {
	struct { short Width; short Height; } Header;
	int Open(const char *, int) { Header.Width = g_tga_w; Header.Height = g_tga_h; return g_tga_ok ? 0 : 1; }
};
static void Get_WW3D_Format(WW3DFormat & dest_format, WW3DFormat & src_format, unsigned & src_bpp, const Targa &) { dest_format = WW3D_FORMAT_UNKNOWN; src_format = g_tga_fmt; src_bpp = 32; }

static int g_reduction = 0, g_min_dim = 0;
struct WW3D { static int Get_Texture_Reduction() { return g_reduction; } static int Get_Texture_Min_Dimension() { return g_min_dim; } };

#include "tl_info.inc"

}

extern "C" int ref_dw4_tl_list(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	int nl = (int)*p++, nt = (int)*p++;
	std::vector<TextureLoadTaskListClass*> lists;
	for (int i = 0; i < nl; i++) lists.push_back(new TextureLoadTaskListClass());
	std::vector<TextureLoadTaskClass*> tasks;
	for (int i = 0; i < nt; i++) { TextureLoadTaskClass * t = new TextureLoadTaskClass(); t->id = i; t->List = 0; tasks.push_back(t); }
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		uint32_t res = 0xFFFFFFFF;
		switch (op) {
		case 0: { int l = (int)*p++, t = (int)*p++; lists[l]->Push_Front(tasks[t]); break; }
		case 1: { int l = (int)*p++, t = (int)*p++; lists[l]->Push_Back(tasks[t]); break; }
		case 2: { int l = (int)*p++; TextureLoadTaskClass * t = lists[l]->Pop_Front(); if (t) res = t->id; break; }
		case 3: { int l = (int)*p++; TextureLoadTaskClass * t = lists[l]->Pop_Back(); if (t) res = t->id; break; }
		case 4: { int l = (int)*p++, t = (int)*p++; lists[l]->Remove(tasks[t]); break; }
		}
		o.push_back(res);
		for (int l = 0; l < nl; l++) {
			for (TextureLoadTaskListNodeClass * n = lists[l]->Root.Next; n != &lists[l]->Root; n = n->Next) o.push_back(((TextureLoadTaskClass*)n)->id);
			o.push_back(0xFFFFFFFE);
			o.push_back(lists[l]->Is_Empty() ? 1 : 0);
		}
		for (int t = 0; t < nt; t++) {
			uint32_t li = 0xFFFFFFFF;
			for (int l = 0; l < nl; l++) if (tasks[t]->List == lists[l]) li = l;
			o.push_back(li);
		}
	}
	for (size_t i = 0; i < tasks.size(); i++) delete tasks[i];
	for (size_t i = 0; i < lists.size(); i++) delete lists[i];
	if ((int)o.size() > cap) return -1;
	memcpy(out, o.data(), o.size() * 4);
	return (int)o.size();
}

/* in = [thumb_present, thumb_fmt, thumb_w, thumb_h, thumb_mips, dds_ok, dds_w, dds_h, dds_d, dds_fmt, dds_mips, tga_ok, tga_w, tga_h,
**       tga_fmt, ww3d_reduction, min_dim, ncalls, (reduction_in, compressed)...];
** out per call: ret, then (if ret) reduction, w, h, d, format, mip_count. */
extern "C" int ref_dw4_tl_info(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	g_thumb_present = (int)*p++; g_thumb.fmt = (WW3DFormat)*p++; g_thumb.w = *p++; g_thumb.h = *p++; g_thumb.mips = *p++;
	g_dds_ok = (int)*p++; g_dds_w = *p++; g_dds_h = *p++; g_dds_d = *p++; g_dds_fmt = (WW3DFormat)*p++; g_dds_mips = *p++;
	g_tga_ok = (int)*p++; g_tga_w = (short)*p++; g_tga_h = (short)*p++; g_tga_fmt = (WW3DFormat)*p++;
	g_reduction = (int)*p++; g_min_dim = (int)*p++;
	int n = (int)*p++;
	for (int k = 0; k < n; k++) {
		unsigned reduction = *p++; bool compressed = *p++ != 0;
		unsigned w = 0, h = 0, d = 0, mips = 0; WW3DFormat fmt = WW3D_FORMAT_UNKNOWN;
		bool ok = Get_Texture_Information("x", reduction, w, h, d, fmt, mips, compressed);
		o.push_back(ok ? 1 : 0);
		if (ok) { o.push_back(reduction); o.push_back(w); o.push_back(h); o.push_back(d); o.push_back((uint32_t)fmt); o.push_back(mips); }
	}
	if ((int)o.size() > cap) return -1;
	memcpy(out, o.data(), o.size() * 4);
	return (int)o.size();
}
