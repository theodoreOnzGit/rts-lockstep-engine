/* Differential-test wrapper for the line group (GitHub #65, batch e;
** upstream commit 0a05454d):
**   WW3D2/linegrp.cpp from its first #include to the end (LineGroupClass:
**     the constructor, the destructor, Set_Arrays, the setters and getters,
**     Set_Flag, Get_Flag, Set_Texture, Get_Texture, Peek_Texture,
**     Set_Shader, Get_Shader, Set_Line_Mode, Get_Line_Mode, Render,
**     Get_Polygon_Count), COMPILED VERBATIM (w3d_gen/linegrp_def.cpp, see
**     build.rs), with linegrp.h, sharebuf.h, wwmath.h, matrix3d.h and
**     shader.h unchanged; Matrix3D::Get_Orthogonal_Inverse from
**     upstream_wwmath (matrix3d.cpp), the _PresetAdditiveSpriteShader from
**     the emitter library (shader.cpp's preset, verbatim) and
**     DX8Wrapper::Convert_Color's transcription from the mesh library
**     (ref_dx8_convert_color). WWMath::Cos and Sin are the portable branches
**     (cosf, sinf), bound to the Rust translation's petir functions in the
**     test binary; the library is compiled with -fno-builtin-cosf and
**     -fno-builtin-sinf so that the compiler cannot fold them with the host's.
**
** The device (shim_w3d/linegrp_prelude.h) is a recorder: Render's
** Draw_Triangles or Insert_Triangles logs the vertex and index buffers as
** Render wrote them, the shader, material and texture it set, which of the two
** drew, and whether the view had been set (to the identity: TRANSFORM off).
** Classes are renamed RefLineGrp* by the prelude.
**
** The driver (ref_linegrp_seq), words in (u32, floats as their bits): a line
** group is made, then operations until the input ends, each a code and its
** words:
**   0 setters: a mask, then for each bit set, in order: 0x1 texture (0/1),
**     0x2 shader, 0x4 Set_Line_Mode (0 tetrahedron, 1 prism), 0x8 Set_Flag(
**     TRANSFORM, on), 0x10 Set_Line_Size, 0x20 Set_Line_Color (r g b),
**     0x40 Set_Line_Alpha, 0x80 Set_Line_UCoord, 0x100 Set_Tail_Diffuse
**     (r g b a)
**   1 Set_Arrays: n, a mask of the optional arrays (0x1 diffuse, 0x2 tail
**     diffuse, 0x4 ALT, 0x8 sizes, 0x10 U coordinates), the active count (an
**     int), then n start points (3 each), n end points (3 each) and, present,
**     n diffuse (4 each), n tail diffuse (4 each), n ALT entries, n sizes, n
**     U coordinates
**   2 Render: sorting enabled (0/1), the camera's transform (12, rows)
**   Words out, per operation: the code, then for Render the draw if there is
**   one ('C', the sorting flag, the vertex count, each vertex's x y z
**   diffuse u v, the triangle count, each triangle's I J K, the shader bits,
**   the material's preset, the texture's presence, whether the view was set)
**   and 'E'; after every operation Flags, DefaultLineSize, DefaultLineColor
**   (3), DefaultLineAlpha, DefaultLineUCoord, DefaultTailDiffuse (4),
**   LineMode, LineCount, the shader bits, the texture's presence and
**   Get_Polygon_Count(). tests/differential_linegroup.rs builds the same
**   words. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

#define private public
#define protected public
#include "sharebuf.h"
#include "linegrp.h"
#undef private
#undef protected

extern "C" unsigned ref_dx8_convert_color(const float *rgba); /* wrap_w3d_mesh.cpp */

static std::vector<uint32_t> g_out;
static VertexFormatXYZNDUV2 g_vb[6 * 1024];
static unsigned short g_ib[24 * 1024];
static int g_mat;
static int g_tex;
static unsigned g_shader;
static unsigned g_vcount, g_icount;
static bool g_sorting;
static int g_view_set;

static void put(uint32_t w) { g_out.push_back(w); }
static void putf(float f) { uint32_t w; memcpy(&w, &f, 4); put(w); }

bool WW3D::Is_Sorting_Enabled(void) { return g_sorting; }

static VertexMaterialClass g_presets[2];
VertexMaterialClass *VertexMaterialClass::Get_Preset(PresetType type)
{
	g_presets[type].Preset = type;
	return &g_presets[type];
}

