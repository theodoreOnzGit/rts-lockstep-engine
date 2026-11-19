/* Differential-test wrapper for src/drafts/sweep_ww3d/mapper.rs (GitHub #215, group sweepww3d; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_sweepww3d`), from WW3D2/mapper.cpp: GridTextureMapperClass::Set_Frame_Per_Second, initialize,
** update_temporal_state, calculate_uv_offset.
** Stand-ins: GridTextureMapperClass is the fields of mapper.h (copied unchanged) and the four members above, without the
** TextureMapperClass base; WW3D::Get_Sync_Time() returns g_sync (set by the test before each call).
** Entry point: ref_dsww3d_map_run(in, nin, out, cap): in = [fps bits, gridwidth_log2, last_frame, offset, nops, ops...];
**   the constructor (mapper.cpp:196) is replayed as the fields it assigns, then initialize(); ops:
**   0 Set_Frame_Per_Second(fps bits, sync)  1 update_temporal_state(sync)  2 calculate_uv_offset -> 2 words
**   every op is followed by the 9 state words (Sign, MSPerFrame, OOGridWidth, GridWidthLog2, LastFrame, Offset, Remainder,
**   CurrentFrame, LastUsedSyncTime). */
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static unsigned int g_sync = 0;

static_assert(sizeof(fabs(1.0f)) == 8, "fabs(float) must be the C double function (MSVC 6's; the cut divides in double)");

struct WW3D { static unsigned int Get_Sync_Time(void) { return g_sync; } };

class GridTextureMapperClass
{
public:
	void Set_Frame_Per_Second(float fps);
	void initialize(float fps, unsigned int gridwidth_log2);
	void update_temporal_state(void);
	void calculate_uv_offset(float * u_offset, float * v_offset);

	int				Sign;
	unsigned int	MSPerFrame;
	float				OOGridWidth;
	unsigned int	GridWidthLog2;
	unsigned int	LastFrame;
	unsigned int	Offset;
	unsigned int	Remainder;
	unsigned int	CurrentFrame;
	unsigned int	LastUsedSyncTime;
};

#include "mp_sfps.inc"
#include "mp_init.inc"
#include "mp_uts.inc"
#include "mp_cuv.inc"

static void state(std::vector<uint32_t> & o, const GridTextureMapperClass & g)
{
	o.push_back((uint32_t)g.Sign); o.push_back(g.MSPerFrame); o.push_back(w_of(g.OOGridWidth)); o.push_back(g.GridWidthLog2);
	o.push_back(g.LastFrame); o.push_back(g.Offset); o.push_back(g.Remainder); o.push_back(g.CurrentFrame); o.push_back(g.LastUsedSyncTime);
}

}

extern "C" int ref_dsww3d_map_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	GridTextureMapperClass g;
	memset(&g, 0, sizeof(g));
	float fps = f_of(*p++);
	unsigned int log2 = *p++;
	g.LastFrame = *p++;
	g.Offset = *p++;
	g_sync = *p++;
	g.initialize(fps, log2);
	state(o, g);
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		switch (op) {
		case 0: { float f = f_of(*p++); g_sync = *p++; g.Set_Frame_Per_Second(f); break; }
		case 1: { g_sync = *p++; g.update_temporal_state(); break; }
		case 2: { float u, v; g.calculate_uv_offset(&u, &v); o.push_back(w_of(u)); o.push_back(w_of(v)); break; }
		}
		state(o, g);
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
