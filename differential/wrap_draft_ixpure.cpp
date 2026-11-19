/* Differential-test wrapper for the ray-intersection arithmetic drafts (GitHub #215, upstream commit 0a05454d), group "ww3dpure":
** src/drafts/ww3d/intersec.rs (the members the first pass translated: Set, Copy_Partial_Results, Copy_Results,
** Intersect_Sphere_Quick, Intersect_Sphere, Append_Object_Array, Intersect_Box, Intersect_Object_Array, Append_Hierarchy_Objects).
** (The scene-walking members are src/drafts/sweep_ww3d/intersec.rs, tested by tests/differential_draft_intersec.rs.)
**
** The REAL WW3D2/intersec.h is included (IntersectionClass, IntersectionResultClass; the inline members Set, Copy_*,
** Intersect_Sphere_Quick, Intersect_Sphere are compiled from it as they stand). From intersec.cpp, cut VERBATIM by build.rs
** (`drafts_ww3dpure`, anchors asserted once): the three static vectors, Append_Object_Array, Intersect_Box,
** Append_Hierarchy_Objects and both Intersect_Object_Array overloads. The only stand-in is RenderObjClass (incomplete in
** intersec.h; the real one is the scene graph): a scripted object whose Intersect, Intersect_Sphere_Quick, Get_Num_Sub_Objects,
** Get_Sub_Object and Release_Ref answer from the case and log an event (200 id Intersect, 202 id Intersect_Sphere_Quick, 201 id
** Release_Ref, which the draft has no counterpart for and the test drops). Intersect fills EVERY field of the result from the
** script (so that Copy_Results copies something in each); Intersect_Sphere_Quick sets Alpha, Beta and Intersects.
**
** Entry points (extern "C", tests/differential_draft_ixpure.rs), all uint32 words (floats by bits):
**   ref_dpw_ix_setup(in): [n, per object: id, quick, hit, range, alpha, beta, polygon, type, intersection 3, model_location 3,
**       matrix 12, nsubs, sub indices...]
**   ref_dpw_ix_run(in, out): in = [op, ray_loc 3, ray_dir 3, max_distance, convex_test, has_normal, normal 3, FINAL (26), SOURCE (26),
**       sphere center 3, radius, box min 3, box max 3, test_bounding_sphere, convex, count/object index, max_count, current_count,
**       current objects...]; returns the number of words written; out = ret, FINAL (26), normal 3, array count, array ids, log.
**   op 0 Intersect_Sphere_Quick, 1 Intersect_Sphere, 2 Copy_Partial_Results, 3 Copy_Results, 4 Append_Object_Array,
**   5 Intersect_Box, 6 Intersect_Object_Array (5 args), 7 Intersect_Object_Array (6 args), 8 Append_Hierarchy_Objects.
** A RESULT is 26 words: IntersectedRenderObject (object id + 1, 0 for NULL), IntersectedPolygon, ModelMatrix 12, ModelLocation 3,
**   Intersection 3, Range, Alpha, Beta, Intersects, CollisionType, IntersectionType. */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <assert.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "intersec.h"

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

static std::vector<W> g_log;

class RenderObjClass
{
public:
	int id;
	bool quick, hit;
	float range, alpha, beta;
	int polygon, type;
	Vector3 isect, mloc;
	float m[12];
	std::vector<RenderObjClass *> subs;

