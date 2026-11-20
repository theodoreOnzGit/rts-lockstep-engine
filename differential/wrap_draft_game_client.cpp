/* Differential-test wrapper for the game client draft (#215, group lc3, upstream commit 0a05454d), src/drafts/sweep_common_client/game_client.rs. Cut verbatim by build.rs:
**   gc_text.inc  GameClient.cpp  addTextBearingDrawable and flushTextBearingDrawables (to evaluateContextCommand);
**   gc_ray.inc   GameClient.cpp  removeFromRayEffects (to releaseShadows);
**   lc3_ray_effect.inc RayEffect.cpp from `class RayEffectSystem *TheRayEffects = NULL;` to the end, with RayEffect.h read unchanged (`protected` as `public`),
**   as in wrap_draft_ray_effect.cpp: the ray effect system `removeFromRayEffects` calls.
**
** Stand-ins (outside the cuts): `Drawable` (complete here; RayEffect.h only declares it): an object whose drawUIText() logs its id; a drawable is &g_draws[id];
** class GameClient with only m_textBearingDrawableList (a std::list of Drawable*, as GameClient.h declares it) and the three functions; SubsystemInterface and
** Coord3D are the shim's.
**
** Entry point ref_lc3_gc_run(prog, nprog, out). Ops (4 words [kind, id, x, y]): 0 addTextBearingDrawable(draw id) (id 0 is NULL); 1 flushTextBearingDrawables;
** 2 addRayEffect(draw id, start (x,0,0), end (y,0,0)); 3 removeFromRayEffects(draw id) (0 NULL).
** After each op: the log (count, ids drawn), the length of the text list, and the ray table's 128 (id, start.x) pairs. */
#include "Lib/BaseType.h"
#define protected public
#include "GameClient/RayEffect.h"
#undef protected
#include <list>
#include <vector>
#ifndef NULL
#define NULL 0
#endif
#include "lc3_ray_effect.inc"

std::vector<int> g_log;

class Drawable
{
public:
	int id;
	void drawUIText() { g_log.push_back(id); }
};
Drawable g_draws[64];

namespace {


typedef std::list<Drawable *> TextBearingDrawableList;
typedef TextBearingDrawableList::iterator TextBearingDrawableListIterator;

class GameClient
{
public:
	TextBearingDrawableList m_textBearingDrawableList;
	void addTextBearingDrawable(Drawable *tbd);
	void flushTextBearingDrawables(void);
	void removeFromRayEffects(Drawable *draw);
};

#include "gc_text.inc"
#include "gc_ray.inc"

int idOf(const Drawable *d) { return d ? (int)(d - g_draws) : 0; }

}

extern "C" int ref_lc3_gc_run(const int *p, int np, int *out)
{
	for (int i = 0; i < 64; ++i) g_draws[i].id = i;
	TheRayEffects = new RayEffectSystem;
	GameClient gc;
	int n = 0;
	for (int k = 0; k + 4 <= np; k += 4) {
		int kind = p[k], id = p[k + 1];
		Drawable *d = id ? &g_draws[id] : NULL;
		g_log.clear();
		if (kind == 0) gc.addTextBearingDrawable(d);
		else if (kind == 1) gc.flushTextBearingDrawables();
		else if (kind == 2) {
			Coord3D s, e;
			s.x = (float)p[k + 2]; s.y = 0; s.z = 0;
			e.x = (float)p[k + 3]; e.y = 0; e.z = 0;
			TheRayEffects->addRayEffect(d, &s, &e);
		}
		else gc.removeFromRayEffects(d);
		out[n++] = (int)g_log.size();
		for (size_t i = 0; i < g_log.size(); ++i) out[n++] = g_log[i];
		out[n++] = (int)gc.m_textBearingDrawableList.size();
		for (int i = 0; i < 128; ++i) {
			out[n++] = idOf(TheRayEffects->m_effectData[i].draw);
			out[n++] = (int)TheRayEffects->m_effectData[i].startLoc.x;
		}
	}
	delete TheRayEffects;
	TheRayEffects = NULL;
	return n;
}
