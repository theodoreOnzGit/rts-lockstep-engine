/* Differential-test wrapper for the segmented-line render object (GitHub #169; upstream commit 0a05454d):
**   WW3D2/segline.cpp from its first #include to the end (SegmentedLineClass: every function), COMPILED VERBATIM
**     (w3d_gen/segline_obj_def.cpp, see build.rs), with segline.h unchanged;
**   WW3D2/shader.cpp's SC_ADDITIVE preset (from `#define SC_ADDITIVE` to `_PresetAdditiveShader`'s definition), VERBATIM
**     (w3d_gen/segline_obj_preset.cpp), the shader W3DLaserDraw gives its lines;
** on the renderer of the segmented-line library (upstream_w3d_segline: seglinerenderer.cpp verbatim but for the #185
** edit, and its recording device, differential/wrap_segline.cpp, reached through its ref_segline_clock, _view, _seed,
** _gen and _take), linked after this library. The render object system is shim_w3d/segline_obj_prelude.h's.
**
** Two uses:
** 1. ref_segline_obj_seq (tests/differential_segline_obj.rs), words in (u32, floats as their bits):
**      s3a s3b    the randomizers' Random3Class reseeded
**      sync       the clock; the line is made
**    then operations until the input ends, each a code and its words:
**      0 Set_Points: n, then n points (3 each)
**      1 Set_Point_Location: idx, x y z
**      2 Get_Point_Location: idx (records x y z)
**      3 Add_Point: x y z
**      4 Delete_Point: idx
**      5 Set_Texture (0/1), Set_Shader (bits), Set_Width, Set_Color (r g b), Set_Opacity
**      6 Set_Noise_Amplitude, Set_Merge_Abort_Factor, Set_Subdivision_Levels, Set_Texture_Mapping_Mode,
**        Set_Texture_Tile_Factor, Set_UV_Offset_Rate (u v), flags (bit 0 Set_Merge_Intersections, 1 Set_Freeze_Random,
**        2 Set_Disable_Sorting, 3 Set_End_Caps)
**      7 Render: sync, the view's three rows (12); records the chunks (wrap_segline.cpp's words) then 'E'
**      8 Prepare_LOD: the camera's screen size (records 1 for Add_Object, or 2 and the cost for Add_Cost)
**      9 Increment_LOD   10 Decrement_LOD   11 Set_LOD_Level: lod (an int)   12 Reset_Line: sync
**      13 Set_Visible, Set_Hidden, Set_Animation_Hidden, Set_Force_Visible (0/1 each), Set_Collision_Type
**      14 the line replaced by its copy (the copy constructor, Clone's)
**      15 the line replaced by a new line assigned from it (operator=)
**      (for both, the old line's point array is detached before it is released: see detach_points)
**      16 the Transform's 12 words (rows): object space to the world
**    Words out, per operation: the code, its records, then the state: Get_Num_Points and each point, the texture's
**    presence, the shader's bits, Get_Width, Get_Color (3), Get_Opacity, Get_Noise_Amplitude, Get_Merge_Abort_Factor,
**    Get_Subdivision_Levels, Get_Texture_Mapping_Mode, Get_Texture_Tile_Factor, Get_UV_Offset_Rate (2), the four flags,
**    Get_Num_Polys, Get_Cost, Get_Value, Get_Post_Increment_Value, Get_LOD_Level, Get_LOD_Count, the object-space
**    bounding box (centre, extent) and sphere (centre, radius), Bits, the renderer's LastUsedSyncTime and
**    CurrentUVOffset (2), and the shared generator's Seed and Index.
** 2. ref_laser_segl_bind() fills the laser library's table (differential/laser_segl_api.h) with this library's calls, so
**    that W3DLaserDraw (differential/wrap_laser_draw.cpp) drives REAL SegmentedLineClass lines. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

#include "random.h"
#include "v3_rnd.h"
#include "w3d_file.h"
#define private public
#define protected public
#include "seglinerenderer.h"
#include "segline.h"
#undef private
#undef protected
#include "laser_segl_api.h"

extern "C" {
void ref_segline_clock(unsigned sync);
void ref_segline_view(const float *v);
void ref_segline_seed(unsigned a, unsigned b);
void ref_segline_gen(unsigned *seed_index);
int ref_segline_take(unsigned *out, int cap);
}

/* ---- the stand-ins' definitions (shim_w3d/segline_obj_prelude.h, segline_prelude.h) ---- */
const float RenderObjClass::AT_MIN_LOD = FLT_MAX;
const float RenderObjClass::AT_MAX_LOD = -1.0f;
static float g_screen_size;
float RenderObjClass::Get_Screen_Size(CameraClass &) { return g_screen_size; }
static int g_lod_request; /* 1 Add_Object, 2 Add_Cost */
static float g_lod_cost;
void PredictiveLODOptimizerClass::Add_Object(RenderObjClass *) { g_lod_request = 1; }
void PredictiveLODOptimizerClass::Add_Cost(float cost)
{
	g_lod_request = 2;
	g_lod_cost = cost;
}
/* lineseg.cpp is not compiled here: Cast_Ray, which calls it, is never called (weak: another library may define it) */
__attribute__((weak)) bool LineSegClass::Find_Intersection(const LineSegClass &, Vector3 *, float *, Vector3 *, float *) const { abort(); }
bool WW3D::Is_Sorting_Enabled(void) { return true; }
bool WW3D::Are_Static_Sort_Lists_Enabled(void) { return true; }
void WW3D::Add_To_Static_Sort_List(void *, unsigned int) { abort(); }
/* ShaderClass::Guess_Sort_Level: Render calls it only with sorting off, never (a weak stand-in: other libraries define
** their own) */
__attribute__((weak)) int ShaderClass::Guess_Sort_Level(void) const { abort(); }

