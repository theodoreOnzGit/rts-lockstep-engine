/* Differential-test wrapper for the W3D particle emitter render object
** (GitHub #49; upstream commit 0a05454d):
**   WW3D2/part_emt.cpp: ParticleEmitterClass from the global
**     InheritedWorldSpaceEmitterVel to Initialize_Particle (constructors,
**     Create_From_Definition, Clone, Restart, Notify_Added, Notify_Removed,
**     Scale, On_Frame_Update, Reset, Start, Stop, the randomizer and
**     velocity setters, Emit, Update_Cached_Bounding_Volumes,
**     Create_New_Particles, Initialize_Particle) and Set_Name,
**     Update_On_Visibilty, COMPILED from verbatim segments;
**   WW3D2/part_buf.cpp: ParticleBufferClass from PermutationArray to ~~the
**     comment of Prepare_LOD~~ the end (corrected 2026-11-14, #65 batch b:
**     Prepare_LOD included) (the statics, constructors, destructor, Render,
**     Generate_APT, Combine_Color_And_Alpha, Render_Particles, Render_Line,
**     Render_Line_Group, Scale, On_Frame_Update, the bounding volumes) and
**     from Increment_LOD to the end (the LOD accessors, Reset_Colors ...
**     Reset_Blur_Times, Emitter_Is_Dead, Set_Emitter,
**     Add_Uninitialized_New_Particle, Update_Kinematic_Particle_State,
**     Update_Visual_Particle_State, Update_Bounding_Box, Get_New_Particles,
**     Kill_Old_Particles, Update_Non_New_Particles, the keyframe getters, the
**     line accessors, Determine_Tail_Diffuse, Get_Texture, Set_Texture,
**     Get_Shader), COMPILED from verbatim segments;
**   the class bodies of part_buf.h and part_emt.h, verbatim
**     (w3d_gen/part_emt_def.h);
**   WW3D2/shader.cpp's Enable_Fog and Report_Unable_To_Fog, WWMath/vp.cpp's
**     three Vector4 Copy overloads and Clamp, verbatim segments;
**     WWMath/pot.cpp and WWLib/gcd_lcm.cpp unchanged; with the emitter
**     definition loader of wrap_w3d_emit.cpp (part_ldr.cpp), v3_rnd.cpp and
**     random.cpp in the same library, and quat.cpp (upstream_w3d),
**     wwmath.cpp and matrix3d.cpp (upstream_wwmath) linked after it.
** See build.rs for the segments (each marker asserted to occur once).
**
** Only the update half of the render side runs: the point group, segment
** line renderer and line group are stand-ins that record what
** ParticleBufferClass hands them (shim_w3d/part_prelude.h); their own
** Render needs the DX8 device and is not compiled.
**
** TRANSCRIBED (stand-ins), never called by the driver below:
**   ~~ParticleBufferClass::Prepare_LOD (needs the camera and the predictive LOD
**   optimizer),~~ (corrected 2026-11-14, #65 batch b: Prepare_LOD is compiled
**   from part_buf.cpp, over the prelude's camera stand-in, and driven by
**   wrap_part_lod.cpp) ParticleEmitterClass::Build_Definition, Save and
**   Add_Dependencies_To_List (the definition's save side and the file
**   list), ShaderClass::Guess_Sort_Level (Render reaches it only when
**   sorting is disabled; the WW3D stand-in reports it enabled).
** TRANSCRIBED: WW3DAssetManager::Get_Texture(name, mips, format) (renamed
**   EmitAssetShim): a stand-in texture for a non-empty name (the real one
**   needs the renderer); RenderObjClass::AT_MIN_LOD and AT_MAX_LOD
**   (rendobj.cpp's values).
**
** The driver (ref_w3d_part_run): the definition loaded from the bytes
** (ParticleEmitterDefClass::Load_W3D), the two random generators seeded
** (ref_part_seed, generated after the segments), the clock set, then
** Create_From_Definition, Set_Transform, Notify_Added and Set_Hidden(0),
** and for each frame: the clock, Set_Transform, the frame's operation
** (Set_Hidden, Set_One_Time_Burst, Reset), the emitter's On_Frame_Update,
** and while its buffer is in the scene the buffer's On_Frame_Update (which
** calls Emit) and Render. tests/differential_w3d.rs (section 16) drives the
** Rust the same way and reproduces the log. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

#define private public
#define protected public
#include "part_emt_def.h"
#include "part_ldr_def.h"
#include "w3d_file.h"
#include "chunkio.h"
#undef private
#undef protected
#include "assetmgr.h" /* the prelude's stand-in, renamed EmitAssetShim */

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}
void ref_part_seed(unsigned s3a, unsigned s3b, unsigned s4); /* generated, part_def.cpp */

