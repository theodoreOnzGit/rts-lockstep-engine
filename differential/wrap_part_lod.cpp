/* Differential-test wrapper for the particle buffer's predictive LOD and the
** emitter's release and removal (GitHub #65, batch b; upstream commit
** 0a05454d):
**   WW3D2/part_buf.cpp: ParticleBufferClass::Prepare_LOD, Increment_LOD,
**     Decrement_LOD, Get_Cost, Get_Value, Get_Post_Increment_Value,
**     Set_LOD_Level, Get_LOD_Level, Get_LOD_Count, Calculate_Cost_Value_Arrays,
**     Set_LOD_Max_Screen_Size, Get_LOD_Max_Screen_Size, the constructor's cost
**     and value arrays, On_Frame_Update, Notify_Added, Notify_Removed, Render
**     (with its LOD decimation: Generate_APT, Render_Line's unrolling, the
**     visual update skipped when the whole buffer is decimated), COMPILED from
**     the verbatim part_def.cpp (see build.rs and wrap_w3d_part.cpp);
**   WW3D2/part_buf.h: Set_LOD_Bias (inline), verbatim (part_emt_def.h);
**   WW3D2/part_emt.cpp / part_emt.h: the constructor, the destructor
**     (Emitter_Is_Dead), On_Frame_Update (its RELEASE registration), Set_LOD_Bias,
**     Remove_Buffer_From_Scene, Buffer_Scene_Not_Needed,
**     Enable_Remove_On_Complete, Set_Hidden, Set_Visible, Set_Animation_Hidden,
**     Set_Invisible, Stop, Start, Reset, Scale, Set_One_Time_Burst,
**     Set_Emission_Rate, Notify_Added, Notify_Removed, Emit, COMPILED (same
**     library).
** TRANSCRIBED (the stand-ins of shim_w3d/part_prelude.h): the camera Prepare_LOD
**   reads, PredictiveLODOptimizerClass::Add_Object (here: records the cost it
**   takes, Get_Cost()), the scene (Register counts and lists RELEASE
**   registrations; this driver then removes each listed object, as
**   SimpleSceneClass::Post_Render_Processing does for an object with no
**   container), RenderObjClass's bits, LOD defaults and bounding-volume cache.
** ADDED (test harness, not upstream): the static LODMaxScreenSizes is reset to
**   NO_MAX_SCREEN_SIZE at the start of each sequence (it is a process global
**   upstream; the Rust holds it per caller); the emitter is "destroyed" by
**   taking a reference to its buffer (the scene's, upstream) and releasing the
**   emitter.
**
** Input words (floats as bits): s3a, s3b, s4 (the two generators' seeds),
** create_sync, then the constructor's arguments: emit_rate, burst_size, the
** position randomizer (class 0..3, three floats), base_vel (3), the velocity
** randomizer (class, three floats), out_vel, vel_inherit, the colour, opacity,
** size and rotation properties, orient_rnd, the frame and blur-time properties
** (each: start (3 floats for colour, else 1), rand (likewise), the key count,
** then per key its time and value), accel (3), max_age, future_start, has
** texture, shader bits, max_particles, max_buffer_size, pingpong, render_mode,
** frame_mode, has line properties and their 7 words; the first transform (12);
** then operations, each its code and arguments (see the switch). Output words:
** 'C' then the state; per operation its code, its own words, then the state
** (log_state). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <float.h>
#include <vector>

#define private public
#define protected public
#include "part_emt_def.h"
#include "w3d_file.h"
#undef private
#undef protected

void ref_part_seed(unsigned s3a, unsigned s3b, unsigned s4); /* generated, part_def.cpp */

/* ---- the prelude's globals ---------------------------------------------- */
int g_part_release_count;
static std::vector<RenderObjClass *> g_release_list;
static std::vector<float> g_plo_costs;
static bool g_undefined;
/* GUARD (test harness): would a LOD array read past its end? The Rust reports these as undefined and the input stops
** before them, so with a correct translation the guard never fires; under a planted bug the two can disagree, and the
** guard turns the C++'s undefined read into a 'U' word (a mismatch) instead of a crash. */
static bool lod_read_undefined(const ParticleBufferClass *b, unsigned extra, unsigned size)
{
	return (b->LodCount - 1 + extra) - b->DecimationThreshold >= size;
}
void PredictiveLODOptimizerClass::Add_Object(RenderObjClass *robj)
{
	/* predlod.cpp: the object is listed and its cost added to the total (only buffers are added here) */
	if (lod_read_undefined(static_cast<ParticleBufferClass *>(robj), 0, 17)) { g_undefined = true; return; }
	g_plo_costs.push_back(robj->Get_Cost());
}

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