	bool Intersect_Sphere_Quick(IntersectionClass *ic, IntersectionResultClass *r)
	{
		(void)ic;
		g_log.push_back(202); g_log.push_back((W)id);
		r->Alpha = alpha; r->Beta = beta; r->Intersects = quick;
		return quick;
	}
	bool Intersect(IntersectionClass *ic, IntersectionResultClass *r)
	{
		(void)ic;
		g_log.push_back(200); g_log.push_back((W)id);
		r->IntersectedRenderObject = this;
		r->IntersectedPolygon = (POLYGONINDEX)polygon;
		r->ModelMatrix = Matrix3D(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
		r->ModelLocation = mloc;
		r->Intersection = isect;
		r->Range = range; r->Alpha = alpha; r->Beta = beta;
		r->Intersects = hit;
		r->IntersectionType = (IntersectionResultClass::INTERSECTION_TYPE)type;
		return hit;
	}
	int Get_Num_Sub_Objects(void) const { return (int)subs.size(); }
	RenderObjClass *Get_Sub_Object(int i) const { return subs[i]; }
	void Release_Ref(void) const { g_log.push_back(201); g_log.push_back((W)id); }
};

#include "ix_statics.inc"
#include "ix_append.inc"
#include "ix_box.inc"
#include "ix_hier.inc"
#include "ix_oa5.inc"
#include "ix_oa6.inc"

static std::vector<RenderObjClass *> g_objs;

extern "C" void ref_dpw_ix_setup(const W *in)
{
	for (size_t i = 0; i < g_objs.size(); i++) delete g_objs[i];
	g_objs.clear();
	const W *p = in;
	W n = *p++;
	std::vector<std::vector<W> > subs;
	for (W i = 0; i < n; i++) {
		RenderObjClass *o = new RenderObjClass();
		o->id = (int)p[0]; o->quick = p[1] != 0; o->hit = p[2] != 0;
		o->range = F(p[3]); o->alpha = F(p[4]); o->beta = F(p[5]);
		o->polygon = (int)p[6]; o->type = (int)p[7];
		o->isect = Vector3(F(p[8]), F(p[9]), F(p[10]));
		o->mloc = Vector3(F(p[11]), F(p[12]), F(p[13]));
		for (int k = 0; k < 12; k++) o->m[k] = F(p[14 + k]);
		W ns = p[26];
		std::vector<W> s;
		for (W k = 0; k < ns; k++) s.push_back(p[27 + k]);
		subs.push_back(s);
		p += 27 + ns;
		g_objs.push_back(o);
	}
	for (W i = 0; i < n; i++)
		for (size_t k = 0; k < subs[i].size(); k++) g_objs[i]->subs.push_back(g_objs[subs[i][k]]);
}

static const W *ReadResult(const W *p, IntersectionResultClass &r)
{
	r.IntersectedRenderObject = p[0] ? g_objs[p[0] - 1] : NULL;
	r.IntersectedPolygon = (POLYGONINDEX)p[1];
	r.ModelMatrix = Matrix3D(F(p[2]), F(p[3]), F(p[4]), F(p[5]), F(p[6]), F(p[7]), F(p[8]), F(p[9]), F(p[10]), F(p[11]), F(p[12]), F(p[13]));
	r.ModelLocation = Vector3(F(p[14]), F(p[15]), F(p[16]));
	r.Intersection = Vector3(F(p[17]), F(p[18]), F(p[19]));
	r.Range = F(p[20]); r.Alpha = F(p[21]); r.Beta = F(p[22]);
	r.Intersects = p[23] != 0;
	r.CollisionType = (int)p[24];
	r.IntersectionType = (IntersectionResultClass::INTERSECTION_TYPE)p[25];
	return p + 26;
}

static W *WriteResult(const IntersectionResultClass &r, W *o)
{
	*o++ = r.IntersectedRenderObject ? (W)(r.IntersectedRenderObject->id + 1) : 0;
	*o++ = r.IntersectedPolygon;
	for (int i = 0; i < 3; i++) for (int j = 0; j < 4; j++) *o++ = B(r.ModelMatrix[i][j]);
	*o++ = B(r.ModelLocation.X); *o++ = B(r.ModelLocation.Y); *o++ = B(r.ModelLocation.Z);
	*o++ = B(r.Intersection.X); *o++ = B(r.Intersection.Y); *o++ = B(r.Intersection.Z);
	*o++ = B(r.Range); *o++ = B(r.Alpha); *o++ = B(r.Beta);
	*o++ = r.Intersects;
	*o++ = (W)r.CollisionType;
	*o++ = (W)r.IntersectionType;
	return o;
}

extern "C" int ref_dpw_ix_run(const W *in, W *out)
{
	g_log.clear();
	int op = (int)in[0];
	Vector3 loc(F(in[1]), F(in[2]), F(in[3])), dir(F(in[4]), F(in[5]), F(in[6]));
	float maxd = F(in[7]);
	bool convex_test = in[8] != 0;
	bool has_normal = in[9] != 0;
	Vector3 normal(F(in[10]), F(in[11]), F(in[12]));
	IntersectionResultClass fin, src;
	ReadResult(in + 13, fin);
	ReadResult(in + 39, src);
	Vector3 center(F(in[65]), F(in[66]), F(in[67]));
	float radius = F(in[68]);
	Vector3 bmin(F(in[69]), F(in[70]), F(in[71])), bmax(F(in[72]), F(in[73]), F(in[74]));
	bool tbs = in[75] != 0, convex = in[76] != 0;
	int count = (int)in[77], maxcount = (int)in[78], cur = (int)in[79];
	IntersectionClass ic;
	ic.Set(&loc, &dir, has_normal ? &normal : NULL, false, maxd, convex_test);
	W *o = out;
	bool ret = false;
	std::vector<RenderObjClass *> arr(maxcount + 8 > count + 8 ? maxcount + 8 : count + 8, (RenderObjClass *)NULL);
	bool is_append = false;
	switch (op) {
	case 0: { SphereClass s(center, radius); ret = ic.Intersect_Sphere_Quick(s, &fin); break; }
	case 1: { SphereClass s(center, radius); ret = ic.Intersect_Sphere(s, &fin); break; }
	case 2: ic.Copy_Partial_Results(&fin, &src); break;
	case 3: ic.Copy_Results(&fin, &src); break;
	case 4: {
		is_append = true;
		for (int i = 0; i < cur; i++) arr[i] = g_objs[in[80 + i]];
		ic.Append_Object_Array(maxcount, cur, arr.data(), g_objs[count]);
		break;
	}
	case 5: ret = ic.Intersect_Box(bmin, bmax, &fin); break;
	case 6:
	case 7: {
		for (int i = 0; i < count; i++) arr[i] = g_objs[i];
		IntersectionResultClass temp[IntersectionClass::MAX_HIERARCHY_NODE_COUNT];
		if (op == 6) ret = ic.Intersect_Object_Array(count, arr.data(), &fin, tbs, convex);
		else ret = ic.Intersect_Object_Array(count, arr.data(), &fin, temp, tbs, convex);
		break;
	}
	case 8: {
		is_append = true;
		for (int i = 0; i < cur; i++) arr[i] = g_objs[in[80 + i]];
		ic.Append_Hierarchy_Objects(maxcount, cur, arr.data(), g_objs[count], tbs, convex);
		break;
	}
	}
	*o++ = ret;
	o = WriteResult(fin, o);
	*o++ = B(normal.X); *o++ = B(normal.Y); *o++ = B(normal.Z);
	*o++ = is_append ? (W)cur : 0;
	if (is_append) for (int i = 0; i < cur; i++) *o++ = (W)arr[i]->id;
	for (size_t i = 0; i < g_log.size(); i++) *o++ = g_log[i];
	return (int)(o - out);
}