/* ---- the clock (WW3D stand-in) ------------------------------------------ */
static unsigned g_sync, g_frame_time, g_frame_count;
unsigned int WW3D::Get_Sync_Time(void) { return g_sync; }
unsigned int WW3D::Get_Frame_Time(void) { return g_frame_time; }
unsigned int WW3D::Get_Frame_Count(void) { return g_frame_count; }

const float RenderObjClass::AT_MIN_LOD = FLT_MAX;
const float RenderObjClass::AT_MAX_LOD = -1.0f;

/* ---- what the renderers were given ------------------------------------- */
static int g_point_renders, g_line_group_renders;
struct LineRunRec { std::vector<Vector3> pts; std::vector<Vector4> cols; };
static std::vector<LineRunRec> g_runs;
void PointGroupClass::Render(RenderInfoClass &) { g_point_renders++; }
void LineGroupClass::Render(RenderInfoClass &) { g_line_group_renders++; }
void SegLineRendererClass::Render(RenderInfoClass &, const Matrix3D &, unsigned int point_count, Vector3 *points, const SphereClass &, Vector4 *rgbas)
{
	LineRunRec r;
	r.pts.assign(points, points + point_count);
	r.cols.assign(rgbas, rgbas + point_count);
	g_runs.push_back(r);
}

/* ---- #65 (batch b): what wrap_part_lod.cpp reads of the above ----------- */
int ref_part_lod_point_renders(void) { return g_point_renders; }
int ref_part_lod_line_group_renders(void) { return g_line_group_renders; }
int ref_part_lod_runs(std::vector<std::vector<Vector3> > *pts, std::vector<std::vector<Vector4> > *cols)
{
	for (size_t r = 0; r < g_runs.size(); ++r) { pts->push_back(g_runs[r].pts); cols->push_back(g_runs[r].cols); }
	return (int)g_runs.size();
}
void ref_part_lod_clear_runs(void) { g_runs.clear(); }
extern "C" void ref_part_lod_set_clock(unsigned sync, unsigned frame_time, unsigned frame_count)
{
	g_sync = sync;
	g_frame_time = frame_time;
	g_frame_count = frame_count;
}

/* ---- never called ------------------------------------------------------- */
ParticleEmitterDefClass *ParticleEmitterClass::Build_Definition(void) const { abort(); }
WW3DErrorType ParticleEmitterClass::Save(ChunkSaveClass &) const { abort(); }
void ParticleEmitterClass::Add_Dependencies_To_List(DynamicVectorClass<StringClass> &, bool) { abort(); }
int ShaderClass::Guess_Sort_Level(void) const { abort(); }

/* ---- the texture lookup of Create_From_Definition ----------------------- */
static TextureClass g_part_texture;
TextureClass *WW3DAssetManager::Get_Texture(const char *filename, int, int)
{
	if ((filename == NULL) || (strlen(filename) == 0)) return NULL;
	g_part_texture.Format = WW3D_FORMAT_A8R8G8B8;
	return &g_part_texture;
}

