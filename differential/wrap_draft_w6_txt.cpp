/* Differential-test wrapper for the legacy text texture's layout arithmetic (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/txt.rs (TextTextureClass::is_texture_valid, build_texture) against WW3D2/txt.cpp's TextTextureClass::Is_Texture_Valid and
** Build_Texture, cut VERBATIM by build.rs `drafts_ww3d6` (txt_valid.inc, txt_build.inc; each anchor asserted to occur once). Rect and TPoint2D are the real
** WWLib/trect.h and Point.h; Find_POT is the real WWMath/pot.cpp (linked from another library).
**
** Stand-ins, each for something the cut calls outside itself (WWLib's legacy font and surface code, not in the verified port, and unused by the game, whose text goes
** through Render2DSentenceClass):
**  - FontClass: String_Pixel_Width and Get_Height return the case's numbers; Print records that it was called and the palette it was given;
**  - ConvertClass: an empty class (the cut passes it on and compares its address);
**  - BSurface: Fill does nothing; Blit_From records the destination and source Rect (X, Y, Width, Height);
**  - TextureClass: Release_Ref only (REF_PTR_RELEASE on `Texture`);
**  - nstrdup: new[] and strcpy; the class TextTextureClass around the two functions has the members of txt.h.
**
** Entry point (extern "C", tests/differential_draft_txt.rs), uint32 words:
**   ref_w6_txt_run(in, out) -> n    in = [nops, per op: kind, 7 words]; the case has 3 fonts, 3 converts and 3 distinct strings (pool "alpha", "beta", "gamma")
**     kind 0 make a texture exist (the draft's `has_texture`, which the owner sets after Build_Texture: upstream's `TODO: copy bsurf2 into texture` leaves Texture NULL)
**     kind 1 Is_Texture_Valid(font, string, fore, back, conv): string 3 means the object's own TextureString pointer (upstream compares POINTERS)
**     kind 2 Build_Texture(font, string, fore, back, conv) with the font's String_Pixel_Width = fw - 1 and Get_Height = fh - 1 (words 5, 6 = fw, fh)
**   out per op: kind 1: [result]; kind 2: [result, TextureSize, blit count, per blit: dest X Y Width Height, src X Y Width Height] */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <vector>
#include "always.h"
#include "pot.h"
#include "trect.h"
#include "Point.h"

typedef uint32_t W;

/* the prelude forward-declares the real TextureClass: the stand-in is renamed, and the cut's `TextureClass` means it */
#define TextureClass W6TextureClass
/* refcount.h's macro (WWLib/refcount.h:80) */
#define REF_PTR_RELEASE(x) { if (x) x->Release_Ref(); x = NULL; }

namespace {

struct TextureClass { void Release_Ref(void) {} };

class ConvertClass {};
class BSurface;
std::vector<W> g_blits;
int g_prints = 0;
unsigned char g_pal0 = 0, g_pal1 = 0;

class FontClass {
public:
	int pw = 0, h = 0;
	int String_Pixel_Width(const char *) { return pw; }
	int Get_Height(void) { return h; }
	void Print(const char *, BSurface &, Rect &, TPoint2D<int>, ConvertClass &, unsigned char * palette) { g_prints++; g_pal0 = palette[0]; g_pal1 = palette[1]; }
};

class BSurface {
public:
	BSurface(int, int, int) {}
	void Fill(unsigned char) {}
	void Blit_From(Rect const & dest, BSurface &, Rect const & src)
	{
		g_blits.push_back((W)dest.X); g_blits.push_back((W)dest.Y); g_blits.push_back((W)dest.Width); g_blits.push_back((W)dest.Height);
		g_blits.push_back((W)src.X); g_blits.push_back((W)src.Y); g_blits.push_back((W)src.Width); g_blits.push_back((W)src.Height);
	}
};

char * nstrdup(const char * s) { char * d = new char[strlen(s) + 1]; strcpy(d, s); return d; }

class TextTextureClass {
public:
	TextTextureClass(void) : TextureString(NULL), Texture(NULL), TextureSize(0), ForegroundColor(0), BackgroundColor(0), Font(0), Convert(0) {}
	bool Build_Texture(FontClass &font, const char *str, int fore, int back, ConvertClass &conv);
	bool Is_Texture_Valid(FontClass &font, const char *str, int fore, int back, ConvertClass &conv);
	char * TextureString;
	TextureClass * Texture;
	int TextureSize;
	int ForegroundColor;
	int BackgroundColor;
	FontClass * Font;
	ConvertClass * Convert;
};

#include "txt_valid.inc"
#include "txt_build.inc"

}

extern "C" int ref_w6_txt_run(const W *in, W *out)
{
	static const char * pool[3] = { "alpha", "beta", "gamma" };
	FontClass fonts[3];
	ConvertClass convs[3];
	TextureClass tex;
	TextTextureClass t;
	W *o = out;
	int nops = (int)in[0];
	const W *p = in + 1;
	for (int i = 0; i < nops; i++, p += 8) {
		int kind = (int)p[0];
		int font = (int)p[1], str = (int)p[2], fore = (int)p[3], back = (int)p[4], conv = (int)p[5];
		if (kind == 0) { t.Texture = &tex; continue; }
		const char * s = str == 3 ? t.TextureString : pool[str];
		if (str == 3 && !s) s = pool[0];
		if (kind == 1) { *o++ = (W)t.Is_Texture_Valid(fonts[font], s, fore, back, convs[conv]); continue; }
		fonts[font].pw = (int)p[6] - 1;
		fonts[font].h = (int)p[7] - 1;
		g_blits.clear();
		g_prints = 0;
		bool r = t.Build_Texture(fonts[font], s, fore, back, convs[conv]);
		*o++ = (W)r;
		*o++ = (W)t.TextureSize;
		*o++ = (W)(g_blits.size() / 8);
		for (size_t k = 0; k < g_blits.size(); k++) *o++ = g_blits[k];
	}
	return (int)(o - out);
}
