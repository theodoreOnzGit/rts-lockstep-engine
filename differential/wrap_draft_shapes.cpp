/* Differential-test wrapper for the procedural-shape render object drafts (GitHub #215, upstream commit 0a05454d), group
** "ww3dgeo": src/drafts/sweep_ww3d/{ringobj,sphereobj,boxrobj}.rs (the non-drawing members only).
**
** VERBATIM cuts by build.rs (`drafts_ww3dgeo`, anchors asserted to occur once) #included below after the stand-ins:
**   ringobj.cpp   RingRenderObjClass::Generate_Shared_Mesh_Arrays, calculate_value_array, Init_Material,
**                 Get_Default_Color / _Alpha / _Inner_Scale / _Outer_Scale, update_cached_box; RingMeshClass::Set_Tiling
**   sphereobj.cpp SphereRenderObjClass::Generate_Shared_Mesh_Arrays, calculate_value_array, Init_Material,
**                 Get_Default_Color / _Alpha / _Scale / _Vector, update_cached_box
**   boxrobj.cpp   AABoxRenderObjClass::update_cached_box, OBBoxRenderObjClass::update_cached_box
**   rendobj.cpp   RenderObjClass::AT_MIN_LOD and AT_MAX_LOD (the two initialisers; FLT_MAX from <float.h>)
** Stand-ins (what the cuts call outside themselves; each class has only the members the cuts use, with upstream's types):
**   RingRenderObjClass / SphereRenderObjClass / AABoxRenderObjClass / OBBoxRenderObjClass: the fields (Transform, LODBias,
**   Color/Alpha/Scale/Vector current values and their channels, ObjSpaceCenter/Extent, CachedBox, the material and shader
**   pointers/values). RingMeshClass / SphereMeshClass: Generate (logs its arguments and sets the polygon count to
**   (int)(size * 12.0f) + 4), Get_Num_Polys, Set_Alpha_Vector (logs), and for the ring the tiling fields (Slices, TileCount,
**   Vertex_ct, vtx_uv). VertexMaterialClass (Set_* log the call), ShaderClass (_PresetAlphaShader a tagged value, Set_Cull_Mode
**   logs). The key channels (Get_Key_Count, Get_Key(i).Get_Value()). AlphaVectorStruct: sphereobj.h's is a Quaternion and an
**   intensity; here five floats (X, Y, Z, W, intensity). REF_PTR_RELEASE / NEW_REF: refcount.h's, minus the counting.
** Event codes: 420 Ring Generate(radius bits, size bits); 421 Sphere Generate(radius, size, size); 422 Set_Alpha_Vector(5
** words, 2 flags); 400..406 Set_Ambient, Set_Diffuse, Set_Specular, Set_Emissive (3 floats each), Set_Opacity, Set_Shininess,
** Set_Lighting (1 word); 410 Set_Cull_Mode(mode); 411 shader tag after the assignment.
**
** Entry point (extern "C", tests/differential_draft_shapes.rs): ref_dsh_run(in, nin, out, cap); the ops are listed there. */
#include <stdint.h>
#include <string.h>
#include <float.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector2.h"
#include "vector3.h"
#include "matrix3d.h"
#include "matrix3.h"
#include "aabox.h"
#include "obbox.h"

#define REF_PTR_RELEASE(x)		{ if (x) x->Release_Ref(); x = NULL; }
#define NEW_REF(C, P)			(new C P)

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static std::vector<uint32_t> g_log;

#define RING_NUM_LOD	(20)
#define RING_LOWEST_LOD (10)
#define RING_HIGHEST_LOD (50)
#define SPHERE_NUM_LOD		(10)
#define SPHERE_LOWEST_LOD	(7)
#define SPHERE_HIGHEST_LOD (17)

struct AlphaVectorStruct { float X, Y, Z, W; float intensity; };

class RenderObjClass { public: static const float AT_MIN_LOD; static const float AT_MAX_LOD; };
#include "ro_at.inc"
static const float AT_MIN_LOD = RenderObjClass::AT_MIN_LOD;
static const float AT_MAX_LOD = RenderObjClass::AT_MAX_LOD;