/* The state after each operation: see state_words in tests/differential_part_lod.rs. */
void log_state(ParticleEmitterClass *e, ParticleBufferClass *b)
{
	wu(e != NULL);
	if (e) {
		wu(e->EmitRate); wu(e->BurstSize); wu(e->OneTimeBurstSize); wu(e->OneTimeBurst);
		wv3(e->BaseVel); wf(e->OutwardVel); wf(e->VelInheritFactor);
		wu(e->EmitRemain); wf(e->PrevQ.X); wf(e->PrevQ.Y); wf(e->PrevQ.Z); wf(e->PrevQ.W); wv3(e->PrevOrig);
		wu(e->Active); wu(e->FirstTime); wu(e->BufferSceneNeeded); wu((unsigned)e->ParticlesLeft); wu(e->IsComplete);
		wu(e->RemoveOnComplete); wu(e->IsInScene); wu(e->GroupID); wu(e->IsInvisible);
		wu(e->Hidden); wu(e->AnimHidden); wu(e->Visible); wu(e->Is_Not_Hidden_At_All());
	}
	wu(b->Is_In_Scene()); wu(b->Hidden); wu(b->AnimHidden); wu(b->Visible);
	wu(b->NewParticleQueueStart); wu(b->NewParticleQueueEnd); wu((unsigned)b->NewParticleQueueCount);
	for (int k = 0, q = b->NewParticleQueueStart; k < b->NewParticleQueueCount; ++k) {
		const NewParticleStruct &np = b->NewParticleQueue[q];
		wv3(np.Position); wv3(np.Velocity); wu(np.TimeStamp); wu(np.GroupID);
		if (++q == (int)b->MaxNum) q = 0;
	}
	wu(b->RenderMode); wv3(b->Accel); wu(b->HasAccel); wu(b->MaxAge); wu(b->LastUpdateTime); wu(b->IsEmitterDead); wf(b->MaxSize);
	wu(b->MaxNum); wu(b->Start); wu(b->End); wu(b->NewEnd); wu((unsigned)b->NonNewNum); wu((unsigned)b->NewNum);
	wv3(b->BoundingBox.Center); wv3(b->BoundingBox.Extent); wu(b->BoundingBoxDirty);
	wf(b->SizeRandom);
	wu(b->Emitter != NULL); wu(b->CurrentGroupID);
	/* the ring's particles (both subranges, as every loop of part_buf.cpp walks them) */
	unsigned sub1_end, sub2_start;
	if ((b->Start < b->End) || ((b->Start == b->End) && b->NonNewNum == 0)) { sub1_end = b->End; sub2_start = b->End; }
	else { sub1_end = b->MaxNum; sub2_start = 0; }
	int pp = b->PingPongPosition ? (WW3D::Get_Frame_Count() & 0x1) : 0;
	for (int r = 0; r < 2; ++r) {
		unsigned i0 = r ? sub2_start : b->Start, i1 = r ? b->End : sub1_end;
		for (unsigned i = i0; i < i1; ++i) {
			wv3(b->Position[pp]->Get_Array()[i]); wv3(b->Velocity[i]); wu(b->TimeStamp[i]); wu(b->GroupID->Get_Array()[i]);
		}
	}
	wu(b->DecimationThreshold); wu(b->LodCount); wf(b->LodBias); wf(b->ProjectedArea);
	for (int k = 0; k < 17; ++k) wf(b->Cost[k]);
	for (int k = 0; k < 18; ++k) wf(b->Value[k]);
	wu(b->BVValid);
	if (b->BVValid) { wv3(b->CachedBoundingSphere.Center); wf(b->CachedBoundingSphere.Radius); }
	if (b->LineRenderer) { wf(b->LineRenderer->Width); wf(b->LineRenderer->Props.NoiseAmplitude); }
	wu(g_part_release_count);
}

} // namespace

/* the stand-in renderers' Render are wrap_w3d_part.cpp's (they count and record) */
extern int ref_part_lod_point_renders(void);
extern int ref_part_lod_line_group_renders(void);
extern int ref_part_lod_runs(std::vector<std::vector<Vector3> > *pts, std::vector<std::vector<Vector4> > *cols);
extern void ref_part_lod_clear_runs(void);

namespace {

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

/* SimpleSceneClass::Post_Render_Processing for objects with no container */
void post_render(void)
{
	for (size_t k = 0; k < g_release_list.size(); ++k) g_release_list[k]->Remove();
	g_release_list.clear();
}

TextureClass g_lod_texture;

} // namespace

void ref_part_lod_note_release(RenderObjClass *obj) { g_release_list.push_back(obj); }

