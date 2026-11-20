/* Differential-test wrapper for the streak renderer draft (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/streak_render.rs (StreakRendererClass) against WW3D2/streakRender.cpp (the Zero Hour copy of the segmented-line renderer with per-point colours and
** widths), COMPILED VERBATIM (build.rs `drafts_ww3d6`: streakRender.cpp and streakRender.h under the lower-case name streakrender.h that the .cpp includes) but for ONE named
** edit: RenderStreak's parallel merge of a parallel intersection reads `new_direction` before it is written (undefined, as in seglinerenderer.cpp, GitHub #185); the cut
** declares it `Vector3 new_direction(0,0,0)`, the value the draft starts from, and the wrapper counts how often the branch that reads it ran (g_undefined).
**
** The device (differential/shim_draft_ww3d6/streak_prelude.h, a copy of shim_w3d/segline_prelude.h with the classes renamed RefStreak*) is a recorder: each chunk's Draw_Triangles or
** Insert_Triangles logs the vertex and index buffers as RenderStreak wrote them, the shader, material and texture it set, and which of the two drew. Beyond segline's it needs
** DX8Wrapper::Convert_Color_Clamp (cut VERBATIM from dx8wrapper.h, convert_clamp.inc) over Clamp_Color, which is TRANSCRIBED below from the x86 asm of dx8wrapper.h (the CMOV path
** the shipped game ran: a negative value or its sign bit gives 0, then an unsigned compare against 1.0f's bits), and Convert_Color (the mesh library's x87 transcription).
**
** VectorProcessorClass::Transform comes from the deformation library (vp.cpp verbatim), Matrix3D::mul from upstream_wwmath, random.cpp and v3_rnd.cpp (the shared
** Vector3Randomizer::Randomizer) from the emitter library; IndexBufferExceptionFunc from the segline recorder library.
**
** Driver (ref_w6_streak_seq), words in (u32, floats as their bits): s3a s3b (the shared Random3Class reseeded), then operations until the input ends:
**   0 Init: flags level noise abort            1 setters: texture(0/1) shader width r g b opacity
**   2 RenderStreak: view 12 (rows of the 4x4's top three rows), transform 12, n, n points (3 each), n+1 RGBAs, n+1 widths, m personalities, m words
**   Words out, per operation: the code, then for a render the chunks (each 'C', sorting, vertex count, each vertex's x y z diffuse u v, triangle count, each triangle's I J K, shader
**   bits, material, texture present), then 'E'; after every operation the renderer's Width, NoiseAmplitude, MergeAbortFactor, SubdivisionLevel, Bits, the shared generator's
**   Seed and Index; and at the very end the number of renders that ran the undefined-read branch. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

#define private public
#define protected public
#include "random.h"
#include "v3_rnd.h"
#include "w3d_file.h"
#include "streakrender.h"
#undef private
#undef protected
#include "sphere.h"

extern "C" unsigned ref_dx8_convert_color(const float *rgba); /* wrap_w3d_mesh.cpp */

static Matrix4x4 g_view(true);
static std::vector<uint32_t> g_out;
static unsigned char g_vb[44 * 1024];
static unsigned short g_ib[3 * 1024];
static int g_mat;
static int g_tex;
static unsigned g_shader;
static unsigned g_vcount, g_icount;

static void put(uint32_t w) { g_out.push_back(w); }
static void putf(float f) { uint32_t w; memcpy(&w, &f, 4); put(w); }

static VertexMaterialClass g_presets[2];
VertexMaterialClass *VertexMaterialClass::Get_Preset(PresetType type)
{
	g_presets[type].Preset = type;
	return &g_presets[type];
}