class VertexMaterialClass
{
public:
	void Release_Ref(void) { delete this; }
	void Set_Ambient(float r, float g, float b) { g_log.push_back(400); g_log.push_back(w_of(r)); g_log.push_back(w_of(g)); g_log.push_back(w_of(b)); }
	void Set_Diffuse(float r, float g, float b) { g_log.push_back(401); g_log.push_back(w_of(r)); g_log.push_back(w_of(g)); g_log.push_back(w_of(b)); }
	void Set_Specular(float r, float g, float b) { g_log.push_back(402); g_log.push_back(w_of(r)); g_log.push_back(w_of(g)); g_log.push_back(w_of(b)); }
	void Set_Emissive(float r, float g, float b) { g_log.push_back(403); g_log.push_back(w_of(r)); g_log.push_back(w_of(g)); g_log.push_back(w_of(b)); }
	void Set_Opacity(float o) { g_log.push_back(404); g_log.push_back(w_of(o)); }
	void Set_Shininess(float s) { g_log.push_back(405); g_log.push_back(w_of(s)); }
	void Set_Lighting(bool l) { g_log.push_back(406); g_log.push_back(l ? 1u : 0u); }
};

class ShaderClass
{
public:
	enum { CULL_MODE_DISABLE = 0, CULL_MODE_ENABLE = 1 };
	uint32_t tag;
	ShaderClass() : tag(0) {}
	static ShaderClass _PresetAlphaShader;
	void Set_Cull_Mode(int m) { g_log.push_back(410); g_log.push_back((uint32_t)m); }
};
ShaderClass ShaderClass::_PresetAlphaShader;

template<class T> class Key { public: T v; T Get_Value(void) const { return v; } };
template<class T> class Channel
{
public:
	std::vector<Key<T> > keys;
	int Get_Key_Count(void) const { return (int)keys.size(); }
	Key<T> Get_Key(int i) const { return keys[i]; }
};

/* ---- ring ---- */
static bool Ring_Array_Valid = false;
class RingMeshClass
{
public:
	int Slices; int TileCount; int Vertex_ct; Vector2 * vtx_uv; int polys;
	void Generate(float radius, float size)
	{
		g_log.push_back(420); g_log.push_back(w_of(radius)); g_log.push_back(w_of(size));
		polys = (int)(size * 12.0f) + 4;
	}
	int Get_Num_Polys(void) const { return polys; }
	void Set_Tiling(int count);
};
RingMeshClass RingMeshArray[RING_NUM_LOD];
float RingLODCosts[RING_NUM_LOD + 1];
#include "ri_set_tiling.inc"

class RingRenderObjClass
{
public:
	Matrix3D Transform; float LODBias;
	Vector3 Color; float Alpha; Vector2 InnerScale; Vector2 OuterScale;
	Channel<Vector3> ColorChannel; Channel<float> AlphaChannel; Channel<Vector2> InnerScaleChannel; Channel<Vector2> OuterScaleChannel;
	Vector3 ObjSpaceCenter, ObjSpaceExtent; AABoxClass CachedBox;
	VertexMaterialClass * RingMaterial; ShaderClass RingShader;
	RingRenderObjClass() : RingMaterial(NULL) {}
	void Generate_Shared_Mesh_Arrays(void);
	void calculate_value_array(float screen_area, float *values) const;
	void Init_Material(void);
	Vector3 Get_Default_Color(void) const;
	float Get_Default_Alpha(void) const;
	Vector2 Get_Default_Inner_Scale(void) const;
	Vector2 Get_Default_Outer_Scale(void) const;
	void update_cached_box(void);
};
#include "ri_gen.inc"
#include "ri_cva.inc"
#include "ri_mat.inc"
#include "ri_gdc.inc"
#include "ri_gda.inc"
#include "ri_gdi.inc"
#include "ri_gdo.inc"
#include "ri_ucb.inc"