static TextureClass g_seg_texture;
static std::vector<uint32_t> g_out;
static void put(uint32_t w) { g_out.push_back(w); }
static void putf(float f)
{
	uint32_t w;
	memcpy(&w, &f, 4);
	put(w);
}
static float fw(uint32_t w)
{
	float f;
	memcpy(&f, &w, 4);
	return f;
}
static std::vector<unsigned> g_take(1 << 16);
/* the renderer's draws logged by wrap_segline.cpp, appended */
static void take_chunks(void)
{
	for (;;) {
		int n = ref_segline_take(&g_take[0], (int)g_take.size());
		if (n <= (int)g_take.size()) {
			g_out.insert(g_out.end(), g_take.begin(), g_take.begin() + n);
			return;
		}
		abort(); /* more words than the buffer (a chunk is at most 1024 vertices: never) */
	}
}

/* SimpleDynVecClass has no copy constructor nor assignment: the copy constructor's and operator='s memberwise copies of
** PointLocations SHARE the source's array (undefined upstream once both lines are destroyed: a double delete; the
** first run of this test aborted on it, issue169b_defects.md). Before the source is released, its PointLocations lets
** go of the array (no delete), so the copy owns it alone: what the copy is meant to be, and what the Rust's copy is. */
struct PointsPeek : public SimpleDynVecClass<Vector3>
{
	static void detach(SimpleDynVecClass<Vector3> &v)
	{
		PointsPeek &p = static_cast<PointsPeek &>(v);
		p.Vector = NULL;
		p.VectorMax = 0;
		p.ActiveCount = 0;
	}
};
static void detach_points(SegmentedLineClass *l) { PointsPeek::detach(l->PointLocations); }