DynamicVBAccessClass::DynamicVBAccessClass(unsigned type, unsigned, unsigned short vertex_count) : Type(type), Count(vertex_count)
{
	if (vertex_count > 1024) abort();
	g_vcount = vertex_count;
	memset(g_vb, 0, sizeof(g_vb));
}
unsigned char *DynamicVBAccessClass::WriteLockClass::Get_Formatted_Vertex_Array() { return g_vb; }
DynamicIBAccessClass::DynamicIBAccessClass(unsigned short type, unsigned short index_count) : Type(type), Count(index_count)
{
	if (index_count > 3 * 1024) abort();
	g_icount = index_count;
}
unsigned short *DynamicIBAccessClass::WriteLockClass::Get_Index_Array() { return g_ib; }

void DX8Wrapper::Get_Transform(D3DTRANSFORMSTATETYPE transform, Matrix4x4 &m)
{
	if (transform != D3DTS_VIEW) abort();
	m = g_view;
}
void DX8Wrapper::Set_Transform(D3DTRANSFORMSTATETYPE, const Matrix4x4 &) {}
unsigned int DX8Wrapper::Convert_Color(const Vector4 &color)
{
	float c[4] = {color.X, color.Y, color.Z, color.W};
	return ref_dx8_convert_color(c);
}
unsigned int DX8Wrapper::Convert_Color(const Vector3 &color, const float alpha)
{
	float c[4] = {color.X, color.Y, color.Z, alpha};
	return ref_dx8_convert_color(c);
}

/* dx8wrapper.h's Clamp_Color, the CMOV path's asm transcribed: per component, mask = ~(bits >> 31) (zero for a negative value, -0.0 and negative NaNs included),
** value = bits & mask, then an unsigned compare with 1.0f's bits (0x3f800000): "cmp edi,edx; cmovnb edi,edx" sets 1.0 when value >= it (so +inf and positive NaNs give 1.0) */
void DX8Wrapper::Clamp_Color(Vector4 &color)
{
	for (int i = 0; i < 4; ++i) {
		uint32_t bits;
		memcpy(&bits, &color[i], 4);
		int32_t sbits = (int32_t)bits;
		uint32_t mask = ~(uint32_t)(sbits >> 31);
		uint32_t v = bits & mask;
		if (v >= 0x3f800000u) v = 0x3f800000u;
		memcpy(&color[i], &v, 4);
	}
}
#include "convert_clamp.inc"

void DX8Wrapper::Set_Index_Buffer(const DynamicIBAccessClass &, unsigned short) {}
void DX8Wrapper::Set_Vertex_Buffer(const DynamicVBAccessClass &) {}
void DX8Wrapper::Set_Material(const VertexMaterialClass *material) { g_mat = material ? material->Preset : -1; }
void DX8Wrapper::Set_Texture(unsigned, TextureClass *texture) { g_tex = texture != NULL; }
void DX8Wrapper::Set_Shader(const ShaderClass &shader) { g_shader = shader.Get_Bits(); }

static void log_chunk(int sorting, unsigned short polygon_count, unsigned short vertex_count)
{
	put('C');
	put(sorting);
	if (vertex_count != g_vcount || polygon_count * 3u != g_icount) abort();
	put(vertex_count);
	for (unsigned i = 0; i < vertex_count; ++i) {
		const unsigned char *v = g_vb + 44 * i;
		float x, y, z, u, w;
		uint32_t d;
		memcpy(&x, v + 0, 4);
		memcpy(&y, v + 4, 4);
		memcpy(&z, v + 8, 4);
		memcpy(&d, v + 24, 4);
		memcpy(&u, v + 28, 4);
		memcpy(&w, v + 32, 4);
		putf(x); putf(y); putf(z); put(d); putf(u); putf(w);
	}
	put(polygon_count);
	for (unsigned i = 0; i < 3u * polygon_count; ++i) put(g_ib[i]);
	put(g_shader);
	put((uint32_t)g_mat);
	put((uint32_t)g_tex);
}
void DX8Wrapper::Draw_Triangles(unsigned short, unsigned short polygon_count, unsigned short, unsigned short vertex_count)
{
	log_chunk(0, polygon_count, vertex_count);
}
void SortingRendererClass::Insert_Triangles(const SphereClass &, unsigned short, unsigned short polygon_count, unsigned short,
	unsigned short vertex_count)
{
	log_chunk(1, polygon_count, vertex_count);
}

