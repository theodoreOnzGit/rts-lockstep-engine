/* Differential-test wrapper for the bitmap handler draft (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/bitmaphandler.rs against WW3D2/bitmaphandler.cpp and the inline members of WW3D2/bitmaphandler.h.
**
** The REAL bitmaphandler.h, colorspace.h (Recolor) and ww3dformat.h are used; bitmaphandler.cpp is compiled unchanged, with
** `Bitmap_Assert` renamed W6_Bitmap_Assert on the command line (wrap_texture.cpp already defines Bitmap_Assert; both are WWASSERT, empty in the
** release build). Stand-ins come from the forced-included shim_w3d/mesh_prelude.h (dx8wrapper.h declares DX8Wrapper::Convert_Color, defined,
** x87 asm transcribed, in wrap_w3d_mesh.cpp, which Recolor calls) and Get_Bytes_Per_Pixel is the real ww3dformat.cpp (wrap_w3d_mesh library).
**
** Entry points (extern "C", tests/differential_draft_bitmaphandler.rs). All surfaces are owned by the test; formats are the WW3DFormat numbers.
**   ref_w6_bm_read(fmt, pal, pal_len, pal_bpp, src, argb_in[4], out[8])      out = the unsigned char* overload's 4 bytes, the unsigned& overload's 4 bytes
**   ref_w6_bm_read_xy(fmt, x, y, w, h, src, pal, pal_bpp, argb_in) -> argb   the surface overload
**   ref_w6_bm_write(fmt, argb, buf8)                                        both overloads write into buf8 (twice; the second must equal the first)
**   ref_w6_bm_copy_pixel(dfmt, sfmt, src, pal, pal_bpp, use_hsv, hsv, buf8) Copy_Pixel with or without the HSV shift
**   ref_w6_bm_combine(a, b, c, d)
**   ref_w6_bm_mip(dest, dpitch, src, spitch, w, h)
**   ref_w6_bm_genmip(w, h, dest, dpitch, dfmt, src, spitch, sfmt, mip, mpitch, hsv)
**   ref_w6_bm_copy_image(dest, dw, dh, dp, dfmt, src, sw, sh, sp, sfmt, pal, pal_bpp, genmip, hsv)
** The unsigned-char / unsigned& overloads of Read_B8G8R8A8 on a uint8 buffer assume the little-endian x86-64 layout (as Win32). */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include "always.h"
#include "ww3dformat.h"
#include "vector3.h"
#include "bitmaphandler.h"

extern "C" {

void ref_w6_bm_read(unsigned fmt, const unsigned char *pal, unsigned pal_len, unsigned pal_bpp, const unsigned char *src, const unsigned char *argb_in, unsigned char *out)
{
	(void)pal_len;
	unsigned char b[4];
	memcpy(b, argb_in, 4);
	BitmapHandlerClass::Read_B8G8R8A8(b, src, (WW3DFormat)fmt, pal, pal_bpp);
	memcpy(out, b, 4);
	unsigned w;
	memcpy(&w, argb_in, 4);
	BitmapHandlerClass::Read_B8G8R8A8(w, src, (WW3DFormat)fmt, pal, pal_bpp);
	memcpy(out + 4, &w, 4);
}

unsigned ref_w6_bm_read_xy(unsigned fmt, int x, int y, int w, int h, const unsigned char *src, const unsigned char *pal, unsigned pal_bpp, unsigned argb_in)
{
	unsigned argb = argb_in;
	BitmapHandlerClass::Read_B8G8R8A8(argb, src, (WW3DFormat)fmt, x, y, w, h, pal, pal_bpp);
	return argb;
}

void ref_w6_bm_write(unsigned fmt, unsigned argb, unsigned char *buf8)
{
	BitmapHandlerClass::Write_B8G8R8A8(buf8, (WW3DFormat)fmt, argb);
}

void ref_w6_bm_copy_pixel(unsigned dfmt, unsigned sfmt, const unsigned char *src, const unsigned char *pal, unsigned pal_bpp, int use_hsv, const float *hsv, unsigned char *buf8)
{
	if (use_hsv) {
		Vector3 h(hsv[0], hsv[1], hsv[2]);
		BitmapHandlerClass::Copy_Pixel(buf8, (WW3DFormat)dfmt, src, (WW3DFormat)sfmt, pal, pal_bpp, h);
	} else {
		BitmapHandlerClass::Copy_Pixel(buf8, (WW3DFormat)dfmt, src, (WW3DFormat)sfmt, pal, pal_bpp);
	}
}

unsigned ref_w6_bm_combine(unsigned a, unsigned b, unsigned c, unsigned d) { return BitmapHandlerClass::Combine_A8R8G8B8(a, b, c, d); }

void ref_w6_bm_mip(unsigned char *dest, unsigned dpitch, unsigned char *src, unsigned spitch, unsigned w, unsigned h)
{
	BitmapHandlerClass::Create_Mipmap_B8G8R8A8(dest, dpitch, src, spitch, w, h);
}

void ref_w6_bm_genmip(unsigned w, unsigned h, unsigned char *dest, unsigned dpitch, unsigned dfmt, unsigned char *src, unsigned spitch, unsigned sfmt, unsigned char *mip, unsigned mpitch, const float *hsv)
{
	Vector3 hs(hsv[0], hsv[1], hsv[2]);
	BitmapHandlerClass::Copy_Image_Generate_Mipmap(w, h, dest, dpitch, (WW3DFormat)dfmt, src, spitch, (WW3DFormat)sfmt, mip, mpitch, hs);
}

void ref_w6_bm_copy_image(unsigned char *dest, unsigned dw, unsigned dh, unsigned dp, unsigned dfmt, unsigned char *src, unsigned sw, unsigned sh, unsigned sp, unsigned sfmt,
	const unsigned char *pal, unsigned pal_bpp, int genmip, const float *hsv)
{
	Vector3 hs(hsv[0], hsv[1], hsv[2]);
	BitmapHandlerClass::Copy_Image(dest, dw, dh, dp, (WW3DFormat)dfmt, src, sw, sh, sp, (WW3DFormat)sfmt, pal, pal_bpp, genmip != 0, hs);
}

}
