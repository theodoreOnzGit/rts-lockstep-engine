/* Differential-test wrapper for MeshClass::Cast_Ray and what it reaches (GitHub #149, upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/mesh_cast_gen:
**   mc_tri.inc            Libraries/Source/WWVegas/WWMath/tri.cpp, from `struct FDPRec` to the end: find_dominant_plane,
**                         TriClass::Find_Dominant_Plane, TriClass::Contains_Point;
**   mc_lineseg_set.inc    WWMath/lineseg.cpp, LineSegClass::Set(that, tm) (to Set_Random's banner);
**   mc_overlap_line.inc   WWMath/colmathaabox.cpp, Overlap_Test(AABoxClass, LineSegClass), the compiled one (to the
**                         `#if 0` of the alternate), from after its banner's `*` `/`;
**   mc_collide_tri.inc    WWMath/colmathline.cpp, Collide(LineSegClass, TriClass) (to the sphere collide);
**   mc_leaf_flag.inc      WW3D2/aabtree.h, `#define AABTREE_LEAF_FLAG` (to the AABTreeClass comment);
**   mc_node.inc           aabtree.h, `struct CullNodeStruct` (to OBBoxAPTContextStruct's comment), inside the stand-in tree;
**   mc_tree_cast.inc      aabtree.h, the inline AABTreeClass::Cast_Ray (to Cast_Semi_Infinite_Axis_Aligned_Ray);
**   mc_node_impl.inc      aabtree.h, CullNodeStruct's Is_Leaf, Get_Front_Child, Get_Back_Child, Get_Poly0 and
**                         Get_Poly_Count (to Set_Front_Child);
**   mc_tree_recursive.inc WW3D2/aabtree.cpp, Cast_Ray_Recursive (to the next banner);
**   mc_tree_polys.inc     aabtree.cpp, Cast_Ray_To_Polys (to the `#ifdef _DEBUG` after it);
**   mc_geom_defs.inc      WW3D2/meshgeometry.h, `typedef Vector3i16 TriIndex;` to the MeshGeometryClass comment (with
**                         OPTIMIZE_PLANEEQ_RAM 1 and OPTIMIZE_VNORM_RAM 0);
**   mc_geom_flags.inc     meshgeometry.h, `enum FlagsType` (to Reset_Geometry), inside the stand-in geometry;
**   mc_geom_surface.inc   meshgeometry.h, the inline Get_Poly_Surface_Type (to the include guard's #endif);
**   mc_geom_planeeq.inc   WW3D2/meshgeometry.cpp, the static _PlaneEQArray (to OPTIMIZE_VNORM_RAM's);
**   mc_geom_cast.inc      meshgeometry.cpp, MeshGeometryClass::Cast_Ray (to Cast_AABox's banner);
**   mc_geom_brute.inc     meshgeometry.cpp, cast_ray_brute_force (to cast_aabox_brute_force's banner);
**   mc_geom_peq.inc       meshgeometry.cpp, Compute_Plane_Equations (to Compute_Vertex_Normals' banner);
**   mc_geom_planes.inc    meshgeometry.cpp, get_planes and Get_Plane_Array (to Compute_Plane's banner);
**   mc_mesh_cast.inc      WW3D2/mesh.cpp, MeshClass::Cast_Ray (to Cast_AABox's banner);
**   mc_mesh_coll.inc      mesh.cpp, MeshClass::Init's two collision-type lines (to Set_Hidden), as the body of the
**                         stand-in's `initCollisionType`;
**   mc_coll_type.inc      WW3D2/rendobj.h, Get_Collision_Type and Set_Collision_Type (to Is_Complete), inside the
**                         stand-in render object.
** No edit. The real WWMath headers (lineseg.h, castres.h, aabox.h, obbox.h, tri.h, colmath.h, colmathinlines.h,
** vector4.h, Vector3i.h) and WW3D2's coltest.h and coltype.h; Matrix3D's Obj_Look_At and
** Get_Inverse are upstream_wwmath's (matrix3d.cpp, D3DXMatrixInverse the shim's stand-in). TriClass, LineSegClass,
** CollisionMath, the collision test classes, RenderObjClass, MeshClass, MeshModelClass, MeshGeometryClass and AABTreeClass
** are renamed by #define (Mc...) so that no symbol meets another archive's. The stand-ins: the render object (its bits,
** hidden, alpha, animation hidden), the mesh (its transform, its model), the geometry (its arrays as share buffers, its
** flags, its tree), the tree (its nodes, poly indices and mesh) and WWLib's SimpleVecClass as far as _PlaneEQArray uses
** it (simplevec.h itself does not compile here: SimpleDynVecClass needs the this-> edits of build.rs's SIMPLEVEC_EDITS).
**
** Entry point (extern "C", tests/differential_mesh_cast.rs):
**   ref_mesh_cast_seq(in, nin, out, cap): `in` is the mesh [vertex count, poly count, node count, poly index count, the
**   geometry's flags, W3dAttributes, hidden, alpha, animation hidden], its vertices (3 bits each), polygons (3 indices
**   each), surface types (one each), nodes (8 words each: Min and Max bits, FrontOrPoly0, BackOrPolyCount) and poly
**   indices (no node: no CullTree); then steps of 32 words [kind, ...]:
**     kind 0  MeshClass::Cast_Ray: [the transform's 12 bits (row-major), ray start, end bits, result Fraction bits,
**             StartBad, ComputeContactPoint, the test's CollisionType, CheckTranslucent, CheckHidden, the tree (0: the
**             model's CullTree NULL)] -> [0, return, StartBad, Fraction bits, Normal bits, ContactPoint bits,
**             SurfaceType, CollidedRenderObj is the mesh]
**     kind 1  MeshGeometryClass::Cast_Ray (the model, in its frame): [ray start, end bits, Fraction bits, StartBad,
**             ComputeContactPoint, the tree] -> [1, return, StartBad, Fraction bits, Normal bits, ContactPoint bits,
**             SurfaceType]
**     kind 2  Collide(LineSegClass, TriClass): [ray start, end bits, the vertices' 9 bits, 0 Compute_Normal | 1 the
**             normal given, the normal's 3 bits, Fraction bits, ComputeContactPoint] -> [2, return, Fraction bits,
**             Normal bits, ContactPoint bits]
**     kind 3  TriClass: [the vertices' 9 bits, the normal's 3 bits, the point's 3 bits] -> [3, Contains_Point,
**             Find_Dominant_Plane's two axes]
**     kind 4  Overlap_Test(min, max, LineSegClass): [min, max, ray start, end bits] -> [4, the OverlapType]
**     kind 5  LineSegClass(that, tm): [the transform's 12 bits, ray start, end bits] -> [5, P0, P1, DP, Dir bits,
**             Length bits]
**     kind 6  MeshClass::Init's collision type: [the bits before, W3dAttributes] -> [6, Get_Collision_Type] */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <vector>
#include "Lib/BaseType.h"
#include "wwmath.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3.h"
#include "matrix3d.h"
#include "Vector3i.h"
#define TriClass McTriClass
#define LineSegClass McLineSegClass
#define CollisionMath McCollisionMath
#define CollisionTestClass McCollisionTestClass
#define RayCollisionTestClass McRayCollisionTestClass
#define AABoxCollisionTestClass McAABoxCollisionTestClass
#define OBBoxCollisionTestClass McOBBoxCollisionTestClass
#define OBBoxIntersectionTestClass McOBBoxIntersectionTestClass
#define RenderObjClass McRenderObjClass
#define MeshClass McMeshClass
#define MeshModelClass McMeshModelClass
#define MeshGeometryClass McMeshGeometryClass
#define AABTreeClass McAABTreeClass
#include "lineseg.h"
#include "castres.h"
#include "aabox.h"
#include "obbox.h"
#include "tri.h"
#include "colmath.h"
#include "colmathinlines.h"
#include "coltype.h"
#include "coltest.h"

