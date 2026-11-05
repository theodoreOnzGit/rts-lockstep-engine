/* Forced include (-include, after w3d_prelude.h) for the shader library of
** differential/wrap_w3d_shader.cpp, differential tests only: what
** ShaderClass::Apply (WW3D2/shader.cpp, copied verbatim by build.rs) needs
** from Direct3D 8 and the DX8 wrapper.
**
** - The Direct3D 8 constants it names, with the SDK's values (d3d8types.h,
**   d3d8caps.h). The SDK is not in EA's release.
** - dx8wrapper.h, dx8caps.h, ww3d.h are replaced (their include guards
**   defined): a device whose caps, fog state and N-patch level the test
**   sets, and which records every render state and texture-stage state
**   written (last write wins, with a written flag). DX8CALL (the Voodoo 3
**   path's direct device calls) counts its calls; the test's device is never
**   a Voodoo 3, so the count must stay 0.
** - ShaderClass, DX8Wrapper, DX8Caps and WW3D are renamed (RefShader*), so
**   that this library's classes cannot collide with the mesh library's
**   stand-ins of the same names (shim_w3d/mesh_prelude.h,
**   wrap_w3d_mesh.cpp's Get_SS_Category stub). */
#ifndef SHIM_SHADER_PRELUDE_H
#define SHIM_SHADER_PRELUDE_H

#define ShaderClass RefShaderClass
#define DX8Wrapper RefShaderDX8Wrapper
#define DX8Caps RefShaderDX8Caps
#define WW3D RefShaderWW3D

/* windows.h */
#ifndef TRUE
#define TRUE 1
#define FALSE 0
#endif

#define DX8_WRAPPER_H
#define DX8CAPS_H
#define WW3D_H

enum D3DBLEND { D3DBLEND_ZERO = 1, D3DBLEND_ONE = 2, D3DBLEND_SRCCOLOR = 3, D3DBLEND_INVSRCCOLOR = 4, D3DBLEND_SRCALPHA = 5,
	D3DBLEND_INVSRCALPHA = 6, D3DBLEND_DESTALPHA = 7, D3DBLEND_INVDESTALPHA = 8, D3DBLEND_DESTCOLOR = 9, D3DBLEND_INVDESTCOLOR = 10 };
enum D3DCMPFUNC { D3DCMP_NEVER = 1, D3DCMP_LESS = 2, D3DCMP_EQUAL = 3, D3DCMP_LESSEQUAL = 4, D3DCMP_GREATER = 5,
	D3DCMP_NOTEQUAL = 6, D3DCMP_GREATEREQUAL = 7, D3DCMP_ALWAYS = 8 };
enum D3DCULL { D3DCULL_NONE = 1, D3DCULL_CW = 2, D3DCULL_CCW = 3 };
enum D3DTEXTUREOP { D3DTOP_DISABLE = 1, D3DTOP_SELECTARG1 = 2, D3DTOP_SELECTARG2 = 3, D3DTOP_MODULATE = 4, D3DTOP_MODULATE2X = 5,
	D3DTOP_MODULATE4X = 6, D3DTOP_ADD = 7, D3DTOP_ADDSIGNED = 8, D3DTOP_ADDSIGNED2X = 9, D3DTOP_SUBTRACT = 10,
	D3DTOP_ADDSMOOTH = 11, D3DTOP_BLENDDIFFUSEALPHA = 12, D3DTOP_BLENDTEXTUREALPHA = 13, D3DTOP_BLENDFACTORALPHA = 14,
	D3DTOP_BLENDTEXTUREALPHAPM = 15, D3DTOP_BLENDCURRENTALPHA = 16, D3DTOP_PREMODULATE = 17,
	D3DTOP_MODULATEALPHA_ADDCOLOR = 18, D3DTOP_MODULATECOLOR_ADDALPHA = 19, D3DTOP_MODULATEINVALPHA_ADDCOLOR = 20,
	D3DTOP_MODULATEINVCOLOR_ADDALPHA = 21, D3DTOP_BUMPENVMAP = 22, D3DTOP_BUMPENVMAPLUMINANCE = 23, D3DTOP_DOTPRODUCT3 = 24 };
