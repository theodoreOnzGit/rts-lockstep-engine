/* Differential-test wrapper for the ray effect draft (#215, upstream commit 0a05454d), src/drafts/logic_common/ray_effect.rs:
**   GeneralsMD/Code/GameEngine/Include/GameClient/RayEffect.h (read unchanged; `protected` read as `public`) and
**   GameEngine/Source/GameClient/System/RayEffect.cpp from `class RayEffectSystem *TheRayEffects = NULL;` to the end of the file
**   (findEntry, the constructor and destructor, init, reset, addRayEffect, deleteRayEffect, getRayEffectData), cut verbatim by
**   build.rs into ray_effect.inc.
**
** Stand-ins: SubsystemInterface (RayEffect.h's base class; its own header pulls in INI.h and the engine) is the one of
** differential/shim/Common; Drawable is an incomplete type, as in the header, and a drawable
** is the pointer made of its id; Coord3D is the shim's (BaseType.h's data members and zero()).
**
** Entry point (extern "C", tests/differential_draft_ray_effect.rs):
**   ref_ray_run(ops, nops, out): runs a sequence on one RayEffectSystem (constructed by the sequence's start) and writes words
**   (floats as bits) to out; returns their count. Each op is 8 words [kind, id, start xyz, end xyz] (3 + 3 floats as bits):
**     kind 0  addRayEffect(draw(id), &start, &end)            kind 1  deleteRayEffect(draw(id))
**     kind 2  getRayEffectData(draw(id), &data) with data pre-set to a sentinel -> id of data.draw, start xyz, end xyz
**     kind 3  reset()                                         kind 4  addRayEffect(NULL, ...)
**     kind 5  addRayEffect(draw(id), NULL, &end)              kind 6  addRayEffect(draw(id), &start, NULL)
**     kind 7  getRayEffectData(NULL, &data) with the sentinel -> as kind 2
**     kind 8  deleteRayEffect(NULL)
**   Every 16th op, and after the last, the whole table: 128 entries of [id, start xyz, end xyz]. */
#include "Lib/BaseType.h"
#define protected public
#include "GameClient/RayEffect.h"
#undef protected
#ifndef NULL
#define NULL 0
#endif
#include "ray_effect.inc"

namespace {
const Drawable *draw_of(unsigned id) { return (const Drawable *)(unsigned long)id; }
unsigned id_of(const Drawable *d) { return (unsigned)(unsigned long)d; }
unsigned bits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
float flt(unsigned u) { float f; memcpy(&f, &u, 4); return f; }
Coord3D coord(const unsigned *w) { Coord3D c; c.x = flt(w[0]); c.y = flt(w[1]); c.z = flt(w[2]); return c; }
void put_data(const RayEffectData &d, unsigned *out, int &n)
{
	out[n++] = id_of(d.draw);
	out[n++] = bits(d.startLoc.x); out[n++] = bits(d.startLoc.y); out[n++] = bits(d.startLoc.z);
	out[n++] = bits(d.endLoc.x); out[n++] = bits(d.endLoc.y); out[n++] = bits(d.endLoc.z);
}
void dump(RayEffectSystem &s, unsigned *out, int &n)
{
	for (int i = 0; i < 128; ++i) put_data(s.m_effectData[i], out, n);
}
}

extern "C" int ref_ray_run(const unsigned *ops, int nops, unsigned *out)
{
	RayEffectSystem sys;
	int n = 0;
	for (int k = 0; k < nops; ++k) {
		const unsigned *o = ops + 8 * k;
		Coord3D s = coord(o + 2), e = coord(o + 5);
		unsigned kind = o[0];
		if (kind == 0) sys.addRayEffect(draw_of(o[1]), &s, &e);
		else if (kind == 1) sys.deleteRayEffect(draw_of(o[1]));
		else if (kind == 2 || kind == 7) {
			RayEffectData d;
			d.draw = draw_of(0xDEAD);
			d.startLoc.x = 7; d.startLoc.y = 8; d.startLoc.z = 9;
			d.endLoc.x = 17; d.endLoc.y = 18; d.endLoc.z = 19;
			sys.getRayEffectData(kind == 2 ? draw_of(o[1]) : NULL, &d);
			put_data(d, out, n);
		}
		else if (kind == 3) sys.reset();
		else if (kind == 4) sys.addRayEffect(NULL, &s, &e);
		else if (kind == 5) sys.addRayEffect(draw_of(o[1]), NULL, &e);
		else if (kind == 6) sys.addRayEffect(draw_of(o[1]), &s, NULL);
		else if (kind == 8) sys.deleteRayEffect(NULL);
		if (k % 16 == 15) dump(sys, out, n);
	}
	dump(sys, out, n);
	return n;
}