DynamicVBAccessClass::DynamicVBAccessClass(unsigned type, unsigned, unsigned short vertex_count) : Type(type), Count(vertex_count)
{
	if (vertex_count > 6 * 1024) abort();
	g_vcount = vertex_count;
	memset(g_vb, 0, sizeof(g_vb));
}
VertexFormatXYZNDUV2 *DynamicVBAccessClass::WriteLockClass::Get_Formatted_Vertex_Array() { return g_vb; }
DynamicIBAccessClass::DynamicIBAccessClass(unsigned short type, unsigned short index_count) : Type(type), Count(index_count)
{
	if (index_count > 24 * 1024) abort();
	g_icount = index_count;
}
unsigned short *DynamicIBAccessClass::WriteLockClass::Get_Index_Array() { return g_ib; }
void IndexBufferExceptionFunc(void) {}

void DX8Wrapper::Get_Transform(D3DTRANSFORMSTATETYPE transform, Matrix4x4 &m)
{
	if (transform != D3DTS_VIEW) abort();
	m = Matrix4x4(true);
}
void DX8Wrapper::Set_Transform(D3DTRANSFORMSTATETYPE transform, const Matrix4x4 &)
{
	if (transform == D3DTS_VIEW) g_view_set = 1;
}
unsigned int DX8Wrapper::Convert_Color(const Vector4 &color)
{
	float c[4] = {color.X, color.Y, color.Z, color.W};
	return ref_dx8_convert_color(c);
}
void DX8Wrapper::Set_Index_Buffer(const DynamicIBAccessClass &, unsigned short) {}
void DX8Wrapper::Set_Vertex_Buffer(const DynamicVBAccessClass &) {}
void DX8Wrapper::Set_Material(const VertexMaterialClass *material) { g_mat = material ? material->Preset : -1; }
void DX8Wrapper::Set_Texture(unsigned, TextureClass *texture) { g_tex = texture != NULL; }
void DX8Wrapper::Set_Shader(const ShaderClass &shader) { g_shader = shader.Get_Bits(); }

/* the draw: see the words above */
static void log_draw(int sorting, unsigned short start_index, unsigned short polygon_count, unsigned short min_vertex_index,
	unsigned short vertex_count)
{
	if (start_index != 0 || min_vertex_index != 0) abort();
	if (vertex_count != g_vcount || polygon_count * 3u != g_icount) abort();
	put('C');
	put(sorting);
	put(vertex_count);
	for (unsigned i = 0; i < vertex_count; ++i) {
		const VertexFormatXYZNDUV2 &v = g_vb[i];
		putf(v.x); putf(v.y); putf(v.z); put(v.diffuse); putf(v.u1); putf(v.v1);
	}
	put(polygon_count);
	for (unsigned i = 0; i < 3u * polygon_count; ++i) put(g_ib[i]);
	put(g_shader);
	put((uint32_t)g_mat);
	put((uint32_t)g_tex);
	put((uint32_t)g_view_set);
}
void DX8Wrapper::Draw_Triangles(unsigned short start_index, unsigned short polygon_count, unsigned short min_vertex_index,
	unsigned short vertex_count)
{
	log_draw(0, start_index, polygon_count, min_vertex_index, vertex_count);
}
void SortingRendererClass::Insert_Triangles(unsigned short start_index, unsigned short polygon_count, unsigned short min_vertex_index,
	unsigned short vertex_count)
{
	log_draw(1, start_index, polygon_count, min_vertex_index, vertex_count);
}

