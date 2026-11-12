/* Differential-test wrapper for the text measure (GitHub #153, upstream commit 0a05454d), cut verbatim by build.rs from
** Libraries/Source/WWVegas/WW3D2/render2dsentence.cpp into OUT_DIR/render2d_sentence_gen:
**   rs_chars.inc    from `FontCharsClass::Get_Char_Data` to the Free_GDI_Font banner: Get_Char_Data, Get_Char_Width,
**                   Get_Char_Spacing, Blit_Char, Store_GDI_Char, Update_Current_Buffer, Create_GDI_Font
**   rs_grow.inc     FontCharsClass::Grow_Unicode_Array (to the Free_Character_Arrays banner)
**   rs_extents.inc  Render2DSentenceClass::Build_Sentence_Not_Centered (to the Build_Sentence banner)
**   rs_surface.inc  Render2DSentenceClass::Allocate_New_Surface (to `FindStartingXPos`)
** No edit. Compiled inside namespace r2d on stand-ins (the same model as the Rust test's host,
** tests/differential_render2d.rs):
** - FontCharsClass and Render2DSentenceClass hold the members the cuts use, initialised as upstream's constructors do
**   (Render2DSentenceClass: no surface, as a string only measured and never drawn has, `ParseHotKey` false,
**   `TextureSizeHint` the sequence's); `Record_Sentence_Chunk` and `Reset_Sentence_Data` do nothing (no sentence is
**   drawn: only `justCalcExtents` is called, so `Allocate_New_Surface` makes no surface); W3DNEW is new.
** - GDI: fonts are numbered in the order `CreateFont` makes them (each call recorded); a DC's font is the last font
**   selected into it; `GetTextMetrics` gives that font's height, ascent and overhang from the sequence;
**   `GetTextExtentPoint32W` gives cx = the sequence's table[font][ch % 16] and cy = the font's height;
**   `CreateDIBSection` gives zeroed bits (1 MiB); the other calls do nothing; MulDiv is Win32's (rounded half away from
**   zero, -1 for a zero denominator).
**
** Entry point (extern "C"): ref_render2d_seq(in, nin, out, cap). `in` is 8 words [point size, bold, the face (0 Arial, 1
** Generals, 2 Arial Unicode MS, 3 Times New Roman), the alternate Unicode font (0 none, 1 itself, 2 an Arial Unicode MS
** of the same size and bold), TextureSizeHint, useHardWordWrap, 0, 0], 6 words of metrics [height, ascent, overhang] for
** fonts 0 and 1, 32 words of widths (16 for font 0, then 16 for font 1), then strings of [wrap width, length, the
** characters]. Out: [1, height, width, weight, FNV-1a of the face] for each CreateFont; [2, PixelOverlap, CharHeight,
** CharAscent, CharOverhang] for the font (and its alternate, when another); then for each string [3, the extents' X and
** Y as float bits, computeExtents' width and height, the first character's Get_Char_Spacing (0 for none)]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

typedef void *HDC;
typedef void *HFONT;
typedef void *HBITMAP;
typedef void *HWND;
typedef void *HGDIOBJ;
typedef void *HANDLE;
typedef unsigned int DWORD;
typedef unsigned int UINT;
typedef unsigned short WCHAR;
typedef unsigned char uint8;
typedef unsigned short uint16;
struct SIZE { long cx, cy; };
struct RECT { long left, top, right, bottom; };
struct TEXTMETRIC { long tmHeight, tmAscent, tmDescent, tmInternalLeading, tmExternalLeading, tmAveCharWidth, tmMaxCharWidth, tmWeight, tmOverhang; };
struct BITMAPINFOHEADER { DWORD biSize; long biWidth; long biHeight; unsigned short biPlanes; unsigned short biBitCount; DWORD biCompression; DWORD biSizeImage; long biXPelsPerMeter; long biYPelsPerMeter; DWORD biClrUsed; DWORD biClrImportant; };
struct BITMAPINFO { BITMAPINFOHEADER bmiHeader; };
enum { FW_NORMAL = 400, FW_BOLD = 700, DEFAULT_CHARSET = 1, OUT_DEFAULT_PRECIS = 0, CLIP_DEFAULT_PRECIS = 0, ANTIALIASED_QUALITY = 4, VARIABLE_PITCH = 2, BI_RGB = 0, DIB_RGB_COLORS = 0, ETO_OPAQUE = 2 };
#define RGB(r, g, b) ((DWORD)(((uint8)(r) | ((DWORD)(uint8)(g) << 8)) | (((DWORD)(uint8)(b)) << 16)))
#define FALSE 0
#define TRUE 1

static std::vector<int> g_out;
static void rec(std::initializer_list<int> w) { g_out.insert(g_out.end(), w.begin(), w.end()); }
static int fnv(const char *s) { unsigned h = 2166136261u; for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; } return (int)h; }

/* the sequence's GDI */
struct Cfg { int metrics[2][3]; int widths[2][16]; int texture; };
static Cfg g_cfg;
static int g_fonts = 0;
static std::vector<int> g_dc_font;	/* by DC (1-based handles) */
static std::vector<uint8 *> g_bits;