/* ---- WWMath: the triangle, the segment's transform, the box-line overlap, the triangle collide ---- */
#include "mc_tri.inc"
#include "mc_lineseg_set.inc"
#include "mc_overlap_line.inc"
#include "mc_collide_tri.inc"

#include "mc_leaf_flag.inc"
#include "mc_geom_defs.inc"

/* ---- the stand-ins ---- */
/* WWLib's SimpleVecClass, as far as meshgeometry.cpp's _PlaneEQArray uses it (its constructor and Uninitialised_Grow,
** which grows without copying; the planes are recomputed on every Get_Plane_Array) */
template <class T> class SimpleVecClass
{
public:
	std::vector<T> v;
	SimpleVecClass( int size ) : v((size_t)size) {}
	bool Uninitialised_Grow( int newsize ) { if ((size_t)newsize > v.size()) { v.resize((size_t)newsize); return true; } return false; }
	T & operator [] ( int i ) { return v[(size_t)i]; }
};
template <class T> class McShareBuffer
{
public:
	std::vector<T> v;
	T *Get_Array( void ) { return v.empty() ? NULL : &v[0]; }
};

class McMeshGeometryClass;

class McAABTreeClass
{
public:
#include "mc_node.inc"
	CullNodeStruct *Nodes;
	uint32 *PolyIndices;
	McMeshGeometryClass *Mesh;
	bool Cast_Ray( RayCollisionTestClass & raytest );
	bool Cast_Ray_Recursive( CullNodeStruct * node, RayCollisionTestClass & raytest );
	bool Cast_Ray_To_Polys( CullNodeStruct * node, RayCollisionTestClass & raytest );
};

