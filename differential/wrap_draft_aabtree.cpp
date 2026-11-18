/* Differential-test wrapper for the AABTreeClass recursive casts and active-polygon-table generators (GitHub #215,
** upstream commit 0a05454d), group "anim", part 5: src/drafts/sweep_ww3d/aabtree.rs.
**
** VERBATIM cuts by build.rs (`drafts_anim`, anchors asserted to occur once) into OUT_DIR/draft_anim_gen:
**   dab_class.inc   aabtree.h, from `struct CullNodeStruct` to the declaration of Generate_OBBox_APT_Recursive: CullNodeStruct,
**                   OBBoxAPTContextStruct, OBBoxRayAPTContextStruct (inside the stand-in class);
**   dab_inline.inc  aabtree.h, CullNodeStruct's accessors Is_Leaf to Get_Poly_Count;
**   dab_funcs.inc   aabtree.cpp, from Generate_OBBox_APT_Recursive(..OBBoxAPTContextStruct..) (line 359) to
**                   Update_Bounding_Boxes_Recursive: both Generate_OBBox_APT_Recursive and Generate_APT(box, viewdir),
**                   Cast_Ray_Recursive, the three Cast_*_Recursive, Intersect_OBBox_Recursive, Cast_Ray_To_Polys, the
**                   semi-infinite ray, AABox and OBBox leaf casts and Intersect_OBBox_With_Polys.
** OPTIMIZE_PLANEEQ_RAM is 1 as in meshgeometry.h (the cut's `#if` is kept), so every triangle's normal is Compute_Normal'd.
**
** Stand-ins, each for something the cut calls outside itself (none of them is under test):
**   AABTreeClass          the members the cut uses (NodeCount, Nodes, PolyIndices, Mesh) and its method declarations;
**   MeshGeometryClass     Get_Vertex_Array, Get_Polygon_Array (TriIndex = the real Vector3i16), Get_Plane_Array and
**                         Get_Poly_Surface_Type (a pseudo-random value from the oracle below, logged);
**   TriClass              V[3], N and Compute_Normal copied from WWMath/tri.h:60-76 (the real tri.h also defines
**                         Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle, which is replaced below);
**   OBBoxClass, AABoxClass (Init_Min_Max keeps min and max), the ray, and the four test classes (Cull, Box, Move, Result,
**   Ray, context): data holders;
**   CollisionMath         every overload the cut calls, and Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle: each is an
**                         ORACLE. It appends its arguments' bits to the log and answers from a counter-driven
**                         SplitMix64 hash of the case's seed, so the Rust test's host (tests/differential_draft_aabtree.rs)
**                         answers identically without a collision maths in common. The log therefore compares every
**                         argument, every call order and every skipped call.
** Oracle: call k (from 0) draws h = splitmix64(seed + k * 0x9E3779B97F4A7C15 ... see mix()); kinds and percentages in
** the Rust host's twin.
**
** Entry point (extern "C"): ref_dab_run(in, nin, out, cap):
**   in = [op, seed, nnodes, nodes*8 words (min 3, max 3, FrontOrPoly0, BackOrPolyCount), npi, poly indices,
**         nverts, 3 floats each, npolys, 3 words each, nplanes, 4 floats each, then the op's operands]
**   op 1 apt_box()                  Generate_APT(box, apt)         -> log, 0xFFFFFFFE, count, indices
**   op 2 apt_dir(vx, vy, vz)        Generate_APT(box, viewdir, apt)
**   op 3 semi(sx, sy, sz, axis_dir, flags_in)  Cast_Semi_Infinite_Axis_Aligned_Ray_Recursive(Nodes[0], ..) with the
**                                   inline wrapper's axis tables -> log, 0xFFFFFFFE, count, flags
**   op 4 aabox()   op 5 obbox()   Cast_AABox_Recursive / Cast_OBBox_Recursive(Nodes[0]) -> log, 0xFFFFFFFE, result, StartBad,
**                                   SurfaceType (start 0xDEADBEEF)
**   op 6 intersect()                Intersect_OBBox_Recursive -> log, 0xFFFFFFFE, result */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <assert.h>
#include <vector>
#include "dsimplevec_this.h"
#include "vector3.h"
#include "vector4.h"
#include "Vector3i.h"

