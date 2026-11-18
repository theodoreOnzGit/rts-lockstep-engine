/* Differential-test wrapper for the drafts src/drafts/wwlib/{cullsys, aabtreecull, gridcull}.rs (#215), upstream
** commit 0a05454d:
**   Libraries/Source/WWVegas/WWMath/cullsys.h + cullsys.cpp, aabtreecull.h + aabtreecull.cpp, gridcull.h +
**   gridcull.cpp. cullsys.cpp is compiled unchanged; aabtreecull.cpp and gridcull.cpp are compiled with ONE mechanical
**   edit each (build.rs, drafts_wwmath): DEFINE_AUTO_POOL(T, 256); defines a member of a class template specialisation
**   without `template<>`, which MSVC accepts and clang does not, so it is spelled `template<> ObjectPoolClass<T, 256>
**   AutoPoolClass<T, 256>::Allocator{};` (the same edit multilist.cpp and hanim.cpp get). clang++ -fms-extensions, with
**   vector_this.h / simplevec_this.h (the `this->` edits) forced in as the W3D builds do. The collision functions the
**   culling calls (CollisionMath::Overlap_Test ...) are colmath*.cpp, compiled unchanged in the same library.
** Stand-in: C's rand() (the split-plane choice in aabtreecull.cpp) is `-Drand=draft_crt_rand`, the MSVC C runtime's
** generator (rand.c: `_holdrand = _holdrand * 214013 + 2531011; return (_holdrand >> 16) & 0x7fff`), seeded by each
** program; the Rust closure is the same formula.
**
** A PROGRAM (draft_cull_run) is a float stream: kind (0 = AABTreeCullSystemClass, 1 = GridCullSystemClass), then
** operations, each an opcode and arguments (see the table in tests/differential_draft_cull.rs). Objects are
** CullableClass instances numbered by the program. After every operation but NEWOBJ the structure is dumped (as 32-bit
** words: floats by their bits) and, after a Collect_Objects, the collected ids in collection order.
** The dump gives every object a small id (its index in the table). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "aabtreecull.h"
#include "gridcull.h"
#include "frustum.h"
#include "sphere.h"
#include "obbox.h"

static uint32_t g_holdrand = 1;
extern "C" int draft_crt_rand(void)
{
	g_holdrand = g_holdrand * 214013u + 2531011u;
	return (int)((g_holdrand >> 16) & 0x7fff);
}

static const int MAXOBJ = 128;
static CullableClass *g_objs[MAXOBJ];

static int IdOf(CullableClass *o)
{
	if (o == NULL) return -1;
	for (int i = 0; i < MAXOBJ; i++)
		if (g_objs[i] == o) return i;
	return -2;
}

struct Out {
	uint32_t *p;
	int n;
	void u(uint32_t v) { p[n++] = v; }
	void i(int v) { p[n++] = (uint32_t)v; }
	void f(float v) { uint32_t b; memcpy(&b, &v, 4); p[n++] = b; }
	void box(const AABoxClass &b) { f(b.Center.X); f(b.Center.Y); f(b.Center.Z); f(b.Extent.X); f(b.Extent.Y); f(b.Extent.Z); }
};

class TestTree : public AABTreeCullSystemClass {
public:
	void AddObj(CullableClass *o, int idx) { Add_Object_Internal(o, idx); }
	void RemoveObj(CullableClass *o) { Remove_Object_Internal(o); }
	int NodeCnt() { return NodeCount; }
	void CollectedDump(Out &o)
	{
		int n = 0;
		for (CullableClass *c = Peek_First_Collected_Object_Internal(); c != NULL; c = Peek_Next_Collected_Object_Internal(c)) n++;
		o.i(n);
		for (CullableClass *c = Peek_First_Collected_Object_Internal(); c != NULL; c = Peek_Next_Collected_Object_Internal(c)) o.i(IdOf(c));
	}
	void Dump(Out &o)
	{
		o.i(ObjectCount); o.i(NodeCount); o.i(Partition_Node_Count()); o.i(Partition_Tree_Depth());
		o.box(Get_Bounding_Box());
		DumpNode(o, RootNode);
		for (int i = 0; i < NodeCount; i++) {
			AABoxClass b;
			Get_Node_Bounds(i, &b);
			o.box(b);
		}
	}
	void DumpNode(Out &o, AABTreeNodeClass *n)
	{
		o.u(n->Index); o.box(n->Box);
		o.i(n->Parent ? (int)n->Parent->Index : -1);
		o.i(n->Front ? (int)n->Front->Index : -1);
		o.i(n->Back ? (int)n->Back->Index : -1);
		o.u(n->UserData);
		o.i(n->Object_Count());
		for (CullableClass *c = n->Object; c != NULL; c = ((AABTreeLinkClass *)c->Get_Cull_Link())->NextObject) o.i(IdOf(c));
		if (n->Front) DumpNode(o, n->Front);
		if (n->Back) DumpNode(o, n->Back);
	}
	void SetUserData(int ni, unsigned ud) { if (ni >= 0 && ni < NodeCount) IndexedNodes[ni]->UserData = ud; }
	void Walk(Out &o, AABTreeIterator &it)
	{
		AABoxClass b;
		it.Get_Current_Box(&b);
		o.i(it.Get_Current_Node_Index()); o.box(b);
		bool hf = it.Has_Front_Child(), hb = it.Has_Back_Child();
		o.i(hf); o.i(hb);
		if (it.Enter_Front_Child()) {
			o.i(1);
			Walk(o, it);
			bool sib = it.Enter_Sibling();   /* front -> back */
			o.i(sib);
			if (sib) { Walk(o, it); o.i(it.Enter_Sibling()); }   /* back -> front */
			o.i(it.Enter_Parent());
		} else {
			o.i(0);
			if (it.Enter_Back_Child()) { o.i(1); Walk(o, it); o.i(it.Enter_Parent()); } else o.i(0);
		}
	}
};

