/* Differential-test wrapper for the light drafts (GitHub #215, upstream commit 0a05454d), group "ww3d5":
** src/drafts/ww3d/light.rs (LightClass: the constructor, the accessors, Set_Spot_Angle, the bounding volumes, Load_W3D) and
** src/drafts/ww3d/lightenvironment.rs (LightEnvironmentClass and its InputLightStruct / OutputLightStruct).
**
** From upstream, cut VERBATIM by build.rs (`drafts_ww3d5`, each anchor asserted to occur once) into OUT_DIR/draft_ww3d5_gen:
**   light.h               the whole `class LightClass` (w5_light_class.inc: its inline accessors, Set_Flag, Get_Flag, Set_Spot_Angle ...)
**   light.cpp             the constructor, the copy constructor, operator=, the destructor, Get_Obj_Space_Bounding_Sphere / _Box and
**                         Load_W3D (w5_light_*.inc)
**   lightenvironment.cpp  from `const float DIFFUSE_TO_AMBIENT_FRACTION` to the end of the file (w5_lightenv.inc), with ONE edit:
**                         `static _LightingLODCutoff` / `static _LightingLODCutoff2` become `static int ...`. Upstream declares them with
**                         no type (implicit int, which MSVC 6 accepted and gave the type int; clang and gcc refuse the declaration).
**                         So `= 0.5f` stores 0, and Set_Lighting_LOD_Cutoff truncates its float argument.
** The real lightenvironment.h (with `protected` read as `public` so that the output lights can be dumped), colorspace.h's two inline functions RGB_To_HSV and HSV_To_RGB (cut verbatim, w5_rgb2hsv.inc and w5_hsv2rgb.inc; the rest of the header pulls in dx8wrapper.h) , chunkio.h (ChunkLoadClass), w3d_util.h (Convert_Color and Convert_Vector are upstream_w3d_mesh's
** w3d_util.cpp) and the WWMath headers are the real ones.
**
** Stand-ins, each for something the files call outside themselves:
**   RenderObjClass   the scene-graph base class of LightClass (rendobj.h pulls in the whole renderer): the transform, the
**                    force-visible bit, and the virtuals LightClass overrides. Get_Position is `Transform.Get_Translation()` as
**                    rendobj.cpp:508 (Validate_Transform is a no-op for a transform nobody invalidates).
**   LightClass::Clone, Notify_Added, Notify_Removed, Get_Factory, Save, Load   not under test (scene registration and persistence).
**   TestFile         GameFileClass's Read and Seek over the RAMFile (as wrap_draft_htree.cpp's).
**
** Entry point (extern "C", tests/differential_draft_light.rs): ref_dw5_light(in, nin, out, cap) returns the number of words
** written (-1 if `out` is too small). Floats are their bits, a double is two words (low, high).
**   LIGHT DESCRIPTOR (LD): type, intensity, ambient 3, diffuse 3, specular 3, far start (2), far end (2), near start (2), near end (2),
**       far flag (0 untouched, 1 Set_Flag true, 2 Set_Flag false), near flag (same), shadows, set far range, set near range,
**       spot set, spot angle, spot direction 3, spot exponent, transform 12.
**   LIGHT STATE (LS): type, Get_Flag(NEAR), Get_Flag(FAR), shadows, intensity, ambient 3, diffuse 3, specular 3, near range (4),
**       far range (4), Get_Attenuation_Range, spot angle, spot angle cos, spot direction 3, spot exponent, sphere (centre 3, radius),
**       box (centre 3, extent 3), force visible, vertex processor.
**   op 0  apply:  [0, LD] -> LS
**   op 1  load:   [1, type, blob length, blob bytes (one per word)] -> [error code, LS]  (the blob is a W3D_CHUNK_LIGHT chunk;
**         the wrapper opens it and calls Load_W3D with the reader inside, as the asset manager does)
**   op 2  environment: [2, lod flag, lod value, ENV, post-op count, post-ops (0 Add_Fill_Light, 1 Calculate_Fill_Light,
**         2 Pre_Render_Update with the camera of the ENV)] -> ED
**         ENV: centre 3, ambient 3, fill intensity, light count, LD * count, camera 12, pre-render flag
**         ED: light count, equivalent ambient 3, per light: direction 3, diffuse 3, isPointLight, [for a point light: inner radius,
**         outer radius, point diffuse 3, point ambient 3, point centre 3], then if pre-rendered or a post-op ran Pre_Render_Update:
**         per light: output direction 3, output diffuse 3
**   op 3  equality: [3, ENV a, ENV b] -> [a == b, b == a, a == a]
**   op 4  lod: [4] -> [Get_Lighting_LOD_Cutoff bits]   (the value as the process started, before any op 2 sets it) */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>
#include <new>