/* ---- sphere ---- */
static bool Sphere_Array_Valid = false;
class SphereMeshClass
{
public:
	int polys;
	void Generate(float radius, float xsize, float ysize)
	{
		g_log.push_back(421); g_log.push_back(w_of(radius)); g_log.push_back(w_of(xsize)); g_log.push_back(w_of(ysize));
		polys = (int)(xsize * 12.0f) + 4;
	}
	int Get_Num_Polys(void) const { return polys; }
	void Set_Alpha_Vector(const AlphaVectorStruct & v, bool a, bool b)
	{
		g_log.push_back(422); g_log.push_back(w_of(v.X)); g_log.push_back(w_of(v.Y)); g_log.push_back(w_of(v.Z)); g_log.push_back(w_of(v.W)); g_log.push_back(w_of(v.intensity));
		g_log.push_back(a ? 1u : 0u); g_log.push_back(b ? 1u : 0u);
	}
};
SphereMeshClass SphereMeshArray[SPHERE_NUM_LOD];
float SphereLODCosts[SPHERE_NUM_LOD + 1];

class SphereRenderObjClass
{
public:
	Matrix3D Transform; float LODBias;
	Vector3 CurrentColor; float CurrentAlpha; Vector3 CurrentScale; AlphaVectorStruct CurrentVector;
	Channel<Vector3> ColorChannel; Channel<float> AlphaChannel; Channel<Vector3> ScaleChannel; Channel<AlphaVectorStruct> VectorChannel;
	Vector3 ObjSpaceCenter, ObjSpaceExtent; AABoxClass CachedBox;
	VertexMaterialClass * SphereMaterial; ShaderClass SphereShader;
	SphereRenderObjClass() : SphereMaterial(NULL) {}
	void Generate_Shared_Mesh_Arrays(const AlphaVectorStruct & alphavector);
	void calculate_value_array(float screen_area, float *values) const;
	void Init_Material(void);
	Vector3 Get_Default_Color(void) const;
	float Get_Default_Alpha(void) const;
	Vector3 Get_Default_Scale(void) const;
	AlphaVectorStruct Get_Default_Vector(void) const;
	void update_cached_box(void);
};
#include "sp_gen.inc"
#include "sp_cva.inc"
#include "sp_mat.inc"
#include "sp_gdc.inc"
#include "sp_gda.inc"
#include "sp_gds.inc"
#include "sp_gdv.inc"
#include "sp_ucb.inc"

/* ---- boxes ---- */
class AABoxRenderObjClass { public: Matrix3D Transform; Vector3 ObjSpaceCenter, ObjSpaceExtent; AABoxClass CachedBox; void update_cached_box(void); };
class OBBoxRenderObjClass { public: Matrix3D Transform; Vector3 ObjSpaceCenter, ObjSpaceExtent; OBBoxClass CachedBox; void update_cached_box(void); };
#include "bx_aab.inc"
#include "bx_obb.inc"

static Matrix3D mat_in(const uint32_t * p) { return Matrix3D(f_of(p[0]), f_of(p[1]), f_of(p[2]), f_of(p[3]), f_of(p[4]), f_of(p[5]), f_of(p[6]), f_of(p[7]), f_of(p[8]), f_of(p[9]), f_of(p[10]), f_of(p[11])); }
static Vector3 v3_in(const uint32_t * p) { return Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2])); }
static void v3_out(std::vector<uint32_t> & o, const Vector3 & v) { o.push_back(w_of(v.X)); o.push_back(w_of(v.Y)); o.push_back(w_of(v.Z)); }

}