static HDC GetDC(HWND) { return (HDC)0; }
static int ReleaseDC(HWND, HDC) { return 1; }
static int MulDiv(int a, int b, int c)
{
	if (c == 0) return -1;
	long long n = (long long)a * b, d = c;
	long long an = n < 0 ? -n : n, ad = d < 0 ? -d : d;
	long long q = (an + ad / 2) / ad;
	return (int)(((n < 0) != (d < 0)) ? -q : q);
}
static HFONT CreateFont(int h, int w, int, int, int weight, DWORD, DWORD, DWORD, DWORD, DWORD, DWORD, DWORD, DWORD, const char *face)
{
	rec({1, h, w, weight, fnv(face)});
	return (HFONT)(intptr_t)(++g_fonts);
}
static HBITMAP CreateDIBSection(HDC, const BITMAPINFO *, UINT, void **bits, HANDLE, DWORD)
{
	uint8 *b = (uint8 *)calloc(1 << 20, 1);
	g_bits.push_back(b);
	*bits = b;
	return (HBITMAP)(intptr_t)(100000 + (int)g_bits.size());
}
static HDC CreateCompatibleDC(HDC) { g_dc_font.push_back(0); return (HDC)(intptr_t)g_dc_font.size(); }
static HGDIOBJ SelectObject(HDC dc, HGDIOBJ obj)
{
	intptr_t o = (intptr_t)obj;
	if (o > 0 && o < 100000) g_dc_font[(intptr_t)dc - 1] = (int)o;
	return (HGDIOBJ)0;
}
static DWORD SetBkColor(HDC, DWORD) { return 0; }
static DWORD SetTextColor(HDC, DWORD) { return 0; }
static int font_of(HDC dc) { int f = g_dc_font[(intptr_t)dc - 1] - 1; return f < 0 ? 0 : (f > 1 ? 1 : f); }
static int GetTextMetrics(HDC dc, TEXTMETRIC *tm)
{
	int f = font_of(dc);
	tm->tmHeight = g_cfg.metrics[f][0];
	tm->tmAscent = g_cfg.metrics[f][1];
	tm->tmOverhang = g_cfg.metrics[f][2];
	return 1;
}
static int ExtTextOutW(HDC, int, int, UINT, const RECT *, const WCHAR *, UINT, const int *) { return 1; }
static int GetTextExtentPoint32W(HDC dc, const WCHAR *ch, int, SIZE *s)
{
	int f = font_of(dc);
	s->cx = g_cfg.widths[f][*ch % 16];
	s->cy = g_cfg.metrics[f][0];
	return 1;
}