#define OPTIMIZE_PLANEEQ_RAM 1
#define WWASSERT(x)

namespace {

std::vector<uint32_t> g_log;
uint64_t g_seed;
uint64_t g_calls;
void L(uint32_t v) { g_log.push_back(v); }
void LF(float f) { uint32_t v; memcpy(&v, &f, 4); g_log.push_back(v); }
void LV(const Vector3 &v) { LF(v.X); LF(v.Y); LF(v.Z); }

uint64_t mix()
{
	uint64_t z = g_seed + (++g_calls) * 0x9E3779B97F4A7C15ull;
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
	z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
	return z ^ (z >> 31);
}
bool chance(unsigned pct) { return (mix() >> 20) % 100 < pct; }

struct CastResultStruct { bool StartBad; unsigned int SurfaceType; };

class TriClass
{
public:
	const Vector3 *N;
	const Vector3 *V[3];
	void Compute_Normal()
	{
		Vector3::Cross_Product(*(V[1]) - *(V[0]), *(V[2]) - *(V[0]), (Vector3 *)N);
		((Vector3 *)N)->Normalize();
	}
};

typedef Vector3i16 TriIndex;

class AABoxClass
{
public:
	Vector3 Min, Max;
	void Init_Min_Max(const Vector3 &min, const Vector3 &max) { Min = min; Max = max; }
};
class OBBoxClass {};
class RayStand {};

class MeshGeometryClass
{
public:
	std::vector<Vector3> verts;
	std::vector<TriIndex> polys;
	std::vector<Vector4> planes;
	const Vector3 *Get_Vertex_Array(void) { return verts.data(); }
	const TriIndex *Get_Polygon_Array(void) { return polys.data(); }
	const Vector4 *Get_Plane_Array(void) { return planes.data(); }
	unsigned int Get_Poly_Surface_Type(int poly)
	{
		uint32_t v = (uint32_t)(mix() >> 24) & 0xFF;
		L(9); L((uint32_t)poly); L(v);
		return v;
	}
};

void LTri(const TriClass &t)
{
	for (int i = 0; i < 3; ++i) LV(*t.V[i]);
	LV(*t.N);
}

class CollisionMath
{
public:
	static bool Intersection_Test(const OBBoxClass &, const AABoxClass &n)
	{
		bool r = chance(70);
		L(1); LV(n.Min); LV(n.Max); L(r);
		return r;
	}
	static bool Intersection_Test(const OBBoxClass &, const TriClass &t)
	{
		bool r = chance(50);
		L(2); LTri(t); L(r);
		return r;
	}
	static bool Collide(const AABoxClass &, const Vector3 &, const TriClass &t, CastResultStruct *res)
	{
		bool r = chance(25);
		res->StartBad = chance(6);
		L(5); LTri(t); L(r); L(res->StartBad);
		return r;
	}
	static bool Collide(const OBBoxClass &, const Vector3 &, const TriClass &t, const Vector3 &z, CastResultStruct *res)
	{
		bool r = chance(25);
		res->StartBad = chance(6);
		L(6); LTri(t); LV(z); L(r); L(res->StartBad);
		return r;
	}
	static bool Collide(const RayStand &, const TriClass &t, CastResultStruct *res)
	{
		bool r = chance(25);
		res->StartBad = chance(6);
		L(10); LTri(t); L(r); L(res->StartBad);
		return r;
	}
};

bool Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle(const Vector3 &v0, const Vector3 &v1, const Vector3 &v2, const Vector4 &plane,
	const Vector3 &start, int axis_r, int axis_1, int axis_2, int direction, unsigned char &flags)
{
	L(8); LV(v0); LV(v1); LV(v2); LF(plane.X); LF(plane.Y); LF(plane.Z); LF(plane.W); LV(start);
	L(axis_r); L(axis_1); L(axis_2); L(direction); L(flags);
	uint64_t h = mix();
	bool r = (h >> 20) % 100 < 50;
	flags |= (unsigned char)((h >> 40) & 7);
	L(r); L(flags);
	return r;
}

class RayCollisionTestClass
{
public:
	RayStand Ray;
	CastResultStruct *Result;
	bool Cull(const Vector3 &mn, const Vector3 &mx) { bool r = chance(25); L(4); LV(mn); LV(mx); L(r); return r; }
};
class AABoxCollisionTestClass
{
public:
	AABoxClass Box;
	Vector3 Move;
	CastResultStruct *Result;
	bool Cull(const Vector3 &mn, const Vector3 &mx) { bool r = chance(25); L(4); LV(mn); LV(mx); L(r); return r; }
};
class OBBoxCollisionTestClass
{
public:
	OBBoxClass Box;
	Vector3 Move;
	CastResultStruct *Result;
	bool Cull(const Vector3 &mn, const Vector3 &mx) { bool r = chance(25); L(4); LV(mn); LV(mx); L(r); return r; }
};
class OBBoxIntersectionTestClass
{
public:
	OBBoxClass Box;
	bool Cull(const Vector3 &mn, const Vector3 &mx) { bool r = chance(25); L(4); LV(mn); LV(mx); L(r); return r; }
};

class AABTreeClass
{
public:
	int NodeCount;
	int PolyCount;
	MeshGeometryClass *Mesh;
#include "dab_class.inc"
	CullNodeStruct *Nodes;
	uint32_t *PolyIndices;

