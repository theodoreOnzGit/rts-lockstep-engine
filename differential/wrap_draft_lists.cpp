/* Differential-test wrapper for the small WW3D container drafts (GitHub #215, upstream commit 0a05454d), group "ww3dgeo":
** src/drafts/sweep_ww3d/{predlod,visrasterizer,seglinerenderer,streak_render,collect,distlod}.rs.
**
** VERBATIM cuts by build.rs (`drafts_ww3dgeo`, each anchor asserted to occur once, the function from its signature line to
** its closing brace, no edit), #included below after the stand-in classes they are members of:
**   predlod.cpp        PredictiveLODOptimizerClass::AllocVisibleObjArrays                         -> pl_alloc.inc
**   visrasterizer.cpp  VisRasterizerClass::Get_Temp_Vertex_Buffer                                  -> vr_temp.inc
**   seglinerenderer.cpp SegLineRendererClass::getVertexBuffer                                      -> sl_getvb.inc
**   streakRender.cpp   StreakRendererClass::getVertexBuffer                                        -> st_getvb.inc
**   collect.cpp        CollectionClass::Add_Sub_Object, Snap_Point_Count, Get_Snap_Point, Get_Proxy, Get_Proxy_Count
**   distlod.cpp        DistLODClass::Get_Num_Snap_Points, Get_Snap_Point, Update_Lod, Increment_Lod, Decrement_Lod
** Stand-ins (what the cuts call outside themselves; each class carries ONLY the members its cuts touch):
**   LODHeapNode (predlod.cpp's private class): an empty node that counts its default constructions (the allocation size).
**   PredictiveLODOptimizerClass / VisRasterizerClass / SegLineRendererClass / StreakRendererClass / CollectionClass /
**   DistLODClass: the data members of the headers' classes the cuts use, with the same types (SimpleVecClass<Vector3>,
**   DynamicVectorClass<...>, the vertex arrays, int/float fields), and the inherited RenderObjClass calls (Is_In_Scene,
**   Update_Sub_Object_Bits, Update_Obj_Space_Bounding_Volumes) as logging stubs.
**   RenderObjClass: Add_Ref, Set_Container, Set_Transform, Notify_Added/Removed log an event; Get_Num_Snap_Points,
**   Get_Snap_Point, Get_Bounding_Sphere answer from a table. CameraClass: Get_Position. ProxyClass (proxy.h): a name and a
**   transform (the real one holds a StringClass, which would pull wwstring.cpp in for nothing the cut does).
**   VertexFormatXYZDUV1 / VertexFormatXYZUV1 (dx8fvf.h): the field layouts.
** Event codes written to the log (the Rust hosts write the same): 100 Add_Ref id; 101 Set_Container id; 102 Set_Transform id
** + 12 float bits; 103 Notify_Added id; 104 Notify_Removed id; 110 Update_Sub_Object_Bits; 111 Update_Obj_Space_Bounding_Volumes.
**
** Entry point (extern "C", tests/differential_draft_lists.rs): ref_dls_run(in, nin, out, cap) -> words written or -1. */
#include <stdint.h>
#include <string.h>
#include <vector>
#include <string>
#include <new>
#include "always.h"
#include "wwdebug.h"
#include "vector3.h"
#include "matrix3d.h"
#include "sphere.h"
#include "vector.h"
#include "simplevec.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static std::vector<uint32_t> g_log;

class SceneClass {};
class CollectionClass;

class RenderObjClass
{
public:
	int id;
	SphereClass sphere;
	std::vector<Vector3> snaps;
	void Add_Ref(void) { g_log.push_back(100); g_log.push_back(id); }
	void Set_Container(CollectionClass *) { g_log.push_back(101); g_log.push_back(id); }
	void Set_Transform(const Matrix3D & tm)
	{
		g_log.push_back(102); g_log.push_back(id);
		for (int r = 0; r < 3; r++) { g_log.push_back(w_of(tm[r][0])); g_log.push_back(w_of(tm[r][1])); g_log.push_back(w_of(tm[r][2])); g_log.push_back(w_of(tm[r][3])); }
	}
	void Notify_Added(SceneClass *) { g_log.push_back(103); g_log.push_back(id); }
	void Notify_Removed(SceneClass *) { g_log.push_back(104); g_log.push_back(id); }
	int Get_Num_Snap_Points(void) { return (int)snaps.size(); }
	void Get_Snap_Point(int index, Vector3 * set) { *set = snaps[index]; }
	const SphereClass & Get_Bounding_Sphere(void) const { return sphere; }
};

