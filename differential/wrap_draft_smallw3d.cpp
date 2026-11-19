/* Differential-test wrapper for the small WW3D2 drafts of group sweepww3d (GitHub #215; upstream commit 0a05454d):
**   src/drafts/sweep_ww3d/layer.rs, scene.rs, vertmaterial.rs, w3d_util.rs.
** VERBATIM cuts by build.rs (`drafts_sweepww3d`):
**   layer.cpp         LayerClass::Set_Scene, Get_Scene, Peek_Camera
**   scene.cpp         SimpleSceneClass::Remove_All_Render_Objects, Create_Iterator, Destroy_Iterator;
**                     SimpleSceneIterator::SimpleSceneIterator, First, Next, Is_Done, Current_Item
**   vertmaterial.cpp  VertexMaterialClass::Set_Emissive (both), Get_UV_Source, Apply_Null
**   w3d_util.cpp      W3dUtilityClass::Convert_Quaternion (both overloads)
** Stand-ins (what the cuts call outside themselves):
**   SceneClass / RenderObjClass: Add_Ref / Release_Ref / Notify, logging "event, handle" (handles are small integers).
**   SceneClass::Remove_Render_Object (the qualified base call of Remove_All_Render_Objects): logs 20 + handle.
**   RefRenderObjListClass / RefRenderObjListIterator: a circular doubly linked list with a Head node whose Object is NULL, the
**     shape of WWLib/multilist.h (Is_Done is CurNode == &Head, Next is CurNode = CurNode->Next, which WRAPS from the head to
**     the first object; Remove_Head returns the first object or NULL).
**   SceneIterator: the virtual base. SimpleSceneClass: the RenderList and the two members cut; the constructor of the iterator
**     is public here (friend in scene.cpp).
**   DX8Wrapper: Set_DX8_Render_State / Set_DX8_Material / Set_DX8_Texture_Stage_State log their arguments; the D3D constants
**     are d3d8types.h's values, read from Wine's include/d3d8types.h on 2026-11-19 (D3DRS_LIGHTING 137, D3DRS_DIFFUSEMATERIALSOURCE
**     145, D3DRS_AMBIENTMATERIALSOURCE 147, D3DRS_EMISSIVEMATERIALSOURCE 148, D3DTSS_TEXCOORDINDEX 11,
**     D3DTSS_TEXTURETRANSFORMFLAGS 24, D3DTSS_TCI_PASSTHRU 0, D3DTTFF_DISABLE 0, D3DMCS_MATERIAL 0).
**   VertexMaterialClass: Material (D3DMATERIAL8 with Emissive), CRCDirty, UVSource[8] (MeshBuilderClass::MAX_STAGES = 8).
**   Quaternion: WWMath/quat.h unchanged. W3dQuaternionStruct: w3d_file.h's IOQuaternionStruct { float Q[4]; }.
** Entry points (extern "C", tests/differential_draft_smallw3d.rs): ref_dsww3d_layer_run, ref_dsww3d_scene_run,
** ref_dsww3d_vmat_run, ref_dsww3d_wutil_run; the ops are listed at each. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector3.h"
#include "quat.h"

#ifndef FALSE
#define FALSE 0
#endif

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static std::vector<uint32_t> g_log;

class RenderObjClass {
public:
	int id;
	void Release_Ref(void) { g_log.push_back(3); g_log.push_back((uint32_t)id); }
};

/* layer and scene share the stand-in SceneClass: Add_Ref / Release_Ref log, and the static base Remove_Render_Object */
struct SceneClass {
	int id;
	void Add_Ref(void) const { g_log.push_back(1); g_log.push_back((uint32_t)id); }
	void Release_Ref(void) const { g_log.push_back(2); g_log.push_back((uint32_t)id); }
	static void Remove_Render_Object(RenderObjClass * obj) { g_log.push_back(20); g_log.push_back((uint32_t)obj->id); }
};
struct CameraClass { int id; };

class LayerClass {
public:
	void Set_Scene(SceneClass * scene);
	SceneClass * Get_Scene(void) const;
	CameraClass * Peek_Camera(void) const;
	SceneClass * Scene;
	CameraClass * Camera;
};

#include "ly_set.inc"
#include "ly_get.inc"
#include "ly_cam.inc"

struct ListNode { ListNode * Next; ListNode * Prev; RenderObjClass * Object; };

class RefRenderObjListClass {
public:
	RefRenderObjListClass(void) { Head.Next = &Head; Head.Prev = &Head; Head.Object = NULL; }
	~RefRenderObjListClass(void) { while (Remove_Head() != NULL) { } }
	void Add_Tail(RenderObjClass * obj) {
		ListNode * n = new ListNode; n->Object = obj;
		n->Prev = Head.Prev; n->Next = &Head; Head.Prev->Next = n; Head.Prev = n;
	}
	RenderObjClass * Remove_Head(void) {
		if (Head.Next == &Head) return NULL;
		ListNode * n = Head.Next;
		RenderObjClass * o = n->Object;
		Head.Next = n->Next; n->Next->Prev = &Head;
		delete n;
		return o;
	}
	ListNode Head;
};