	void Generate_APT(const OBBoxClass &box, SimpleDynVecClass<uint32_t> &apt);
	void Generate_APT(const OBBoxClass &box, const Vector3 &viewdir, SimpleDynVecClass<uint32_t> &apt);
	void Generate_OBBox_APT_Recursive(CullNodeStruct *node, OBBoxAPTContextStruct &context);
	void Generate_OBBox_APT_Recursive(CullNodeStruct *node, OBBoxRayAPTContextStruct &context);
	bool Cast_Ray_Recursive(CullNodeStruct *node, RayCollisionTestClass &raytest);
	int Cast_Semi_Infinite_Axis_Aligned_Ray_Recursive(CullNodeStruct *node, const Vector3 &start_point,
		int axis_r, int axis_1, int axis_2, int direction, unsigned char &flags);
	bool Cast_AABox_Recursive(CullNodeStruct *node, AABoxCollisionTestClass &boxtest);
	bool Cast_OBBox_Recursive(CullNodeStruct *node, OBBoxCollisionTestClass &boxtest);
	bool Intersect_OBBox_Recursive(CullNodeStruct *node, OBBoxIntersectionTestClass &boxtest);
	bool Cast_Ray_To_Polys(CullNodeStruct *node, RayCollisionTestClass &raytest);
	int Cast_Semi_Infinite_Axis_Aligned_Ray_To_Polys(CullNodeStruct *node, const Vector3 &start_point,
		int axis_r, int axis_1, int axis_2, int direction, unsigned char &flags);
	bool Cast_AABox_To_Polys(CullNodeStruct *node, AABoxCollisionTestClass &boxtest);
	bool Cast_OBBox_To_Polys(CullNodeStruct *node, OBBoxCollisionTestClass &boxtest);
	bool Intersect_OBBox_With_Polys(CullNodeStruct *node, OBBoxIntersectionTestClass &boxtest);
};

#define AABTREE_LEAF_FLAG 0x80000000
#include "dab_inline.inc"
#include "dab_funcs.inc"

struct Reader {
	const uint32_t *p;
	int i;
	uint32_t u() { return p[i++]; }
	float f() { uint32_t v = p[i++]; float x; memcpy(&x, &v, 4); return x; }
};

}