int g_w6_streak_undefined = 0; /* set by the edit of the cut in the branch that reads new_direction before writing it */

namespace {
struct StreakSeedAccess : public Vector3Randomizer {
	static Random3Class &Gen() { return Randomizer; }
};
float fw(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
}

static TextureClass g_texture;

extern "C" {

int ref_w6_streak_seq(const unsigned *in, int nin, unsigned *out, int cap)
{
	g_out.clear();
	g_w6_streak_undefined = 0;
	int k = 0;
	int renders_undefined = 0;
#define NEXT() (k < nin ? in[k++] : (abort(), 0u))
	unsigned s3a = NEXT(), s3b = NEXT();
	StreakSeedAccess::Gen() = Random3Class(s3a, s3b);
	StreakRendererClass *r = new StreakRendererClass;
	std::vector<Vector3> pts;
	std::vector<Vector4> cols;
	std::vector<float> widths;
	std::vector<unsigned> pers;
	while (k < nin) {
		unsigned op = NEXT();
		put(op);
		switch (op) {
		case 0: {
			W3dEmitterLinePropertiesStruct p;
			memset(&p, 0, sizeof(p));
			p.Flags = NEXT();
			p.SubdivisionLevel = NEXT();
			p.NoiseAmplitude = fw(NEXT());
			p.MergeAbortFactor = fw(NEXT());
			r->Init(p);
			break;
		}
		case 1: {
			unsigned tex = NEXT();
			r->Set_Texture(tex ? &g_texture : NULL);
			r->Set_Shader(ShaderClass(NEXT()));
			r->Set_Width(fw(NEXT()));
			float cr = fw(NEXT()), cg = fw(NEXT()), cb = fw(NEXT());
			r->Set_Color(Vector3(cr, cg, cb));
			r->Set_Opacity(fw(NEXT()));
			break;
		}
		case 2: {
			float v[12], t[12];
			for (int i = 0; i < 12; ++i) v[i] = fw(NEXT());
			for (int i = 0; i < 12; ++i) t[i] = fw(NEXT());
			g_view.Init(Vector4(v[0], v[1], v[2], v[3]), Vector4(v[4], v[5], v[6], v[7]), Vector4(v[8], v[9], v[10], v[11]),
				Vector4(0, 0, 0, 1));
			Matrix3D transform(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11]);
			unsigned n = NEXT();
			pts.resize(n + 1);
			cols.resize(n + 1);
			widths.resize(n + 1);
			for (unsigned i = 0; i < n; ++i) {
				float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
				pts[i].Set(x, y, z);
			}
			for (unsigned i = 0; i <= n; ++i) {
				float a = fw(NEXT()), b = fw(NEXT()), c = fw(NEXT()), d = fw(NEXT());
				cols[i].Set(a, b, c, d);
			}
			for (unsigned i = 0; i <= n; ++i) widths[i] = fw(NEXT());
			unsigned m = NEXT();
			pers.resize(m + 1);
			for (unsigned i = 0; i < m; ++i) pers[i] = NEXT();
			static char rinfo[16];
			SphereClass sphere;
			g_w6_streak_undefined = 0;
			r->RenderStreak(*reinterpret_cast<RenderInfoClass *>(rinfo), transform, n, &pts[0], &cols[0], &widths[0], sphere, &pers[0]);
			renders_undefined += g_w6_streak_undefined != 0;
			put('E');
			break;
		}
		default: abort();
		}
		putf(r->Width);
		putf(r->NoiseAmplitude);
		putf(r->MergeAbortFactor);
		put(r->SubdivisionLevel);
		put(r->Bits);
		Random3Class &g = StreakSeedAccess::Gen();
		put((uint32_t)g.Seed);
		put((uint32_t)g.Index);
	}
#undef NEXT
	put((uint32_t)renders_undefined);
	delete r;
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], 4 * n);
	return (int)g_out.size();
}

} /* extern "C" */