extern "C" {

void ref_part_lod_set_clock(unsigned sync, unsigned frame_time, unsigned frame_count); /* wrap_w3d_part.cpp */

int ref_part_lod_seq(const unsigned *input, int nin, unsigned *out, int cap)
{
	In in = { input, nin, 0 };
	g_out.clear();
	g_release_list.clear();
	g_plo_costs.clear();
	g_undefined = false;
	g_part_release_count = 0;
	for (int k = 0; k < 17; ++k) ParticleBufferClass::LODMaxScreenSizes[k] = NO_MAX_SCREEN_SIZE;
	unsigned s3a = in.u(), s3b = in.u(), s4 = in.u();
	ref_part_seed(s3a, s3b, s4);
	unsigned sync = in.u(), frame_count = 0;
	ref_part_lod_set_clock(sync, 0, 0);
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
	ParticleEmitterClass *e = new ParticleEmitterClass(emit_rate, burst, pos_rnd, base_vel, vel_rnd, out_vel, inherit, color.p, opacity.p,
		size.p, rotation.p, orient_rnd, frames.p, blur.p, accel, max_age, future_start, has_tex ? &g_lod_texture : NULL, shader,
		max_particles, max_buffer_size, pingpong, render_mode, frame_mode, has_lp ? &lp : NULL);
	ParticleBufferClass *b = e->Peek_Buffer();
	SceneClass scene;
	e->Set_Transform(read_m(in));
	e->Notify_Added(&scene);
	e->Set_Hidden(0);
	wu('C');
	log_state(e, b);
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
			if (e) e->Set_Transform(m);
			int before = g_part_release_count;
			/* only what is in the scene is registered for ON_FRAME_UPDATE (Notify_Added / Notify_Removed) */
			if (e && e->Is_In_Scene()) e->On_Frame_Update();
			if (b->Is_In_Scene()) {
				b->On_Frame_Update();
				log_render(b);
			} else {
				wu('N');
			}
			wu((unsigned)(g_part_release_count - before));
			post_render();
			break;
		}
		case 1: { /* Prepare_LOD */
			CameraClass cam;
			cam.Position = in.v3();
			cam.Viewport.Min.X = in.f(); cam.Viewport.Min.Y = in.f(); cam.Viewport.Max.X = in.f(); cam.Viewport.Max.Y = in.f();
			cam.ViewPlaneMin.X = in.f(); cam.ViewPlaneMin.Y = in.f(); cam.ViewPlaneMax.X = in.f(); cam.ViewPlaneMax.Y = in.f();
			size_t n0 = g_plo_costs.size();
			b->Prepare_LOD(cam);
			if (g_undefined) { wu('U'); goto done; }
			wu(g_plo_costs.size() != n0);
			if (g_plo_costs.size() != n0) wf(g_plo_costs.back());
			break;
		}
		case 2: b->Increment_LOD(); break;
		case 3: b->Decrement_LOD(); break;
		case 4: b->Set_LOD_Level(in.s()); break;
		case 5: { float f = in.f(); if (e) e->Set_LOD_Bias(f); else b->Set_LOD_Bias(f); break; }
		case 6: { int l = in.s(); float f = in.f(); if (l == 17) { wu('U'); goto done; } ParticleBufferClass::Set_LOD_Max_Screen_Size(l, f); break; }
		case 7: {
			int l = in.s();
			if (l == 17 || lod_read_undefined(b, 0, 17) || lod_read_undefined(b, 0, 18) || lod_read_undefined(b, 1, 18)) { wu('U'); goto done; }
			wf(b->Get_Cost()); wf(b->Get_Value()); wf(b->Get_Post_Increment_Value()); wu((unsigned)b->Get_LOD_Level());
			wu((unsigned)b->Get_LOD_Count()); wf(ParticleBufferClass::Get_LOD_Max_Screen_Size(l));
			if (e) { wf(e->Get_Cost()); wf(e->Get_Value()); wf(e->Get_Post_Increment_Value()); wu((unsigned)e->Get_LOD_Level()); wu((unsigned)e->Get_LOD_Count()); }
			break;
		}
		case 8: e->Set_Hidden(in.s()); break;
		case 9: e->Set_Visible(in.s()); break;
		case 10: e->Set_Animation_Hidden(in.s()); break;
		case 11: e->Set_Invisible(in.u() != 0); break;
		case 12: e->Stop(); break;
		case 13: e->Start(); break;
		case 14: e->Reset(); break;
		case 15: e->Remove_Buffer_From_Scene(); break;
		case 16: e->Remove(); break;
		case 17: scene.Add_Render_Object(e); break;
		case 18: /* the emitter destroyed; the scene keeps the buffer */
			b->Add_Ref();
			e->Release_Ref();
			e = NULL;
			break;
		case 19: b->Set_Hidden(in.s()); break;
		case 20: e->Scale(in.f()); break;
		case 21: e->Set_One_Time_Burst(in.s()); break;
		case 22: e->Enable_Remove_On_Complete(in.u() != 0); break;
		case 23: e->Buffer_Scene_Not_Needed(); break;
		case 24: e->Set_Emission_Rate(in.f()); break;
		default: abort();
		}
		log_state(e, b);
	}
done:
	if (e) e->Release_Ref();
	else b->Release_Ref();
	int n = (int)g_out.size();
	if (n <= cap && n > 0) memcpy(out, &g_out[0], n * 4);
	return n;
}

} /* extern "C" */