class RefRenderObjListIterator {
public:
	RefRenderObjListIterator(RefRenderObjListClass * list) : List(list) { First(); }
	void First(void) { CurNode = List->Head.Next; }
	void Next(void) { CurNode = CurNode->Next; }
	bool Is_Done(void) { return (CurNode == &(List->Head)); }
	RenderObjClass * Peek_Obj(void) { return CurNode->Object; }
	RefRenderObjListClass * List;
	ListNode * CurNode;
};

class SceneIterator {
public:
	virtual ~SceneIterator(void) { }
	virtual void First(void) = 0;
	virtual void Next(void) = 0;
	virtual bool Is_Done(void) = 0;
	virtual RenderObjClass * Current_Item(void) = 0;
};

class SimpleSceneIterator : public SceneIterator {
public:
	virtual void First(void);
	virtual void Next(void);
	virtual bool Is_Done(void);
	virtual RenderObjClass * Current_Item(void);
	SimpleSceneIterator(RefRenderObjListClass * renderlist,bool onlyvis);
	RefRenderObjListIterator RobjIterator;
	bool OnlyVis;
};

class SimpleSceneClass {
public:
	void Remove_All_Render_Objects(void);
	SceneIterator * Create_Iterator(bool onlyvisible);
	void Destroy_Iterator(SceneIterator * it);
	RefRenderObjListClass RenderList;
};

#include "sc_rem.inc"
#include "sc_create.inc"
#include "sc_destroy.inc"
#include "sc_itctor.inc"
#include "sc_first.inc"
#include "sc_next.inc"
#include "sc_done.inc"
#include "sc_cur.inc"

/* ---------------- vertmaterial ---------------- */
#define D3DRS_LIGHTING 137
#define D3DRS_DIFFUSEMATERIALSOURCE 145
#define D3DRS_AMBIENTMATERIALSOURCE 147
#define D3DRS_EMISSIVEMATERIALSOURCE 148
#define D3DTSS_TEXCOORDINDEX 11
#define D3DTSS_TEXTURETRANSFORMFLAGS 24
#define D3DTSS_TCI_PASSTHRU 0
#define D3DTTFF_DISABLE 0
#define D3DMCS_MATERIAL 0

struct D3DCOLORVALUE { float r, g, b, a; };
struct D3DMATERIAL8 { D3DCOLORVALUE Diffuse, Ambient, Specular, Emissive; float Power; };

struct DX8Wrapper {
	static void Set_DX8_Render_State(unsigned state, unsigned value) { g_log.push_back(30); g_log.push_back(state); g_log.push_back(value); }
	static void Set_DX8_Material(const D3DMATERIAL8 * m) {
		g_log.push_back(31);
		const float * f = &m->Diffuse.r;
		for (int i = 0; i < 17; i++) g_log.push_back(w_of(f[i]));
	}
	static void Set_DX8_Texture_Stage_State(unsigned stage, unsigned type, unsigned value) { g_log.push_back(32); g_log.push_back(stage); g_log.push_back(type); g_log.push_back(value); }
};

struct MeshBuilderClass { enum { MAX_STAGES = 8 }; };

class VertexMaterialClass {
public:
	void Set_Emissive(const Vector3 & color);
	void Set_Emissive(float r,float g,float b);
	int Get_UV_Source(int stage);
	void Apply_Null(void);
	D3DMATERIAL8 * Material;
	unsigned int UVSource[MeshBuilderClass::MAX_STAGES];
	mutable bool CRCDirty;
};

#include "vm_em3.inc"
#include "vm_emf.inc"
#include "vm_uv.inc"
#include "vm_null.inc"

/* ---------------- w3d_util ---------------- */
struct IOQuaternionStruct { float Q[4]; };
typedef IOQuaternionStruct W3dQuaternionStruct;
class W3dUtilityClass {
public:
	static void Convert_Quaternion(const W3dQuaternionStruct & q,Quaternion * set);
	static void Convert_Quaternion(const Quaternion & q,W3dQuaternionStruct * set);
};
#include "wu_from.inc"
#include "wu_to.inc"

}

