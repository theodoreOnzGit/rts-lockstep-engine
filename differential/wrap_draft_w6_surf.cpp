/* Differential-test wrapper for the surface class's pixel arithmetic (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/surfaceclass.rs (`pixel_size`, `convert_pixel_read`, `convert_pixel_write`, `has_alpha`, `alpha_bits`, and SurfaceClass's `clear`, `copy_from_bytes`,
** `copy_rect_from_bytes`, `create_copy`, `find_bb`, `is_transparent_column`, `get_pixel`, `draw_pixel`, `draw_hline`, `is_monochrome`, `hue_shift`) against WW3D2/surfaceclass.cpp.
**
** The REAL surfaceclass.h (class SurfaceClass and its SurfaceDescription), ww3dformat.h (Has_Alpha, Alpha_Bits), vector2i.h, WWLib/bound.h are used. These are cut VERBATIM from
** surfaceclass.cpp by build.rs `drafts_ww3d6` (each anchor asserted to occur once): PixelSize, both Convert_Pixel overloads, SurfaceClass::{Lock, Unlock, Clear, Copy(bytes),
** Copy(rect, bytes), CreateCopy, FindBB, Is_Transparent_Column, Get_Pixel, DrawPixel, DrawHLine, Is_Monochrome, Hue_Shift}; colorspace.h's RGB_To_HSV, HSV_To_RGB and
** Recolor(Vector3 &, ...) are cut verbatim too (the header includes the whole DX8 wrapper). ONE named edit: `(unsigned int)lock_rect.pBits` (FindBB, Is_Transparent_Column) is
** `(uintptr_t)lock_rect.pBits`: the pointer truncates to 32 bits on the Win32 target the code was written for, and would be wrong on this one.
**
** Stand-ins, each for something the cut calls outside itself (the classes SurfaceClass and IDirect3DSurface8 are renamed W6SurfaceClass / W6D3DSurface so that no other
** wrapper's stand-in of the same name is confused with them):
**  - IDirect3DSurface8: a byte buffer with a tight pitch (width * PixelSize); GetDesc, LockRect(rect or whole surface: pBits is the rect's top-left pixel, as Direct3D gives it),
**    UnlockRect, AddRef, Release. A real D3D surface may be padded; that is the device's and is not modelled (the draft assumes the same tight pitch).
**  - SurfaceClass::Get_Description (D3DFormat_To_WW3DFormat over a D3DSURFACE_DESC): the scripted format, width and height; the constructor, destructor and
**    Copy(...other surface) (D3DX: the compressed branch of Is_Monochrome only) are bodies the cut does not reach (Copy and the constructor abort).
**  - D3DLOCKED_RECT, RECT, D3DLOCK_READONLY, ZeroMemory, DX8_ErrorCode (always.h supplies min).
**
** Entry point (extern "C", tests/differential_draft_surfaceclass.rs): ref_w6_surf_run(in, out) -> n; words in/out are uint32, floats by bits.
**   in = [format, width, height, nbytes, bytes (one per word), nops, per op: kind + 6 words]
**   op 0 formats: out [PixelSize, Has_Alpha, Alpha_Bits]   op 1 Convert_Pixel(read): in [rgb 3 (initial value)] and the pixel at offset 0 of the surface: out rgb 3
**   op 2 Convert_Pixel(write): in [rgb 3]: out the 8 bytes at offset 0   op 3 Clear   op 4 Copy(bytes): the next width*height*size input bytes
**   ... (see the test): after every op that changes the surface, its bytes are written. */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>
#include "always.h"
#include "vector2i.h"
#include "vector3.h"
#include "wwmath.h"
#include "bound.h"

/* the real classes' names are mapped away from other wrappers' stand-ins (see the header) */
#define SurfaceClass W6SurfaceClass
#define IDirect3DSurface8 W6D3DSurface
#include "surfaceclass.h"

namespace {

typedef int W6HRESULT;
struct D3DLOCKED_RECT { int Pitch; void * pBits; };
struct W6RECT { int left, top, right, bottom; };
#define RECT W6RECT
struct D3DSURFACE_DESC { WW3DFormat Format; unsigned Width, Height; };
#define D3DLOCK_READONLY 0x10u
#define ZeroMemory(p, n) memset((p), 0, (n))
#define DX8_ErrorCode(x) (x)

/* colorspace.h's inline colour conversions, cut VERBATIM (colorspace_conv.inc) */
#include "surf_colorspace.inc"

}

struct IDirect3DSurface8
{
	std::vector<unsigned char> bytes;
	unsigned width = 0, height = 0, bpp = 0;
	WW3DFormat format = WW3D_FORMAT_UNKNOWN;
	int refs = 1;
	W6HRESULT GetDesc(D3DSURFACE_DESC * d) { d->Format = format; d->Width = width; d->Height = height; return 0; }
	W6HRESULT LockRect(D3DLOCKED_RECT * lr, const RECT * rect, unsigned) {
		lr->Pitch = (int)(width * bpp);
		size_t off = rect ? (size_t)rect->top * width * bpp + (size_t)rect->left * bpp : 0;
		lr->pBits = bytes.data() + off;
		return 0;
	}
	W6HRESULT UnlockRect() { return 0; }
	void AddRef() { refs++; }
	void Release() { refs--; }
};