class CameraClass
{
public:
	Vector3 pos;
	Vector3 Get_Position(void) const { return pos; }
};

/* ---- predlod.cpp ---- */
class LODHeapNode
{
public:
	static int constructed;
	LODHeapNode() { constructed++; }
};
int LODHeapNode::constructed = 0;

class PredictiveLODOptimizerClass
{
public:
	static void AllocVisibleObjArrays(int num_objects);
	static LODHeapNode * VisibleObjArray1;
	static LODHeapNode * VisibleObjArray2;
	static int VisibleObjArraySize;
};
LODHeapNode * PredictiveLODOptimizerClass::VisibleObjArray1;
LODHeapNode * PredictiveLODOptimizerClass::VisibleObjArray2;
int PredictiveLODOptimizerClass::VisibleObjArraySize;
#include "pl_alloc.inc"

/* ---- visrasterizer.cpp ---- */
class VisRasterizerClass
{
public:
	SimpleVecClass<Vector3> TempVertexBuffer;
	Vector3 * Get_Temp_Vertex_Buffer(int count);
};
#include "vr_temp.inc"

/* ---- seglinerenderer.cpp / streakRender.cpp ---- */
struct VertexFormatXYZDUV1 { float x, y, z; unsigned long diffuse; float u1, v1; };
struct VertexFormatXYZUV1 { float x, y, z; float u1, v1; };

class SegLineRendererClass
{
public:
	SegLineRendererClass() : m_vertexBufferSize(0), m_vertexBuffer(NULL) {}
	unsigned int m_vertexBufferSize;
	VertexFormatXYZDUV1 * m_vertexBuffer;
	VertexFormatXYZDUV1 * getVertexBuffer(unsigned int number);
};
#include "sl_getvb.inc"

class StreakRendererClass
{
public:
	StreakRendererClass() : m_vertexBufferSize(0), m_vertexBuffer(NULL) {}
	unsigned int m_vertexBufferSize;
	VertexFormatXYZUV1 * m_vertexBuffer;
	VertexFormatXYZUV1 * getVertexBuffer(unsigned int number);
};
#include "st_getvb.inc"

/* ---- collect.cpp ---- */
class ProxyClass
{
public:
	std::string Name;
	Matrix3D Transform;
	bool operator==(const ProxyClass & s) const { return Name == s.Name && Transform == s.Transform; }
	bool operator!=(const ProxyClass & s) const { return !(*this == s); }
};

class CollectionClass
{
public:
	CollectionClass() : SnapPoints(NULL), Scene(NULL), in_scene(false), Transform(true) {}
	DynamicVectorClass<RenderObjClass *> SubObjects;
	DynamicVectorClass<Vector3> * SnapPoints;
	DynamicVectorClass<ProxyClass> ProxyList;
	Matrix3D Transform;
	SceneClass * Scene;
	bool in_scene;
	bool Is_In_Scene(void) const { return in_scene; }
	void Update_Sub_Object_Bits(void) { g_log.push_back(110); }
	void Update_Obj_Space_Bounding_Volumes(void) { g_log.push_back(111); }
	int Add_Sub_Object(RenderObjClass * subobj);
	int Snap_Point_Count(void);
	void Get_Snap_Point(int index, Vector3 * set);
	bool Get_Proxy(int index, ProxyClass & proxy) const;
	int Get_Proxy_Count(void) const;
};
#include "co_add.inc"
#include "co_spc.inc"
#include "co_gsp.inc"
#include "co_gp.inc"
#include "co_gpc.inc"