/* layer: in = [scene id or 0, camera id or 0 (stored), nops, ops...]; ops: 0 Set_Scene(id or 0), 1 Get_Scene, 2 Peek_Camera.
** out per op: the log (count, words), then for 1 and 2 the returned id (0 for NULL); then the layer's scene id. */
extern "C" int ref_dsww3d_layer_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	static SceneClass scenes[8]; static CameraClass cams[8];
	for (int i = 0; i < 8; i++) { scenes[i].id = i + 1; cams[i].id = i + 1; }
	LayerClass l;
	uint32_t s = *p++, c = *p++;
	l.Scene = s ? &scenes[s - 1] : NULL;
	l.Camera = c ? &cams[c - 1] : NULL;
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		g_log.clear();
		uint32_t ret = 0;
		if (op == 0) { uint32_t id = *p++; l.Set_Scene(id ? &scenes[id - 1] : NULL); }
		else if (op == 1) { SceneClass * r = l.Get_Scene(); ret = r ? (uint32_t)r->id : 0; }
		else { CameraClass * r = l.Peek_Camera(); ret = r ? (uint32_t)r->id : 0; }
		o.push_back((uint32_t)g_log.size());
		for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
		o.push_back(ret);
		o.push_back(l.Scene ? (uint32_t)l.Scene->id : 0);
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}

/* scene: in = [n objects (ids 1..n added at the tail), nops, ops...]; ops: 0 Remove_All_Render_Objects -> the log;
** 1 Create_Iterator(onlyvisible) (replaces the iterator; the iterator is created over the CURRENT list)
** 2 First  3 Next  4 Is_Done  5 Current_Item  6 Destroy_Iterator. Per op: Is_Done as a word, Current_Item id (0 NULL) after First/Next. */
extern "C" int ref_dsww3d_scene_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	static RenderObjClass objs[32];
	for (int i = 0; i < 32; i++) objs[i].id = i + 1;
	SimpleSceneClass sc;
	int n = (int)*p++;
	for (int i = 0; i < n; i++) sc.RenderList.Add_Tail(&objs[i]);
	SceneIterator * it = NULL;
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		g_log.clear();
		switch (op) {
		case 0: sc.Remove_All_Render_Objects(); break;
		case 1: { bool v = *p++ != 0; if (it) sc.Destroy_Iterator(it); it = sc.Create_Iterator(v); break; }
		case 2: if (it) it->First(); break;
		case 3: if (it) it->Next(); break;
		case 4: if (it) o.push_back(it->Is_Done() ? 1u : 0u); break;
		case 5: if (it) { RenderObjClass * r = it->Current_Item(); o.push_back(r ? (uint32_t)r->id : 0u); } break;
		case 6: if (it) { sc.Destroy_Iterator(it); it = NULL; } break;
		}
		o.push_back((uint32_t)g_log.size());
		for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
	}
	if (it) sc.Destroy_Iterator(it);
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}

/* vmat: in = [emissive r g b (initial), uvsource[8], crc_dirty, nops, ops...]; ops: 0 Set_Emissive(Vector3) r g b,
** 1 Set_Emissive(r, g, b), 2 Get_UV_Source(stage) -> word, 3 Apply_Null -> the log. Per op: the log, then the 3 emissive words and CRCDirty. */
extern "C" int ref_dsww3d_vmat_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	D3DMATERIAL8 mat; memset(&mat, 0, sizeof(mat));
	mat.Emissive.r = f_of(p[0]); mat.Emissive.g = f_of(p[1]); mat.Emissive.b = f_of(p[2]); p += 3;
	VertexMaterialClass vm;
	vm.Material = &mat;
	for (int i = 0; i < 8; i++) vm.UVSource[i] = *p++;
	vm.CRCDirty = *p++ != 0;
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		g_log.clear();
		switch (op) {
		case 0: { Vector3 v(f_of(p[0]), f_of(p[1]), f_of(p[2])); p += 3; vm.Set_Emissive(v); break; }
		case 1: { float r = f_of(p[0]), g = f_of(p[1]), b = f_of(p[2]); p += 3; vm.Set_Emissive(r, g, b); break; }
		case 2: o.push_back((uint32_t)vm.Get_UV_Source((int)*p++)); break;
		case 3: vm.Apply_Null(); break;
		}
		o.push_back((uint32_t)g_log.size());
		for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
		o.push_back(w_of(mat.Emissive.r)); o.push_back(w_of(mat.Emissive.g)); o.push_back(w_of(mat.Emissive.b)); o.push_back(vm.CRCDirty ? 1u : 0u);
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}

/* wutil: in = [op, 4 words]; 0 file quaternion -> Quaternion (X Y Z W), 1 Quaternion -> file quaternion (Q[0..4]). Out: 4 words. */
extern "C" int ref_dsww3d_wutil_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	if (cap < 4) return -1;
	if (in[0] == 0) {
		W3dQuaternionStruct q; for (int i = 0; i < 4; i++) q.Q[i] = f_of(in[1 + i]);
		Quaternion r; W3dUtilityClass::Convert_Quaternion(q, &r);
		out[0] = w_of(r.X); out[1] = w_of(r.Y); out[2] = w_of(r.Z); out[3] = w_of(r.W);
	} else {
		Quaternion q(f_of(in[1]), f_of(in[2]), f_of(in[3]), f_of(in[4]));
		W3dQuaternionStruct r; W3dUtilityClass::Convert_Quaternion(q, &r);
		for (int i = 0; i < 4; i++) out[i] = w_of(r.Q[i]);
	}
	return 4;
}
