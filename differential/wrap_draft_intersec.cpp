/* Differential-test wrapper for the picking-ray drafts (GitHub #215, upstream commit 0a05454d), group "ww3dgeo":
** src/drafts/sweep_ww3d/intersec.rs.
**
** VERBATIM cuts by build.rs (`drafts_ww3dgeo`, anchors asserted to occur once, from the signature line to the closing brace)
** from WW3D2/intersec.cpp: IntersectionClass::Intersect_Screen_Point_RenderObject, Intersect_RenderObject,
** Intersect_Screen_Point_Layer, Intersect_Hierarchy_Sphere_Quick, Intersect_Hierarchy_Sphere (ix_*.inc), #included after
** the stand-in classes. Stand-ins: IntersectionClass (only Result, ScreenX/Y and the two members the cuts call,
** Get_Screen_Ray - intersec.inl's, which needs the camera matrices - and Intersect_Layer - intersec.cpp:122, which walks
** the layer's objects), IntersectionResultClass (Intersects, Range), LayerClass (Camera), CameraClass (Get_Viewport,
** Get_Depth), ViewportClass (Min, Max), RenderObjClass (Intersect, Get_Num_Sub_Objects, Get_Sub_Object, Release_Ref,
** Intersect_Sphere_Quick, Intersect_Sphere). Each stand-in call is scripted from the case and logs an event:
**   200 id: Intersect; 201 id: Release_Ref; 202 id: Intersect_Sphere_Quick; 203 id: Intersect_Sphere;
**   210 layer, screen_x bits, screen_y bits: Get_Screen_Ray; 211 layer test_all: Intersect_Layer.
** A call that writes a result writes Range = id + 0.5 and Intersects = the scripted hit.
**
** Entry point (extern "C", tests/differential_draft_intersec.rs):
**   ref_dix_setup(in, nin): [nobj, per object: id, hit, hit_quick, hit_sphere, nsub, sub indices...,
**       layer: id, min_x, min_y, max_x, max_y, depth, layer_hit]  (floats as bits)
**   ref_dix_run(in, nin, out, cap): in = [op, screen_x, screen_y, robj index, has_final, final.Intersects, final.Range,
**       Result.Intersects, Result.Range]; op 0 Intersect_Screen_Point_RenderObject, 1 Intersect_RenderObject,
**       2 Intersect_Screen_Point_Layer, 3 Intersect_Hierarchy_Sphere_Quick, 4 Intersect_Hierarchy_Sphere.
**   out = [return, Result.Intersects, Result.Range bits, ScreenX bits, ScreenY bits, final.Intersects, final.Range bits, log...] */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector2.h"

namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }
static std::vector<uint32_t> g_log;

class IntersectionClass;
class LayerClass;

class IntersectionResultClass
{
public:
	bool Intersects;
	float Range;
};

class ViewportClass { public: Vector2 Min; Vector2 Max; };
class CameraClass
{
public:
	ViewportClass vp;
	float depth;
	const ViewportClass & Get_Viewport(void) const { return vp; }
	float Get_Depth(void) const { return depth; }
};
class LayerClass
{
public:
	int id;
	bool hit;
	CameraClass * Camera;
};

class RenderObjClass
{
public:
	int id;
	bool hit, hit_quick, hit_sphere;
	std::vector<RenderObjClass *> subs;
	bool Intersect(IntersectionClass * ic, IntersectionResultClass * r)
	{
		g_log.push_back(200); g_log.push_back(id);
		r->Range = (float)id + 0.5f; r->Intersects = hit; return hit;
	}
	int Get_Num_Sub_Objects(void) const { return (int)subs.size(); }
	RenderObjClass * Get_Sub_Object(int index) const { return subs[index]; }
	void Release_Ref(void) const { g_log.push_back(201); g_log.push_back(id); }
	bool Intersect_Sphere_Quick(IntersectionClass * ic, IntersectionResultClass * r)
	{
		g_log.push_back(202); g_log.push_back(id);
		r->Range = (float)id + 0.5f; r->Intersects = hit_quick; return hit_quick;
	}
	bool Intersect_Sphere(IntersectionClass * ic, IntersectionResultClass * r)
	{
		g_log.push_back(203); g_log.push_back(id);
		r->Range = (float)id + 0.5f; r->Intersects = hit_sphere; return hit_sphere;
	}
};

