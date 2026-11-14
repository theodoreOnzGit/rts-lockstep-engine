/* Differential-test wrapper for the segmented-line renderer (GitHub #65;
** upstream commit 0a05454d):
**   WW3D2/seglinerenderer.cpp from its first #include to the end
**     (SegLineRendererClass: the constructors, operator=, the destructor,
**     Init, Set_Texture, Get_Texture, Set_Current_UV_Offset,
**     Set_Texture_Tile_Factor, Reset_Line, Render, subdivision_util, Scale,
**     getVertexBuffer), COMPILED VERBATIM (w3d_gen/segline_def.cpp, see
**     build.rs), with seglinerenderer.h, vp.h, Vector3i.h, random.h and
**     v3_rnd.h unchanged; random.cpp and v3_rnd.cpp come from the emitter
**     library (upstream_w3d_emit), VectorProcessorClass::Transform from the
**     deformation library (upstream_w3d_deform, vp.cpp verbatim),
**     Matrix3D::mul from upstream_wwmath (matrix3d.cpp), the
**     _PresetAdditiveSpriteShader from the emitter library (shader.cpp's
**     preset, verbatim) and DX8Wrapper::Convert_Color's transcription from
**     the mesh library (ref_dx8_convert_color).
**
** The device (shim_w3d/segline_prelude.h) is a recorder: each chunk's
** Draw_Triangles or Insert_Triangles logs the vertex and index buffers as
** Render wrote them, the shader, material and texture it set, and which of
** the two drew. Classes are renamed RefSegLine* by the prelude.
**
** The driver (ref_segline_seq), words in (u32, floats as their bits):
**   s3a s3b              the randomizers' Random3Class (Vector3Randomizer::
**                        Randomizer) reseeded
**   sync                 the clock; the renderer is made
**   then operations until the input ends, each a code and its words:
**     0 Init: flags level noise abort tile u_per_sec v_per_sec
**     1 setters: texture(0/1) shader width r g b opacity
**     2 Render: sync, view (12, rows of the 4x4's top three rows; the
**       fourth row is 0 0 0 1), transform (12), n, has_colours, then n
**       points (3 each) and, with colours, n RGBAs (4 each)
**     3 Scale: factor
**     4 Reset_Line: sync
**     5 Set_Texture_Tile_Factor: factor
**   Words out, per operation: the code, then for Render the chunks (each
**   'C', the sorting flag, the vertex count, each vertex's x y z diffuse u
**   v, the triangle count, each triangle's I J K, the shader bits, the
**   material's preset, the texture's presence), then 'E'; after every
**   operation the renderer's Width, NoiseAmplitude, TextureTileFactor,
**   MergeAbortFactor, SubdivisionLevel, Bits, LastUsedSyncTime,
**   CurrentUVOffset (2), UVOffsetDeltaPerMS (2) and the shared generator's
**   Seed and Index. tests/differential_segline.rs builds the same words. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

#define private public
#define protected public
#include "random.h"
#include "v3_rnd.h"
#include "w3d_file.h"
#include "seglinerenderer.h"
#undef private
#undef protected
#include "sphere.h"

extern "C" unsigned ref_dx8_convert_color(const float *rgba); /* wrap_w3d_mesh.cpp */

static unsigned g_sync;
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

unsigned int WW3D::Get_Sync_Time(void) { return g_sync; }

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
void IndexBufferExceptionFunc(void) {}

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
void DX8Wrapper::Set_Index_Buffer(const DynamicIBAccessClass &, unsigned short) {}
void DX8Wrapper::Set_Vertex_Buffer(const DynamicVBAccessClass &) {}
void DX8Wrapper::Set_Material(const VertexMaterialClass *material) { g_mat = material ? material->Preset : -1; }
void DX8Wrapper::Set_Texture(unsigned, TextureClass *texture) { g_tex = texture != NULL; }
void DX8Wrapper::Set_Shader(const ShaderClass &shader) { g_shader = shader.Get_Bits(); }

/* one chunk drawn: see the words above */
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

namespace {
struct SeglineSeedAccess : public Vector3Randomizer {
	static Random3Class &Gen() { return Randomizer; }
};
float fw(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
}

static TextureClass g_texture;

extern "C" {

int ref_segline_seq(const unsigned *in, int nin, unsigned *out, int cap)
{
	g_out.clear();
	int k = 0;
#define NEXT() (k < nin ? in[k++] : (abort(), 0u))
	unsigned s3a = NEXT(), s3b = NEXT();
	SeglineSeedAccess::Gen() = Random3Class(s3a, s3b);
	g_sync = NEXT();
	SegLineRendererClass *r = new SegLineRendererClass;
	std::vector<Vector3> pts;
	std::vector<Vector4> cols;
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
			p.TextureTileFactor = fw(NEXT());
			p.UPerSec = fw(NEXT());
			p.VPerSec = fw(NEXT());
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
			g_sync = NEXT();
			float v[12], t[12];
			for (int i = 0; i < 12; ++i) v[i] = fw(NEXT());
			for (int i = 0; i < 12; ++i) t[i] = fw(NEXT());
			g_view.Init(Vector4(v[0], v[1], v[2], v[3]), Vector4(v[4], v[5], v[6], v[7]), Vector4(v[8], v[9], v[10], v[11]),
				Vector4(0, 0, 0, 1));
			Matrix3D transform(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11]);
			unsigned n = NEXT();
			unsigned has_cols = NEXT();
			pts.resize(n + 1);
			cols.resize(n + 1);
			for (unsigned i = 0; i < n; ++i) {
				float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
				pts[i].Set(x, y, z);
			}
			if (has_cols) {
				for (unsigned i = 0; i < n; ++i) {
					float a = fw(NEXT()), b = fw(NEXT()), c = fw(NEXT()), d = fw(NEXT());
					cols[i].Set(a, b, c, d);
				}
			}
			static char rinfo[16];
			SphereClass sphere;
			r->Render(*reinterpret_cast<RenderInfoClass *>(rinfo), transform, n, &pts[0], sphere, has_cols ? &cols[0] : NULL);
			put('E');
			break;
		}
		case 3: r->Scale(fw(NEXT())); break;
		case 4: g_sync = NEXT(); r->Reset_Line(); break;
		case 5: r->Set_Texture_Tile_Factor(fw(NEXT())); break;
		default: abort();
		}
		putf(r->Width);
		putf(r->NoiseAmplitude);
		putf(r->TextureTileFactor);
		putf(r->MergeAbortFactor);
		put(r->SubdivisionLevel);
		put(r->Bits);
		put(r->LastUsedSyncTime);
		putf(r->CurrentUVOffset.X);
		putf(r->CurrentUVOffset.Y);
		putf(r->UVOffsetDeltaPerMS.X);
		putf(r->UVOffsetDeltaPerMS.Y);
		Random3Class &g = SeglineSeedAccess::Gen();
		put((uint32_t)g.Seed);
		put((uint32_t)g.Index);
	}
#undef NEXT
	delete r;
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], 4 * n);
	return (int)g_out.size();
}

} /* extern "C" */