static void line_state(SegmentedLineClass *l)
{
	int n = l->Get_Num_Points();
	put((uint32_t)n);
	for (int i = 0; i < n; ++i) {
		putf(l->PointLocations[i].X);
		putf(l->PointLocations[i].Y);
		putf(l->PointLocations[i].Z);
	}
	put(l->Get_Texture() != NULL);
	put(l->Get_Shader().Get_Bits());
	putf(l->Get_Width());
	Vector3 c;
	l->Get_Color(c);
	putf(c.X);
	putf(c.Y);
	putf(c.Z);
	putf(l->Get_Opacity());
	putf(l->Get_Noise_Amplitude());
	putf(l->Get_Merge_Abort_Factor());
	put(l->Get_Subdivision_Levels());
	put((uint32_t)l->Get_Texture_Mapping_Mode());
	putf(l->Get_Texture_Tile_Factor());
	Vector2 r = l->Get_UV_Offset_Rate();
	putf(r.X);
	putf(r.Y);
	put(l->Is_Merge_Intersections() != 0);
	put(l->Is_Freeze_Random() != 0);
	put(l->Is_Sorting_Disabled() != 0);
	put(l->Are_End_Caps_Enabled() != 0);
	put((uint32_t)l->Get_Num_Polys());
	putf(l->Get_Cost());
	putf(l->Get_Value());
	putf(l->Get_Post_Increment_Value());
	put((uint32_t)l->Get_LOD_Level());
	put((uint32_t)l->Get_LOD_Count());
	AABoxClass box;
	l->Get_Obj_Space_Bounding_Box(box);
	putf(box.Center.X);
	putf(box.Center.Y);
	putf(box.Center.Z);
	putf(box.Extent.X);
	putf(box.Extent.Y);
	putf(box.Extent.Z);
	SphereClass sphere;
	l->Get_Obj_Space_Bounding_Sphere(sphere);
	putf(sphere.Center.X);
	putf(sphere.Center.Y);
	putf(sphere.Center.Z);
	putf(sphere.Radius);
	put((uint32_t)l->Bits);
	put(l->LineRenderer.LastUsedSyncTime);
	putf(l->LineRenderer.CurrentUVOffset.X);
	putf(l->LineRenderer.CurrentUVOffset.Y);
	unsigned g[2];
	ref_segline_gen(g);
	put(g[0]);
	put(g[1]);
}

static void render(SegmentedLineClass *l, unsigned sync, const float *view)
{
	static char rinfo[16];
	ref_segline_clock(sync);
	ref_segline_view(view);
	l->Render(*reinterpret_cast<RenderInfoClass *>(rinfo));
	take_chunks();
}

