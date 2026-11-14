/* Differential-test wrapper for an HLOD's particle emitters in a scene over
** frames: the scene's update order, the release of a complete emitter from its
** HLOD, and the buffers left drawing (GitHub #65, batch c; upstream commit
** 0a05454d):
**   WW3D2/hlod.cpp: HLodClass::Notify_Added, Notify_Removed, Remove_Sub_Object,
**     COMPILED from the verbatim w3d_gen/hlod_release_def.cpp (see build.rs);
**   WW3D2/hlod.h: ModelNodeClass, ModelArrayClass, verbatim
**     (w3d_gen/hlod_model_arrays.inc);
**   WW3D2/scene.cpp: SimpleSceneClass::Add_Render_Object, Remove_Render_Object,
**     Register, Unregister, Post_Render_Processing, COMPILED (same file);
**   WW3D2/part_emt.cpp, part_buf.cpp: the emitter and buffer (constructor,
**     destructor's Emitter_Is_Dead, Notify_Added, Notify_Removed,
**     On_Frame_Update, Emit, Render, Set_Hidden, Enable_Remove_On_Complete),
**     COMPILED from part_def.cpp (see wrap_w3d_part.cpp).
** TRANSCRIBED: the stand-ins of shim_w3d/hlod_release_prelude.h (the lists,
**   the scene's members, the HLOD's members and Free) and part_prelude.h; the
**   frame as SimpleSceneClass::Customized_Render and RTS3DScene::Customized_Render
**   run it: the visibility check first (here: the objects in the render list
**   before the frame are the visible ones; one added during the frame has
**   DEFAULT_BITS's clear visible bit), then the update list's loop, VERBATIM in
**   form:
**     RefRenderObjListIterator it(&UpdateList);
**     for (it.First(); !it.Is_Done(); it.Next()) { it.Peek_Obj()->On_Frame_Update(); }
**   then the visible buffers' Render; then, for a simple scene,
**   Post_Render_Processing (SceneClass::Render's last call; RTS3DScene::Render,
**   the world, never makes it). The HLOD's transform update
**   (Update_Sub_Object_Transforms with no hierarchy: each sub-object takes the
**   HLOD's transform). The HLOD's removal as W3DModelDraw::nukeCurrentRender
**   does it (Remove_Render_Object, then REF_PTR_RELEASE).
** ADDED (test harness, not upstream): the HLOD is built by hand (Add_Lod_Model's
**   and Add_Sub_Object_To_Bone's bookkeeping: the node added, the container
**   set; the creation reference handed over), each emitter shown after the
**   HLOD is added (Set_Hidden(0), as the Rust's update_emitters does); the
**   lists are emptied at the end.
**
** Input words: s3a, s3b, s4, create_sync, the scene (0 world, 1 simple), the
** emitter count n, how many are in Lod[0] (the rest are AdditionalModels), then
** per emitter the constructor's words as wrap_part_lod.cpp reads them, then the
** HLOD's transform (12); then operations: 0 a frame (dt, transform), 1 the HLOD
** removed, 2 Enable_Remove_On_Complete(k, on), 3 Set_Hidden(k, on) (k: the
** emitter's place among the sub-objects). Output words: 'C' then the state;
** per operation its code, a frame's draws and releases, then the state
** (log_state). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <float.h>
#include <vector>
#include <map>

#define private public
#define protected public
#include "part_emt_def.h"
#include "w3d_file.h"
#include "hlod_release_prelude.h"
#undef private
#undef protected

void ref_part_seed(unsigned s3a, unsigned s3b, unsigned s4); /* generated, part_def.cpp */
extern "C" void ref_part_lod_set_clock(unsigned sync, unsigned frame_time, unsigned frame_count); /* wrap_w3d_part.cpp */
extern int ref_part_lod_point_renders(void);
extern int ref_part_lod_line_group_renders(void);
extern int ref_part_lod_runs(std::vector<std::vector<Vector3> > *pts, std::vector<std::vector<Vector4> > *cols);
extern void ref_part_lod_clear_runs(void);