namespace {

/* The same file as wrap_w3d.cpp: GameFileClass's Read and Seek over RAMFile. */
class TestFile : public FileClass
{
public:
	TestFile(const char *data, int len) : m_theFile(ref_w3d_ramfile_open(data, len)) {}
	~TestFile() { ref_w3d_ramfile_close(m_theFile); }
	virtual char const *File_Name(void) const { return "test.w3d"; }
	virtual char const *Set_Name(char const *) { return "test.w3d"; }
	virtual int Create(void) { abort(); }
	virtual int Delete(void) { abort(); }
	virtual bool Is_Available(int = false) { return true; }
	virtual bool Is_Open(void) const { return true; }
	virtual int Open(char const *, int = READ) { return 1; }
	virtual int Open(int = READ) { return 1; }
	virtual int Read(void *buffer, int len) { return ref_w3d_ramfile_read(m_theFile, buffer, len); }
	virtual int Seek(int pos, int dir = SEEK_CUR)
	{
		int mode = 1;
		switch (dir) {
		default:
		case SEEK_CUR: mode = 1; break;
		case SEEK_SET: mode = 0; break;
		case SEEK_END: mode = 2; break;
		}
		return ref_w3d_ramfile_seek(m_theFile, pos, mode);
	}
	virtual int Size(void) { return ref_w3d_ramfile_size(m_theFile); }
	virtual int Write(void const *, int) { abort(); }
	virtual void Close(void) {}
private:
	void *m_theFile;
};

std::vector<unsigned char> g_log;
void put(const void *p, size_t n) { g_log.insert(g_log.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_log.push_back((unsigned char)v); }
void put32(uint32_t v) { put(&v, 4); }
void putF(float f) { put(&f, 4); }
void putV3(const Vector3 &v) { putF(v.X); putF(v.Y); putF(v.Z); }
void putV4(const Vector4 &v) { putF(v.X); putF(v.Y); putF(v.Z); putF(v.W); }

/* a keyframe track: count, then times, values, deltas (each: present, then the entries) */
template <class T>
void log_track(unsigned n, const unsigned *times, const T *values, const T *deltas, void (*val)(const T &))
{
	put32(n);
	unsigned m = n > 0 ? n : 1;
	put8(times != NULL);
	if (times) for (unsigned i = 0; i < n; ++i) put32(times[i]);
	put8(values != NULL);
	if (values) for (unsigned i = 0; i < m; ++i) val(values[i]);
	put8(deltas != NULL);
	if (deltas) for (unsigned i = 0; i < n; ++i) val(deltas[i]);
}
void valF(const float &f) { putF(f); }
void valV3(const Vector3 &v) { putV3(v); }
template <class T>
void log_table(unsigned minus1, const T *e, void (*val)(const T &))
{
	put32(minus1);
	put8(e != NULL);
	if (e) for (unsigned i = 0; i <= minus1; ++i) val(e[i]);
}

/* The emitter and buffer as created: see rust_part_log_created in tests/differential_w3d.rs. */
void log_created(ParticleEmitterClass *e)
{
	ParticleBufferClass *b = e->Buffer;
	put8('C');
	put32(e->EmitRate); put32(e->BurstSize); put32((uint32_t)e->MaxParticles);
	putV3(e->BaseVel); putF(e->OutwardVel); putF(e->VelInheritFactor);
	put32(b->MaxNum); put32(b->MaxAge); put32(b->FutureStartTime); put32((uint32_t)b->RenderMode);
	put8(b->HasAccel); putV3(b->Accel); putF(b->MaxSize);
	put32(b->PointGroup ? b->PointGroup->Shader.Get_Bits() : b->LineGroup ? b->LineGroup->Shader.Get_Bits() : b->LineRenderer ? b->LineRenderer->Shader.Get_Bits() : 0xFFFFFFFFu);
	put8(b->Color != NULL); put8(b->Alpha != NULL); put8(b->Size != NULL); put8(b->Orientation != NULL); put8(b->Frame != NULL); put8(b->UCoord != NULL);
	log_track<Vector3>(b->NumColorKeyFrames, b->ColorKeyFrameTimes, b->ColorKeyFrameValues, b->ColorKeyFrameDeltas, valV3);
	log_track<float>(b->NumAlphaKeyFrames, b->AlphaKeyFrameTimes, b->AlphaKeyFrameValues, b->AlphaKeyFrameDeltas, valF);
	log_track<float>(b->NumSizeKeyFrames, b->SizeKeyFrameTimes, b->SizeKeyFrameValues, b->SizeKeyFrameDeltas, valF);
	log_track<float>(b->NumFrameKeyFrames, b->FrameKeyFrameTimes, b->FrameKeyFrameValues, b->FrameKeyFrameDeltas, valF);
	log_track<float>(b->NumBlurTimeKeyFrames, b->BlurTimeKeyFrameTimes, b->BlurTimeKeyFrameValues, b->BlurTimeKeyFrameDeltas, valF);
	put32(b->NumRotationKeyFrames);
	put8(b->RotationKeyFrameTimes != NULL);
	if (b->RotationKeyFrameTimes) {
		for (unsigned i = 0; i < b->NumRotationKeyFrames; ++i) {
			put32(b->RotationKeyFrameTimes[i]); putF(b->RotationKeyFrameValues[i]);
			putF(b->HalfRotationKeyFrameDeltas[i]); putF(b->OrientationKeyFrameValues[i]);
		}
	}
	if (b->Color) log_table<Vector3>(b->NumRandomColorEntriesMinus1, b->RandomColorEntries, valV3);
	if (b->Alpha) log_table<float>(b->NumRandomAlphaEntriesMinus1, b->RandomAlphaEntries, valF);
	if (b->Size) log_table<float>(b->NumRandomSizeEntriesMinus1, b->RandomSizeEntries, valF);
	if (b->Orientation) {
		log_table<float>(b->NumRandomRotationEntriesMinus1, b->RandomRotationEntries, valF);
		log_table<float>(b->NumRandomOrientationEntriesMinus1, b->RandomOrientationEntries, valF);
	}
	if (b->Frame || b->UCoord) log_table<float>(b->NumRandomFrameEntriesMinus1, b->RandomFrameEntries, valF);
	if (b->BlurTimeKeyFrameTimes) log_table<float>(b->NumRandomBlurTimeEntriesMinus1, b->RandomBlurTimeEntries, valF);
}

/* One frame's state: see rust_part_log_frame in tests/differential_w3d.rs. */
void log_state(ParticleEmitterClass *e)
{
	ParticleBufferClass *b = e->Buffer;
	put8('S');
	put32(e->EmitRemain); put32((uint32_t)e->ParticlesLeft); put8(e->IsComplete); put8(e->Active); put8(e->GroupID);
	putV3(e->PrevOrig); putF(e->PrevQ.X); putF(e->PrevQ.Y); putF(e->PrevQ.Z); putF(e->PrevQ.W);
	put32(b->Start); put32(b->End); put32(b->NewEnd); put32((uint32_t)b->NonNewNum); put32((uint32_t)b->NewNum);
	put32(b->NewParticleQueueStart); put32(b->NewParticleQueueEnd); put32((uint32_t)b->NewParticleQueueCount);
	put32(b->LastUpdateTime);
	putV3(b->BoundingBox.Center); putV3(b->BoundingBox.Extent);
}

void log_points(ParticleBufferClass *b)
{
	PointGroupClass *g = b->PointGroup;
	put8('P');
	put8(g->Mode == PointGroupClass::QUADS); put8(g->Frccl2);
	unsigned n = (unsigned)g->Count;
	put32(n);
	for (unsigned k = 0; k < n; ++k) {
		unsigned i = g->Apt ? g->Apt->Get_Array()[k] : k;
		put32(i);
		putV3(g->Locs->Get_Array()[i]);
		if (g->Diffuse) putV4(g->Diffuse->Get_Array()[i]);
		else { putV3(g->DefColor); putF(g->DefAlpha); }
		putF(g->Sizes ? g->Sizes->Get_Array()[i] : g->DefSize);
		put8(g->Orientations ? g->Orientations->Get_Array()[i] : g->DefOrientation);
		put8(g->Frames ? g->Frames->Get_Array()[i] : g->DefFrame);
	}
}

void log_lines(ParticleBufferClass *b)
{
	put8('L');
	putF(b->LineRenderer->Width);
	put32((uint32_t)g_runs.size());
	for (size_t r = 0; r < g_runs.size(); ++r) {
		put32((uint32_t)g_runs[r].pts.size());
		for (size_t i = 0; i < g_runs[r].pts.size(); ++i) { putV3(g_runs[r].pts[i]); putV4(g_runs[r].cols[i]); }
	}
}

void log_line_group(ParticleBufferClass *b)
{
	LineGroupClass *g = b->LineGroup;
	put8('G');
	put8(g->Mode == LineGroupClass::PRISM);
	unsigned n = (unsigned)g->Count;
	put32(n);
	for (unsigned k = 0; k < n; ++k) {
		unsigned i = g->Alt ? g->Alt->Get_Array()[k] : k;
		put32(i);
		putV3(g->Head->Get_Array()[i]);
		putV3(g->Tail->Get_Array()[i]);
		if (g->Diffuse) putV4(g->Diffuse->Get_Array()[i]);
		else { putV3(g->DefColor); putF(g->DefAlpha); }
		putV4(g->TailDiffuse ? g->TailDiffuse->Get_Array()[i] : g->DefTail);
		putF(g->Sizes ? g->Sizes->Get_Array()[i] : g->DefSize);
		putF(g->UCoords ? g->UCoords->Get_Array()[i] : g->DefUCoord);
	}
}

} // namespace

extern "C" {

/* Load the emitter definition in `data` (an emitter chunk; `alpha` answers a
** version-1 texture's alpha), seed the generators with seeds[0..3], create
** the emitter at sync time clock[0] and run `frames` frames: frame f's
** clock is script[5f..5f+3] (sync time, frame time, frame count), its
** operation script[5f+3] (0 none, 1 Set_Hidden(1), 2 Set_Hidden(0), 3
** Set_One_Time_Burst(arg), 4 Reset, 5 Stop, 6 Start) with argument
** script[5f+4], and its transform mats[12f..12f+12] (rows); mats[-12..0]
** (the first 12) is the transform before the first frame. */
int ref_w3d_part_run(const char *data, int len, int alpha, const unsigned *seeds, unsigned create_sync, const unsigned *script,
	const float *mats, int frames, unsigned char *log, int cap, int *log_len)
{
	(void)alpha;
	g_log.clear();
	g_runs.clear();
	ParticleEmitterDefClass *def = new ParticleEmitterDefClass;
	WW3DErrorType r;
	{
		TestFile file(data, len);
		ChunkLoadClass cload(&file);
		if (!cload.Open_Chunk()) abort();
		r = def->Load_W3D(cload);
	}
	if (r != WW3D_ERROR_OK) {
		put8('e');
	} else {
		ref_part_seed(seeds[0], seeds[1], seeds[2]);
		g_sync = create_sync;
		g_frame_time = 0;
		g_frame_count = 0;
		ParticleEmitterClass *e = ParticleEmitterClass::Create_From_Definition(*def);
		log_created(e);
		SceneClass scene;
		Matrix3D m;
		const float *m0 = mats;
		m.Set(m0[0], m0[1], m0[2], m0[3], m0[4], m0[5], m0[6], m0[7], m0[8], m0[9], m0[10], m0[11]);
		e->Set_Transform(m);
		e->Notify_Added(&scene);
		e->Set_Hidden(0);
		for (int f = 0; f < frames; ++f) {
			const unsigned *s = script + 5 * f;
			g_sync = s[0];
			g_frame_time = s[1];
			g_frame_count = s[2];
			const float *mf = mats + 12 * (f + 1);
			m.Set(mf[0], mf[1], mf[2], mf[3], mf[4], mf[5], mf[6], mf[7], mf[8], mf[9], mf[10], mf[11]);
			e->Set_Transform(m);
			switch (s[3]) {
			case 1: e->Set_Hidden(1); break;
			case 2: e->Set_Hidden(0); break;
			case 3: e->Set_One_Time_Burst((int)s[4]); break;
			case 4: e->Reset(); break;
			case 5: e->Stop(); break;
			case 6: e->Start(); break;
			default: break;
			}
			put8('F');
			e->On_Frame_Update();
			ParticleBufferClass *b = e->Peek_Buffer();
			if (b->Is_In_Scene()) {
				b->On_Frame_Update();
				int pr = g_point_renders, gr = g_line_group_renders;
				g_runs.clear();
				/* the render info is never read (the renderers are stand-ins) */
				static char rinfo[16];
				b->Render(*reinterpret_cast<RenderInfoClass *>(rinfo));
				if (g_point_renders != pr) log_points(b);
				else if (g_line_group_renders != gr) log_line_group(b);
				else if (b->RenderMode == W3D_EMITTER_RENDER_MODE_LINE && b->LineRenderer) log_lines(b);
				else put8('X');
			} else {
				put8('N');
			}
			log_state(e);
		}
		e->Release_Ref();
	}
	delete def;
	int n = (int)g_log.size();
	*log_len = n;
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	return 0;
}

} /* extern "C" */