#include "always.h"
#include "wwdebug.h"
#include "vector3.h"
#include "matrix3d.h"
#include "sphere.h"
#include "aabox.h"
#include "wwmath.h"
#include "w3d_file.h"
#include "w3derr.h"
#include "chunkio.h"
#include "w3d_util.h"

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
void ref_wwmath_init_tables(void); /* wrap_wwmath.cpp: the _FastSinTable of WWMath::Init (verbatim there) */
}

class SceneClass;
class RenderInfoClass;
class PersistFactoryClass;
class ChunkSaveClass;

enum { CLASSID_LIGHT = 7 };

/* stand-in: the scene-graph base class (see the comment above) */
class RenderObjClass
{
public:
	RenderObjClass(void) : Transform(1), ForceVisible(false) {}
	virtual ~RenderObjClass(void) {}
	RenderObjClass &operator=(const RenderObjClass &that)
	{
		Transform = that.Transform;
		ForceVisible = that.ForceVisible;
		return *this;
	}
	virtual RenderObjClass *Clone(void) const = 0;
	virtual int Class_ID(void) const = 0;
	virtual void Render(RenderInfoClass &) = 0;
	virtual bool Is_Vertex_Processor(void) = 0;
	virtual void Notify_Added(SceneClass *) = 0;
	virtual void Notify_Removed(SceneClass *) = 0;
	virtual void Get_Obj_Space_Bounding_Sphere(SphereClass &sphere) const = 0;
	virtual void Get_Obj_Space_Bounding_Box(AABoxClass &box) const = 0;
	virtual const PersistFactoryClass &Get_Factory(void) const = 0;
	virtual bool Save(ChunkSaveClass &) = 0;
	virtual bool Load(ChunkLoadClass &) = 0;
	virtual void Set_Force_Visible(int onoff) { ForceVisible = onoff != 0; }
	const Matrix3D &Get_Transform(void) const { return Transform; }
	Vector3 Get_Position(void) const { return Transform.Get_Translation(); }
	void Set_Transform(const Matrix3D &m) { Transform = m; }
	bool Is_Force_Visible(void) const { return ForceVisible; }

protected:
	Matrix3D Transform;
	bool ForceVisible;
};

#include "w5_light_class.inc"

/* stand-ins: LightClass's virtuals that are scene registration and persistence */
RenderObjClass *LightClass::Clone(void) const { return new LightClass(*this); }
void LightClass::Notify_Added(SceneClass *) {}
void LightClass::Notify_Removed(SceneClass *) {}
const PersistFactoryClass &LightClass::Get_Factory(void) const { abort(); }
bool LightClass::Save(ChunkSaveClass &) { return false; }
bool LightClass::Load(ChunkLoadClass &) { return false; }

#include "w5_light_ctor.inc"
#include "w5_light_copy.inc"
#include "w5_light_assign.inc"
#include "w5_light_dtor.inc"
#include "w5_light_sphere.inc"
#include "w5_light_box.inc"
#include "w5_light_load.inc"

#include "w5_rgb2hsv.inc"
#include "w5_hsv2rgb.inc"

#define protected public
#include "lightenvironment.h"
#undef protected

#include "w5_lightenv.inc"