class TestGrid : public GridCullSystemClass {
public:
	void AddObj(CullableClass *o) { Add_Object_Internal(o); }
	void RemoveObj(CullableClass *o) { Remove_Object_Internal(o); }
	void CollectedDump(Out &o)
	{
		int n = 0;
		for (CullableClass *c = Peek_First_Collected_Object_Internal(); c != NULL; c = Peek_Next_Collected_Object_Internal(c)) n++;
		o.i(n);
		for (CullableClass *c = Peek_First_Collected_Object_Internal(); c != NULL; c = Peek_Next_Collected_Object_Internal(c)) o.i(IdOf(c));
	}
	void DumpList(Out &o, CullableClass *head)
	{
		int n = 0;
		for (CullableClass *c = head; c != NULL; c = ((GridLinkClass *)c->Get_Cull_Link())->Next) n++;
		o.i(n);
		for (CullableClass *c = head; c != NULL; c = ((GridLinkClass *)c->Get_Cull_Link())->Next) {
			GridLinkClass *l = (GridLinkClass *)c->Get_Cull_Link();
			o.i(IdOf(c)); o.i(l->GridAddress); o.i(IdOf(l->Prev)); o.i(IdOf(l->Next));
		}
	}
	void Dump(Out &o)
	{
		o.i(ObjCount); o.i(CellCount[0]); o.i(CellCount[1]); o.i(CellCount[2]);
		o.f(Origin.X); o.f(Origin.Y); o.f(Origin.Z);
		o.f(CellDim.X); o.f(CellDim.Y); o.f(CellDim.Z);
		o.f(OOCellDim.X); o.f(OOCellDim.Y); o.f(OOCellDim.Z);
		o.f(MaxObjExtent); o.i(TerminationCellCount);
		o.f(MinCellSize.X); o.f(MinCellSize.Y); o.f(MinCellSize.Z);
		o.i(Stats.NodeCount); o.i(Stats.NodesAccepted); o.i(Stats.NodesTriviallyAccepted); o.i(Stats.NodesRejected);
		int cells = CellCount[0] * CellCount[1] * CellCount[2];
		int nonempty = 0;
		for (int c = 0; c < cells; c++) if (Cells[c] != NULL) nonempty++;
		o.i(nonempty);
		for (int c = 0; c < cells; c++) {
			if (Cells[c] == NULL) continue;
			o.i(c);
			DumpList(o, Cells[c]);
		}
		DumpList(o, NoGridList);
	}
	/* the protected geometry helpers, for a direct comparison */
	void Helpers(Out &o, const float *p)
	{
		AABoxClass b;
		int ci = (int)p[0], cj = (int)p[1], ck = (int)p[2];
		if (ci >= 0 && cj >= 0 && ck >= 0 && ci < CellCount[0] && cj < CellCount[1] && ck < CellCount[2]) {
			compute_box(ci, cj, ck, &b);
			o.box(b);
		}
		VolumeStruct v;
		init_volume(Vector3(p[3], p[4], p[5]), Vector3(p[6], p[7], p[8]), &v);
		for (int a = 0; a < 3; a++) { o.i(v.Min[a]); o.i(v.Max[a]); }
		compute_box(v, &b);
		o.box(b);
		int addr;
		bool in = map_point_to_address(Vector3(p[9], p[10], p[11]), addr);
		o.i(in); o.i(addr);
	}
};

static CullableClass *Obj(int id) { return g_objs[id]; }

static AABoxClass BoxOf(const float *p)
{
	return AABoxClass(Vector3(p[0], p[1], p[2]), Vector3(p[3], p[4], p[5]));
}