namespace r2d {

template <class T> T min(T a, T b) { return (a < b) ? a : b; }
template <class T> T max(T a, T b) { return (a > b) ? a : b; }
#define W3DNEW new
#define W3DNEWARRAY new
#define WWASSERT(x) ((void)0)

struct WW3D { static void *Get_Window() { return 0; } };

template <class T> class DynamicVectorClass
{
public:
	std::vector<T> v;
	int Count() const { return (int)v.size(); }
	T &operator[](int i) { return v[i]; }
	bool Add(const T &t) { v.push_back(t); return true; }
	bool Delete(int i) { v.erase(v.begin() + i); return true; }
};

class FontCharsClassCharDataStruct
{
public:
	WCHAR Value;
	short Width;
	uint16 *Buffer;
};
enum { CHAR_BUFFER_LEN = 32768 };
class FontCharsBuffer
{
public:
	uint16 Buffer[CHAR_BUFFER_LEN];
};

class FontCharsClass
{
public:
	FontCharsClass *AlternateUnicodeFont;
	FontCharsClass() : OldGDIFont(NULL), OldGDIBitmap(NULL), GDIFont(NULL), GDIBitmap(NULL), GDIBitmapBits(NULL), MemDC(NULL),
		CurrPixelOffset(0), PointSize(0), CharHeight(0), UnicodeCharArray(NULL), FirstUnicodeChar(0xFFFF), LastUnicodeChar(0), IsBold(false)
	{
		AlternateUnicodeFont = NULL;
		::memset(ASCIICharArray, 0, sizeof(ASCIICharArray));
	}
	~FontCharsClass()
	{
		for (int i = 0; i < BufferList.Count(); ++i) delete BufferList[i];
		for (int i = 0; i < 256; ++i) delete ASCIICharArray[i];
		if (UnicodeCharArray) {
			for (int i = 0; i <= (int)LastUnicodeChar - (int)FirstUnicodeChar; ++i) delete UnicodeCharArray[i];
			delete[] UnicodeCharArray;
		}
	}
	void Init(const char *font_name, int point_size, bool is_bold) { PointSize = point_size; IsBold = is_bold; Create_GDI_Font(font_name); }
	int Get_Char_Height(void) { return CharHeight; }
	int Get_Char_Width(WCHAR ch);
	int Get_Char_Spacing(WCHAR ch);
	int Get_Extra_Overlap(void) { return PixelOverlap; }
	void Blit_Char(WCHAR ch, uint16 *dest_ptr, int dest_stride, int x, int y);
	void Create_GDI_Font(const char *font_name);
	const FontCharsClassCharDataStruct *Store_GDI_Char(WCHAR ch);
	void Update_Current_Buffer(int char_width);
	const FontCharsClassCharDataStruct *Get_Char_Data(WCHAR ch);
	void Grow_Unicode_Array(WCHAR ch);

	DynamicVectorClass<FontCharsBuffer *> BufferList;
	HFONT OldGDIFont;
	HBITMAP OldGDIBitmap;
	HFONT GDIFont;
	HBITMAP GDIBitmap;
	uint8 *GDIBitmapBits;
	HDC MemDC;
	int CurrPixelOffset;
	int PointSize;
	int CharHeight;
	int CharAscent;
	int CharOverhang;
	int PixelOverlap;
	FontCharsClassCharDataStruct *ASCIICharArray[256];
	FontCharsClassCharDataStruct **UnicodeCharArray;
	uint16 FirstUnicodeChar;
	uint16 LastUnicodeChar;
	bool IsBold;
};

struct Vector2 { float X, Y; Vector2() : X(0), Y(0) {} Vector2(float x, float y) : X(x), Y(y) {} void Set(float x, float y) { X = x; Y = y; } };
struct Vector2i { int I, J; Vector2i() : I(0), J(0) {} void Set(int i, int j) { I = i; J = j; } };
class SurfaceClass { public: void *Lock(int *) { return NULL; } void Unlock(void) {} void Add_Ref(void) {} };
#define REF_PTR_RELEASE(x) ((void)0)
#define NEW_REF(C, P) ((C *)0)
enum { WW3D_FORMAT_A4R4G4B4 = 0 };
struct PendingSurfaceStruct { SurfaceClass *Surface; };

class Render2DSentenceClass
{
public:
	Render2DSentenceClass() : Font(NULL), CurSurface(NULL), LockedPtr(NULL), LockedStride(0), CurrTextureSize(0), TextureStartX(0), WrapWidth(0), ParseHotKey(false), useHardWordWrap(false), TextureSizeHint(0) {}
	Vector2 Build_Sentence_Not_Centered(const WCHAR *text, int *hkX, int *hkY, bool justCalcExtents);
	void Reset_Sentence_Data(void) {}
	void Record_Sentence_Chunk(void) {}
	void Allocate_New_Surface(const WCHAR *text, bool justCalcExtents = false);
	FontCharsClass *Font;
	SurfaceClass *CurSurface;
	DynamicVectorClass<PendingSurfaceStruct> PendingSurfaces;
	uint16 *LockedPtr;
	int LockedStride;
	int CurrTextureSize;
	Vector2 Cursor;
	Vector2i TextureOffset;
	int TextureStartX;
	float WrapWidth;
	bool ParseHotKey;
	bool useHardWordWrap;
	int TextureSizeHint;
};

#define TEXTURE_OFFSET 2

#include "rs_chars.inc"
#include "rs_grow.inc"
#include "rs_extents.inc"
#include "rs_surface.inc"

static const char *const FACES[4] = { "Arial", "Generals", "Arial Unicode MS", "Times New Roman" };

}  // namespace r2d