/* ops (all `in` words after the op code):
**  0 reset the two "array valid" flags -> nothing
**  1 ring Generate_Shared_Mesh_Arrays -> [RingLODCosts (21)] then the log
**  2 ring calculate_value_array [screen_area, LODBias] -> [values (22)]
**  3 ring Init_Material -> [shader tag] then the log
**  4 ring Get_Default_* [n_color_keys, color key (3), current color (3), n_alpha_keys, key, current, n_inner, key (2), current (2),
**    n_outer, key (2), current (2)] -> [color (3), alpha, inner (2), outer (2)]; a key list has 0 or 1 entries (only key 0 is read)
**  5 ring update_cached_box [matrix (12), center (3), extent (3)] -> [center (3), extent (3)]
**  6 ring Set_Tiling [Slices, TileCount, Vertex_ct, count, prefill u, prefill v for each vertex] -> [TileCount, uv...]
**  7 sphere Generate_Shared_Mesh_Arrays [alphavector (5)] -> [SphereLODCosts (11)] then the log
**  8 sphere calculate_value_array [screen_area, LODBias] -> [values (12)]
**  9 sphere Init_Material -> [shader tag] then the log
** 10 sphere Get_Default_* [n_color, key (3), current (3), n_alpha, key, current, n_scale, key (3), current (3), n_vec, key (5),
**    current (5)] -> [color (3), alpha, scale (3), vector (5)]
** 11 sphere update_cached_box (as 5)
** 12 AABox update_cached_box (as 5)
** 13 OBBox update_cached_box [as 5] -> [center (3), extent (3), basis (9, row major)]
** 14 the two AT_ constants -> [AT_MIN_LOD, AT_MAX_LOD] */
extern "C" int ref_dsh_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	uint32_t op = *p++;
	g_log.clear();
	switch (op) {
	case 0: Ring_Array_Valid = false; Sphere_Array_Valid = false; break;
	case 1: {
		RingRenderObjClass r; r.Generate_Shared_Mesh_Arrays();
		for (int i = 0; i <= RING_NUM_LOD; i++) o.push_back(w_of(RingLODCosts[i]));
		break; }
	case 2: {
		RingRenderObjClass r; r.LODBias = f_of(p[1]);
		float v[RING_NUM_LOD + 2]; for (int i = 0; i < RING_NUM_LOD + 2; i++) v[i] = 0.0f;
		r.calculate_value_array(f_of(p[0]), v);
		for (int i = 0; i < RING_NUM_LOD + 2; i++) o.push_back(w_of(v[i]));
		break; }
	case 3: { RingRenderObjClass r; r.Init_Material(); o.push_back(r.RingShader.tag); break; }
	case 4: {
		RingRenderObjClass r;
		uint32_t n = *p++; Key<Vector3> kc; kc.v = v3_in(p); p += 3; if (n) r.ColorChannel.keys.push_back(kc); r.Color = v3_in(p); p += 3;
		n = *p++; Key<float> ka; ka.v = f_of(*p++); if (n) r.AlphaChannel.keys.push_back(ka); r.Alpha = f_of(*p++);
		n = *p++; Key<Vector2> ki; ki.v = Vector2(f_of(p[0]), f_of(p[1])); p += 2; if (n) r.InnerScaleChannel.keys.push_back(ki); r.InnerScale = Vector2(f_of(p[0]), f_of(p[1])); p += 2;
		n = *p++; Key<Vector2> ko; ko.v = Vector2(f_of(p[0]), f_of(p[1])); p += 2; if (n) r.OuterScaleChannel.keys.push_back(ko); r.OuterScale = Vector2(f_of(p[0]), f_of(p[1])); p += 2;
		Vector3 c = r.Get_Default_Color(); v3_out(o, c);
		o.push_back(w_of(r.Get_Default_Alpha()));
		Vector2 a = r.Get_Default_Inner_Scale(); o.push_back(w_of(a.X)); o.push_back(w_of(a.Y));
		Vector2 b = r.Get_Default_Outer_Scale(); o.push_back(w_of(b.X)); o.push_back(w_of(b.Y));
		break; }
	case 5: {
		RingRenderObjClass r; r.Transform = mat_in(p); r.ObjSpaceCenter = v3_in(p + 12); r.ObjSpaceExtent = v3_in(p + 15);
		r.update_cached_box(); v3_out(o, r.CachedBox.Center); v3_out(o, r.CachedBox.Extent);
		break; }
	case 6: {
		RingMeshClass m; m.Slices = (int)p[0]; m.TileCount = (int)p[1]; m.Vertex_ct = (int)p[2]; int count = (int)p[3];
		std::vector<Vector2> uv; for (int i = 0; i < m.Vertex_ct; i++) uv.push_back(Vector2(f_of(p[4 + 2 * i]), f_of(p[5 + 2 * i])));
		m.vtx_uv = uv.empty() ? 0 : &uv[0];
		m.Set_Tiling(count);
		o.push_back((uint32_t)m.TileCount);
		for (int i = 0; i < m.Vertex_ct; i++) { o.push_back(w_of(uv[i].X)); o.push_back(w_of(uv[i].Y)); }
		break; }
	case 7: {
		SphereRenderObjClass s; AlphaVectorStruct av; av.X = f_of(p[0]); av.Y = f_of(p[1]); av.Z = f_of(p[2]); av.W = f_of(p[3]); av.intensity = f_of(p[4]);
		s.Generate_Shared_Mesh_Arrays(av);
		for (int i = 0; i <= SPHERE_NUM_LOD; i++) o.push_back(w_of(SphereLODCosts[i]));
		break; }
	case 8: {
		SphereRenderObjClass s; s.LODBias = f_of(p[1]);
		float v[SPHERE_NUM_LOD + 2]; for (int i = 0; i < SPHERE_NUM_LOD + 2; i++) v[i] = 0.0f;
		s.calculate_value_array(f_of(p[0]), v);
		for (int i = 0; i < SPHERE_NUM_LOD + 2; i++) o.push_back(w_of(v[i]));
		break; }
	case 9: { SphereRenderObjClass s; s.Init_Material(); o.push_back(s.SphereShader.tag); break; }
	case 10: {
		SphereRenderObjClass s;
		uint32_t n = *p++; Key<Vector3> kc; kc.v = v3_in(p); p += 3; if (n) s.ColorChannel.keys.push_back(kc); s.CurrentColor = v3_in(p); p += 3;
		n = *p++; Key<float> ka; ka.v = f_of(*p++); if (n) s.AlphaChannel.keys.push_back(ka); s.CurrentAlpha = f_of(*p++);
		n = *p++; Key<Vector3> ks; ks.v = v3_in(p); p += 3; if (n) s.ScaleChannel.keys.push_back(ks); s.CurrentScale = v3_in(p); p += 3;
		n = *p++;
		Key<AlphaVectorStruct> kv; kv.v.X = f_of(p[0]); kv.v.Y = f_of(p[1]); kv.v.Z = f_of(p[2]); kv.v.W = f_of(p[3]); kv.v.intensity = f_of(p[4]); p += 5;
		if (n) s.VectorChannel.keys.push_back(kv);
		s.CurrentVector.X = f_of(p[0]); s.CurrentVector.Y = f_of(p[1]); s.CurrentVector.Z = f_of(p[2]); s.CurrentVector.W = f_of(p[3]); s.CurrentVector.intensity = f_of(p[4]);
		Vector3 c = s.Get_Default_Color(); v3_out(o, c);
		o.push_back(w_of(s.Get_Default_Alpha()));
		Vector3 sc = s.Get_Default_Scale(); v3_out(o, sc);
		AlphaVectorStruct v = s.Get_Default_Vector();
		o.push_back(w_of(v.X)); o.push_back(w_of(v.Y)); o.push_back(w_of(v.Z)); o.push_back(w_of(v.W)); o.push_back(w_of(v.intensity));
		break; }
	case 11: {
		SphereRenderObjClass s; s.Transform = mat_in(p); s.ObjSpaceCenter = v3_in(p + 12); s.ObjSpaceExtent = v3_in(p + 15);
		s.update_cached_box(); v3_out(o, s.CachedBox.Center); v3_out(o, s.CachedBox.Extent);
		break; }
	case 12: {
		AABoxRenderObjClass b; b.Transform = mat_in(p); b.ObjSpaceCenter = v3_in(p + 12); b.ObjSpaceExtent = v3_in(p + 15);
		b.update_cached_box(); v3_out(o, b.CachedBox.Center); v3_out(o, b.CachedBox.Extent);
		break; }
	case 13: {
		OBBoxRenderObjClass b; b.Transform = mat_in(p); b.ObjSpaceCenter = v3_in(p + 12); b.ObjSpaceExtent = v3_in(p + 15);
		b.update_cached_box(); v3_out(o, b.CachedBox.Center); v3_out(o, b.CachedBox.Extent);
		for (int r = 0; r < 3; r++) for (int c = 0; c < 3; c++) o.push_back(w_of(b.CachedBox.Basis[r][c]));
		break; }
	case 14: o.push_back(w_of(AT_MIN_LOD)); o.push_back(w_of(AT_MAX_LOD)); break;
	default: return -1;
	}
	for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
	(void)nin;
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
