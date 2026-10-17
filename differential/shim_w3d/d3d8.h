/* Shim, differential tests only: the part of the Direct3D 8 SDK's d3d8.h /
** d3d8types.h that formconv.cpp, ww3dformat.cpp and ddsfile.cpp name. The
** SDK is not in EA's release. The D3DFORMAT values are the documented ones
** of d3d8types.h (the DXT codes are MAKEFOURCC('D','X','T','n')). The enum
** has `int` as its underlying type, as MSVC gives every enum, so a FourCC
** with the top bit set is a negative D3DFORMAT here as in the original. */
#ifndef SHIM_D3D8_H
#define SHIM_D3D8_H

#define MAKEFOURCC(ch0, ch1, ch2, ch3) \
	((unsigned int)(unsigned char)(ch0) | ((unsigned int)(unsigned char)(ch1) << 8) | \
	((unsigned int)(unsigned char)(ch2) << 16) | ((unsigned int)(unsigned char)(ch3) << 24))

enum D3DFORMAT : int
{
	D3DFMT_UNKNOWN = 0,
	D3DFMT_R8G8B8 = 20,
	D3DFMT_A8R8G8B8 = 21,
	D3DFMT_X8R8G8B8 = 22,
	D3DFMT_R5G6B5 = 23,
	D3DFMT_X1R5G5B5 = 24,
	D3DFMT_A1R5G5B5 = 25,
	D3DFMT_A4R4G4B4 = 26,
	D3DFMT_R3G3B2 = 27,
	D3DFMT_A8 = 28,
	D3DFMT_A8R3G3B2 = 29,
	D3DFMT_X4R4G4B4 = 30,
	D3DFMT_A8P8 = 40,
	D3DFMT_P8 = 41,
	D3DFMT_L8 = 50,
	D3DFMT_A8L8 = 51,
	D3DFMT_A4L4 = 52,
	D3DFMT_V8U8 = 60,
	D3DFMT_L6V5U5 = 61,
	D3DFMT_X8L8V8U8 = 62,
	D3DFMT_Q8W8V8U8 = 63,
	D3DFMT_V16U16 = 64,
	D3DFMT_W11V11U10 = 65,
	D3DFMT_UYVY = (int)MAKEFOURCC('U', 'Y', 'V', 'Y'),
	D3DFMT_YUY2 = (int)MAKEFOURCC('Y', 'U', 'Y', '2'),
	D3DFMT_DXT1 = (int)MAKEFOURCC('D', 'X', 'T', '1'),
	D3DFMT_DXT2 = (int)MAKEFOURCC('D', 'X', 'T', '2'),
	D3DFMT_DXT3 = (int)MAKEFOURCC('D', 'X', 'T', '3'),
	D3DFMT_DXT4 = (int)MAKEFOURCC('D', 'X', 'T', '4'),
	D3DFMT_DXT5 = (int)MAKEFOURCC('D', 'X', 'T', '5'),
	D3DFMT_D16_LOCKABLE = 70,
	D3DFMT_D32 = 71,
	D3DFMT_D15S1 = 73,
	D3DFMT_D24S8 = 75,
	D3DFMT_D16 = 80,
	D3DFMT_D24X8 = 77,
	D3DFMT_D24X4S4 = 79,
	D3DFMT_VERTEXDATA = 100,
	D3DFMT_INDEX16 = 101,
	D3DFMT_INDEX32 = 102,
	D3DFMT_FORCE_DWORD = 0x7fffffff
};

/* ddsfile.cpp's IDirect3DSurface8 overload of Copy_Level_To_Surface (not
** called by the tests): the members it uses. */
typedef long HRESULT;
struct D3DSURFACE_DESC
{
	D3DFORMAT Format;
	int Type;
	unsigned int Usage;
	int Pool;
	unsigned int Size;
	int MultiSampleType;
	unsigned int Width;
	unsigned int Height;
};
struct D3DLOCKED_RECT
{
	int Pitch;
	void *pBits;
};
struct RECT;
struct IDirect3DSurface8
{
	HRESULT GetDesc(D3DSURFACE_DESC *) { abort(); }
	HRESULT LockRect(D3DLOCKED_RECT *, const RECT *, unsigned int) { abort(); }
	HRESULT UnlockRect() { abort(); }
};
#endif