/* ---- distlod.cpp ---- */
class DistLODClass
{
public:
	class LODNodeClass { public: RenderObjClass * Model; float ResUpDist; float ResDownDist; };
	DistLODClass() : LodCount(0), CurLod(0), Lods(NULL), Scene(NULL), in_scene(false) {}
	int LodCount;
	int CurLod;
	LODNodeClass * Lods;
	SceneClass * Scene;
	bool in_scene;
	bool Is_In_Scene(void) const { return in_scene; }
	int Get_Num_Snap_Points(void);
	void Get_Snap_Point(int index, Vector3 * set);
	void Update_Lod(const CameraClass & camera);
	void Increment_Lod(void);
	void Decrement_Lod(void);
};
#include "dl_gnsp.inc"
#include "dl_gsp.inc"
#include "dl_upd.inc"
#include "dl_inc.inc"
#include "dl_dec.inc"

/* ---- state and the entry point ---- */
CollectionClass g_col;
std::vector<RenderObjClass *> g_objs;
DynamicVectorClass<Vector3> g_snaps;
bool g_has_snaps = false;
VisRasterizerClass g_vr;
SegLineRendererClass g_sl;
StreakRendererClass g_st;
DistLODClass g_dl;
std::vector<DistLODClass::LODNodeClass> g_lods;
std::vector<RenderObjClass *> g_models;

static RenderObjClass * obj_of(int id)
{
	for (size_t i = 0; i < g_objs.size(); i++) if (g_objs[i]->id == id) return g_objs[i];
	RenderObjClass * o = new RenderObjClass();
	o->id = id;
	g_objs.push_back(o);
	return o;
}

static void put_log(std::vector<uint32_t> & o)
{
	for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
	g_log.clear();
}

template<class T> static void reset(T & x) { x.~T(); new (&x) T(); }

static uint32_t pattern(uint32_t seed, uint32_t i) { return (seed * 2654435761u) ^ (i * 40503u + 12345u); }

}

