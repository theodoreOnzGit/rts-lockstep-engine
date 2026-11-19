/* Differential-test wrapper for the draft src/drafts/ww3d/texturefilter.rs (GitHub #215, group ww3dlogic; upstream commit
** 0a05454d): WW3D2/texturefilter.h unchanged and texturefilter.cpp WHOLE (#included below) from the copy build.rs writes to
** OUT_DIR/draft_ww3dlogic_gen/tf (two mechanical edits, each asserted once: `#include "dx8wrapper.h"` -> `"tf_dx8wrapper.h"`,
** the stand-in of differential/shim_draft_ww3dl_tf/, and `for (i=0;i<MAX_TEXTURE_STAGES;++i) {` -> `for (int i=...`, because
** upstream relies on MSVC 6's leaked loop variable).
** Stand-in: DX8Wrapper::Get_Current_Caps()->Get_DX8_Caps().TextureFilterCaps (g_dww3dl_tf_caps) and Set_DX8_Texture_Stage_State
** (logs stage, state, value).
**
** Entry point (extern "C", tests/differential_draft_texturefilter.rs):
**   ref_dww3dl_tf_run(in, nin, out, cap): in = [nops, ops...]; the three tables are zeroed first and one filter object
**   (mip count 0) exists. ops: 0 _Init_Filters(mode, caps) -> then 96 words (as op 12); 1/2/3 _Set_Default_Min/Mag/Mip_Filter(f); 4 construct(mipcount);
**   5/6/7 Set_Min/Mag/Mip(f); 8/9 Set_U/V_Addr_Mode(m); 10 Apply(stage) -> count then the log words; 11 getters -> 5 words
**   (min, mag, mip, u, v); 12 dump -> 96 words (min, mag, mip tables, stage-major). Returns the words written, or -1. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "tf_dx8wrapper.h"

std::vector<uint32_t> g_dww3dl_tf_log;
D3DCAPS8 g_dww3dl_tf_caps;

#include "texturefilter.h"
#include "texturefilter.cpp"

static void dump(std::vector<uint32_t> &o)
{
	for (int s = 0; s < MAX_TEXTURE_STAGES; s++) for (int t = 0; t < 4; t++) o.push_back(_MinTextureFilters[s][t]);
	for (int s = 0; s < MAX_TEXTURE_STAGES; s++) for (int t = 0; t < 4; t++) o.push_back(_MagTextureFilters[s][t]);
	for (int s = 0; s < MAX_TEXTURE_STAGES; s++) for (int t = 0; t < 4; t++) o.push_back(_MipMapFilters[s][t]);
}

extern "C" int ref_dww3dl_tf_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	std::vector<uint32_t> o;
	memset(_MinTextureFilters, 0, sizeof(_MinTextureFilters));
	memset(_MagTextureFilters, 0, sizeof(_MagTextureFilters));
	memset(_MipMapFilters, 0, sizeof(_MipMapFilters));
	TextureFilterClass *f = new TextureFilterClass((MipCountType)0);
	int p = 0;
	int nops = (int)in[p++];
	for (int k = 0; k < nops; k++) {
		int op = (int)in[p++];
		switch (op) {
		case 0: { TextureFilterClass::TextureFilterMode m = (TextureFilterClass::TextureFilterMode)in[p++]; g_dww3dl_tf_caps.TextureFilterCaps = in[p++]; g_dww3dl_tf_log.clear(); TextureFilterClass::_Init_Filters(m); dump(o); break; }
		case 1: TextureFilterClass::_Set_Default_Min_Filter((TextureFilterClass::FilterType)in[p++]); break;
		case 2: TextureFilterClass::_Set_Default_Mag_Filter((TextureFilterClass::FilterType)in[p++]); break;
		case 3: TextureFilterClass::_Set_Default_Mip_Filter((TextureFilterClass::FilterType)in[p++]); break;
		case 4: delete f; f = new TextureFilterClass((MipCountType)in[p++]); break;
		case 5: f->Set_Min_Filter((TextureFilterClass::FilterType)in[p++]); break;
		case 6: f->Set_Mag_Filter((TextureFilterClass::FilterType)in[p++]); break;
		case 7: f->Set_Mip_Mapping((TextureFilterClass::FilterType)in[p++]); break;
		case 8: f->Set_U_Addr_Mode((TextureFilterClass::TxtAddrMode)in[p++]); break;
		case 9: f->Set_V_Addr_Mode((TextureFilterClass::TxtAddrMode)in[p++]); break;
		case 10: {
			g_dww3dl_tf_log.clear();
			f->Apply(in[p++]);
			o.push_back((uint32_t)g_dww3dl_tf_log.size());
			for (size_t i = 0; i < g_dww3dl_tf_log.size(); i++) o.push_back(g_dww3dl_tf_log[i]);
			break;
		}
		case 11:
			o.push_back((uint32_t)f->Get_Min_Filter()); o.push_back((uint32_t)f->Get_Mag_Filter()); o.push_back((uint32_t)f->Get_Mip_Mapping());
			o.push_back((uint32_t)f->Get_U_Addr_Mode()); o.push_back((uint32_t)f->Get_V_Addr_Mode());
			break;
		case 12: dump(o); break;
		}
	}
	delete f;
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
