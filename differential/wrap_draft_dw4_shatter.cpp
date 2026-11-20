/* Differential-test wrapper for src/drafts/sweep_ww3d/shattersystem.rs (GitHub #215, group ww3d4; upstream commit 0a05454d).
** VERBATIM cut by build.rs (`drafts_ww3d4`), from WW3D2/shattersystem.cpp: BSPClass::Set_Plane_From_Transform.
** Stand-in: BSPClass is the one member the cut writes (PlaneClass Plane, shattersystem.cpp:196) and the function; Matrix3D and
** PlaneClass are the real matrix3d.h / plane.h.
** Entry point: ref_dw4_shatter_run(in, nin, out, cap): in = [the 12 floats of the transform (row-major 3x4), the plane's 4 initial
**  words (N.x N.y N.z D)]; out = [N.x, N.y, N.z, D] bits. */
#include <stdint.h>
#include <string.h>
#include "always.h"
#include "vector3.h"
#include "matrix3d.h"
#include "plane.h"

namespace {

class BSPClass {
public:
	void Set_Plane_From_Transform(const Matrix3D & tm);
	PlaneClass Plane;
};

#include "ss_plane.inc"

}

extern "C" int ref_dw4_shatter_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	float m[12];
	memcpy(m, in, 48);
	BSPClass b;
	float init[4];
	memcpy(init, in + 12, 16);
	b.Plane.Set(init[0], init[1], init[2], init[3]);
	b.Set_Plane_From_Transform(Matrix3D(m));
	if (cap < 4) return -1;
	memcpy(out, &b.Plane.N.X, 4); memcpy(out + 1, &b.Plane.N.Y, 4); memcpy(out + 2, &b.Plane.N.Z, 4); memcpy(out + 3, &b.Plane.D, 4);
	return 4;
}