int g_hlod_bv_updates;

namespace {

struct In
{
	const unsigned *w;
	int n, i;
	unsigned u(void) { if (i >= n) abort(); return w[i++]; }
	int s(void) { return (int)u(); }
	float f(void) { unsigned x = u(); float r; memcpy(&r, &x, 4); return r; }
	Vector3 v3(void) { float a = f(), b = f(), c = f(); return Vector3(a, b, c); }
};

std::vector<unsigned> g_out;
void wu(unsigned x) { g_out.push_back(x); }
void wf(float f) { unsigned x; memcpy(&x, &f, 4); g_out.push_back(x); }
void wv3(const Vector3 &v) { wf(v.X); wf(v.Y); wf(v.Z); }
void wv4(const Vector4 &v) { wf(v.X); wf(v.Y); wf(v.Z); wf(v.W); }

/* the constructor's words (as wrap_part_lod.cpp) */
Vector3Randomizer *read_rand(In &in)
{
	unsigned cls = in.u();
	float a = in.f(), b = in.f(), c = in.f();
	switch (cls) {
	case 0: return new Vector3SolidBoxRandomizer(Vector3(a, b, c));
	case 1: return new Vector3SolidSphereRandomizer(a);
	case 2: return new Vector3HollowSphereRandomizer(a);
	default: return new Vector3SolidCylinderRandomizer(a, b);
	}
}
struct PropsF { ParticlePropertyStruct<float> p; std::vector<float> t, v; };
struct PropsV { ParticlePropertyStruct<Vector3> p; std::vector<float> t; std::vector<Vector3> v; };
void read_f(In &in, PropsF &o)
{
	o.p.Start = in.f(); o.p.Rand = in.f(); o.p.NumKeyFrames = in.u();
	for (unsigned k = 0; k < o.p.NumKeyFrames; ++k) { o.t.push_back(in.f()); o.v.push_back(in.f()); }
	o.p.KeyTimes = o.t.empty() ? NULL : &o.t[0];
	o.p.Values = o.v.empty() ? NULL : &o.v[0];
}
void read_v(In &in, PropsV &o)
{
	o.p.Start = in.v3(); o.p.Rand = in.v3(); o.p.NumKeyFrames = in.u();
	for (unsigned k = 0; k < o.p.NumKeyFrames; ++k) { o.t.push_back(in.f()); o.v.push_back(in.v3()); }
	o.p.KeyTimes = o.t.empty() ? NULL : &o.t[0];
	o.p.Values = o.v.empty() ? NULL : &o.v[0];
}
Matrix3D read_m(In &in)
{
	float m[12];
	for (int k = 0; k < 12; ++k) m[k] = in.f();
	Matrix3D r;
	r.Set(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11]);
	return r;
}

TextureClass g_rel_texture;

ParticleEmitterClass *read_emitter(In &in)
{
	float emit_rate = in.f();
	unsigned burst = in.u();
	Vector3Randomizer *pos_rnd = read_rand(in);
	Vector3 base_vel = in.v3();
	Vector3Randomizer *vel_rnd = read_rand(in);
	float out_vel = in.f(), inherit = in.f();
	PropsV color; PropsF opacity, size, rotation, frames, blur;
	read_v(in, color); read_f(in, opacity); read_f(in, size); read_f(in, rotation);
	float orient_rnd = in.f();
	read_f(in, frames); read_f(in, blur);
	Vector3 accel = in.v3();
	float max_age = in.f(), future_start = in.f();
	unsigned has_tex = in.u();
	ShaderClass shader(in.u());
	int max_particles = in.s(), max_buffer_size = in.s();
	bool pingpong = in.u() != 0;
	int render_mode = in.s(), frame_mode = in.s();
	unsigned has_lp = in.u();
	W3dEmitterLinePropertiesStruct lp;
	memset(&lp, 0, sizeof(lp));
	lp.Flags = in.u(); lp.SubdivisionLevel = in.u(); lp.NoiseAmplitude = in.f(); lp.MergeAbortFactor = in.f();
	lp.TextureTileFactor = in.f(); lp.UPerSec = in.f(); lp.VPerSec = in.f();
	return new ParticleEmitterClass(emit_rate, burst, pos_rnd, base_vel, vel_rnd, out_vel, inherit, color.p, opacity.p,
		size.p, rotation.p, orient_rnd, frames.p, blur.p, accel, max_age, future_start, has_tex ? &g_rel_texture : NULL, shader,
		max_particles, max_buffer_size, pingpong, render_mode, frame_mode, has_lp ? &lp : NULL);
}