/* the cut: PixelSize and the two Convert_Pixel overloads (free functions of surfaceclass.cpp) */
namespace {
#include "surf_pixelsize.inc"
#include "surf_conv_read.inc"
#include "surf_conv_write.inc"
}

SurfaceClass::SurfaceClass(IDirect3DSurface8 * d3d_surface) : D3DSurface(d3d_surface), SurfaceFormat(WW3D_FORMAT_UNKNOWN) {}
SurfaceClass::SurfaceClass(unsigned, unsigned, WW3DFormat) : D3DSurface(NULL), SurfaceFormat(WW3D_FORMAT_UNKNOWN) { abort(); }
SurfaceClass::~SurfaceClass(void) {}
void SurfaceClass::Get_Description(SurfaceDescription & sd)
{
	D3DSURFACE_DESC d;
	D3DSurface->GetDesc(&d);
	sd.Format = d.Format; sd.Height = d.Height; sd.Width = d.Width;
}
void SurfaceClass::Copy(unsigned, unsigned, unsigned, unsigned, unsigned, unsigned, const SurfaceClass *) { abort(); }

#include "surf_lock.inc"
#include "surf_unlock.inc"
#include "surf_clear.inc"
#include "surf_copy1.inc"
#include "surf_copy2.inc"
#include "surf_createcopy.inc"
#include "surf_findbb.inc"
#include "surf_transcol.inc"
#include "surf_getpixel.inc"
#include "surf_drawpixel.inc"
#include "surf_drawhline.inc"
#include "surf_ismono.inc"
#include "surf_hueshift.inc"

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

extern "C" int ref_w6_surf_run(const W *in, W *out)
{
	const W *p = in;
	IDirect3DSurface8 d3d;
	d3d.format = (WW3DFormat)*p++;
	d3d.width = *p++;
	d3d.height = *p++;
	SurfaceClass::SurfaceDescription sd0;
	sd0.Format = d3d.format; sd0.Width = d3d.width; sd0.Height = d3d.height;
	d3d.bpp = PixelSize(sd0);
	unsigned nbytes = *p++;
	d3d.bytes.resize(nbytes + 16, 0);
	for (unsigned i = 0; i < nbytes; i++) d3d.bytes[i] = (unsigned char)p[i];
	p += nbytes;
	SurfaceClass surf(&d3d);
	W *o = out;
	auto dump = [&]() { for (unsigned i = 0; i < nbytes; i++) *o++ = d3d.bytes[i]; };
	unsigned nops = *p++;
	for (unsigned k = 0; k < nops; k++, p += 8) {
		unsigned kind = p[0];
		const W *a = p + 1;
		switch (kind) {
		case 0: { *o++ = PixelSize(sd0); *o++ = (W)Has_Alpha(d3d.format); *o++ = (W)Alpha_Bits(d3d.format); break; }
		case 1: { Vector3 rgb(F(a[0]), F(a[1]), F(a[2])); Convert_Pixel(rgb, sd0, d3d.bytes.data()); *o++ = B(rgb.X); *o++ = B(rgb.Y); *o++ = B(rgb.Z); break; }
		case 2: { Vector3 rgb(F(a[0]), F(a[1]), F(a[2])); Convert_Pixel(d3d.bytes.data(), sd0, rgb); for (int i = 0; i < 8; i++) *o++ = d3d.bytes[i]; break; }
		case 3: surf.Clear(); dump(); break;
		case 4: { std::vector<unsigned char> other(nbytes + 16, 0); for (unsigned i = 0; i < nbytes; i++) other[i] = (unsigned char)p[8 + i]; p += nbytes; surf.Copy(other.data()); dump(); break; }
		case 5: { std::vector<unsigned char> other(nbytes + 16, 0); Vector2i mn((int)a[0], (int)a[1]), mx((int)a[2], (int)a[3]); for (unsigned i = 0; i < nbytes; i++) other[i] = (unsigned char)p[8 + i]; p += nbytes; surf.Copy(mn, mx, other.data()); dump(); break; }
		case 6: { int w, h, s; unsigned char * c = surf.CreateCopy(&w, &h, &s, a[0] != 0); *o++ = (W)w; *o++ = (W)h; *o++ = (W)s; for (int i = 0; i < w * h * s; i++) *o++ = c[i]; delete[] c; break; }
		case 7: { Vector2i mn((int)a[0], (int)a[1]), mx((int)a[2], (int)a[3]); surf.FindBB(&mn, &mx); *o++ = (W)mn.I; *o++ = (W)mn.J; *o++ = (W)mx.I; *o++ = (W)mx.J; break; }
		case 8: *o++ = (W)surf.Is_Transparent_Column(a[0]); break;
		case 9: { Vector3 rgb(F(a[2]), F(a[3]), F(a[4])); surf.Get_Pixel(rgb, (int)a[0], (int)a[1]); *o++ = B(rgb.X); *o++ = B(rgb.Y); *o++ = B(rgb.Z); break; }
		case 10: surf.DrawPixel(a[0], a[1], a[2]); dump(); break;
		case 11: surf.DrawHLine(a[0], a[1], a[2], a[3]); dump(); break;
		case 12: *o++ = (W)surf.Is_Monochrome(); break;
		default: { Vector3 hsv(F(a[0]), F(a[1]), F(a[2])); surf.Hue_Shift(hsv); dump(); break; }
		}
	}
	return (int)(o - out);
}
