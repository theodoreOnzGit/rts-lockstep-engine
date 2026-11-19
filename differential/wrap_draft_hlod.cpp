/* Differential-test wrapper for src/drafts/sweep_ww3d/hlod.rs (GitHub #215, group sweepww3d; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_sweepww3d`), from WW3D2/hlod.cpp: HLodClass::Get_Max_Screen_Size, Get_Lod_Model_Count,
** Get_Lod_Model, Get_Lod_Model_Bone, Get_Additional_Model_Count, Peek_Additional_Model, Get_Additional_Model,
** Get_Additional_Model_Bone, Is_NULL_Lod_Included, Get_Proxy_Count, Get_Proxy, Add_Sub_Object, Get_Num_Sub_Objects_On_Bone,
** Get_Current_LOD, Get_Num_Snap_Points, Get_Snap_Point, Set_HTree.
** Stand-ins (what the cuts reach outside themselves):
**   RenderObjClass: a reference count (Add_Ref), Class_ID() and CLASSID_NULL; one object per host handle (handle h is
**     object h-1; 0 is NULL).
**   HLodClass: the fields of hlod.h these read, ModelNodeClass / ModelArrayClass (MaxScreenSize, Count(), operator[]) as in hlod.h
**     with a std::vector for DynamicVectorClass; SnapPoints and ProxyArray with Count()/Length() and operator[]; HTree
**     (Base_Update logs, Get_Transform(bone) returns a table entry); Set_Hierarchy_Valid logs; Get_Transform() returns the
**     identity; Add_Sub_Object_To_Bone and Animatable3DObjClass::Set_HTree log their arguments and Add_Sub_Object_To_Bone
**     returns 1000 + bone + handle.
**   ProxyClass: proxy.h's, with std::string for StringClass. NO_MAX_SCREEN_SIZE is w3d_file.h's WWMATH_FLOAT_MAX.
** Entry point: ref_dsww3d_hlod_run(in, nin, out, cap), the layout is in tests/differential_draft_hlod.rs. */
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector3.h"
#include "matrix3d.h"
#include "wwmath.h"

#define NO_MAX_SCREEN_SIZE WWMATH_FLOAT_MAX

/* HTreeClass completes the prelude's global forward declaration (w3d_prelude.h), so it is not in the anonymous namespace */
static std::vector<uint32_t> g_log;
class HTreeClass {
public:
	std::vector<Matrix3D> bones;
	void Base_Update(const Matrix3D & root) { g_log.push_back(1); }
	Matrix3D Get_Transform(int bone) const { g_log.push_back(2); return bones[bone]; }
};

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

class RenderObjClass {
public:
	enum { CLASSID_MESH = 0, CLASSID_NULL = 13 };
	int cid; int refs;
	int Class_ID(void) const { return cid; }
	void Add_Ref(void) { refs++; }
};
static RenderObjClass g_objs[64];

class ProxyClass {
public:
	std::string Name; Matrix3D Transform;
	ProxyClass (void) { }
	const char * Get_Name (void) const { return Name.c_str(); }
	void Set_Name (const char *name) { Name = name; }
	const Matrix3D & Get_Transform (void) const { return Transform; }
	void Set_Transform (const Matrix3D &tm) { Transform = tm; }
};

class ProxyRecordClass {
public:
	std::string Name; int Bone;
	int Get_Bone_Index(void) const { return Bone; }
	const char * Get_Name(void) const { return Name.c_str(); }
};
class ProxyArrayClass {
public:
	std::vector<ProxyRecordClass> v;
	int Length(void) const { return (int)v.size(); }
	ProxyRecordClass & operator [] (int i) { return v[i]; }
};
class SnapPointsClass {
public:
	std::vector<Vector3> v;
	int Count(void) const { return (int)v.size(); }
	Vector3 & operator [] (int i) { return v[i]; }
};

class Animatable3DObjClass {
public:
	void Set_HTree(HTreeClass * htree) { g_log.push_back(3); g_log.push_back(htree == NULL ? 0u : 1u); }
};