/* ---- the laser library's table (laser_segl_api.h) ---- */
static void ls_clock(unsigned sync) { ref_segline_clock(sync); }
static void *ls_make(void) { return NEW_REF(SegmentedLineClass, ()); }
static void ls_release(void *l)
{
	SegmentedLineClass *p = (SegmentedLineClass *)l;
	REF_PTR_RELEASE(p);
}
static void ls_set_texture(void *l, int has) { ((SegmentedLineClass *)l)->Set_Texture(has ? &g_seg_texture : NULL); }
static void ls_set_shader_additive(void *l) { ((SegmentedLineClass *)l)->Set_Shader(ShaderClass::_PresetAdditiveShader); }
static void ls_set_width(void *l, float w) { ((SegmentedLineClass *)l)->Set_Width(w); }
static void ls_set_color(void *l, float r, float g, float b) { ((SegmentedLineClass *)l)->Set_Color(Vector3(r, g, b)); }
static void ls_set_uv_offset_rate(void *l, float u, float v) { ((SegmentedLineClass *)l)->Set_UV_Offset_Rate(Vector2(u, v)); }
static void ls_set_texture_mapping_mode(void *l, int m)
{
	((SegmentedLineClass *)l)->Set_Texture_Mapping_Mode((SegLineRendererClass::TextureMapMode)m);
}
static void ls_set_visible(void *l, int onoff) { ((SegmentedLineClass *)l)->Set_Visible(onoff); }
static void ls_set_texture_tile_factor(void *l, float f) { ((SegmentedLineClass *)l)->Set_Texture_Tile_Factor(f); }
static void ls_set_points(void *l, unsigned n, const float *xyz)
{
	std::vector<Vector3> v(n + 1);
	for (unsigned i = 0; i < n; ++i) v[i].Set(xyz[3 * i], xyz[3 * i + 1], xyz[3 * i + 2]);
	((SegmentedLineClass *)l)->Set_Points(n, &v[0]);
}
static int copy_words(int *out, int cap)
{
	int n = (int)g_out.size();
	int m = n > cap ? cap : n;
	if (m > 0) memcpy(out, &g_out[0], 4 * m);
	g_out.clear();
	return n;
}
/* the laser's line: the texture's presence, the shader's bits, Get_Width, Get_Color (3), Get_UV_Offset_Rate (2),
** Get_Texture_Mapping_Mode, Is_Visible, Get_Texture_Tile_Factor, Get_Num_Points and each point, Bits */
static int ls_state(void *lp, int *out, int cap)
{
	SegmentedLineClass *l = (SegmentedLineClass *)lp;
	g_out.clear();
	put(l->Get_Texture() != NULL);
	put(l->Get_Shader().Get_Bits());
	putf(l->Get_Width());
	Vector3 c;
	l->Get_Color(c);
	putf(c.X);
	putf(c.Y);
	putf(c.Z);
	Vector2 r = l->Get_UV_Offset_Rate();
	putf(r.X);
	putf(r.Y);
	put((uint32_t)l->Get_Texture_Mapping_Mode());
	put(l->Is_Visible() != 0);
	putf(l->Get_Texture_Tile_Factor());
	int n = l->Get_Num_Points();
	put((uint32_t)n);
	for (int i = 0; i < n; ++i) {
		Vector3 p;
		l->Get_Point_Location(i, p);
		putf(p.X);
		putf(p.Y);
		putf(p.Z);
	}
	put((uint32_t)l->Bits);
	return copy_words(out, cap);
}
static int ls_render(void *l, unsigned sync, const float *view, int *out, int cap)
{
	g_out.clear();
	render((SegmentedLineClass *)l, sync, view);
	return copy_words(out, cap);
}

