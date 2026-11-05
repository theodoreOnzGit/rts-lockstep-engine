/* Differential-test wrapper for ShaderClass (upstream commit 0a05454d),
** GitHub issue #10, step 4:
**   WW3D2/shader.cpp   ShaderClass::Apply (with srcBlendLUT, dstBlendLUT,
**                      class Blend, ShaderDirty, CurrentShader,
**                      _PolygonCullMode), Get_SS_Category, Guess_Sort_Level
** COMPILED from verbatim segments of shader.cpp (build.rs writes them to
** OUT_DIR/shader_def.cpp) with shader.h, against the recording device of
** shim_w3d/shader_prelude.h. This file only sets the device up, applies a
** shader with ShaderDirty set (Invalidate), and reads back what was written.
**
** ref_w3d_shader_apply's `out` (52 words):
**   [0..26)  13 render states, each (written, value), in the order
**            ALPHABLENDENABLE, SRCBLEND, DESTBLEND, ALPHATESTENABLE, ALPHAREF,
**            ALPHAFUNC, FOGENABLE, FOGCOLOR, SPECULARENABLE, ZFUNC,
**            ZWRITEENABLE, CULLMODE, PATCHSEGMENTS (a float's bits);
**   [26..50) texture stages 0 and 1, each COLOROP, COLORARG1, COLORARG2,
**            ALPHAOP, ALPHAARG1, ALPHAARG2 as (written, value);
**   [50]     Get_SS_Category(), [51] Guess_Sort_Level().
** Returns the number of direct device calls (the Voodoo 3 path; 0 here). */
#include <string.h>
#include "shader.h"

DX8Caps DX8Wrapper::CurrentCaps;
bool DX8Wrapper::FogEnable;
D3DCOLOR DX8Wrapper::FogColor;
unsigned DX8Wrapper::RenderStates[256];
bool DX8Wrapper::RenderStateWritten[256];
unsigned DX8Wrapper::StageStates[8][32];
bool DX8Wrapper::StageStateWritten[8][32];
int DX8Wrapper::DirectCalls;
int WW3D::NPatchesLevel = 1;

void DX8Wrapper::Apply_Shader(ShaderClass &shader) { shader.Apply(); }

extern unsigned long _PolygonCullMode; /* shader.cpp */

extern "C" int ref_w3d_shader_apply(unsigned bits, unsigned tex_op_caps, int mod_alpha_add_clr, int fog_allowed, int fog_enable,
	unsigned fog_color, int cull_ccw, int npatches_level, unsigned *out)
{
	memset(DX8Wrapper::RenderStates, 0, sizeof(DX8Wrapper::RenderStates));
	memset(DX8Wrapper::RenderStateWritten, 0, sizeof(DX8Wrapper::RenderStateWritten));
	memset(DX8Wrapper::StageStates, 0, sizeof(DX8Wrapper::StageStates));
	memset(DX8Wrapper::StageStateWritten, 0, sizeof(DX8Wrapper::StageStateWritten));
	DX8Wrapper::DirectCalls = 0;
	DX8Wrapper::CurrentCaps.Caps.TextureOpCaps = tex_op_caps;
	DX8Wrapper::CurrentCaps.ModAlphaAddClr = mod_alpha_add_clr != 0;
	DX8Wrapper::CurrentCaps.FogAllowed = fog_allowed != 0;
	DX8Wrapper::FogEnable = fog_enable != 0;
	DX8Wrapper::FogColor = fog_color;
	WW3D::NPatchesLevel = npatches_level;
	_PolygonCullMode = cull_ccw ? D3DCULL_CCW : D3DCULL_CW;

	ShaderClass shader(bits);
	ShaderClass::Invalidate();
	DX8Wrapper::Apply_Shader(shader);

	static const int rs[13] = { D3DRS_ALPHABLENDENABLE, D3DRS_SRCBLEND, D3DRS_DESTBLEND, D3DRS_ALPHATESTENABLE, D3DRS_ALPHAREF,
		D3DRS_ALPHAFUNC, D3DRS_FOGENABLE, D3DRS_FOGCOLOR, D3DRS_SPECULARENABLE, D3DRS_ZFUNC, D3DRS_ZWRITEENABLE, D3DRS_CULLMODE,
		D3DRS_PATCHSEGMENTS };
	int k = 0;
	for (int i = 0; i < 13; ++i) {
		out[k++] = DX8Wrapper::RenderStateWritten[rs[i]];
		out[k++] = DX8Wrapper::RenderStates[rs[i]];
	}
	static const int ts[6] = { D3DTSS_COLOROP, D3DTSS_COLORARG1, D3DTSS_COLORARG2, D3DTSS_ALPHAOP, D3DTSS_ALPHAARG1, D3DTSS_ALPHAARG2 };
	for (int stage = 0; stage < 2; ++stage) {
		for (int i = 0; i < 6; ++i) {
			out[k++] = DX8Wrapper::StageStateWritten[stage][ts[i]];
			out[k++] = DX8Wrapper::StageStates[stage][ts[i]];
		}
	}
	out[k++] = (unsigned)shader.Get_SS_Category();
	out[k++] = (unsigned)shader.Guess_Sort_Level();
	return DX8Wrapper::DirectCalls;
}