class HLodClass : public Animatable3DObjClass
{
public:
	class ModelNodeClass { public: RenderObjClass * Model; int BoneIndex; };
	class ModelArrayClass {
	public:
		ModelArrayClass(void) : MaxScreenSize(NO_MAX_SCREEN_SIZE) {}
		std::vector<ModelNodeClass> v;
		int Count(void) const { return (int)v.size(); }
		ModelNodeClass & operator [] (int i) { return v[i]; }
		const ModelNodeClass & operator [] (int i) const { return v[i]; }
		float MaxScreenSize;
	};

	float Get_Max_Screen_Size(int lod_index) const;
	int Get_Lod_Model_Count(int lod_index) const;
	RenderObjClass *Get_Lod_Model(int lod_index, int model_index) const;
	int Get_Lod_Model_Bone(int lod_index, int model_index) const;
	int Get_Additional_Model_Count(void) const;
	RenderObjClass * Peek_Additional_Model (int model_index) const;
	RenderObjClass * Get_Additional_Model (int model_index) const;
	int Get_Additional_Model_Bone (int model_index) const;
	bool Is_NULL_Lod_Included(void) const;
	int Get_Proxy_Count(void) const;
	bool Get_Proxy (int index, ProxyClass &proxy) const;
	int Add_Sub_Object(RenderObjClass * subobj);
	int Get_Num_Sub_Objects_On_Bone(int boneindex) const;
	RenderObjClass * Get_Current_LOD(void);
	int Get_Num_Snap_Points(void);
	void Get_Snap_Point(int index,Vector3 * set);
	void Set_HTree(HTreeClass * htree);

	const Matrix3D & Get_Transform(void) const { return Identity; }
	void Set_Hierarchy_Valid(bool onoff) const { g_log.push_back(4); }
	int Add_Sub_Object_To_Bone(RenderObjClass * subobj, int bone) { g_log.push_back(5); g_log.push_back((uint32_t)bone); return 1000 + bone + (int)(subobj - g_objs) + 1; }

	int LodCount;
	int CurLod;
	ModelArrayClass * Lod;
	ModelArrayClass AdditionalModels;
	SnapPointsClass * SnapPoints;
	ProxyArrayClass * ProxyArray;
	HTreeClass * HTree;
	Matrix3D Identity;
};

#include "hl_mss.inc"
#include "hl_lmc.inc"
#include "hl_glm.inc"
#include "hl_lmb.inc"
#include "hl_amc.inc"
#include "hl_pam.inc"
#include "hl_gam.inc"
#include "hl_amb.inc"
#include "hl_nli.inc"
#include "hl_gpc.inc"
#include "hl_gp.inc"
#include "hl_aso.inc"
#include "hl_gnso.inc"
#include "hl_gcl.inc"
#include "hl_gnsp.inc"
#include "hl_gsp.inc"
#include "hl_sht.inc"

static uint32_t handle_of(RenderObjClass * o) { return o == NULL ? 0u : (uint32_t)(o - g_objs) + 1u; }

}