namespace {

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

typedef uint32_t W;

struct Reader {
	const W *p;
	int i;
	W u() { return p[i++]; }
	float f() { W v = p[i++]; float x; memcpy(&x, &v, 4); return x; }
	double d() { uint64_t lo = p[i++]; uint64_t hi = p[i++]; uint64_t v = lo | (hi << 32); double x; memcpy(&x, &v, 8); return x; }
	Vector3 v3() { float a = f(); float b = f(); float c = f(); return Vector3(a, b, c); }
	Matrix3D m()
	{
		float a[12];
		for (int k = 0; k < 12; ++k) a[k] = f();
		return Matrix3D(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]);
	}
};

std::vector<W> g_out;

void put(W w) { g_out.push_back(w); }
void putf(float x) { W v; memcpy(&v, &x, 4); g_out.push_back(v); }
void putd(double x) { uint64_t v; memcpy(&v, &x, 8); g_out.push_back((W)(v & 0xFFFFFFFFu)); g_out.push_back((W)(v >> 32)); }
void putv(const Vector3 &v) { putf(v.X); putf(v.Y); putf(v.Z); }

void apply(Reader &r, LightClass &l)
{
	l.Set_Intensity(r.f());
	Vector3 a = r.v3();
	Vector3 d = r.v3();
	Vector3 s = r.v3();
	l.Set_Ambient(a);
	l.Set_Diffuse(d);
	l.Set_Specular(s);
	double fs = r.d(), fe = r.d(), ns = r.d(), ne = r.d();
	W far_flag = r.u(), near_flag = r.u(), shadows = r.u(), set_far = r.u(), set_near = r.u();
	if (set_far) l.Set_Far_Attenuation_Range(fs, fe);
	if (set_near) l.Set_Near_Attenuation_Range(ns, ne);
	if (far_flag) l.Set_Flag(LightClass::FAR_ATTENUATION, far_flag == 1);
	if (near_flag) l.Set_Flag(LightClass::NEAR_ATTENUATION, near_flag == 1);
	l.Enable_Shadows(shadows != 0);
	W spot_set = r.u();
	float angle = r.f();
	Vector3 dir = r.v3();
	float expo = r.f();
	if (spot_set) {
		l.Set_Spot_Angle(angle);
		l.Set_Spot_Direction(dir);
		l.Set_Spot_Exponent(expo);
	}
	l.Set_Transform(r.m());
}

void dump_light(LightClass &l)
{
	put((W)l.Get_Type());
	put((W)l.Get_Flag(LightClass::NEAR_ATTENUATION));
	put((W)l.Get_Flag(LightClass::FAR_ATTENUATION));
	put((W)l.Are_Shadows_Enabled());
	putf(l.Get_Intensity());
	Vector3 v;
	l.Get_Ambient(&v); putv(v);
	l.Get_Diffuse(&v); putv(v);
	l.Get_Specular(&v); putv(v);
	double a, b;
	l.Get_Near_Attenuation_Range(a, b); putd(a); putd(b);
	l.Get_Far_Attenuation_Range(a, b); putd(a); putd(b);
	putf(l.Get_Attenuation_Range());
	putf(l.Get_Spot_Angle());
	putf(l.Get_Spot_Angle_Cos());
	l.Get_Spot_Direction(v); putv(v);
	putf(l.Get_Spot_Exponent());
	SphereClass sp;
	l.Get_Obj_Space_Bounding_Sphere(sp);
	putv(sp.Center); putf(sp.Radius);
	AABoxClass bx;
	l.Get_Obj_Space_Bounding_Box(bx);
	putv(bx.Center); putv(bx.Extent);
	put((W)l.Is_Force_Visible());
	put((W)l.Is_Vertex_Processor());
}

/* a LightEnvironmentClass in zeroed memory (the game's lives in zeroed or stack memory; the fields the constructor leaves alone are
   then zero rather than whatever the allocator held) */
struct Env {
	alignas(LightEnvironmentClass) unsigned char mem[sizeof(LightEnvironmentClass)];
	LightEnvironmentClass *e;
	Matrix3D camera;
	bool rendered;
	Env() : camera(1), rendered(false)
	{
		memset(mem, 0, sizeof(mem));
		e = new (mem) LightEnvironmentClass();
	}
};

