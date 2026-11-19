/* Differential-test wrapper for the metal map manager draft (GitHub #215, upstream commit 0a05454d), group "ww3dpure":
** src/drafts/ww3d/metalmap.rs against WW3D2/metalmap.cpp and the three VectorProcessorClass functions it calls (WWMath/vp.cpp).
**
** metalmap.h is the real header, included with `private` made `public` (the cut touches the class statics and members from
** outside). The following are cut VERBATIM from upstream by build.rs `drafts_ww3dpure` (each anchor asserted to occur once)
** and #included below (one mechanical edit, in the constructor: `for (int lp = 0; ; lp++)` -> `int lp; for (lp = 0; ; lp++)`, because
** `lp` is read after the loop that declares it, VC6's pre-standard for-scope): metalmap.cpp's `_NormalTable` definition, the constructor, Get_Metal_Map, Metal_Map_Count,
** Update_Lighting, Update_Textures, initialize_normal_table and initialize_metal_params (not the destructor, which only
** releases references, and which the test never runs); vp.cpp's DotProduct, ClampMin and Power.
**
** Stand-ins, each for something the cut calls outside itself:
**  - INIClass (WWLib/INI.H, a file parser): Find_Section / Get_Point / Get_Float answer from the script the test gives
**    (a section exists for the first N metals; each entry is present or absent, an absent one returns the caller's default).
**  - WW3D::Get_Device_Resolution (the display device): returns the bit depth the test gives.
**  - TextureClass, TextureFilterClass, SurfaceClass, StringClass, WW3DFormat, NEW_REF and REF_PTR_RELEASE: the texture is a
**    16 x 16 byte buffer (bytes per pixel 2 for WW3D_FORMAT_A4R4G4B4, else 4) that Get_Surface_Level(0)->Lock(&pitch) hands
**    out with the unpadded pitch (METALMAP_SIZE * bytes per pixel, the same assumption the draft makes), recording the name,
**    format and clamp modes it is given. A D3D surface may be padded; that is the device's and is not modelled.
**  - TPoint3D<float> is the real WWLib/Point.h.
**
** Entry points (extern "C", tests/differential_draft_metalmap.rs):
**   ref_dpw_metal_normals(out)  -> 256 x 3 floats: _NormalTable after initialize_normal_table
**   ref_dpw_metal(in, out)      builds the manager from the scripted INI, updates the lighting and the textures; returns the
**     number of words written to `out` (uint32; floats by bits):
**     in:  bits, section_count, then per section 4 present-flags (Ambient, Diffuse, Specular, Shininess) and 10 floats
**          (3 + 3 + 3 colour points in INI units, shininess); then ambient 3, main light colour 3, main light dir 3, camera dir 3.
**     out: MapCount, Use16Bit, per map: MetalParameters (10 floats), texture name (16 chars), format, clamp U, clamp V,
**          then the pixel bytes of the map after Update_Textures, one byte per word (256 x bpp). */
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <stdlib.h>
#include <math.h>
#include <assert.h>
#include "always.h"
#include "point.h"
#include "wwmath.h"

#define private public
#define protected public
#include "metalmap.h"
#undef private
#undef protected
#include "vp.h"

#ifndef NEW_REF
#define NEW_REF(C, P) (new C P)
#endif
#ifndef REF_PTR_RELEASE
#define REF_PTR_RELEASE(x) { if (x) x->Release_Ref(); x = NULL; }
#endif

/* ---- stand-ins ---- */
enum WW3DFormat { WW3D_FORMAT_A4R4G4B4 = 0, WW3D_FORMAT_A8R8G8B8 = 1 };
enum MipCountType { MIP_LEVELS_1 = 1 };

class StringClass {
public:
	char Buf[64];
	StringClass() { Buf[0] = 0; }
	void Format(const char *fmt, ...) { va_list a; va_start(a, fmt); vsnprintf(Buf, sizeof(Buf), fmt, a); va_end(a); }
};

class TextureFilterClass {
public:
	enum TextureAddressMode { TEXTURE_ADDRESS_REPEAT = 0, TEXTURE_ADDRESS_CLAMP = 1 };
	int UMode, VMode;
	TextureFilterClass() : UMode(0), VMode(0) {}
	void Set_U_Addr_Mode(TextureAddressMode m) { UMode = (int)m; }
	void Set_V_Addr_Mode(TextureAddressMode m) { VMode = (int)m; }
};

class SurfaceClass {
public:
	unsigned char *Pixels;
	int Pitch;
	void *Lock(int *pitch) { *pitch = Pitch; return Pixels; }
	void Unlock(void) {}
	void Release_Ref(void) {}
};