class McMeshGeometryClass
{
public:
#include "mc_geom_flags.inc"
	int Flags;
	uint32 W3dAttributes;
	int PolyCount;
	int VertexCount;
	McShareBuffer<TriIndex> *Poly;
	McShareBuffer<Vector3> *Vertex;
	McShareBuffer<uint8> *PolySurfaceType;
	McAABTreeClass *CullTree;
	int Get_Flag( FlagsType flag ) { return (Flags & flag) == flag; }
	void Set_Flag( FlagsType flag, bool onoff ) { if (onoff) { Flags |= flag; } else { Flags &= ~flag; } }
	int Get_Polygon_Count( void ) { return PolyCount; }
	const Vector3 *Get_Vertex_Array( void ) { return Vertex->Get_Array(); }
	const TriIndex *Get_Polygon_Array( void ) { return Poly->Get_Array(); }
	uint8 Get_Poly_Surface_Type( int poly_index ) const;
	Vector4 *get_planes( bool create );
	const Vector4 *Get_Plane_Array( bool create = true );
	void Compute_Plane_Equations( Vector4 * array );
	bool Cast_Ray( RayCollisionTestClass & raytest );
	bool cast_ray_brute_force( RayCollisionTestClass & raytest );
};
class McMeshModelClass : public McMeshGeometryClass {};

#include "mc_tree_cast.inc"
#include "mc_node_impl.inc"
#include "mc_tree_recursive.inc"
#include "mc_tree_polys.inc"
#include "mc_geom_surface.inc"
#include "mc_geom_planeeq.inc"
#include "mc_geom_cast.inc"
#include "mc_geom_brute.inc"
#include "mc_geom_peq.inc"
#include "mc_geom_planes.inc"