extern "C" int ref_dsww3d_hlod_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	g_log.clear();
	int nobj = (int)*p++;
	for (int i = 0; i < 64; i++) { g_objs[i].cid = (i < nobj) ? (int)*p++ : 0; g_objs[i].refs = 1; }
	HLodClass h;
	h.Identity = Matrix3D(true);
	h.LodCount = (int)*p++;
	h.CurLod = (int)*p++;
	std::vector<HLodClass::ModelArrayClass> lods(h.LodCount > 0 ? h.LodCount : 1);
	for (int l = 0; l < h.LodCount; l++) {
		lods[l].MaxScreenSize = f_of(*p++);
		int n = (int)*p++;
		for (int m = 0; m < n; m++) {
			HLodClass::ModelNodeClass nd;
			uint32_t hd = *p++;
			nd.Model = hd == 0 ? NULL : &g_objs[hd - 1];
			nd.BoneIndex = (int)*p++;
			lods[l].v.push_back(nd);
		}
	}
	h.Lod = &lods[0];
	{
		int n = (int)*p++;
		for (int m = 0; m < n; m++) {
			HLodClass::ModelNodeClass nd;
			uint32_t hd = *p++;
			nd.Model = hd == 0 ? NULL : &g_objs[hd - 1];
			nd.BoneIndex = (int)*p++;
			h.AdditionalModels.v.push_back(nd);
		}
	}
	ProxyArrayClass pa; SnapPointsClass sp; HTreeClass tree;
	h.ProxyArray = NULL; h.SnapPoints = NULL; h.HTree = &tree;
	if (*p++) {
		h.ProxyArray = &pa;
		int n = (int)*p++;
		for (int i = 0; i < n; i++) {
			ProxyRecordClass r;
			r.Bone = (int)*p++;
			int len = (int)*p++;
			for (int c = 0; c < len; c++) r.Name.push_back((char)*p++);
			pa.v.push_back(r);
		}
	}
	if (*p++) {
		h.SnapPoints = &sp;
		int n = (int)*p++;
		for (int i = 0; i < n; i++) { sp.v.push_back(Vector3(f_of(p[0]), f_of(p[1]), f_of(p[2]))); p += 3; }
	}
	{
		int nb = (int)*p++;
		for (int b = 0; b < nb; b++) { float m[12]; for (int i = 0; i < 12; i++) m[i] = f_of(*p++); tree.bones.push_back(Matrix3D(m)); }
	}
	int nops = (int)*p++;
	for (int k = 0; k < nops; k++) {
		int op = (int)*p++;
		g_log.clear();
		int refs_before[64]; for (int i = 0; i < 64; i++) refs_before[i] = g_objs[i].refs;
		switch (op) {
		case 0: o.push_back(w_of(h.Get_Max_Screen_Size((int)p[0]))); p += 1; break;
		case 1: o.push_back((uint32_t)h.Get_Lod_Model_Count((int)p[0])); p += 1; break;
		case 2: { RenderObjClass * r = h.Get_Lod_Model((int)p[0], (int)p[1]); p += 2; o.push_back(handle_of(r)); break; }
		case 3: o.push_back((uint32_t)h.Get_Lod_Model_Bone((int)p[0], (int)p[1])); p += 2; break;
		case 4: o.push_back((uint32_t)h.Get_Additional_Model_Count()); break;
		case 5: { RenderObjClass * r = h.Peek_Additional_Model((int)p[0]); p += 1; o.push_back(handle_of(r)); break; }
		case 6: { RenderObjClass * r = h.Get_Additional_Model((int)p[0]); p += 1; o.push_back(handle_of(r)); break; }
		case 7: o.push_back((uint32_t)h.Get_Additional_Model_Bone((int)p[0])); p += 1; break;
		case 8: o.push_back(h.Is_NULL_Lod_Included() ? 1u : 0u); break;
		case 9: o.push_back((uint32_t)h.Get_Proxy_Count()); break;
		case 10: {
			ProxyClass px; px.Transform = Matrix3D(false);
			bool r = h.Get_Proxy((int)p[0], px); p += 1;
			o.push_back(r ? 1u : 0u);
			o.push_back((uint32_t)strlen(px.Get_Name()));
			for (const char * c = px.Get_Name(); *c; c++) o.push_back((uint32_t)(unsigned char)*c);
			const Matrix3D & t = px.Get_Transform();
			for (int r2 = 0; r2 < 3; r2++) for (int c2 = 0; c2 < 4; c2++) o.push_back(w_of(t[r2][c2]));
			break; }
		case 11: { uint32_t hd = p[0]; p += 1; o.push_back((uint32_t)h.Add_Sub_Object(hd == 0 ? NULL : &g_objs[hd - 1])); break; }
		case 12: o.push_back((uint32_t)h.Get_Num_Sub_Objects_On_Bone((int)p[0])); p += 1; break;
		case 13: { RenderObjClass * r = h.Get_Current_LOD(); o.push_back(handle_of(r)); break; }
		case 14: o.push_back((uint32_t)h.Get_Num_Snap_Points()); break;
		case 15: { Vector3 v(9.0f, 9.0f, 9.0f); h.Get_Snap_Point((int)p[0], &v); p += 1; o.push_back(w_of(v.X)); o.push_back(w_of(v.Y)); o.push_back(w_of(v.Z)); break; }
		case 16: { uint32_t hd = p[0]; p += 1; h.Set_HTree(hd == 0 ? NULL : &tree); break; }
		}
		// the calls the stand-ins saw, then the reference-count change of every object
		o.push_back((uint32_t)g_log.size());
		for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
		for (int i = 0; i < 64; i++) if (g_objs[i].refs != refs_before[i]) { o.push_back((uint32_t)(i + 1)); o.push_back((uint32_t)(g_objs[i].refs - refs_before[i])); }
		o.push_back(0);
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