class TextureClass {
public:
	TextureClass(int w, int h, WW3DFormat f, MipCountType) : Format(f), Surface()
	{
		(void)w; (void)h;
		int bpp = (f == WW3D_FORMAT_A4R4G4B4) ? 2 : 4;
		Surface.Pixels = (unsigned char *)calloc(16 * 16 * bpp, 1);
		Surface.Pitch = 16 * bpp;
		Name[0] = 0;
	}
	TextureFilterClass &Get_Filter(void) { return Filter; }
	void Set_Texture_Name(const StringClass &s) { strncpy(Name, s.Buf, 63); Name[63] = 0; }
	SurfaceClass *Get_Surface_Level(int) { return &Surface; }
	void Add_Ref(void) {}
	void Release_Ref(void) {}
	WW3DFormat Format;
	TextureFilterClass Filter;
	SurfaceClass Surface;
	char Name[64];
};

static const float *g_script;      /* the INI script, see the header */
static int g_section_count;
static int g_bits;

class INIClass {
public:
	bool Find_Section(const char *section) const
	{
		int n = -1;
		if (sscanf(section, "Metal%d", &n) != 1) return false;
		return n >= 0 && n < g_section_count;
	}
	TPoint3D<float> const Get_Point(const char *section, const char *entry, TPoint3D<float> const &defvalue) const
	{
		int n = -1;
		sscanf(section, "Metal%d", &n);
		const float *s = g_script + n * 14;
		int k = !strcmp(entry, "AmbientColor") ? 0 : !strcmp(entry, "DiffuseColor") ? 1 : 2;
		if (s[k] == 0.0f) return defvalue;
		const float *c = s + 4 + k * 3;
		return TPoint3D<float>(c[0], c[1], c[2]);
	}
	float Get_Float(const char *section, const char *entry, float defvalue) const
	{
		(void)entry;
		int n = -1;
		sscanf(section, "Metal%d", &n);
		const float *s = g_script + n * 14;
		if (s[3] == 0.0f) return defvalue;
		return s[13];
	}
};

struct WW3D {
	static void Get_Device_Resolution(int &w, int &h, int &bits, bool &windowed) { w = 640; h = 480; bits = g_bits; windowed = false; }
};

/* ---- the cuts ---- */
#include "mm_static.inc"
#include "mm_ctor.inc"
#include "mm_getmap.inc"
#include "mm_count.inc"
#include "mm_light.inc"
#include "mm_update.inc"
#include "mm_norm.inc"
#include "mm_params.inc"
#include "vp_dot.inc"
#include "vp_clamp.inc"
#include "vp_power.inc"

static uint32_t B(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

extern "C" int ref_dpw_metal_normals(float *out)
{
	INIClass ini;
	g_section_count = 1;
	static float one[14] = { 1, 1, 1, 1, 255, 255, 255, 255, 255, 255, 255, 255, 255, 10 };
	g_script = one;
	g_bits = 32;
	MetalMapManagerClass *m = new MetalMapManagerClass(ini);
	(void)m;
	for (int i = 0; i < 256; i++) {
		out[i * 3] = MetalMapManagerClass::_NormalTable[i].X;
		out[i * 3 + 1] = MetalMapManagerClass::_NormalTable[i].Y;
		out[i * 3 + 2] = MetalMapManagerClass::_NormalTable[i].Z;
	}
	return 768;
}

extern "C" int ref_dpw_metal(const uint32_t *in, uint32_t *out)
{
	float f[400];
	int bits = (int)in[0];
	int count = (int)in[1];
	for (int i = 0; i < count * 14 + 12; i++) { uint32_t w = in[2 + i]; memcpy(&f[i], &w, 4); }
	g_script = f;
	g_section_count = count;
	g_bits = bits;
	INIClass ini;
	MetalMapManagerClass *m = new MetalMapManagerClass(ini);
	const float *l = f + count * 14;
	m->Update_Lighting(Vector3(l[0], l[1], l[2]), Vector3(l[3], l[4], l[5]), Vector3(l[6], l[7], l[8]), Vector3(l[9], l[10], l[11]));
	m->Update_Textures();
	uint32_t *o = out;
	*o++ = (uint32_t)m->MapCount;
	*o++ = m->Use16Bit;
	for (int i = 0; i < m->MapCount; i++) {
		const MetalMapManagerClass::MetalParams &p = m->MetalParameters[i];
		*o++ = B(p.AmbientColor.X); *o++ = B(p.AmbientColor.Y); *o++ = B(p.AmbientColor.Z);
		*o++ = B(p.DiffuseColor.X); *o++ = B(p.DiffuseColor.Y); *o++ = B(p.DiffuseColor.Z);
		*o++ = B(p.SpecularColor.X); *o++ = B(p.SpecularColor.Y); *o++ = B(p.SpecularColor.Z);
		*o++ = B(p.Shininess);
		TextureClass *t = m->Textures[i];
		for (int k = 0; k < 16; k++) *o++ = (unsigned char)t->Name[k];
		*o++ = (uint32_t)t->Format; *o++ = (uint32_t)t->Filter.UMode; *o++ = (uint32_t)t->Filter.VMode;
		int bpp = (t->Format == WW3D_FORMAT_A4R4G4B4) ? 2 : 4;
		for (int k = 0; k < 16 * 16 * bpp; k++) *o++ = t->Surface.Pixels[k];
	}
	return (int)(o - out);
}
