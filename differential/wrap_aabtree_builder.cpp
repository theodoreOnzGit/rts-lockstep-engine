/* Differential-test wrapper for AABTreeBuilderClass and the AABTree a collidable mesh without one gets (GitHub #190,
** upstream commit 0a05454d), cut verbatim by build.rs into OUT_DIR/aabtree_builder_gen:
**   ab_leaf_flag.inc      WW3D2/aabtree.h, `#define AABTREE_LEAF_FLAG` (to the AABTreeClass comment);
**   ab_geom_defs.inc      WW3D2/meshgeometry.h, `typedef Vector3i16 TriIndex;` to the MeshGeometryClass comment;
**   ab_builder_class.inc  WW3D2/aabtreebuilder.h, `class AABTreeBuilderClass` to the include guard's #endif;
**   ab_node.inc           WW3D2/aabtree.h, `struct CullNodeStruct` (to OBBoxAPTContextStruct's comment), inside the
**                         stand-in tree;
**   ab_node_set.inc       aabtree.h, CullNodeStruct's Set_Front_Child, Set_Back_Child, Set_Poly0 and Set_Poly_Count
**                         (to the end of the file, its closing #endif left out);
**   ab_builder_impl.inc   WW3D2/aabtreebuilder.cpp, from `#undef WWASSERT` (so COINCIDENCE_EPSILON, the constructor,
**                         Reset, both Build_AABTree, Build_Tree, Select_Splitting_Plane, Compute_Plane_Score,
**                         Which_Side, Split_Polys, Compute_Bounding_Box, Assign_Index, Node_Count, Poly_Count,
**                         Node_Count_Recursive, Update_Min, Update_Max, Update_Min_Max) to Export's banner;
**   ab_builder_w3d.inc    aabtreebuilder.cpp, Build_W3D_AABTree_Recursive (to the end of the file): the tools' export;
**   ab_tree_ctor.inc      WW3D2/aabtree.cpp, AABTreeClass::AABTreeClass(AABTreeBuilderClass *) (to the copy
**                         constructor's banner);
**   ab_tree_build.inc     aabtree.cpp, Build_Tree_Recursive (to Set_Mesh's banner);
**   ab_generate.inc       WW3D2/meshgeometry.cpp, MeshGeometryClass::Generate_Culling_Tree (to Load_W3D's banner);
**   ab_load_tail.inc      WW3D2/meshmdlio.cpp, MeshModelClass::Load_W3D's "If this mesh is collideable and no AABTree
**                         was in the file, generate one now" (to "Transfer the materials"), as the body of the
**                         stand-in's `loadTail`.
** No edit. The real headers: WWMath's vector3.h, Vector3i.h and aaplane.h, w3d_file.h; the shim's wwdebug.h (WWASSERT
** compiled out, as aabtreebuilder.cpp's own `assert` is under NDEBUG: the shipped game's release build). AABTreeBuilderClass,
** AABTreeClass, MeshGeometryClass and MeshModelClass are renamed by #define (Ab...) so that no symbol meets another
** archive's. W3DNEW and W3DNEWARRAY are always.h's release definitions (`new`); WWMEMLOG is empty and NEW_REF is
** refcount.h's `new`, without the reference count.
**
** `rand()`: the cuts' calls go to `ab_crt_rand`, a stand-in for the MSVC C runtime's rand() (the game's compiler;
** rand.c: `_holdrand = _holdrand * 214013 + 2531011; return (_holdrand >> 16) & 0x7fff;`), whose state each case
** sets. glibc's rand() is another generator; the Rust is checked against this stand-in, not against MSVC's binary.
**
** Entry point (extern "C", tests/differential_aabtree_builder.rs):
**   ref_aabtree_builder(in, nin, out, cap): `in` is [vertex count, poly count, W3dAttributes, the file had an AABTree,
**   the rand() seed], the vertices (3 bits each) and the polygons (3 indices each). Out:
**   [loadTail generated a tree, the rand() state after loadTail,
**    NodeCount, PolyCount, the nodes (8 words each: Min, Max bits, FrontOrPoly0, BackOrPolyCount), the PolyIndices,
**    the rand() state after Generate_Culling_Tree,
**    Node_Count(), Poly_Count(), Build_W3D_AABTree_Recursive's nodes (8 words each) and poly indices]
**   loadTail runs from the seed on a stand-in whose CullTree is the file's (a dummy) or NULL; Generate_Culling_Tree and
**   the export's Build_AABTree each run again from the seed. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <vector>
#include "Lib/BaseType.h"
#include "wwmath.h"
#include "vector3.h"
#include "Vector3i.h"
#include "aaplane.h"
#include "w3d_file.h"
#include "wwdebug.h"

#define AABTreeBuilderClass AbAABTreeBuilderClass
#define AABTreeClass AbAABTreeClass
#define MeshGeometryClass AbMeshGeometryClass
#define MeshModelClass AbMeshModelClass
#ifndef W3DNEW
#define W3DNEW new
#endif
#ifndef W3DNEWARRAY
#define W3DNEWARRAY new
#endif
#define WWMEMLOG(x)
#define NEW_REF(C, P) (new C P)

static unsigned int g_holdrand = 1;
static int ab_crt_rand(void)
{
	g_holdrand = g_holdrand * 214013u + 2531011u;
	return (int)((g_holdrand >> 16) & 0x7fff);
}
#define rand() ab_crt_rand()

class ChunkSaveClass;

#include "ab_leaf_flag.inc"
#include "ab_geom_defs.inc"
#include "ab_builder_class.inc"

class AbMeshGeometryClass;

class AbAABTreeClass
{
public:
#include "ab_node.inc"
	int NodeCount;
	CullNodeStruct *Nodes;
	int PolyCount;
	uint32 *PolyIndices;
	AbMeshGeometryClass *Mesh;
	AbAABTreeClass( void ) : NodeCount(0), Nodes(NULL), PolyCount(0), PolyIndices(NULL), Mesh(NULL) {}
	AbAABTreeClass( AABTreeBuilderClass * builder );
	~AbAABTreeClass( void ) { delete[] Nodes; delete[] PolyIndices; }
	void Build_Tree_Recursive( AABTreeBuilderClass::CullNodeStruct * node, int & curpolyindex );
	void Set_Mesh( AbMeshGeometryClass * mesh ) { Mesh = mesh; }
	/* the export (friend access): AABTreeBuilderClass::Export's arrays without the chunk writer */
	static void exportW3d( AABTreeBuilderClass & b, std::vector<W3dMeshAABTreeNode> & nodes, std::vector<uint32> & polys )
	{
		nodes.resize((size_t)b.Node_Count());
		polys.resize((size_t)b.Poly_Count());
		int cur_node = 0;
		int cur_poly = 0;
		b.Build_W3D_AABTree_Recursive(b.Root, nodes.empty() ? NULL : &nodes[0], polys.empty() ? NULL : &polys[0], cur_node, cur_poly);
	}
};