extern "C" int ref_dls_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	switch (p[0]) {
	case 0: { /* reset everything */
		g_log.clear();
		reset(g_col);
		g_snaps.Reset_Active(); g_has_snaps = false;
		reset(g_vr);
		reset(g_sl);
		reset(g_st);
		reset(g_dl);
		PredictiveLODOptimizerClass::VisibleObjArray1 = NULL;
		PredictiveLODOptimizerClass::VisibleObjArray2 = NULL;
		PredictiveLODOptimizerClass::VisibleObjArraySize = 0;
		for (size_t i = 0; i < g_objs.size(); i++) delete g_objs[i];
		g_objs.clear();
		break; }
	case 1: { /* predlod: [1, num_objects] -> [size, array2 - array1, constructed, array1 != NULL] */
		LODHeapNode::constructed = 0;
		PredictiveLODOptimizerClass::AllocVisibleObjArrays((int)p[1]);
		o.push_back((uint32_t)PredictiveLODOptimizerClass::VisibleObjArraySize);
		o.push_back((uint32_t)(PredictiveLODOptimizerClass::VisibleObjArray2 - PredictiveLODOptimizerClass::VisibleObjArray1));
		o.push_back((uint32_t)LODHeapNode::constructed);
		o.push_back(PredictiveLODOptimizerClass::VisibleObjArray1 != NULL ? 1u : 0u);
		break; }
	case 2: { /* visrasterizer: [2, count, seed] -> [Length, grown, n, words...] */
		int before = g_vr.TempVertexBuffer.Length();
		int count = (int)p[1];
		Vector3 * v = g_vr.Get_Temp_Vertex_Buffer(count);
		int len = g_vr.TempVertexBuffer.Length();
		o.push_back((uint32_t)len);
		if (len != before) {
			o.push_back(1);
			o.push_back(0);
			for (int i = 0; i < len; i++) { v[i].X = f_of(pattern(p[2], 3 * i)); v[i].Y = f_of(pattern(p[2], 3 * i + 1)); v[i].Z = f_of(pattern(p[2], 3 * i + 2)); }
		} else {
			int n = count > 0 ? count : 0;
			o.push_back(0);
			o.push_back((uint32_t)n);
			for (int i = 0; i < n; i++) { o.push_back(w_of(v[i].X)); o.push_back(w_of(v[i].Y)); o.push_back(w_of(v[i].Z)); }
		}
		break; }
	case 3: { /* seglinerenderer: [3, number, seed] -> [m_vertexBufferSize, grown, n, words...] (6 words per vertex) */
		unsigned int before = g_sl.m_vertexBufferSize;
		unsigned int number = p[1];
		VertexFormatXYZDUV1 * v = g_sl.getVertexBuffer(number);
		unsigned int len = g_sl.m_vertexBufferSize;
		o.push_back(len);
		if (len != before) {
			o.push_back(1);
			o.push_back(0);
			for (unsigned i = 0; i < len; i++) { v[i].x = f_of(pattern(p[2], 6 * i)); v[i].y = f_of(pattern(p[2], 6 * i + 1)); v[i].z = f_of(pattern(p[2], 6 * i + 2)); v[i].diffuse = pattern(p[2], 6 * i + 3); v[i].u1 = f_of(pattern(p[2], 6 * i + 4)); v[i].v1 = f_of(pattern(p[2], 6 * i + 5)); }
		} else {
			o.push_back(0);
			o.push_back(number);
			for (unsigned i = 0; i < number; i++) { o.push_back(w_of(v[i].x)); o.push_back(w_of(v[i].y)); o.push_back(w_of(v[i].z)); o.push_back((uint32_t)v[i].diffuse); o.push_back(w_of(v[i].u1)); o.push_back(w_of(v[i].v1)); }
		}
		break; }
	case 4: { /* streakRender: [4, number, seed] -> [m_vertexBufferSize, grown, n, words...] (5 words per vertex) */
		unsigned int before = g_st.m_vertexBufferSize;
		unsigned int number = p[1];
		VertexFormatXYZUV1 * v = g_st.getVertexBuffer(number);
		unsigned int len = g_st.m_vertexBufferSize;
		o.push_back(len);
		if (len != before) {
			o.push_back(1);
			o.push_back(0);
			for (unsigned i = 0; i < len; i++) { v[i].x = f_of(pattern(p[2], 5 * i)); v[i].y = f_of(pattern(p[2], 5 * i + 1)); v[i].z = f_of(pattern(p[2], 5 * i + 2)); v[i].u1 = f_of(pattern(p[2], 5 * i + 3)); v[i].v1 = f_of(pattern(p[2], 5 * i + 4)); }
		} else {
			o.push_back(0);
			o.push_back(number);
			for (unsigned i = 0; i < number; i++) { o.push_back(w_of(v[i].x)); o.push_back(w_of(v[i].y)); o.push_back(w_of(v[i].z)); o.push_back(w_of(v[i].u1)); o.push_back(w_of(v[i].v1)); }
		}
		break; }
	case 10: g_col.in_scene = p[1] != 0; break;
	case 11: { /* collection transform: [11, 12 floats] */
		g_col.Transform = Matrix3D(f_of(p[1]), f_of(p[2]), f_of(p[3]), f_of(p[4]), f_of(p[5]), f_of(p[6]), f_of(p[7]), f_of(p[8]), f_of(p[9]), f_of(p[10]), f_of(p[11]), f_of(p[12]));
		break; }
	case 13: { /* add_sub_object: [13, id] -> [return, log...]; the object is new */
		RenderObjClass * ob = obj_of((int)p[1]);
		int r = g_col.Add_Sub_Object(ob);
		o.push_back((uint32_t)r);
		o.push_back((uint32_t)g_col.SubObjects.Count());
		put_log(o);
		break; }
	case 14: { /* snap points: [14, has, n, 3n floats] */
		g_has_snaps = p[1] != 0;
		g_snaps.Reset_Active();
		for (uint32_t i = 0; i < p[2]; i++) g_snaps.Add(Vector3(f_of(p[3 + 3 * i]), f_of(p[4 + 3 * i]), f_of(p[5 + 3 * i])));
		g_col.SnapPoints = g_has_snaps ? &g_snaps : NULL;
		break; }
	case 15: o.push_back((uint32_t)g_col.Snap_Point_Count()); break;
	case 16: { /* get_snap_point: [16, index, 3 prefill floats] -> 3 words */
		Vector3 s(f_of(p[2]), f_of(p[3]), f_of(p[4]));
		g_col.Get_Snap_Point((int)p[1], &s);
		o.push_back(w_of(s.X)); o.push_back(w_of(s.Y)); o.push_back(w_of(s.Z));
		break; }
	case 17: { /* proxies: [17, n, per proxy: name length, chars, 12 floats] */
		g_col.ProxyList.Reset_Active();
		uint32_t k = 2;
		for (uint32_t i = 0; i < p[1]; i++) {
			ProxyClass px;
			uint32_t len = p[k++];
			for (uint32_t c = 0; c < len; c++) px.Name.push_back((char)p[k++]);
			px.Transform = Matrix3D(f_of(p[k]), f_of(p[k + 1]), f_of(p[k + 2]), f_of(p[k + 3]), f_of(p[k + 4]), f_of(p[k + 5]), f_of(p[k + 6]), f_of(p[k + 7]), f_of(p[k + 8]), f_of(p[k + 9]), f_of(p[k + 10]), f_of(p[k + 11]));
			k += 12;
			g_col.ProxyList.Add(px);
		}
		break; }
	case 18: { /* get_proxy: [18, index, prefill name length, chars] -> [return, name length, chars, 12 floats] */
		ProxyClass px;
		uint32_t len = p[2];
		for (uint32_t c = 0; c < len; c++) px.Name.push_back((char)p[3 + c]);
		px.Transform = Matrix3D(true);
		bool r = g_col.Get_Proxy((int)p[1], px);
		o.push_back(r ? 1u : 0u);
		o.push_back((uint32_t)px.Name.size());
		for (size_t c = 0; c < px.Name.size(); c++) o.push_back((uint32_t)(unsigned char)px.Name[c]);
		for (int rr = 0; rr < 3; rr++) for (int cc = 0; cc < 4; cc++) o.push_back(w_of(px.Transform[rr][cc]));
		break; }
	case 19: o.push_back((uint32_t)g_col.Get_Proxy_Count()); break;
	case 30: { /* distlod setup: [30, count, cur, in_scene, per lod: id, up, down, cx, cy, cz, nsnap, 3*nsnap floats] */
		reset(g_dl);
		g_lods.clear();
		for (size_t i = 0; i < g_models.size(); i++) delete g_models[i];
		g_models.clear();
		uint32_t k = 4;
		for (uint32_t i = 0; i < p[1]; i++) {
			RenderObjClass * m = new RenderObjClass();
			m->id = (int)p[k];
			DistLODClass::LODNodeClass ln;
			ln.Model = m; ln.ResUpDist = f_of(p[k + 1]); ln.ResDownDist = f_of(p[k + 2]);
			m->sphere = SphereClass(Vector3(f_of(p[k + 3]), f_of(p[k + 4]), f_of(p[k + 5])), 1.0f);
			uint32_t ns = p[k + 6];
			k += 7;
			for (uint32_t s = 0; s < ns; s++) { m->snaps.push_back(Vector3(f_of(p[k]), f_of(p[k + 1]), f_of(p[k + 2]))); k += 3; }
			g_models.push_back(m);
			g_lods.push_back(ln);
		}
		g_dl.LodCount = (int)p[1];
		g_dl.CurLod = (int)p[2];
		g_dl.in_scene = p[3] != 0;
		g_dl.Lods = g_lods.empty() ? NULL : &g_lods[0];
		break; }
	case 31: { /* update_lod: [31, 3 camera floats] -> [CurLod, log...] */
		CameraClass cam;
		cam.pos = Vector3(f_of(p[1]), f_of(p[2]), f_of(p[3]));
		g_dl.Update_Lod(cam);
		o.push_back((uint32_t)g_dl.CurLod);
		put_log(o);
		break; }
	case 32: g_dl.Increment_Lod(); o.push_back((uint32_t)g_dl.CurLod); put_log(o); break;
	case 33: g_dl.Decrement_Lod(); o.push_back((uint32_t)g_dl.CurLod); put_log(o); break;
	case 34: o.push_back((uint32_t)g_dl.Get_Num_Snap_Points()); break;
	case 35: { /* get_snap_point: [35, index, 3 prefill floats] -> 3 words */
		Vector3 s(f_of(p[2]), f_of(p[3]), f_of(p[4]));
		g_dl.Get_Snap_Point((int)p[1], &s);
		o.push_back(w_of(s.X)); o.push_back(w_of(s.Y)); o.push_back(w_of(s.Z));
		break; }
	default: return -1;
	}
	(void)nin;
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
