/* Differential-test wrapper for the HSV and RGB drafts (#215, upstream commit 0a05454d), src/drafts/wwlib/hsv.rs and rgb.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/hsv.cpp (HSVClass::Adjust, Difference, operator RGBClass) and rgb.cpp
**   (RGBClass::Adjust, Difference, operator HSVClass), #included UNCHANGED, with hsv.h and RGB.H (rgb.h, case).
** Stand-ins: <stddef.h> is read first (always.h uses size_t, which the Windows headers gave it); shim_draft_codec/palette.h, empty (rgb.cpp includes PALETTE.H, the Windows palette class, which it does not use).
**
** Entry points (extern "C", tests/differential_draft_fixed_color.rs), colours as three bytes packed in an unsigned (low byte
** first: red/hue, green/saturation, blue/value):
**   ref_rgb_to_hsv(c)  ref_hsv_to_rgb(c)  ref_rgb_adjust(c, ratio, to)  ref_hsv_adjust(c, ratio, to)
**   ref_rgb_difference(a, b)  ref_hsv_difference(a, b) */
#include <stddef.h>
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/hsv.cpp"
#include "../upstream/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/rgb.cpp"

static unsigned pack(unsigned a, unsigned b, unsigned c) { return (a & 255) | ((b & 255) << 8) | ((c & 255) << 16); }

extern "C" {

unsigned ref_rgb_to_hsv(unsigned c)
{
	RGBClass rgb((unsigned char)c, (unsigned char)(c >> 8), (unsigned char)(c >> 16));
	HSVClass hsv = rgb;
	return pack(hsv.Get_Hue(), hsv.Get_Saturation(), hsv.Get_Value());
}

unsigned ref_hsv_to_rgb(unsigned c)
{
	HSVClass hsv((unsigned char)c, (unsigned char)(c >> 8), (unsigned char)(c >> 16));
	RGBClass rgb = hsv;
	return pack(rgb.Get_Red(), rgb.Get_Green(), rgb.Get_Blue());
}

unsigned ref_rgb_adjust(unsigned c, int ratio, unsigned to)
{
	RGBClass rgb((unsigned char)c, (unsigned char)(c >> 8), (unsigned char)(c >> 16));
	RGBClass dst((unsigned char)to, (unsigned char)(to >> 8), (unsigned char)(to >> 16));
	rgb.Adjust(ratio, dst);
	return pack(rgb.Get_Red(), rgb.Get_Green(), rgb.Get_Blue());
}

unsigned ref_hsv_adjust(unsigned c, int ratio, unsigned to)
{
	HSVClass hsv((unsigned char)c, (unsigned char)(c >> 8), (unsigned char)(c >> 16));
	HSVClass dst((unsigned char)to, (unsigned char)(to >> 8), (unsigned char)(to >> 16));
	hsv.Adjust(ratio, dst);
	return pack(hsv.Get_Hue(), hsv.Get_Saturation(), hsv.Get_Value());
}

int ref_rgb_difference(unsigned a, unsigned b)
{
	RGBClass x((unsigned char)a, (unsigned char)(a >> 8), (unsigned char)(a >> 16));
	RGBClass y((unsigned char)b, (unsigned char)(b >> 8), (unsigned char)(b >> 16));
	return x.Difference(y);
}

int ref_hsv_difference(unsigned a, unsigned b)
{
	HSVClass x((unsigned char)a, (unsigned char)(a >> 8), (unsigned char)(a >> 16));
	HSVClass y((unsigned char)b, (unsigned char)(b >> 8), (unsigned char)(b >> 16));
	return x.Difference(y);
}

}