#include "ab_node_set.inc"
#include "ab_builder_impl.inc"
#include "ab_builder_w3d.inc"
#include "ab_tree_ctor.inc"
#include "ab_tree_build.inc"

template <class T> class AbShareBuffer
{
public:
	std::vector<T> v;
	T *Get_Array( void ) { return v.empty() ? NULL : &v[0]; }
};

class AbMeshGeometryClass
{
public:
	int PolyCount;
	int VertexCount;
	AbShareBuffer<TriIndex> *Poly;
	AbShareBuffer<Vector3> *Vertex;
	AbAABTreeClass *CullTree;
	uint32 W3dAttributes;
	void Generate_Culling_Tree( void );
};
class AbMeshModelClass : public AbMeshGeometryClass
{
public:
	void loadTail( void )
	{
#include "ab_load_tail.inc"
	}
};

#include "ab_generate.inc"

namespace ab {

std::vector<int> g_out;
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
static Real realOf(Int i) { Real r; memcpy(&r, &i, sizeof r); return r; }
static void rec(Int w) { g_out.push_back(w); }

} /* namespace ab */

extern "C" int ref_aabtree_builder(const int *in, int nin, int *out, int cap)
{
	using namespace ab;
	g_out.clear();
	const int nv = in[0], np = in[1];
	const unsigned int seed = (unsigned int)in[4];
	AbShareBuffer<Vector3> verts;
	AbShareBuffer<TriIndex> polys;
	int k = 5;
	for (int i = 0; i < nv && k + 3 <= nin; ++i, k += 3) verts.v.push_back(Vector3(realOf(in[k]), realOf(in[k + 1]), realOf(in[k + 2])));
	for (int i = 0; i < np && k + 3 <= nin; ++i, k += 3) polys.v.push_back(TriIndex((unsigned short)in[k], (unsigned short)in[k + 1], (unsigned short)in[k + 2]));

	/* MeshModelClass::Load_W3D's test */
	AbMeshModelClass model;
	model.PolyCount = np;
	model.VertexCount = nv;
	model.Poly = &polys;
	model.Vertex = &verts;
	model.W3dAttributes = (uint32)in[2];
	AbAABTreeClass file_tree;
	model.CullTree = in[3] ? &file_tree : NULL;
	g_holdrand = seed;
	model.loadTail();
	const bool generated = model.CullTree != NULL && model.CullTree != &file_tree;
	rec(generated ? 1 : 0);
	rec((Int)g_holdrand);
	if (generated) delete model.CullTree;

	/* Generate_Culling_Tree */
	AbMeshGeometryClass geom;
	geom.PolyCount = np;
	geom.VertexCount = nv;
	geom.Poly = &polys;
	geom.Vertex = &verts;
	geom.CullTree = NULL;
	g_holdrand = seed;
	geom.Generate_Culling_Tree();
	AbAABTreeClass *t = geom.CullTree;
	rec(t->NodeCount);
	rec(t->PolyCount);
	for (int i = 0; i < t->NodeCount; ++i) {
		const AbAABTreeClass::CullNodeStruct &n = t->Nodes[i];
		rec(bitsOf(n.Min.X)); rec(bitsOf(n.Min.Y)); rec(bitsOf(n.Min.Z));
		rec(bitsOf(n.Max.X)); rec(bitsOf(n.Max.Y)); rec(bitsOf(n.Max.Z));
		rec((Int)n.FrontOrPoly0);
		rec((Int)n.BackOrPolyCount);
	}
	for (int i = 0; i < t->PolyCount; ++i) rec((Int)t->PolyIndices[i]);
	rec((Int)g_holdrand);
	delete t;

	/* the tools' export: Build_AABTree then Build_W3D_AABTree_Recursive */
	{
		AABTreeBuilderClass b;
		g_holdrand = seed;
		b.Build_AABTree(np, polys.Get_Array(), nv, verts.Get_Array());
		std::vector<W3dMeshAABTreeNode> wn;
		std::vector<uint32> wp;
		AbAABTreeClass::exportW3d(b, wn, wp);
		rec((Int)wn.size());
		rec((Int)wp.size());
		for (size_t i = 0; i < wn.size(); ++i) {
			rec(bitsOf(wn[i].Min.X)); rec(bitsOf(wn[i].Min.Y)); rec(bitsOf(wn[i].Min.Z));
			rec(bitsOf(wn[i].Max.X)); rec(bitsOf(wn[i].Max.Y)); rec(bitsOf(wn[i].Max.Z));
			rec((Int)wn[i].FrontOrPoly0);
			rec((Int)wn[i].BackOrPolyCount);
		}
		for (size_t i = 0; i < wp.size(); ++i) rec((Int)wp[i]);
	}

	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}