namespace {
float fw(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
}

static TextureClass g_texture;

extern "C" {

int ref_linegrp_seq(const unsigned *in, int nin, unsigned *out, int cap)
{
	g_out.clear();
	int k = 0;
#define NEXT() (k < nin ? in[k++] : (abort(), 0u))
	LineGroupClass *g = new LineGroupClass;
	CameraClass camera;
	RenderInfoClass rinfo(camera);
	while (k < nin) {
		unsigned op = NEXT();
		put(op);
		switch (op) {
		case 0: {
			unsigned mask = NEXT();
			if (mask & 0x1) g->Set_Texture(NEXT() ? &g_texture : NULL);
			if (mask & 0x2) g->Set_Shader(ShaderClass(NEXT()));
			if (mask & 0x4) g->Set_Line_Mode(NEXT() ? LineGroupClass::PRISM : LineGroupClass::TETRAHEDRON);
			if (mask & 0x8) g->Set_Flag(LineGroupClass::TRANSFORM, NEXT() != 0);
			if (mask & 0x10) g->Set_Line_Size(fw(NEXT()));
			if (mask & 0x20) {
				float r = fw(NEXT()), gg = fw(NEXT()), b = fw(NEXT());
				g->Set_Line_Color(Vector3(r, gg, b));
			}
			if (mask & 0x40) g->Set_Line_Alpha(fw(NEXT()));
			if (mask & 0x80) g->Set_Line_UCoord(fw(NEXT()));
			if (mask & 0x100) {
				float r = fw(NEXT()), gg = fw(NEXT()), b = fw(NEXT()), a = fw(NEXT());
				g->Set_Tail_Diffuse(Vector4(r, gg, b, a));
			}
			break;
		}
		case 1: {
			int n = (int)NEXT();
			unsigned mask = NEXT();
			int active = (int)NEXT();
			ShareBufferClass<Vector3> *start = new ShareBufferClass<Vector3>(n, "start");
			ShareBufferClass<Vector3> *end = new ShareBufferClass<Vector3>(n, "end");
			ShareBufferClass<Vector4> *diffuse = (mask & 0x1) ? new ShareBufferClass<Vector4>(n, "diffuse") : NULL;
			ShareBufferClass<Vector4> *tail = (mask & 0x2) ? new ShareBufferClass<Vector4>(n, "tail") : NULL;
			ShareBufferClass<unsigned int> *alt = (mask & 0x4) ? new ShareBufferClass<unsigned int>(n, "alt") : NULL;
			ShareBufferClass<float> *sizes = (mask & 0x8) ? new ShareBufferClass<float>(n, "sizes") : NULL;
			ShareBufferClass<float> *ucoords = (mask & 0x10) ? new ShareBufferClass<float>(n, "ucoords") : NULL;
			for (int i = 0; i < n; ++i) {
				float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
				start->Set_Element(i, Vector3(x, y, z));
			}
			for (int i = 0; i < n; ++i) {
				float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT());
				end->Set_Element(i, Vector3(x, y, z));
			}
			ShareBufferClass<Vector4> *v4[2] = {diffuse, tail};
			for (int a = 0; a < 2; ++a) {
				if (!v4[a]) continue;
				for (int i = 0; i < n; ++i) {
					float x = fw(NEXT()), y = fw(NEXT()), z = fw(NEXT()), w = fw(NEXT());
					v4[a]->Set_Element(i, Vector4(x, y, z, w));
				}
			}
			if (alt) for (int i = 0; i < n; ++i) alt->Set_Element(i, NEXT());
			if (sizes) for (int i = 0; i < n; ++i) sizes->Set_Element(i, fw(NEXT()));
			if (ucoords) for (int i = 0; i < n; ++i) ucoords->Set_Element(i, fw(NEXT()));
			g->Set_Arrays(start, end, diffuse, tail, alt, sizes, ucoords, active);
			start->Release_Ref();
			end->Release_Ref();
			if (diffuse) diffuse->Release_Ref();
			if (tail) tail->Release_Ref();
			if (alt) alt->Release_Ref();
			if (sizes) sizes->Release_Ref();
			if (ucoords) ucoords->Release_Ref();
			break;
		}
		case 2: {
			g_sorting = NEXT() != 0;
			float t[12];
			for (int i = 0; i < 12; ++i) t[i] = fw(NEXT());
			camera.Transform = Matrix3D(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11]);
			g_view_set = 0;
			g->Render(rinfo);
			put('E');
			break;
		}
		default: abort();
		}
		put(g->Flags);
		putf(g->DefaultLineSize);
		putf(g->DefaultLineColor.X);
		putf(g->DefaultLineColor.Y);
		putf(g->DefaultLineColor.Z);
		putf(g->DefaultLineAlpha);
		putf(g->DefaultLineUCoord);
		putf(g->DefaultTailDiffuse.X);
		putf(g->DefaultTailDiffuse.Y);
		putf(g->DefaultTailDiffuse.Z);
		putf(g->DefaultTailDiffuse.W);
		put((uint32_t)g->LineMode);
		put((uint32_t)g->LineCount);
		put(g->Shader.Get_Bits());
		put((uint32_t)(g->Peek_Texture() != NULL));
		put((uint32_t)g->Get_Polygon_Count());
	}
#undef NEXT
	delete g;
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], 4 * n);
	return (int)g_out.size();
}

} /* extern "C" */