class McRenderObjClass
{
public:
	int Bits;
	int Hidden;
	int Alpha;
	int AnimHidden;
	Matrix3D Transform;
	McRenderObjClass() : Bits(COLL_TYPE_ALL), Hidden(0), Alpha(0), AnimHidden(0) {}
	virtual ~McRenderObjClass() {}
	enum { COLL_TYPE_MASK = 0x000000FF };
#include "mc_coll_type.inc"
	int Is_Hidden( void ) const { return Hidden; }
	int Is_Alpha( void ) const { return Alpha; }
	int Is_Animation_Hidden( void ) const { return AnimHidden; }
	const Matrix3D & Get_Transform( void ) const { return Transform; }
};

/* W3D_MESH_FLAG_COLLISION_TYPE_MASK, _SHIFT */
#include "w3d_file.h"

class McMeshClass : public McRenderObjClass
{
public:
	McMeshModelClass *Model;
	bool Cast_Ray( RayCollisionTestClass & raytest );
	void initCollisionType( void )
	{
#include "mc_mesh_coll.inc"
	}
};
#include "mc_mesh_cast.inc"

namespace mc {

std::vector<int> g_out;
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
static Real realOf(Int i) { Real r; memcpy(&r, &i, sizeof r); return r; }
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }
static Vector3 vec(const int *w) { return Vector3(realOf(w[0]), realOf(w[1]), realOf(w[2])); }
static void recVec(const Vector3 &v) { rec({bitsOf(v.X), bitsOf(v.Y), bitsOf(v.Z)}); }
static Matrix3D mat(const int *w)
{
	return Matrix3D(realOf(w[0]), realOf(w[1]), realOf(w[2]), realOf(w[3]), realOf(w[4]), realOf(w[5]), realOf(w[6]), realOf(w[7]),
		realOf(w[8]), realOf(w[9]), realOf(w[10]), realOf(w[11]));
}
static void recResult(const CastResultStruct &r)
{
	rec({r.StartBad ? 1 : 0, bitsOf(r.Fraction)});
	recVec(r.Normal);
	recVec(r.ContactPoint);
	rec({(Int)r.SurfaceType});
}

} /* namespace mc */