class IntersectionClass
{
public:
	IntersectionResultClass Result;
	float ScreenX, ScreenY;
	void Get_Screen_Ray(float screen_x, float screen_y, const LayerClass & Layer)
	{
		g_log.push_back(210); g_log.push_back(Layer.id); g_log.push_back(w_of(screen_x)); g_log.push_back(w_of(screen_y));
		ScreenX = screen_x; ScreenY = screen_y;
	}
	bool Intersect_Layer(const LayerClass & Layer, bool Test_All)
	{
		g_log.push_back(211); g_log.push_back(Layer.id); g_log.push_back(Test_All ? 1u : 0u);
		Result.Intersects = Layer.hit;
		return Layer.hit;
	}
	bool Intersect_Screen_Point_RenderObject(float screen_x, float screen_y, const LayerClass &Layer, RenderObjClass *RObj, IntersectionResultClass *FinalResult);
	bool Intersect_RenderObject(RenderObjClass *RObj, IntersectionResultClass *FinalResult);
	bool Intersect_Screen_Point_Layer(float screen_x, float screen_y, const LayerClass &Layer);
	bool Intersect_Hierarchy_Sphere_Quick(RenderObjClass *Hierarchy, IntersectionResultClass *FinalResult);
	bool Intersect_Hierarchy_Sphere(RenderObjClass *Hierarchy, IntersectionResultClass *FinalResult);
};

#include "ix_srpo.inc"
#include "ix_ro.inc"
#include "ix_spl.inc"
#include "ix_hsq.inc"
#include "ix_hs.inc"

std::vector<RenderObjClass *> g_objs;
LayerClass g_layer;
CameraClass g_camera;

}

extern "C" void ref_dix_setup(const uint32_t * in, int nin)
{
	for (size_t i = 0; i < g_objs.size(); i++) delete g_objs[i];
	g_objs.clear();
	const uint32_t * p = in;
	uint32_t n = *p++;
	std::vector<std::vector<uint32_t> > subs;
	for (uint32_t i = 0; i < n; i++) {
		RenderObjClass * o = new RenderObjClass();
		o->id = (int)p[0]; o->hit = p[1] != 0; o->hit_quick = p[2] != 0; o->hit_sphere = p[3] != 0;
		uint32_t ns = p[4];
		std::vector<uint32_t> s;
		for (uint32_t k = 0; k < ns; k++) s.push_back(p[5 + k]);
		subs.push_back(s);
		p += 5 + ns;
		g_objs.push_back(o);
	}
	for (uint32_t i = 0; i < n; i++) for (size_t k = 0; k < subs[i].size(); k++) g_objs[i]->subs.push_back(g_objs[subs[i][k]]);
	g_layer.id = (int)p[0];
	g_camera.vp.Min = Vector2(f_of(p[1]), f_of(p[2]));
	g_camera.vp.Max = Vector2(f_of(p[3]), f_of(p[4]));
	g_camera.depth = f_of(p[5]);
	g_layer.hit = p[6] != 0;
	g_layer.Camera = &g_camera;
	(void)nin;
}

extern "C" int ref_dix_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	g_log.clear();
	IntersectionClass ic;
	ic.ScreenX = 0; ic.ScreenY = 0;
	ic.Result.Intersects = in[7] != 0;
	ic.Result.Range = f_of(in[8]);
	IntersectionResultClass fin;
	fin.Intersects = in[5] != 0;
	fin.Range = f_of(in[6]);
	IntersectionResultClass * fp = in[4] != 0 ? &fin : 0;
	RenderObjClass * ro = g_objs.empty() ? 0 : g_objs[in[3]];
	bool r = false;
	switch (in[0]) {
	case 0: r = ic.Intersect_Screen_Point_RenderObject(f_of(in[1]), f_of(in[2]), g_layer, ro, fp); break;
	case 1: r = ic.Intersect_RenderObject(ro, fp); break;
	case 2: r = ic.Intersect_Screen_Point_Layer(f_of(in[1]), f_of(in[2]), g_layer); break;
	case 3: r = ic.Intersect_Hierarchy_Sphere_Quick(ro, &fin); break;
	case 4: r = ic.Intersect_Hierarchy_Sphere(ro, &fin); break;
	default: return -1;
	}
	std::vector<uint32_t> o;
	o.push_back(r ? 1u : 0u);
	o.push_back(ic.Result.Intersects ? 1u : 0u);
	o.push_back(w_of(ic.Result.Range));
	o.push_back(w_of(ic.ScreenX));
	o.push_back(w_of(ic.ScreenY));
	o.push_back(fin.Intersects ? 1u : 0u);
	o.push_back(w_of(fin.Range));
	for (size_t i = 0; i < g_log.size(); i++) o.push_back(g_log[i]);
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	(void)nin;
	return (int)o.size();
}