/* what the buffer's renderer was given (as wrap_part_lod.cpp's log_render) */
void log_render(ParticleBufferClass *b)
{
	int pr = ref_part_lod_point_renders(), gr = ref_part_lod_line_group_renders();
	ref_part_lod_clear_runs();
	static char rinfo[16];
	b->Render(*reinterpret_cast<RenderInfoClass *>(rinfo));
	if (ref_part_lod_point_renders() != pr) {
		PointGroupClass *g = b->PointGroup;
		wu('P');
		wu(g->Mode == PointGroupClass::QUADS); wu(g->Frccl2);
		unsigned n = (unsigned)g->Count;
		wu(n);
		for (unsigned k = 0; k < n; ++k) {
			unsigned i = g->Apt ? g->Apt->Get_Array()[k] : k;
			wu(i);
			wv3(g->Locs->Get_Array()[i]);
			if (g->Diffuse) wv4(g->Diffuse->Get_Array()[i]);
			else { wv3(g->DefColor); wf(g->DefAlpha); }
			wf(g->Sizes ? g->Sizes->Get_Array()[i] : g->DefSize);
			wu(g->Orientations ? g->Orientations->Get_Array()[i] : g->DefOrientation);
			wu(g->Frames ? g->Frames->Get_Array()[i] : g->DefFrame);
		}
	} else if (ref_part_lod_line_group_renders() != gr) {
		LineGroupClass *g = b->LineGroup;
		wu('G');
		wu(g->Mode == LineGroupClass::PRISM);
		unsigned n = (unsigned)g->Count;
		wu(n);
		for (unsigned k = 0; k < n; ++k) {
			unsigned i = g->Alt ? g->Alt->Get_Array()[k] : k;
			wu(i);
			wv3(g->Head->Get_Array()[i]); wv3(g->Tail->Get_Array()[i]);
			if (g->Diffuse) wv4(g->Diffuse->Get_Array()[i]);
			else { wv3(g->DefColor); wf(g->DefAlpha); }
			wv4(g->TailDiffuse ? g->TailDiffuse->Get_Array()[i] : g->DefTail);
			wf(g->Sizes ? g->Sizes->Get_Array()[i] : g->DefSize);
			wf(g->UCoords ? g->UCoords->Get_Array()[i] : g->DefUCoord);
		}
	} else if (b->RenderMode == W3D_EMITTER_RENDER_MODE_LINE && b->LineRenderer) {
		std::vector<std::vector<Vector3> > pts;
		std::vector<std::vector<Vector4> > cols;
		ref_part_lod_runs(&pts, &cols);
		wu('L');
		wf(b->LineRenderer->Width);
		wu((unsigned)pts.size());
		for (size_t r = 0; r < pts.size(); ++r) {
			wu((unsigned)pts[r].size());
			for (size_t i = 0; i < pts[r].size(); ++i) { wv3(pts[r][i]); wv4(cols[r][i]); }
		}
	} else {
		wu('X');
	}
}

struct Run
{
	SimpleSceneClass scene;
	HLodClass *hlod;
	std::map<RenderObjClass *, unsigned> ids; /* each emitter and its buffer: the emitter's place when made */