extern "C" int ref_mesh_cast_seq(const int *in, int nin, int *out, int cap)
{
	using namespace mc;
	g_out.clear();
	int k = 0;
	const int nv = in[0], np = in[1], nn = in[2], ni = in[3];
	McShareBuffer<Vector3> verts;
	McShareBuffer<TriIndex> polys;
	McShareBuffer<uint8> surf;
	std::vector<McAABTreeClass::CullNodeStruct> nodes;
	std::vector<uint32> pidx;
	McMeshModelClass model;
	model.Flags = in[4];
	McMeshClass mesh;
	mesh.Bits = COLL_TYPE_ALL;
	mesh.Hidden = in[6];
	mesh.Alpha = in[7];
	mesh.AnimHidden = in[8];
	mesh.Model = &model;
	model.W3dAttributes = (uint32)in[5];
	k = 9;
	for (int i = 0; i < nv; ++i, k += 3) verts.v.push_back(vec(in + k));
	for (int i = 0; i < np; ++i, k += 3) polys.v.push_back(TriIndex((unsigned short)in[k], (unsigned short)in[k + 1], (unsigned short)in[k + 2]));
	for (int i = 0; i < np; ++i, ++k) surf.v.push_back((uint8)in[k]);
	for (int i = 0; i < nn; ++i, k += 8) {
		McAABTreeClass::CullNodeStruct n;
		n.Min = vec(in + k);
		n.Max = vec(in + k + 3);
		n.FrontOrPoly0 = (uint32)in[k + 6];
		n.BackOrPolyCount = (uint32)in[k + 7];
		nodes.push_back(n);
	}
	for (int i = 0; i < ni; ++i, ++k) pidx.push_back((uint32)in[k]);
	model.PolyCount = np;
	model.VertexCount = nv;
	model.Poly = &polys;
	model.Vertex = &verts;
	model.PolySurfaceType = &surf;
	McAABTreeClass tree;
	tree.Nodes = nodes.empty() ? NULL : &nodes[0];
	tree.PolyIndices = pidx.empty() ? NULL : &pidx[0];
	tree.Mesh = &model;
	for (; k + 32 <= nin; k += 32) {
		const int *w = in + k + 1;
		switch (in[k]) {
		case 0: {
			mesh.Transform = mat(w);
			model.CullTree = (w[24] && !nodes.empty()) ? &tree : NULL;
			LineSegClass ray(vec(w + 12), vec(w + 15));
			CastResultStruct result;
			result.Fraction = realOf(w[18]);
			result.StartBad = w[19] != 0;
			result.ComputeContactPoint = w[20] != 0;
			RayCollisionTestClass raytest(ray, &result, w[21], w[22] != 0, w[23] != 0);
			bool r = mesh.Cast_Ray(raytest);
			rec({0, r ? 1 : 0});
			recResult(result);
			rec({raytest.CollidedRenderObj == &mesh ? 1 : 0});
			break;
		}
		case 1: {
			model.CullTree = (w[9] && !nodes.empty()) ? &tree : NULL;
			LineSegClass ray(vec(w), vec(w + 3));
			CastResultStruct result;
			result.Fraction = realOf(w[6]);
			result.StartBad = w[7] != 0;
			result.ComputeContactPoint = w[8] != 0;
			RayCollisionTestClass raytest(ray, &result, COLL_TYPE_ALL, false, false);
			bool r = model.Cast_Ray(raytest);
			rec({1, r ? 1 : 0});
			recResult(result);
			break;
		}
		case 2: {
			LineSegClass ray(vec(w), vec(w + 3));
			Vector3 v[3] = { vec(w + 6), vec(w + 9), vec(w + 12) };
			Vector3 n = vec(w + 16);
			TriClass tri;
			tri.V[0] = &v[0];
			tri.V[1] = &v[1];
			tri.V[2] = &v[2];
			tri.N = &n;
			if (w[15] == 0) tri.Compute_Normal();
			CastResultStruct result;
			result.Fraction = realOf(w[19]);
			result.ComputeContactPoint = w[20] != 0;
			bool r = CollisionMath::Collide(ray, tri, &result);
			rec({2, r ? 1 : 0, bitsOf(result.Fraction)});
			recVec(result.Normal);
			recVec(result.ContactPoint);
			break;
		}
		case 3: {
			Vector3 v[3] = { vec(w), vec(w + 3), vec(w + 6) };
			Vector3 n = vec(w + 9);
			TriClass tri;
			tri.V[0] = &v[0];
			tri.V[1] = &v[1];
			tri.V[2] = &v[2];
			tri.N = &n;
			int a1 = -1, a2 = -1;
			tri.Find_Dominant_Plane(&a1, &a2);
			rec({3, tri.Contains_Point(vec(w + 12)) ? 1 : 0, a1, a2});
			break;
		}
		case 4: {
			LineSegClass ray(vec(w + 6), vec(w + 9));
			rec({4, (Int)CollisionMath::Overlap_Test(vec(w), vec(w + 3), ray)});
			break;
		}
		case 5: {
			LineSegClass ray(vec(w + 12), vec(w + 15));
			LineSegClass t(ray, mat(w));
			rec({5});
			recVec(t.Get_P0());
			recVec(t.Get_P1());
			recVec(t.Get_DP());
			recVec(t.Get_Dir());
			rec({bitsOf(t.Get_Length())});
			break;
		}
		case 6: {
			McMeshClass m6;
			m6.Bits = w[0];
			McMeshModelClass mm;
			m6.Model = &mm;
			mm.W3dAttributes = (uint32)w[1];
			m6.initCollisionType();
			rec({6, m6.Get_Collision_Type()});
			break;
		}
		}
	}
	int nn2 = (int)g_out.size();
	if (nn2 > cap)
		nn2 = cap;
	if (nn2 > 0)
		memcpy(out, &g_out[0], nn2 * sizeof(int));
	return nn2;
}