extern "C" int ref_render2d_seq(const int *in, int nin, int *out, int cap)
{
	using namespace r2d;
	g_out.clear();
	g_fonts = 0;
	g_dc_font.clear();
	int point = in[0], bold = in[1], face = in[2], alt = in[3];
	g_cfg.texture = in[4];
	bool hard = in[5] != 0;
	int k = 8;
	for (int f = 0; f < 2; ++f) for (int j = 0; j < 3; ++j) g_cfg.metrics[f][j] = in[k++];
	for (int f = 0; f < 2; ++f) for (int j = 0; j < 16; ++j) g_cfg.widths[f][j] = in[k++];
	FontCharsClass *font = new FontCharsClass;
	font->Init(FACES[face], point, bold != 0);
	FontCharsClass *other = NULL;
	if (alt == 1) font->AlternateUnicodeFont = font;
	if (alt == 2) {
		other = new FontCharsClass;
		other->Init("Arial Unicode MS", point, bold != 0);
		font->AlternateUnicodeFont = other;
	}
	rec({2, font->PixelOverlap, font->CharHeight, font->CharAscent, font->CharOverhang});
	if (other) rec({2, other->PixelOverlap, other->CharHeight, other->CharAscent, other->CharOverhang});
	while (k + 2 <= nin) {
		int wrap = in[k], len = in[k + 1];
		k += 2;
		std::vector<WCHAR> text(len + 1, 0);
		for (int j = 0; j < len; ++j) text[j] = (WCHAR)in[k + j];
		k += len;
		/* W3DDisplayString::computeExtents on a renderer of this font, wrap and hard wrap */
		Render2DSentenceClass r;
		r.Font = font;
		r.WrapWidth = (float)wrap;
		r.useHardWordWrap = hard;
		r.TextureSizeHint = g_cfg.texture;
		Vector2 e = r.Build_Sentence_Not_Centered(&text[0], NULL, NULL, true);
		int sx = 0, sy = 0;
		if (len != 0) { sx = (int)e.X; sy = (int)e.Y; }
		int ex, ey;
		memcpy(&ex, &e.X, 4);
		memcpy(&ey, &e.Y, 4);
		int first = len ? font->Get_Char_Spacing(text[0]) : 0;
		rec({3, ex, ey, sx, sy, first});
	}
	delete font;
	delete other;
	for (size_t i = 0; i < g_bits.size(); ++i) free(g_bits[i]);
	g_bits.clear();
	int n = (int)g_out.size() < cap ? (int)g_out.size() : cap;
	memcpy(out, g_out.data(), n * sizeof(int));
	return n;
}