	/* the live sub-objects, Lod[CurLod] then AdditionalModels */
	std::vector<ParticleEmitterClass *> subs(void)
	{
		std::vector<ParticleEmitterClass *> v;
		if (!hlod) return v;
		for (int m = 0; m < hlod->Lod[hlod->CurLod].Count(); ++m) v.push_back(static_cast<ParticleEmitterClass *>(hlod->Lod[hlod->CurLod][m].Model));
		for (int m = 0; m < hlod->AdditionalModels.Count(); ++m) v.push_back(static_cast<ParticleEmitterClass *>(hlod->AdditionalModels[m].Model));
		return v;
	}
	/* the buffers in the render list, by id */
	std::vector<std::pair<unsigned, ParticleBufferClass *> > buffers(void)
	{
		std::vector<std::pair<unsigned, ParticleBufferClass *> > v;
		for (std::list<RenderObjClass *>::iterator i = scene.RenderList.L.begin(); i != scene.RenderList.L.end(); ++i) {
			if (*i == hlod) continue;
			v.push_back(std::make_pair(ids[*i], static_cast<ParticleBufferClass *>(*i)));
		}
		std::sort(v.begin(), v.end());
		return v;
	}
	void log_release_list(void)
	{
		wu((unsigned)scene.ReleaseList.L.size());
		for (std::list<RenderObjClass *>::iterator i = scene.ReleaseList.L.begin(); i != scene.ReleaseList.L.end(); ++i) wu(ids[*i]);
	}
	/* the state after each operation: see state_words in tests/differential_hlod_release.rs */
	void log_state(void)
	{
		std::vector<ParticleEmitterClass *> s = subs();
		wu('S');
		wu((unsigned)s.size());
		for (size_t k = 0; k < s.size(); ++k) {
			ParticleEmitterClass *e = s[k];
			ParticleBufferClass *b = e->Buffer;
			wu(ids[e]);
			wu(e->Active); wu(e->IsComplete); wu(e->Is_In_Scene()); wu((unsigned)e->ParticlesLeft); wu(e->EmitRemain);
			wu(e->FirstTime); wu(e->BufferSceneNeeded); wu(e->RemoveOnComplete); wu(scene.ReleaseList.Contains(e));
			wu(b->Is_In_Scene()); wu((unsigned)b->NonNewNum); wu((unsigned)b->NewNum); wu(b->IsEmitterDead);
		}
		/* the orphans: buffers in the scene whose emitter is not a live sub-object */
		std::vector<std::pair<unsigned, ParticleBufferClass *> > bs = buffers();
		std::vector<std::pair<unsigned, ParticleBufferClass *> > orphans;
		for (size_t k = 0; k < bs.size(); ++k) {
			bool live = false;
			for (size_t j = 0; j < s.size(); ++j) live = live || s[j]->Buffer == bs[k].second;
			if (!live) orphans.push_back(bs[k]);
		}
		wu('O');
		wu((unsigned)orphans.size());
		for (size_t k = 0; k < orphans.size(); ++k) {
			ParticleBufferClass *b = orphans[k].second;
			wu(orphans[k].first);
			wu(b->Emitter != NULL);
			wu((unsigned)b->NonNewNum); wu((unsigned)b->NewNum); wu(b->IsEmitterDead); wu(b->Is_Complete());
			RenderObjClass *listed = b->Emitter ? static_cast<RenderObjClass *>(b->Emitter) : static_cast<RenderObjClass *>(b);
			wu(scene.ReleaseList.Contains(listed));
		}
		wu('R');
		log_release_list();
	}
};

} // namespace