/* returns the number of words written */
extern "C" int draft_cull_run(const float *prog, int n, uint32_t *out)
{
	Out o = { out, 0 };
	int kind = (int)prog[0];
	int i = 1;
	for (int k = 0; k < MAXOBJ; k++) g_objs[k] = NULL;
	TestTree *tree = NULL;
	TestGrid *grid = NULL;
	if (kind == 0) tree = new TestTree; else grid = new TestGrid;
	CullSystemClass *sys = kind == 0 ? (CullSystemClass *)tree : (CullSystemClass *)grid;
	while (i < n) {
		int op = (int)prog[i++];
		o.i(-1000 - op);   /* a marker per op */
		bool collected = false;
		switch (op) {
		case 1: {   /* NEWOBJ id c e */
			int id = (int)prog[i];
			CullableClass *c = new CullableClass;
			c->Set_Cull_Box(BoxOf(prog + i + 1), true);
			g_objs[id] = c;
			i += 7;
			continue;
		}
		case 2: {   /* ADD id node_index */
			int id = (int)prog[i];
			float spec = prog[i + 1];   /* tree: < 0 is -1 (drop into the tree), else a fraction of NodeCount */
			i += 2;
			if (tree) {
				int ni = -1;
				if (spec >= 0.0f) {
					int cnt = tree->NodeCnt();
					ni = (int)(spec * (float)cnt);
					if (ni >= cnt) ni = cnt - 1;
				}
				tree->AddObj(Obj(id), ni);
			} else grid->AddObj(Obj(id));
			break;
		}
		case 3: {   /* REMOVE id */
			int id = (int)prog[i]; i += 1;
			if (tree) tree->RemoveObj(Obj(id)); else grid->RemoveObj(Obj(id));
			break;
		}
		case 4: {   /* SETBOX id c e just_loaded */
			int id = (int)prog[i];
			Obj(id)->Set_Cull_Box(BoxOf(prog + i + 1), prog[i + 7] != 0.0f);
			i += 8;
			break;
		}
		case 5: case 6: case 7: case 8: case 9: {
			sys->Reset_Collection();
			if (op == 5) { Vector3 v(prog[i], prog[i + 1], prog[i + 2]); i += 3; sys->Collect_Objects(v); }
			else if (op == 6) { AABoxClass b = BoxOf(prog + i); i += 6; sys->Collect_Objects(b); }
			else if (op == 7) {
				const float *q = prog + i;
				Matrix3x3 bs(q[0], q[1], q[2], q[3], q[4], q[5], q[6], q[7], q[8]);
				OBBoxClass b(Vector3(q[9], q[10], q[11]), Vector3(q[12], q[13], q[14]), bs);
				i += 15;
				sys->Collect_Objects(b);
			} else if (op == 8) {
				const float *q = prog + i;
				Matrix3D cam(q[0], q[1], q[2], q[3], q[4], q[5], q[6], q[7], q[8], q[9], q[10], q[11]);
				FrustumClass f;
				f.Init(cam, Vector2(q[12], q[13]), Vector2(q[14], q[15]), q[16], q[17]);
				i += 18;
				sys->Collect_Objects(f);
			} else {
				SphereClass s(Vector3(prog[i], prog[i + 1], prog[i + 2]), prog[i + 3]);
				i += 4;
				tree->Collect_Objects(s);
			}
			collected = true;
			break;
		}
		case 10: g_holdrand = (uint32_t)prog[i]; i += 1; tree->Re_Partition(); break;
		case 11: {
			g_holdrand = (uint32_t)prog[i];
			AABoxClass bounds = BoxOf(prog + i + 1);
			int nb = (int)prog[i + 7];
			SimpleDynVecClass<AABoxClass> boxes;
			for (int b = 0; b < nb; b++) boxes.Add(BoxOf(prog + i + 8 + b * 6));
			i += 8 + nb * 6;
			tree->Re_Partition(bounds, boxes);
			break;
		}
		case 12: tree->Update_Bounding_Boxes(); break;
		case 13: {
			Vector3 mn(prog[i], prog[i + 1], prog[i + 2]), mx(prog[i + 3], prog[i + 4], prog[i + 5]);
			float od = prog[i + 6];
			i += 7;
			grid->Re_Partition(mn, mx, od);
			break;
		}
		case 14: grid->Set_Min_Cell_Size(Vector3(prog[i], prog[i + 1], prog[i + 2])); i += 3; break;
		case 15: grid->Set_Termination_Count((int)prog[i]); i += 1; break;
		case 16: {   /* UPDATE id */
			int id = (int)prog[i]; i += 1;
			sys->Update_Culling(Obj(id));
			break;
		}
		case 17: {   /* ITER */
			AABTreeIterator it(tree);
			tree->Walk(o, it);
			break;
		}
		case 18: {   /* SHUTDOWN: delete the system (it releases its objects), make a new one */
			if (tree) { delete tree; tree = new TestTree; sys = tree; }
			else { delete grid; grid = new TestGrid; sys = grid; }
			break;
		}
		case 19: break;   /* DUMP */
		case 20: {   /* GRID helpers */
			grid->Helpers(o, prog + i);
			i += 12;
			break;
		}
		case 21: {   /* tree: UserData of node index */
			int ni = (int)prog[i]; unsigned ud = (unsigned)prog[i + 1]; i += 2;
			tree->SetUserData(ni, ud);
			break;
		}
		}
		if (collected) { if (tree) tree->CollectedDump(o); else grid->CollectedDump(o); }
		if (tree) tree->Dump(o); else grid->Dump(o);
	}
	/* tear down: the system releases the objects it holds; then our own reference */
	if (tree) delete tree; else delete grid;
	for (int k = 0; k < MAXOBJ; k++) if (g_objs[k]) { g_objs[k]->Release_Ref(); g_objs[k] = NULL; }
	return o.n;
}