extern "C" {

void ref_laser_segl_bind(void)
{
	g_ld_segl_api.clock = ls_clock;
	g_ld_segl_api.make = ls_make;
	g_ld_segl_api.release = ls_release;
	g_ld_segl_api.set_texture = ls_set_texture;
	g_ld_segl_api.set_shader_additive = ls_set_shader_additive;
	g_ld_segl_api.set_width = ls_set_width;
	g_ld_segl_api.set_color = ls_set_color;
	g_ld_segl_api.set_uv_offset_rate = ls_set_uv_offset_rate;
	g_ld_segl_api.set_texture_mapping_mode = ls_set_texture_mapping_mode;
	g_ld_segl_api.set_visible = ls_set_visible;
	g_ld_segl_api.set_texture_tile_factor = ls_set_texture_tile_factor;
	g_ld_segl_api.set_points = ls_set_points;
	g_ld_segl_api.state = ls_state;
	g_ld_segl_api.render = ls_render;
}

int ref_segline_obj_seq(const unsigned *in, int nin, unsigned *out, int cap)
{
	g_out.clear();
	int k = 0;
#define NEXT() (k < nin ? in[k++] : (abort(), 0u))
	unsigned s3a = NEXT(), s3b = NEXT();
	ref_segline_seed(s3a, s3b);
	ref_segline_clock(NEXT());
	ref_segline_take(&g_take[0], 0);
	SegmentedLineClass *l = NEW_REF(SegmentedLineClass, ());
	while (k < nin) {
		unsigned op = NEXT();
		put(op);
		switch (op) {
		case 0: {
			unsigned n = NEXT();
			std::vector<Vector3> v(n + 1);
			for (unsigned i = 0; i < n; ++i) {
				float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
				v[i].Set(x, y, z);
			}
			l->Set_Points(n, &v[0]);
			break;
		}
		case 1: {
			unsigned idx = NEXT();
			float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
			l->Set_Point_Location(idx, Vector3(x, y, z));
			break;
		}
		case 2: {
			Vector3 p;
			l->Get_Point_Location(NEXT(), p);
			putf(p.X);
			putf(p.Y);
			putf(p.Z);
			break;
		}
		case 3: {
			float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
			l->Add_Point(Vector3(x, y, z));
			break;
		}
		case 4: l->Delete_Point(NEXT()); break;
		case 5: {
			l->Set_Texture(NEXT() ? &g_seg_texture : NULL);
			l->Set_Shader(ShaderClass(NEXT()));
			l->Set_Width(fw(NEXT()));
			float r = fw(NEXT()), g = fw(NEXT()), b = fw(NEXT());
			l->Set_Color(Vector3(r, g, b));
			l->Set_Opacity(fw(NEXT()));
			break;
		}
		case 6: {
			l->Set_Noise_Amplitude(fw(NEXT()));
			l->Set_Merge_Abort_Factor(fw(NEXT()));
			l->Set_Subdivision_Levels(NEXT());
			l->Set_Texture_Mapping_Mode((SegLineRendererClass::TextureMapMode)NEXT());
			l->Set_Texture_Tile_Factor(fw(NEXT()));
			float u = fw(NEXT()), v = fw(NEXT());
			l->Set_UV_Offset_Rate(Vector2(u, v));
			unsigned f = NEXT();
			l->Set_Merge_Intersections((f & 1) != 0);
			l->Set_Freeze_Random((f & 2) != 0);
			l->Set_Disable_Sorting((f & 4) != 0);
			l->Set_End_Caps((f & 8) != 0);
			break;
		}
		case 7: {
			unsigned sync = NEXT();
			float view[12];
			for (int i = 0; i < 12; ++i) view[i] = fw(NEXT());
			render(l, sync, view);
			put('E');
			break;
		}
		case 8: {
			g_screen_size = fw(NEXT());
			g_lod_request = 0;
			static char camera[16];
			l->Prepare_LOD(*reinterpret_cast<CameraClass *>(camera));
			put((uint32_t)g_lod_request);
			if (g_lod_request == 2) putf(g_lod_cost);
			break;
		}
		case 9: l->Increment_LOD(); break;
		case 10: l->Decrement_LOD(); break;
		case 11: l->Set_LOD_Level((int)NEXT()); break;
		case 12: ref_segline_clock(NEXT()); l->Reset_Line(); break;
		case 13: {
			l->Set_Visible((int)NEXT());
			l->Set_Hidden((int)NEXT());
			l->Set_Animation_Hidden((int)NEXT());
			l->Set_Force_Visible((int)NEXT());
			l->Set_Collision_Type((int)NEXT());
			break;
		}
		case 14: {
			SegmentedLineClass *c = (SegmentedLineClass *)l->Clone();
			detach_points(l);
			REF_PTR_RELEASE(l);
			l = c;
			break;
		}
		case 15: {
			SegmentedLineClass *c = NEW_REF(SegmentedLineClass, ());
			*c = *l;
			detach_points(l);
			REF_PTR_RELEASE(l);
			l = c;
			break;
		}
		case 16: {
			float t[12];
			for (int i = 0; i < 12; ++i) t[i] = fw(NEXT());
			l->Transform = Matrix3D(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11]);
			break;
		}
		default: abort();
		}
		line_state(l);
	}
#undef NEXT
	REF_PTR_RELEASE(l);
	int n = (int)g_out.size();
	int m = n > cap ? cap : n;
	if (m > 0) memcpy(out, &g_out[0], 4 * m);
	return n;
}

} /* extern "C" */
