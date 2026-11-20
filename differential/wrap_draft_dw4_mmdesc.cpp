/* Differential-test wrapper for src/drafts/sweep_ww3d/meshmatdesc.rs (GitHub #215, group ww3d4; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_ww3d4`), from WW3D2/meshmatdesc.cpp: UVBufferClass::Is_Equal_To and ::Update_CRC.
** Stand-ins: UVBufferClass is the fields of meshmatdesc.h (a Vector2 array, its count, CRC initialised to 0xFFFFFFFF) without the
** ShareBufferClass base; CRC_Memory is the real WWLib/realcrc.cpp, linked from upstream_w3d_mesh.
** Entry point: ref_dw4_mmdesc_run(in, nin, out, cap): in = [na, nb, then 2*na + 2*nb float bits]; out = [crc_a_before, crc_a, crc_b,
**  equal(a, b), equal(a, a)]. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "vector2.h"

unsigned long CRC_Memory(const unsigned char *data, unsigned long length, unsigned long crc = 0);

namespace {

class UVBufferClass {
public:
	UVBufferClass(int count) : Array(count), CRC(0xFFFFFFFF) {}
	Vector2 * Get_Array() { return Array.data(); }
	int Get_Count() { return (int)Array.size(); }
	bool Is_Equal_To(const UVBufferClass & that);
	void Update_CRC(void);
	unsigned int Get_CRC(void) { return CRC; }
	std::vector<Vector2> Array;
	unsigned int CRC;
};

#include "md_eq.inc"
#include "md_crc.inc"

}

extern "C" int ref_dw4_mmdesc_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	const uint32_t * p = in;
	int na = (int)*p++, nb = (int)*p++;
	UVBufferClass a(na), b(nb);
	for (int i = 0; i < na; i++) { memcpy(&a.Array[i].X, p++, 4); memcpy(&a.Array[i].Y, p++, 4); }
	for (int i = 0; i < nb; i++) { memcpy(&b.Array[i].X, p++, 4); memcpy(&b.Array[i].Y, p++, 4); }
	uint32_t before = a.Get_CRC();
	a.Update_CRC(); b.Update_CRC();
	uint32_t r[5] = { before, a.Get_CRC(), b.Get_CRC(), a.Is_Equal_To(b) ? 1u : 0u, a.Is_Equal_To(a) ? 1u : 0u };
	if (cap < 5) return -1;
	memcpy(out, r, sizeof(r));
	return 5;
}
