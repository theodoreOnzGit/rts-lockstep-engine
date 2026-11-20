/* Differential-test wrapper for the draft src/drafts/ww3d/projector.rs (GitHub #215, group "ww3d7"), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/projector.h  the class (w7_proj_h.inc);  projector.cpp from the constructor to the end (w7_proj_cpp.inc)
** cut VERBATIM by build.rs (`drafts_ww3d7`, each anchor asserted to occur once). matrixmapper.h and matrixmapper.cpp are upstream's, copied byte for byte
** (so that their quoted includes find the stand-ins of differential/shim_draft_ww3dl: mapper.h's TextureMapperClass, dx8wrapper.h's logging calls);
** their classes are renamed W7* by -D so that the matrixmapper draft's own library does not meet them. Matrix3D, Matrix4x4 (Init_Ortho,
** Init_Perspective), OBBoxClass::Transform, AABoxClass, Matrix3x3 are WWMath's, linked from the upstream libraries (the C library's `tan` is the
** test support's, petir's).
** Stand-in: the protected members of ProjectorClass are read by a subclass (ProbeProjector), which is what the derived TexProjectClass does.
**
** Entry point (extern "C", tests/differential_draft_ww3d7_projector.rs): ref_dw7_proj(in, nin, out, cap) returns the words written (-1 if too small).
**   in = [n ops, per op (kind, args)]; kinds: 0 Set_Transform(12 floats), 1 Set_Perspective_Projection(hfov, vfov, znear, zfar),
**   2 Set_Ortho_Projection(xmin, xmax, ymin, ymax, znear, zfar), 3 Compute_Texture_Coordinate(x, y, z), 4 Get_Bounding_Volume only.
**   Before the first op and after every op: Transform (12), Projection (16), LocalBoundingVolume (center 3, extent 3), WorldBoundingVolume
**   (center 3, extent 3, basis 9), the mapper's type; a kind 3 op also gives the three texture coordinates first. All words are float bits. */
#include <stdint.h>
#include <string.h>
#include <vector>

#include "always.h"
#include "vector3.h"
#include "matrix3d.h"
#include "matrix4.h"
#include "aabox.h"
#include "obbox.h"
#include "refcount.h" /* projector.cpp includes it (REF_PTR_RELEASE) */
#include "mapper.h"
#include "matrixmapper.h"

std::vector<uint32_t> g_dww3dl_log;

#include "matrixmapper.cpp"

#include "w7_proj_h.inc"
#include "w7_proj_cpp.inc"

typedef uint32_t W;
static std::vector<W> g_out;
static W bits(float f) { W w; memcpy(&w, &f, 4); return w; }
static float flt(W w) { float f; memcpy(&f, &w, 4); return f; }
static void put(float f) { g_out.push_back(bits(f)); }

class ProbeProjector : public ProjectorClass {
public:
	void dump(void)
	{
		for (int i = 0; i < 3; ++i) for (int j = 0; j < 4; ++j) put(Transform[i][j]);
		for (int i = 0; i < 4; ++i) for (int j = 0; j < 4; ++j) put(Projection[i][j]);
		put(LocalBoundingVolume.Center.X); put(LocalBoundingVolume.Center.Y); put(LocalBoundingVolume.Center.Z);
		put(LocalBoundingVolume.Extent.X); put(LocalBoundingVolume.Extent.Y); put(LocalBoundingVolume.Extent.Z);
		const OBBoxClass &b = Get_Bounding_Volume();
		put(b.Center.X); put(b.Center.Y); put(b.Center.Z);
		put(b.Extent.X); put(b.Extent.Y); put(b.Extent.Z);
		for (int i = 0; i < 3; ++i) for (int j = 0; j < 3; ++j) put(b.Basis[i][j]);
		g_out.push_back((W)Peek_Mapper()->Get_Type());
	}
};

extern "C" int ref_dw7_proj(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	g_out.clear();
	const W *p = in;
	ProbeProjector *pr = new ProbeProjector;
	pr->dump();
	W n = *p++;
	for (W k = 0; k < n; ++k) {
		W kind = *p++;
		switch (kind) {
		case 0: {
			Matrix3D m(flt(p[0]), flt(p[1]), flt(p[2]), flt(p[3]), flt(p[4]), flt(p[5]), flt(p[6]), flt(p[7]), flt(p[8]), flt(p[9]), flt(p[10]), flt(p[11]));
			p += 12;
			pr->Set_Transform(m);
			break;
		}
		case 1: pr->Set_Perspective_Projection(flt(p[0]), flt(p[1]), flt(p[2]), flt(p[3])); p += 4; break;
		case 2: pr->Set_Ortho_Projection(flt(p[0]), flt(p[1]), flt(p[2]), flt(p[3]), flt(p[4]), flt(p[5])); p += 6; break;
		case 3: {
			Vector3 pt(flt(p[0]), flt(p[1]), flt(p[2]));
			p += 3;
			Vector3 r;
			pr->Compute_Texture_Coordinate(pt, &r);
			put(r.X); put(r.Y); put(r.Z);
			break;
		}
		default: break;
		}
		pr->dump();
	}
	delete pr;
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
