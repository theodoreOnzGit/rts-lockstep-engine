/* Differential-test wrapper for the seismic filter draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_common_client/terrain_visual.rs, cut verbatim by build.rs into OUT_DIR/draft_guilogic_gen:
**   seismic_h.inc          GameClient/TerrainVisual.h from DEFAULT_SEISMIC_SIMULATION_MAGNITUDE to the LOD enum (the filter base,
**                          SeismicSimulationNode and DomeStyleSeismicFilter);
**   seismic_map_iface.inc  Common/MapObject.h, WorldHeightMapInterfaceClass;  seismic_map_factor.inc  its MAP_XY_FACTOR;
**   seismic.inc            GameClient/Terrain/TerrainVisual.cpp, DomeStyleSeismicFilter::filterCallback and applyGravityCallback
**                          (to the end of the file).
** BaseType.h: seismic_pi.inc (PI) and seismic_sqr.inc (sqr), cut verbatim (the shim BaseType.h has neither).
** Stand-ins: PreRTS.h (shim); REAL_TO_INT_FLOOR (used only by the node constructor that takes a Coord3D, which is not run); the heightmap: a WorldHeightMapInterfaceClass whose border size is given and whose bilinear sample is
** a hash of (x, y, salt) (replacing WorldHeightMap's velocity grid), with every setSeismicZVelocity remembered and returned by later
** samples of that cell (so a draft that interleaved its reads and writes would differ); every call is logged.
**
** Entry points (extern "C", tests/differential_draft_seismic.rs):
**   ref_seismic_filter(null_map, center_x, center_y, radius, life, magnitude_bits, border, salt, out, cap)
**       runs filterCallback on a node with those members; returns the number of words written to out (or -1 if cap was too small);
**       out = [status, then events: 1 x y (a bilinear sample call), 2 x y value-bits (a set call), 3 (getBorderSize call)]
**   ref_seismic_gravity(velocity_bits)  applyGravityCallback, as bits. */
#include "PreRTS.h"
#include "always.h"
#include <list>
#include <vector>
#include <map>
#include <string.h>
#include "Common/Dict.h"
#include "seismic_pi.inc"
#include "seismic_sqr.inc"
#define REAL_TO_INT_FLOOR(x) ((int)floorf(x)) /* stand-in: only the node's (Coord3D, ...) constructor, not run here, uses it */
#include "seismic_map_factor.inc"
#include "seismic_map_iface.inc"
#include "seismic_h.inc"
#include "seismic.inc"

namespace {

std::vector<int> g_log;
std::map<std::pair<int, int>, float> g_over;
unsigned g_salt = 0;
int g_border = 0;

float bitsToFloat(int i) { float f; memcpy(&f, &i, sizeof f); return f; }
int floatToBits(float f) { int i; memcpy(&i, &f, sizeof i); return i; }

float hashSample(int x, int y)
{
	unsigned h = (unsigned)x * 73856093u ^ (unsigned)y * 19349663u ^ g_salt * 83492791u;
	h ^= h >> 15;
	h *= 2246822519u;
	h ^= h >> 13;
	return (float)(int)(h & 0xFFFFF) / 65536.0f - 8.0f;
}

class StandInMap : public WorldHeightMapInterfaceClass
{
public:
	virtual Int getBorderSize() { g_log.push_back(3); return g_border; }
	virtual Real getSeismicZVelocity(Int, Int) const { return 0.0f; }
	virtual void setSeismicZVelocity(Int x, Int y, Real value)
	{
		g_log.push_back(2); g_log.push_back(x); g_log.push_back(y); g_log.push_back(floatToBits(value));
		g_over[std::make_pair(x, y)] = value;
	}
	virtual Real getBilinearSampleSeismicZVelocity(Int x, Int y)
	{
		g_log.push_back(1); g_log.push_back(x); g_log.push_back(y);
		std::map<std::pair<int, int>, float>::iterator it = g_over.find(std::make_pair(x, y));
		return it != g_over.end() ? it->second : hashSample(x, y);
	}
};

}

extern "C" {

int ref_seismic_filter(int null_map, int cx, int cy, int radius, int life, int mag_bits, int border, unsigned salt, int *out, int cap)
{
	g_log.clear();
	g_over.clear();
	g_salt = salt;
	g_border = border;
	SeismicSimulationNode node;
	node.m_center.x = cx;
	node.m_center.y = cy;
	node.m_radius = radius;
	node.m_life = (UnsignedInt)life;
	node.m_magnitude = bitsToFloat(mag_bits);
	StandInMap map;
	DomeStyleSeismicFilter filter;
	SeismicSimulationFilterBase *f = &filter;
	int status = (int)f->filterCallback(null_map ? NULL : &map, &node);
	if ((int)g_log.size() + 1 > cap) return -1;
	out[0] = status;
	for (size_t i = 0; i < g_log.size(); ++i) out[1 + i] = g_log[i];
	return (int)g_log.size() + 1;
}

int ref_seismic_gravity(int velocity_bits)
{
	DomeStyleSeismicFilter filter;
	SeismicSimulationFilterBase *f = &filter;
	return floatToBits(f->applyGravityCallback(bitsToFloat(velocity_bits)));
}

}