extern "C" {

int ref_hlod_release_seq(const unsigned *input, int nin, unsigned *out, int cap)
{
	In in = { input, nin, 0 };
	g_out.clear();
	g_hlod_bv_updates = 0;
	for (int k = 0; k < 17; ++k) ParticleBufferClass::LODMaxScreenSizes[k] = NO_MAX_SCREEN_SIZE;
	unsigned s3a = in.u(), s3b = in.u(), s4 = in.u();
	ref_part_seed(s3a, s3b, s4);
	unsigned sync = in.u(), frame_count = 0;
	ref_part_lod_set_clock(sync, 0, 0);
	bool simple = in.u() != 0;
	unsigned n = in.u(), nlod = in.u();
	Run *r = new Run;
	HLodClass *hlod = new HLodClass;
	r->hlod = hlod;
	hlod->LodCount = 1;
	hlod->CurLod = 0;
	hlod->Lod = new HLodClass::ModelArrayClass[1];
	for (unsigned k = 0; k < n; ++k) {
		ParticleEmitterClass *e = read_emitter(in);
		r->ids[e] = k;
		r->ids[e->Peek_Buffer()] = k;
		/* Add_Lod_Model / Add_Sub_Object_To_Bone: the node, the container (the creation reference is the HLOD's) */
		HLodClass::ModelNodeClass node;
		node.Model = e;
		node.BoneIndex = 0;
		if (k < nlod) hlod->Lod[0].Add(node);
		else hlod->AdditionalModels.Add(node);
		e->Set_Container(hlod);
	}
	Matrix3D tm = read_m(in);
	std::vector<ParticleEmitterClass *> s0 = r->subs();
	for (size_t k = 0; k < s0.size(); ++k) s0[k]->Set_Transform(tm);
	r->scene.Add_Render_Object(hlod);
	for (size_t k = 0; k < s0.size(); ++k) s0[k]->Set_Hidden(0);
	wu('C');
	r->log_state();
	while (in.i < in.n) {
		unsigned op = in.u();
		wu(op);
		switch (op) {
		case 0: { /* a frame */
			unsigned dt = in.u();
			sync += dt;
			frame_count++;
			ref_part_lod_set_clock(sync, dt, frame_count);
			Matrix3D m = read_m(in);
			std::vector<ParticleEmitterClass *> s = r->subs();
			for (size_t k = 0; k < s.size(); ++k) s[k]->Set_Transform(m);
			/* the visibility check: what is in the render list now */
			std::vector<std::pair<unsigned, ParticleBufferClass *> > visible = r->buffers();
			/* the update list's loop (SimpleSceneClass::Customized_Render) */
			RefRenderObjListIterator it(&r->scene.UpdateList);
			for (it.First(); !it.Is_Done(); it.Next()) {
				it.Peek_Obj()->On_Frame_Update();
			}
			/* the visible buffers render */
			wu('D');
			wu((unsigned)visible.size());
			for (size_t k = 0; k < visible.size(); ++k) {
				wu(visible[k].first);
				log_render(visible[k].second);
			}
			wu('P');
			if (simple) {
				r->log_release_list();
				static char rinfo[16];
				r->scene.Post_Render_Processing(*reinterpret_cast<RenderInfoClass *>(rinfo));
			} else {
				wu(0);
			}
			break;
		}
		case 1: /* the HLOD removed and released (W3DModelDraw::nukeCurrentRender) */
			r->scene.Remove_Render_Object(hlod);
			r->hlod = NULL;
			hlod->Release_Ref();
			hlod = NULL;
			break;
		case 2: { unsigned k = in.u(); unsigned on = in.u(); r->subs()[k]->Enable_Remove_On_Complete(on != 0); break; }
		case 3: { unsigned k = in.u(); unsigned on = in.u(); r->subs()[k]->Set_Hidden(on); break; }
		default: abort();
		}
		r->log_state();
	}
	/* ADDED: the lists emptied */
	if (hlod) {
		r->scene.Remove_Render_Object(hlod);
		hlod->Release_Ref();
		r->hlod = NULL;
	}
	while (!r->scene.ReleaseList.Is_Empty()) r->scene.ReleaseList.Release_Head();
	while (!r->scene.RenderList.Is_Empty()) r->scene.Remove_Render_Object(r->scene.RenderList.L.front());
	while (!r->scene.UpdateList.Is_Empty()) r->scene.UpdateList.Release_Head();
	delete r;
	int nw = (int)g_out.size();
	if (nw <= cap && nw > 0) memcpy(out, &g_out[0], nw * 4);
	return nw;
}

} /* extern "C" */
