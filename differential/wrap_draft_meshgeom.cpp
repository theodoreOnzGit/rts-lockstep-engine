/* Differential-test wrapper for the MeshGeometryClass collision-member drafts (GitHub #215, upstream commit 0a05454d),
** group "ww3dgeo": src/drafts/sweep_ww3d/meshgeometry.rs.
**
** VERBATIM cuts by build.rs (`drafts_ww3dgeo`, anchors asserted to occur once, from the signature line to the closing
** brace) from WW3D2/meshgeometry.cpp (mg_*.inc): the three Generate_Rigid_APT overloads, Generate_Skin_APT,
** cast_semi_infinite_axis_aligned_ray, cast_aabox_identity / _z90 / _z180 / _z270, intersect_obbox_brute_force,
** cast_ray_brute_force, cast_aabox_brute_force, cast_obbox_brute_force. #included after the stand-ins below. The cuts'
** `#ifdef COMPUTE_NORMALS` branches stay in the text and are not compiled (COMPUTE_NORMALS is not defined upstream).
**
** Stand-ins (what the cuts call outside themselves; every one scripted, and logging the triangle it was given):
**   MeshGeometryClass: PolyCount, CullTree, Get_Vertex_Array / Get_Plane_Array / Get_Polygon_Array / Get_Polygon_Count /
**     Get_Poly_Surface_Type (= (poly * 7 + 3) % 32), from the case's arrays. TriClass: N and V[3] (tri.h's, minus the
**     members the cuts do not call). TriIndex: Vector3i16, as upstream.
**   OBBoxClass, AABoxClass, LineSegClass: empty (the cuts only pass them on).
**   AABoxCollisionTestClass / OBBoxCollisionTestClass / OBBoxIntersectionTestClass / RayCollisionTestClass (coltest.h): the
**     members the cuts read (Box, Move, Ray, Result). AABoxCollisionTestClass::Translate and Rotate (the real ones move the
**     box) only log, as does the host's cast_aabox_on_copy in the draft.
**   CollisionMath::Intersection_Test, Collide (ray, AABox sweep, OBBox sweep) and the global
**     Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle (tri.h): the n-th call of the case answers from hash(seed, n): hit,
**     StartBad, a normal, flag bits. AABTreeClass (CullTree): Generate_APT (two), Cast_Semi_Infinite_Axis_Aligned_Ray and
**     Cast_AABox answer the same way.
** Event codes: 300 Intersection_Test + triangle; 301 Collide ray + triangle; 302 Collide AABox + triangle; 303 Collide
** OBBox + triangle; 304 axis-aligned ray to triangle + triangle, plane (4), start (3), r, a1, a2, direction; 310 CullTree
** Generate_APT box; 311 Generate_APT box + view dir (3); 312 CullTree ray + start (3) + axis_dir; 313 CullTree Cast_AABox;
** 320 AABox Translate (3); 321 AABox Rotate (type). A triangle is v0, v1, v2, N (12 floats as bits).
**
** Entry point (extern "C", tests/differential_draft_meshgeom.rs): ref_dmg_run(in, nin, out, cap). See the test for the ops. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector3.h"
#include "vector4.h"
#include "vector3i.h"
#include "simplevec.h"
#include "bittype.h"
#include "castres.h"

typedef Vector3i16 TriIndex;

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static std::vector<uint32_t> g_log;
static uint32_t g_seed = 0;
static uint32_t g_call = 0;

static uint32_t next_hash(void)
{
	uint32_t h = (g_seed ^ (g_call * 0x9E3779B1u)) * 2654435761u;
	h ^= h >> 15;
	h *= 2246822519u;
	h ^= h >> 13;
	g_call++;
	return h;
}

class TriClass { public: const Vector3 * N; const Vector3 * V[3]; };
class OBBoxClass {};
class AABoxClass {};
class LineSegClass {};

enum { TRI_RAYCAST_FLAG_NONE = 0x00 };

class AABoxCollisionTestClass
{
public:
	enum RotationType { ROTATE_NONE = 0, ROTATE_Z90, ROTATE_Z180, ROTATE_Z270 };
	AABoxClass Box;
	Vector3 Move;
	CastResultStruct * Result;
	void Translate(const Vector3 & translation) { g_log.push_back(320); g_log.push_back(w_of(translation.X)); g_log.push_back(w_of(translation.Y)); g_log.push_back(w_of(translation.Z)); }
	void Rotate(RotationType type) { g_log.push_back(321); g_log.push_back((uint32_t)type); }
};
class OBBoxCollisionTestClass { public: OBBoxClass Box; Vector3 Move; CastResultStruct * Result; };
class OBBoxIntersectionTestClass { public: OBBoxClass Box; };
class RayCollisionTestClass { public: LineSegClass Ray; CastResultStruct * Result; };

static void log_tri(uint32_t code, const TriClass & t)
{
	g_log.push_back(code);
	for (int i = 0; i < 3; i++) { g_log.push_back(w_of(t.V[i]->X)); g_log.push_back(w_of(t.V[i]->Y)); g_log.push_back(w_of(t.V[i]->Z)); }
	g_log.push_back(w_of(t.N->X)); g_log.push_back(w_of(t.N->Y)); g_log.push_back(w_of(t.N->Z));
}

static void script_result(uint32_t h, CastResultStruct * r)
{
	if ((h >> 1) % 5 == 0) r->StartBad = true;
	r->Normal.X = (float)((int)((h >> 8) & 255) - 128);
	r->Normal.Y = (float)((int)((h >> 16) & 255) - 128);
	r->Normal.Z = (float)((int)((h >> 24) & 255) - 128);
}

class CollisionMath
{
public:
	static bool Intersection_Test(const OBBoxClass &, const TriClass & tri)
	{
		log_tri(300, tri);
		return (next_hash() & 1) != 0;
	}
	static bool Collide(const LineSegClass &, const TriClass & tri, CastResultStruct * res)
	{
		log_tri(301, tri);
		uint32_t h = next_hash();
		bool hit = (h & 1) != 0;
		if (hit) script_result(h, res);
		return hit;
	}
	static bool Collide(const AABoxClass &, const Vector3 &, const TriClass & tri, CastResultStruct * res)
	{
		log_tri(302, tri);
		uint32_t h = next_hash();
		bool hit = (h & 1) != 0;
		if (hit) script_result(h, res);
		return hit;
	}
	static bool Collide(const OBBoxClass &, const Vector3 &, const TriClass & tri, const Vector3 &, CastResultStruct * res)
	{
		log_tri(303, tri);
		uint32_t h = next_hash();
		bool hit = (h & 1) != 0;
		if (hit) script_result(h, res);
		return hit;
	}
};

inline bool Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle(const Vector3 &tri_point0,
	const Vector3 &tri_point1, const Vector3 &tri_point2, const Vector4 &tri_plane,
	const Vector3 &ray_start, int axis_r, int axis_1, int axis_2, int direction,
	unsigned char & flags)
{
	g_log.push_back(304);
	const Vector3 * v[3] = { &tri_point0, &tri_point1, &tri_point2 };
	for (int i = 0; i < 3; i++) { g_log.push_back(w_of(v[i]->X)); g_log.push_back(w_of(v[i]->Y)); g_log.push_back(w_of(v[i]->Z)); }
	g_log.push_back(w_of(tri_plane.X)); g_log.push_back(w_of(tri_plane.Y)); g_log.push_back(w_of(tri_plane.Z)); g_log.push_back(w_of(tri_plane.W));
	g_log.push_back(w_of(ray_start.X)); g_log.push_back(w_of(ray_start.Y)); g_log.push_back(w_of(ray_start.Z));
	g_log.push_back((uint32_t)axis_r); g_log.push_back((uint32_t)axis_1); g_log.push_back((uint32_t)axis_2); g_log.push_back((uint32_t)direction);
	uint32_t h = next_hash();
	flags |= (unsigned char)((h >> 8) & 3);
	return (h & 1) != 0;
}

class AABTreeClass
{
public:
	void Generate_APT(const OBBoxClass &, SimpleDynVecClass<uint32> & apt)
	{
		g_log.push_back(310);
		uint32_t h = next_hash();
		for (uint32_t i = 0; i < h % 4; i++) apt.Add((h >> 4) % 100 + i);
	}
	void Generate_APT(const OBBoxClass &, const Vector3 & viewdir, SimpleDynVecClass<uint32> & apt)
	{
		g_log.push_back(311); g_log.push_back(w_of(viewdir.X)); g_log.push_back(w_of(viewdir.Y)); g_log.push_back(w_of(viewdir.Z));
		uint32_t h = next_hash();
		for (uint32_t i = 0; i < h % 4; i++) apt.Add((h >> 4) % 100 + i);
	}
	int Cast_Semi_Infinite_Axis_Aligned_Ray(const Vector3 & start_point, int axis_dir, unsigned char & flags)
	{
		g_log.push_back(312); g_log.push_back(w_of(start_point.X)); g_log.push_back(w_of(start_point.Y)); g_log.push_back(w_of(start_point.Z)); g_log.push_back((uint32_t)axis_dir);
		uint32_t h = next_hash();
		flags = (unsigned char)((h >> 8) & 7);
		return (int)(h % 6);
	}
	bool Cast_AABox(AABoxCollisionTestClass & boxtest)
	{
		g_log.push_back(313);
		uint32_t h = next_hash();
		bool hit = (h & 1) != 0;
		if (hit) script_result(h, boxtest.Result);
		return hit;
	}
};

std::vector<Vector3> g_verts;
std::vector<Vector4> g_planes;
std::vector<TriIndex> g_polys;

class MeshGeometryClass
{
public:
	int PolyCount;
	AABTreeClass * CullTree;
	const Vector3 * Get_Vertex_Array(void) { return g_verts.empty() ? 0 : &g_verts[0]; }
	const Vector4 * Get_Plane_Array(void) { return g_planes.empty() ? 0 : &g_planes[0]; }
	const TriIndex * Get_Polygon_Array(void) { return g_polys.empty() ? 0 : &g_polys[0]; }
	int Get_Polygon_Count(void) { return PolyCount; }
	uint32 Get_Poly_Surface_Type(int poly) { return (uint32)((poly * 7 + 3) % 32); }

	void Generate_Rigid_APT(const Vector3 & view_dir, SimpleDynVecClass<uint32> & apt);
	void Generate_Rigid_APT(const OBBoxClass & local_box, SimpleDynVecClass<uint32> & apt);
	void Generate_Rigid_APT(const OBBoxClass & local_box, const Vector3 & viewdir, SimpleDynVecClass<uint32> & apt);
	void Generate_Skin_APT(const OBBoxClass & world_box, SimpleDynVecClass<uint32> & apt, const Vector3 *world_vertex_locs);
	int cast_semi_infinite_axis_aligned_ray(const Vector3 & start_point, int axis_dir, unsigned char & flags);
	bool cast_aabox_identity(AABoxCollisionTestClass & boxtest, const Vector3 & translation);
	bool cast_aabox_z90(AABoxCollisionTestClass & boxtest, const Vector3 & translation);
	bool cast_aabox_z180(AABoxCollisionTestClass & boxtest, const Vector3 & translation);
	bool cast_aabox_z270(AABoxCollisionTestClass & boxtest, const Vector3 & translation);
	bool intersect_obbox_brute_force(OBBoxIntersectionTestClass & localtest);
	bool cast_ray_brute_force(RayCollisionTestClass & raytest);
	bool cast_aabox_brute_force(AABoxCollisionTestClass & boxtest);
	bool cast_obbox_brute_force(OBBoxCollisionTestClass & boxtest);
};

#include "mg_apt_dir.inc"
#include "mg_apt_box.inc"
#include "mg_apt_box_dir.inc"
#include "mg_skin.inc"
#include "mg_ray.inc"
#include "mg_z0.inc"
#include "mg_z90.inc"
#include "mg_z180.inc"
#include "mg_z270.inc"
#include "mg_ibf.inc"
#include "mg_rbf.inc"
#include "mg_abf.inc"
#include "mg_obf.inc"

}

/* ref_dmg_run(in, nin, out, cap): in = [op, seed, has_cull, nverts, 3*nverts floats, npolys, per poly: 3 indices and 4 plane
**   floats, then the op's operands]; the script counter starts at 0.
**   ops (operands -> out, after which the event log follows):
**   0 Generate_Rigid_APT(view_dir)      view_dir 3 floats, apt prefill n + words        -> [apt n, apt...]
**   1 Generate_Rigid_APT(box)           apt prefill                                      -> [apt n, apt...]
**   2 Generate_Rigid_APT(box, viewdir)  view_dir 3, apt prefill                          -> [apt n, apt...]
**   3 Generate_Skin_APT                 3*nverts world vertex floats, apt prefill        -> [apt n, apt...]
**   4 cast_semi_infinite_axis_aligned_ray  start 3 floats, axis_dir, flags prefill       -> [count, flags]
**   5..8 cast_aabox_identity / z90 / z180 / z270  translation 3, result prefill (start_bad, normal 3, surface)  -> [ret, start_bad, normal 3, surface]
**   9 intersect_obbox_brute_force                                                        -> [ret]
**   10 cast_ray_brute_force, 11 cast_aabox_brute_force, 12 cast_obbox_brute_force  result prefill -> [ret, start_bad, normal 3, surface] */
extern "C" int ref_dmg_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	g_log.clear();
	const uint32_t * p = in;
	uint32_t op = *p++;
	g_seed = *p++;
	g_call = 0;
	bool has_cull = *p++ != 0;
	uint32_t nv = *p++;
	g_verts.clear();
	for (uint32_t i = 0; i < nv; i++) { g_verts.push_back(Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2]))); p += 3; }
	uint32_t np = *p++;
	g_planes.clear(); g_polys.clear();
	for (uint32_t i = 0; i < np; i++) {
		TriIndex t; t.I = (uint16)p[0]; t.J = (uint16)p[1]; t.K = (uint16)p[2];
		g_polys.push_back(t);
		g_planes.push_back(Vector4(f_of(p[3]), f_of(p[4]), f_of(p[5]), f_of(p[6])));
		p += 7;
	}
	AABTreeClass tree;
	MeshGeometryClass mesh;
	mesh.PolyCount = (int)np;
	mesh.CullTree = has_cull ? &tree : 0;
	std::vector<uint32_t> o;
	SimpleDynVecClass<uint32> apt;
	CastResultStruct res;
	bool is_apt = op <= 3;
	bool is_res = (op >= 5 && op <= 8) || op >= 10;
	std::vector<Vector3> world;
	switch (op) {
	case 0: { Vector3 vd(f_of(p[0]), f_of(p[1]), f_of(p[2])); p += 3; uint32_t n = *p++; for (uint32_t i = 0; i < n; i++) apt.Add(*p++); mesh.Generate_Rigid_APT(vd, apt); break; }
	case 1: { uint32_t n = *p++; for (uint32_t i = 0; i < n; i++) apt.Add(*p++); OBBoxClass b; mesh.Generate_Rigid_APT(b, apt); break; }
	case 2: { Vector3 vd(f_of(p[0]), f_of(p[1]), f_of(p[2])); p += 3; uint32_t n = *p++; for (uint32_t i = 0; i < n; i++) apt.Add(*p++); OBBoxClass b; mesh.Generate_Rigid_APT(b, vd, apt); break; }
	case 3: { for (uint32_t i = 0; i < nv; i++) { world.push_back(Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2]))); p += 3; } uint32_t n = *p++; for (uint32_t i = 0; i < n; i++) apt.Add(*p++); OBBoxClass b; mesh.Generate_Skin_APT(b, apt, world.empty() ? 0 : &world[0]); break; }
	case 4: {
		Vector3 sp(f_of(p[0]), f_of(p[1]), f_of(p[2])); int axis = (int)p[3]; unsigned char flags = (unsigned char)p[4];
		int c = mesh.cast_semi_infinite_axis_aligned_ray(sp, axis, flags);
		o.push_back((uint32_t)c); o.push_back(flags);
		break; }
	case 9: { OBBoxIntersectionTestClass t; o.push_back(mesh.intersect_obbox_brute_force(t) ? 1u : 0u); break; }
	default: break;
	}
	if (is_apt) {
		o.push_back((uint32_t)apt.Count());
		for (int i = 0; i < apt.Count(); i++) o.push_back(apt[i]);
	}
	if (is_res) {
		Vector3 tr(0, 0, 0);
		if (op >= 5 && op <= 8) { tr = Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2])); p += 3; }
		res.StartBad = p[0] != 0; res.Normal = Vector3(f_of(p[1]), f_of(p[2]), f_of(p[3])); res.SurfaceType = p[4];
		bool r = false;
		AABoxCollisionTestClass ab; ab.Result = &res;
		switch (op) {
		case 5: r = mesh.cast_aabox_identity(ab, tr); break;
		case 6: r = mesh.cast_aabox_z90(ab, tr); break;
		case 7: r = mesh.cast_aabox_z180(ab, tr); break;
		case 8: r = mesh.cast_aabox_z270(ab, tr); break;
		case 10: { RayCollisionTestClass t; t.Result = &res; r = mesh.cast_ray_brute_force(t); break; }
		case 11: r = mesh.cast_aabox_brute_force(ab); break;
		case 12: { OBBoxCollisionTestClass t; t.Result = &res; r = mesh.cast_obbox_brute_force(t); break; }
		}
		o.push_back(r ? 1u : 0u); o.push_back(res.StartBad ? 1u : 0u);
		o.push_back(w_of(res.Normal.X)); o.push_back(w_of(res.Normal.Y)); o.push_back(w_of(res.Normal.Z));
		o.push_back(res.SurfaceType);
	}
	for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
	(void)nin;
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