#define D3DTA_DIFFUSE 0x00000000
#define D3DTA_CURRENT 0x00000001
#define D3DTA_TEXTURE 0x00000002
#define D3DTEXOPCAPS_DISABLE 0x00000001
#define D3DTEXOPCAPS_SELECTARG1 0x00000002
#define D3DTEXOPCAPS_SELECTARG2 0x00000004
#define D3DTEXOPCAPS_MODULATE 0x00000008
#define D3DTEXOPCAPS_MODULATE2X 0x00000010
#define D3DTEXOPCAPS_MODULATE4X 0x00000020
#define D3DTEXOPCAPS_ADD 0x00000040
#define D3DTEXOPCAPS_ADDSIGNED 0x00000080
#define D3DTEXOPCAPS_ADDSIGNED2X 0x00000100
#define D3DTEXOPCAPS_SUBTRACT 0x00000200
#define D3DTEXOPCAPS_ADDSMOOTH 0x00000400
#define D3DTEXOPCAPS_BLENDDIFFUSEALPHA 0x00000800
#define D3DTEXOPCAPS_BLENDTEXTUREALPHA 0x00001000
#define D3DTEXOPCAPS_BLENDFACTORALPHA 0x00002000
#define D3DTEXOPCAPS_BLENDTEXTUREALPHAPM 0x00004000
#define D3DTEXOPCAPS_BLENDCURRENTALPHA 0x00008000
#define D3DTEXOPCAPS_PREMODULATE 0x00010000
#define D3DTEXOPCAPS_MODULATEALPHA_ADDCOLOR 0x00020000
#define D3DTEXOPCAPS_MODULATECOLOR_ADDALPHA 0x00040000
#define D3DTEXOPCAPS_MODULATEINVALPHA_ADDCOLOR 0x00080000
#define D3DTEXOPCAPS_MODULATEINVCOLOR_ADDALPHA 0x00100000
#define D3DTEXOPCAPS_BUMPENVMAP 0x00200000
#define D3DTEXOPCAPS_BUMPENVMAPLUMINANCE 0x00400000
#define D3DTEXOPCAPS_DOTPRODUCT3 0x00800000
enum D3DRENDERSTATETYPE { D3DRS_ZWRITEENABLE = 14, D3DRS_ALPHATESTENABLE = 15, D3DRS_SRCBLEND = 19, D3DRS_DESTBLEND = 20,
	D3DRS_CULLMODE = 22, D3DRS_ZFUNC = 23, D3DRS_ALPHAREF = 24, D3DRS_ALPHAFUNC = 25, D3DRS_ALPHABLENDENABLE = 27,
	D3DRS_FOGENABLE = 28, D3DRS_SPECULARENABLE = 29, D3DRS_FOGCOLOR = 34, D3DRS_PATCHSEGMENTS = 164 };
enum D3DTEXTURESTAGESTATETYPE { D3DTSS_COLOROP = 1, D3DTSS_COLORARG1 = 2, D3DTSS_COLORARG2 = 3, D3DTSS_ALPHAOP = 4,
	D3DTSS_ALPHAARG1 = 5, D3DTSS_ALPHAARG2 = 6, D3DTSS_TEXCOORDINDEX = 11 };
#define D3DTSS_TCI_PASSTHRU 0x00000000
typedef unsigned int D3DCOLOR;
struct D3DCAPS8 { DWORD TextureOpCaps; };

class ShaderClass;

class DX8Caps
{
public:
	enum { VENDOR_UNKNOWN = 0, VENDOR_3DFX = 1 };
	enum { DEVICE_UNKNOWN = 0, DEVICE_3DFX_VOODOO_3 = 1 };
	D3DCAPS8 Caps;
	bool FogAllowed;
	bool ModAlphaAddClr;
	const D3DCAPS8 &Get_DX8_Caps() const { return Caps; }
	int Get_Vendor() const { return VENDOR_UNKNOWN; }
	int Get_Device() const { return DEVICE_UNKNOWN; }
	bool Is_Fog_Allowed() const { return FogAllowed; }
	bool Support_ModAlphaAddClr() const { return ModAlphaAddClr; }
};

class DX8Wrapper
{
public:
	static DX8Caps CurrentCaps;
	static bool FogEnable;
	static D3DCOLOR FogColor;
	static unsigned RenderStates[256];
	static bool RenderStateWritten[256];
	static unsigned StageStates[8][32];
	static bool StageStateWritten[8][32];
	static int DirectCalls;
	static DX8Caps *Get_Current_Caps() { return &CurrentCaps; }
	static bool Get_Fog_Enable() { return FogEnable; }
	static D3DCOLOR Get_Fog_Color() { return FogColor; }
	static void Set_DX8_Render_State(D3DRENDERSTATETYPE state, unsigned value)
	{
		RenderStates[state] = value;
		RenderStateWritten[state] = true;
	}
	static void Set_DX8_Texture_Stage_State(unsigned stage, D3DTEXTURESTAGESTATETYPE state, unsigned value)
	{
		StageStates[stage][state] = value;
		StageStateWritten[stage][state] = true;
	}
	/* ShaderClass::Apply is private; DX8Wrapper is its friend */
	static void Apply_Shader(ShaderClass &shader);
};
#define DX8CALL(x) (++RefShaderDX8Wrapper::DirectCalls)
#define SNAPSHOT_SAY(x)

class WW3D
{
public:
	static int NPatchesLevel;
	static int Get_NPatches_Level() { return NPatchesLevel; }
};

#endif
