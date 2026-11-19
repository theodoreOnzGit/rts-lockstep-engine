/* Stand-in for WW3D2/dx8wrapper.h as texturefilter.cpp uses it (#215, group ww3dlogic): the device's texture-filter caps, the
** stage-state call (logged) and the constants, with the DirectX 8 SDK's values (d3d8types.h, d3d8caps.h; the SDK is not
** in the upstream tree, so these are the SDK's published numbers). The class is renamed so it cannot collide with another
** stand-in's DX8Wrapper in the same archive (issue #219). */
#ifndef DRAFT_TF_DX8WRAPPER_STANDIN_H
#define DRAFT_TF_DX8WRAPPER_STANDIN_H

#include <stdint.h>
#include <vector>
#define DX8Wrapper DX8WrapperTf

extern std::vector<uint32_t> g_dww3dl_tf_log;

#define MAX_TEXTURE_STAGES 8

enum { D3DTEXF_NONE = 0, D3DTEXF_POINT = 1, D3DTEXF_LINEAR = 2, D3DTEXF_ANISOTROPIC = 3 };
enum {
	D3DTSS_ADDRESSU = 13, D3DTSS_ADDRESSV = 14, D3DTSS_MAGFILTER = 16, D3DTSS_MINFILTER = 17, D3DTSS_MIPFILTER = 18,
	D3DTSS_MAXANISOTROPY = 21
};
enum { D3DTADDRESS_WRAP = 1, D3DTADDRESS_CLAMP = 3 };
#define D3DPTFILTERCAPS_MINFPOINT 0x00000100L
#define D3DPTFILTERCAPS_MINFLINEAR 0x00000200L
#define D3DPTFILTERCAPS_MINFANISOTROPIC 0x00000400L
#define D3DPTFILTERCAPS_MIPFPOINT 0x00010000L
#define D3DPTFILTERCAPS_MIPFLINEAR 0x00020000L
#define D3DPTFILTERCAPS_MAGFPOINT 0x01000000L
#define D3DPTFILTERCAPS_MAGFLINEAR 0x02000000L
#define D3DPTFILTERCAPS_MAGFANISOTROPIC 0x04000000L

struct D3DCAPS8 { unsigned long TextureFilterCaps; };
extern D3DCAPS8 g_dww3dl_tf_caps;

class Dx8CapsTf { public: const D3DCAPS8 & Get_DX8_Caps(void) const { return g_dww3dl_tf_caps; } };

class DX8Wrapper
{
public:
	static const Dx8CapsTf * Get_Current_Caps(void) { static Dx8CapsTf c; return &c; }
	static void Set_DX8_Texture_Stage_State(unsigned stage, int state, unsigned long value)
	{
		g_dww3dl_tf_log.push_back(stage);
		g_dww3dl_tf_log.push_back((uint32_t)state);
		g_dww3dl_tf_log.push_back((uint32_t)value);
	}
};

#endif
