/* Differential-test wrapper for src/drafts/sweep_ww3d/dazzle.rs (GitHub #215, group sweepww3d; upstream commit 0a05454d):
** DazzleINIClass::Get_Vector2 / Get_Vector3 / Get_Vector4, LensflareTypeClass::Generate_Vertex_Buffers and
** DazzleTypeClass::Calculate_Intensities, cut VERBATIM from WW3D2/dazzle.cpp by build.rs (`drafts_sweepww3d`).
** NOT covered (device or host calls): Render_Dazzle / Render_Dazzle's index fill, vis_render_dazzle, the type tables' lookups
** by name, the visibility lists and the texture loaders.
** Stand-ins: INIClass::Find_Entry returns the entry the test set (INIEntry::Value, a C string or NULL); DX8Wrapper::Convert_Color
** packs (a hash of the colour and alpha bit patterns: 31*r + 131*g + 1031*b + a, all unsigned, wrapping) so that the colour
** passed is observable; VertexFormatXYZNDUV2 has the six fields the cut assigns (x, y, z, u1, v1, diffuse);
** DazzleInitClass the ten fields Calculate_Intensities reads (types as in dazzle.cpp: float, bool for use_camera_translation);
** LensflareInitClass the five fields Generate_Vertex_Buffers reads (flare_count, flare_locations, flare_sizes, flare_colors, flare_uv).
** `pow` and `sqrt` take float arguments: the file asserts below that they resolve to the C `double` functions (MSVC 6's, which
** the upstream was written for), whose results are narrowed on assignment; `powf` is the float function the source names.
** Entry point: ref_dsww3d_dz_run(in, nin, out, cap), the layout is in tests/differential_draft_dazzle.rs. */
#include <stdint.h>
#include <string.h>
#include <stdio.h>
#include <math.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "wwmath.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

static_assert(sizeof(pow(1.0f, 1.0f)) == 8, "pow(float, float) must be the C double function");
static_assert(sizeof(sqrt(1.0f)) == 8, "sqrt(float) must be the C double function");

struct INIEntry { char * Value; };
static INIEntry g_entry;
class INIClass {
public:
	INIEntry * Find_Entry(const char * section, const char * entry) const { return &g_entry; }
};
class DazzleINIClass : public INIClass {
public:
	const Vector2 Get_Vector2(char const *section, char const *entry, const Vector2 & defvalue = Vector2(0,0) );
	const Vector3 Get_Vector3(char const *section, char const *entry, const Vector3 & defvalue = Vector3(0,0,0) );
	const Vector4 Get_Vector4(char const *section, char const *entry, const Vector4 & defvalue = Vector4(0,0,0,0) ) const;
};

#include "dz_v2.inc"
#include "dz_v3.inc"
#include "dz_v4.inc"

struct DX8Wrapper {
	static unsigned Convert_Color(const Vector3 & c, float a) { return 31u * w_of(c.X) + 131u * w_of(c.Y) + 1031u * w_of(c.Z) + w_of(a); }
};

struct VertexFormatXYZNDUV2 { float x, y, z, u1, v1; unsigned diffuse; };

class LensflareInitClass {
public:
	int flare_count;
	float* flare_locations;
	float* flare_sizes;
	Vector3* flare_colors;
	Vector4* flare_uv;
};

class LensflareTypeClass {
public:
	void Generate_Vertex_Buffers(VertexFormatXYZNDUV2* vertex, int& vertex_count, float screen_x_scale, float screen_y_scale,
		float dazzle_intensity, const Vector4& transformed_location);
	LensflareInitClass lic;
};

#include "dz_gvb.inc"

struct DazzleInitClass {
	float dazzle_area;
	float dazzle_direction_area;
	float dazzle_size_pow;
	float dazzle_intensity_pow;
	float dazzle_intensity;
	float halo_intensity_pow;
	float halo_intensity;
	bool use_camera_translation;
	float fadeout_start;
	float fadeout_end;
};

class DazzleTypeClass {
public:
	void Calculate_Intensities(float& dazzle_intensity, float& dazzle_size, float& halo_intensity, const Vector3& camera_dir,
		const Vector3& dazzle_dir, const Vector3& dir_to_dazzle, float distance) const;
	DazzleInitClass ic;
	float fadeout_end_sqr;
	float fadeout_start_sqr;
};

#include "dz_ci.inc"

static Vector3 v3(const uint32_t * p) { return Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2])); }

}