void build_env(Reader &r, Env &env)
{
	Vector3 centre = r.v3();
	Vector3 ambient = r.v3();
	float fill = r.f();
	W n = r.u();
	env.e->Reset(centre, ambient);
	env.e->Set_Fill_Intensity(fill);
	std::vector<LightClass *> lights;
	for (W k = 0; k < n; ++k) {
		LightClass *l = new LightClass((LightClass::LightType)r.u());
		apply(r, *l);
		lights.push_back(l);
	}
	env.camera = r.m();
	W pre = r.u();
	for (size_t k = 0; k < lights.size(); ++k) env.e->Add_Light(*lights[k]);
	for (size_t k = 0; k < lights.size(); ++k) delete lights[k];
	if (pre) {
		env.e->Pre_Render_Update(env.camera);
		env.rendered = true;
	}
}

void dump_env(Env &env)
{
	LightEnvironmentClass &e = *env.e;
	put((W)e.Get_Light_Count());
	putv(e.Get_Equivalent_Ambient());
	for (int i = 0; i < e.Get_Light_Count(); ++i) {
		putv(e.Get_Light_Direction(i));
		putv(e.Get_Light_Diffuse(i));
		put((W)e.isPointLight(i));
		if (e.isPointLight(i)) {
			putf(e.getPointIrad(i)); putf(e.getPointOrad(i));
			putv(e.getPointDiffuse(i)); putv(e.getPointAmbient(i)); putv(e.getPointCenter(i));
		}
	}
	if (env.rendered) {
		for (int i = 0; i < e.Get_Light_Count(); ++i) {
			putv(e.OutputLights[i].Direction);
			putv(e.OutputLights[i].Diffuse);
		}
	}
}

} // namespace

static float g_lod_at_start;
static bool g_lod_read = false;

extern "C" int ref_dw5_light(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	if (!g_lod_read) {
		ref_wwmath_init_tables();
		g_lod_at_start = LightEnvironmentClass::Get_Lighting_LOD_Cutoff();
		g_lod_read = true;
	}
	g_out.clear();
	Reader r = {in, 0};
	W op = r.u();
	switch (op) {
	case 0: {
		LightClass l((LightClass::LightType)r.u());
		apply(r, l);
		dump_light(l);
		break;
	}
	case 1: {
		LightClass l((LightClass::LightType)r.u());
		W len = r.u();
		std::vector<char> blob(len);
		for (W k = 0; k < len; ++k) blob[k] = (char)r.u();
		TestFile f(blob.empty() ? "" : &blob[0], (int)len);
		ChunkLoadClass cload(&f);
		cload.Open_Chunk();
		W err = (W)l.Load_W3D(cload);
		put(err);
		dump_light(l);
		break;
	}
	case 2: {
		W lod_set = r.u();
		float lod = r.f();
		if (lod_set) LightEnvironmentClass::Set_Lighting_LOD_Cutoff(lod);
		Env env;
		build_env(r, env);
		W n = r.u();
		for (W k = 0; k < n; ++k) {
			switch (r.u()) {
			case 0: env.e->Add_Fill_Light(); break;
			case 1: env.e->Calculate_Fill_Light(); break;
			default: env.e->Pre_Render_Update(env.camera); env.rendered = true; break;
			}
		}
		dump_env(env);
		put(0xFFFFFFFFu);
		putf(LightEnvironmentClass::Get_Lighting_LOD_Cutoff());
		break;
	}
	case 3: {
		Env a, b;
		build_env(r, a);
		build_env(r, b);
		put((W)(*a.e == *b.e));
		put((W)(*b.e == *a.e));
		put((W)(*a.e == *a.e));
		break;
	}
	case 4:
		putf(g_lod_at_start);
		break;
	default:
		return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	memcpy(out, &g_out[0], g_out.size() * sizeof(W));
	return (int)g_out.size();
}
