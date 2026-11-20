/* Differential-test wrapper for the draft src/drafts/ww3d/renderobjectrecycler.rs (GitHub #215, group "ww3d7"), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/renderobjectrecycler.h  the class (w7_recycler_h.inc)
**   renderobjectrecycler.cpp  from Reset to the end (w7_recycler_cpp.inc)
**   WWLib/multilist.h and multilist.cpp (the real RefRenderObjListClass = RefMultiListClass<RenderObjClass>): copied by build.rs, the one edit
**   is `this->` before the five uses of CurNode in PriorityMultiListIterator; the four list base classes are renamed W7* by -D.
** Stand-ins, each for something the files call outside themselves:
**   RenderObjClass: a scripted object (id, name, sub-objects, emitter flag, animation flag) deriving from MultiListObjectClass; Add_Ref and
**       Release_Ref do nothing (the draft does not model reference counts); Set_Transform, Set_Animation and ParticleEmitterClass::Reset log an event.
**   ParticleEmitterClass: RenderObjClass with a Reset that logs. Matrix3D: twelve floats. WW3DAssetManager::Create_Render_Obj: makes a model by a
**       fixed rule from the name (see `create`). stricmp: the CRT's, strcasecmp. assert/WWASSERT: compiled out (the release build).
** Entry point (extern "C", tests/differential_draft_ww3d7_recycler.rs): ref_dw7_recycler(in, nin, out, cap) returns the words written.
**   [n models, per model (name index, emitter, animation, n subs, subs...), n ops, per op (kind, a, 12 matrix words)] -> the event log
**   kinds: 0 Get_Render_Object(name index a, matrix), 1 Return_Render_Object(model a), 2 Reset()
**   events: 1 id m0..m11 Set_Transform, 2 id Reset (emitter), 3 id Set_Animation (restart); an op ends with 0xFFFFFFFE then, for a Get, the
**   result (id + 1, or 0 for NULL) */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <stdint.h>
#include <assert.h>
#include <vector>
#include <string>
#include "multilist.h"
#include "multilist.cpp"
#include "robjlist.h" /* the real one: RefRenderObjListClass, RefRenderObjListIterator */

#define stricmp strcasecmp
#define WWASSERT(x) ((void)0)
#define NULL_ANIM ((HAnimClass *)0)

typedef uint32_t W;
static std::vector<W> g_out;
static void put(W w) { g_out.push_back(w); }

class Matrix3D { public: float m[12]; };
class HAnimClass {};
class RenderObjClass;
class ParticleEmitterClass;

struct World {
	std::vector<RenderObjClass *> models;
	int next_name_dummy;
};
static World g_world;

static const char *const NAMES[] = { "Tank", "tank", "TANK", "Jeep", "jeep", "Bush", "xfail", "Xa", "", "tan", "tankk", "BUSH" };
static const int NNAMES = 12;

class RenderObjClass : public MultiListObjectClass {
public:
	enum { CLASSID_UNKNOWN = 0, CLASSID_PARTICLEEMITTER = 7 };
	enum { ANIM_MODE_ONCE = 1 };
	int id;
	int name;
	bool emitter, anim;
	std::vector<int> subs;
	const char *Get_Name(void) const { return NAMES[name]; }
	void Add_Ref(void) {}
	void Release_Ref(void) {}
	void Set_Transform(const Matrix3D &tm)
	{
		put(1); put((W)id);
		for (int k = 0; k < 12; ++k) { W w; memcpy(&w, &tm.m[k], 4); put(w); }
	}
	int Get_Num_Sub_Objects(void) const { return (int)subs.size(); }
	RenderObjClass *Get_Sub_Object(int i) const { return g_world.models[subs[i]]; }
	int Class_ID(void) const { return emitter ? CLASSID_PARTICLEEMITTER : CLASSID_UNKNOWN; }
	HAnimClass *Peek_Animation(void) const { static HAnimClass a; return anim ? &a : 0; }
	void Set_Animation(HAnimClass *, float, int) { put(3); put((W)id); }
};

class ParticleEmitterClass : public RenderObjClass {
public:
	void Reset(void) { put(2); put((W)id); }
};

/* WW3DAssetManager::Create_Render_Obj: NULL for a name starting with x or X; else a new model whose animation flag is (length even) and emitter flag
** (length divisible by 3), without sub-objects */
static RenderObjClass *create(const char *name)
{
	if (name[0] == 'x' || name[0] == 'X') return 0;
	RenderObjClass *m = new RenderObjClass;
	m->id = (int)g_world.models.size();
	m->name = -1;
	for (int k = 0; k < NNAMES; ++k) if (strcmp(NAMES[k], name) == 0) { m->name = k; break; }
	size_t len = strlen(name);
	m->anim = (len % 2) == 0;
	m->emitter = (len % 3) == 0;
	g_world.models.push_back(m);
	return m;
}
class WW3DAssetManager {
public:
	static WW3DAssetManager *Get_Instance(void) { static WW3DAssetManager a; return &a; }
	RenderObjClass *Create_Render_Obj(const char *name) { return create(name); }
};

#include "w7_recycler_h.inc"
#include "w7_recycler_cpp.inc"

extern "C" int ref_dw7_recycler(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	g_out.clear();
	for (size_t k = 0; k < g_world.models.size(); ++k) delete g_world.models[k];
	g_world.models.clear();
	int i = 0;
	W nm = in[i++];
	for (W k = 0; k < nm; ++k) {
		RenderObjClass *m = new RenderObjClass;
		m->id = (int)k;
		m->name = (int)in[i++];
		m->emitter = in[i++] != 0;
		m->anim = in[i++] != 0;
		W ns = in[i++];
		for (W s = 0; s < ns; ++s) m->subs.push_back((int)in[i++]);
		g_world.models.push_back(m);
	}
	{
		RenderObjectRecyclerClass rec;
		W nops = in[i++];
		for (W k = 0; k < nops; ++k) {
			W kind = in[i++], a = in[i++];
			Matrix3D tm;
			for (int j = 0; j < 12; ++j) { W w = in[i++]; memcpy(&tm.m[j], &w, 4); }
			if (kind == 0) {
				RenderObjClass *r = rec.Get_Render_Object(NAMES[a], tm);
				put(0xFFFFFFFEu);
				put(r ? (W)r->id + 1 : 0);
			} else if (kind == 1) {
				rec.Return_Render_Object(g_world.models[a % g_world.models.size()]);
				put(0xFFFFFFFEu);
			} else {
				rec.Reset();
				put(0xFFFFFFFEu);
			}
		}
	}
	if ((int)g_out.size() > cap) return -2;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