extern "C" int ref_dab_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	g_log.clear();
	g_calls = 0;
	Reader rd = { in, 0 };
	uint32_t op = rd.u();
	uint64_t hi = rd.u();
	uint64_t lo = rd.u();
	g_seed = (hi << 32) | lo;
	MeshGeometryClass mesh;
	AABTreeClass tree;
	uint32_t nn = rd.u();
	std::vector<AABTreeClass::CullNodeStruct> nodes(nn);
	for (uint32_t k = 0; k < nn; ++k) {
		float a[6];
		for (int j = 0; j < 6; ++j) a[j] = rd.f();
		nodes[k].Min = Vector3(a[0], a[1], a[2]);
		nodes[k].Max = Vector3(a[3], a[4], a[5]);
		nodes[k].FrontOrPoly0 = rd.u();
		nodes[k].BackOrPolyCount = rd.u();
	}
	uint32_t npi = rd.u();
	std::vector<uint32_t> pis(npi);
	for (uint32_t k = 0; k < npi; ++k) pis[k] = rd.u();
	uint32_t nv = rd.u();
	for (uint32_t k = 0; k < nv; ++k) { float x = rd.f(), y = rd.f(), z = rd.f(); mesh.verts.push_back(Vector3(x, y, z)); }
	uint32_t np = rd.u();
	for (uint32_t k = 0; k < np; ++k) { uint32_t a = rd.u(), b = rd.u(), c = rd.u(); mesh.polys.push_back(TriIndex((unsigned short)a, (unsigned short)b, (unsigned short)c)); }
	uint32_t npl = rd.u();
	for (uint32_t k = 0; k < npl; ++k) { float x = rd.f(), y = rd.f(), z = rd.f(), w = rd.f(); mesh.planes.push_back(Vector4(x, y, z, w)); }
	tree.NodeCount = (int)nn;
	tree.PolyCount = (int)npi;
	tree.Mesh = &mesh;
	tree.Nodes = nodes.data();
	tree.PolyIndices = pis.data();

	std::vector<uint32_t> res;
	OBBoxClass obb;
	switch (op) {
	case 1: {
		SimpleDynVecClass<uint32_t> apt;
		tree.Generate_APT(obb, apt);
		res.push_back((uint32_t)apt.Count());
		for (int k = 0; k < apt.Count(); ++k) res.push_back(apt[k]);
		break;
	}
	case 2: {
		float x = rd.f(), y = rd.f(), z = rd.f();
		Vector3 view(x, y, z);
		SimpleDynVecClass<uint32_t> apt;
		tree.Generate_APT(obb, view, apt);
		res.push_back((uint32_t)apt.Count());
		for (int k = 0; k < apt.Count(); ++k) res.push_back(apt[k]);
		break;
	}
	case 3: {
		float x = rd.f(), y = rd.f(), z = rd.f();
		Vector3 start(x, y, z);
		int axis_dir = (int)rd.u();
		unsigned char flags = (unsigned char)rd.u();
		static const int axis_r[6] = { 0, 0, 1, 1, 2, 2 };
		static const int axis_1[6] = { 1, 1, 2, 2, 0, 0 };
		static const int axis_2[6] = { 2, 2, 0, 0, 1, 1 };
		static const int direction[6] = { 1, 0, 1, 0, 1, 0 };
		int c = tree.Cast_Semi_Infinite_Axis_Aligned_Ray_Recursive(&tree.Nodes[0], start, axis_r[axis_dir], axis_1[axis_dir], axis_2[axis_dir], direction[axis_dir], flags);
		res.push_back((uint32_t)c);
		res.push_back(flags);
		break;
	}
	case 4:
	case 5:
	case 6: {
		CastResultStruct r = { false, 0xDEADBEEFu };
		bool b;
		if (op == 4) {
			AABoxCollisionTestClass t;
			t.Result = &r;
			b = tree.Cast_AABox_Recursive(&tree.Nodes[0], t);
		} else if (op == 5) {
			OBBoxCollisionTestClass t;
			t.Result = &r;
			b = tree.Cast_OBBox_Recursive(&tree.Nodes[0], t);
		} else {
			OBBoxIntersectionTestClass t;
			b = tree.Intersect_OBBox_Recursive(&tree.Nodes[0], t);
		}
		res.push_back(b ? 1 : 0);
		if (op != 6) { res.push_back(r.StartBad ? 1 : 0); res.push_back(r.SurfaceType); }
		break;
	}
	default:
		return -1;
	}
	g_log.push_back(0xFFFFFFFEu);
	g_log.insert(g_log.end(), res.begin(), res.end());
	if ((int)g_log.size() > cap) return -1;
	for (size_t k = 0; k < g_log.size(); ++k) out[k] = g_log[k];
	return (int)g_log.size();
}