/* kind 0/1/2: Get_Vector2/3/4: in = [kind, has_value, len, chars (one word each), defaults (2/3/4 floats)] -> 2/3/4 words
** kind 3: Generate_Vertex_Buffers: in = [3, flare_count, then per flare: location, size, colour 3, uv 4, then screen_x_scale,
**   screen_y_scale, dazzle_intensity, transformed_location 4] -> vertex_count, then per vertex x y z u1 v1 diffuse
** kind 4: Calculate_Intensities: in = [4, ic: area, dir_area, size_pow, intensity_pow, intensity, halo_pow, halo_intensity,
**   use_camera_translation, fadeout_start, fadeout_end, fadeout_start_sqr, fadeout_end_sqr, halo_in, camera_dir 3, dazzle_dir 3,
**   dir_to_dazzle 3, distance] -> dazzle_intensity, dazzle_size, halo_intensity (the size starts at 0.0f) */
extern "C" int ref_dsww3d_dz_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	uint32_t kind = *p++;
	if (kind <= 2) {
		bool has = *p++ != 0;
		int len = (int)*p++;
		std::vector<char> text(len + 1);
		for (int i = 0; i < len; i++) text[i] = (char)*p++;
		text[len] = 0;
		g_entry.Value = has ? &text[0] : NULL;
		DazzleINIClass ini;
		if (kind == 0) {
			Vector2 d(f_of(p[0]), f_of(p[1]));
			Vector2 r = ini.Get_Vector2("s", "e", d);
			o.push_back(w_of(r[0])); o.push_back(w_of(r[1]));
		} else if (kind == 1) {
			Vector3 d(f_of(p[0]), f_of(p[1]), f_of(p[2]));
			Vector3 r = ini.Get_Vector3("s", "e", d);
			o.push_back(w_of(r[0])); o.push_back(w_of(r[1])); o.push_back(w_of(r[2]));
		} else {
			Vector4 d(f_of(p[0]), f_of(p[1]), f_of(p[2]), f_of(p[3]));
			Vector4 r = ini.Get_Vector4("s", "e", d);
			for (int i = 0; i < 4; i++) o.push_back(w_of(r[i]));
		}
	} else if (kind == 3) {
		LensflareTypeClass t;
		int n = (int)*p++;
		std::vector<float> loc(n + 1), size(n + 1);
		std::vector<Vector3> col(n + 1);
		std::vector<Vector4> uv(n + 1);
		for (int a = 0; a < n; a++) {
			loc[a] = f_of(*p++); size[a] = f_of(*p++);
			col[a] = v3(p); p += 3;
			uv[a] = Vector4(f_of(p[0]), f_of(p[1]), f_of(p[2]), f_of(p[3])); p += 4;
		}
		t.lic.flare_count = n;
		t.lic.flare_locations = &loc[0]; t.lic.flare_sizes = &size[0]; t.lic.flare_colors = &col[0]; t.lic.flare_uv = &uv[0];
		float sx = f_of(*p++), sy = f_of(*p++), di = f_of(*p++);
		Vector4 tl(f_of(p[0]), f_of(p[1]), f_of(p[2]), f_of(p[3]));
		std::vector<VertexFormatXYZNDUV2> vb(4 * n + 1);
		int count = 0;
		t.Generate_Vertex_Buffers(&vb[0], count, sx, sy, di, tl);
		o.push_back((uint32_t)count);
		for (int i = 0; i < count; i++) {
			o.push_back(w_of(vb[i].x)); o.push_back(w_of(vb[i].y)); o.push_back(w_of(vb[i].z));
			o.push_back(w_of(vb[i].u1)); o.push_back(w_of(vb[i].v1)); o.push_back(vb[i].diffuse);
		}
	} else {
		DazzleTypeClass t;
		t.ic.dazzle_area = f_of(*p++); t.ic.dazzle_direction_area = f_of(*p++); t.ic.dazzle_size_pow = f_of(*p++);
		t.ic.dazzle_intensity_pow = f_of(*p++); t.ic.dazzle_intensity = f_of(*p++); t.ic.halo_intensity_pow = f_of(*p++);
		t.ic.halo_intensity = f_of(*p++); t.ic.use_camera_translation = *p++ != 0; t.ic.fadeout_start = f_of(*p++);
		t.ic.fadeout_end = f_of(*p++); t.fadeout_start_sqr = f_of(*p++); t.fadeout_end_sqr = f_of(*p++);
		float halo = f_of(*p++);
		Vector3 cam = v3(p); p += 3;
		Vector3 daz = v3(p); p += 3;
		Vector3 dir = v3(p); p += 3;
		float distance = f_of(*p++);
		float di = 0.0f, ds = 0.0f;
		t.Calculate_Intensities(di, ds, halo, cam, daz, dir, distance);
		o.push_back(w_of(di)); o.push_back(w_of(ds)); o.push_back(w_of(halo));
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
