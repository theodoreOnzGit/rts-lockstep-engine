/* Forced include (-include, after w3d_prelude.h) for the segmented-line
** renderer library, differential tests only (GitHub #65; see build.rs and
** differential/wrap_segline.cpp).
**
** WW3D2/seglinerenderer.cpp is compiled VERBATIM (w3d_gen/segline_def.cpp,
** from its first #include to the end). What it needs from the renderer is
** replaced here, the real headers' include guards defined first:
**   ww3d.h (WW3D_H): WW3D::Get_Sync_Time, the wrapper's clock.
**   rinfo.h (RINFO_H): RenderInfoClass, only passed by reference.
**   texture.h (TEXTURE_H): TextureClass, reference counting only (a stand-in
**     the test makes or not; Render only tests it for NULL and hands it on).
**   dx8wrapper.h (DX8_WRAPPER_H): DX8Wrapper's transforms (Get_Transform
**     answers the test's view; Set_Transform records nothing), its
**     Convert_Color (both: the Vector4 one through ref_dx8_convert_color, the
**     mesh library's GNU-asm transcription of the MSVC asm; the
**     Vector3-and-alpha one is the same instructions on the same four
**     floats), and the draw calls, which record what Render set: the vertex
**     and index buffers, the material, texture and shader, and whether the
**     triangles went to the sorting renderer.
**   dx8vertexbuffer.h, dx8indexbuffer.h, dx8fvf.h: DynamicVBAccessClass and
**     DynamicIBAccessClass over the recorder's arrays, FVFInfoClass with the
**     offsets of dynamic_fvf_type (position 0, normal 12, diffuse 24, first
**     texture 28, size 44), VertexFormatXYZDUV1 (dx8fvf.h's struct).
**   sortingrenderer.h (SORTING_RENDERER_H): Insert_Triangles records.
**   vertmaterial.h (VERTMATERIAL_H): VertexMaterialClass::Get_Preset answers
**     a stand-in material carrying the preset's number.
**   meshgeometry.h (MESHGEOMETRY_H): only its TriIndex typedef (Vector3i16).
** The classes are renamed (RefSegLine*) so that they cannot collide with the
** other libraries' stand-ins of the same names (part_prelude.h's
** SegLineRendererClass, mesh_prelude.h's DX8Wrapper and WW3D). */
#ifndef SHIM_SEGLINE_PRELUDE_H
#define SHIM_SEGLINE_PRELUDE_H

#define SegLineRendererClass RefSegLineRendererClass
#define DX8Wrapper RefSegLineDX8Wrapper
#define WW3D RefSegLineWW3D
#define TextureClass RefSegLineTextureClass
#define VertexMaterialClass RefSegLineVertexMaterialClass
#define SortingRendererClass RefSegLineSortingRendererClass
#define DynamicVBAccessClass RefSegLineDynamicVBAccessClass
#define DynamicIBAccessClass RefSegLineDynamicIBAccessClass
#define FVFInfoClass RefSegLineFVFInfoClass
#define RenderInfoClass RefSegLineRenderInfoClass

#define WW3D_H
#define RINFO_H
#define TEXTURE_H
#define DX8_WRAPPER_H
#define DX8VERTEXBUFFER_H
#define DX8INDEXBUFFER_H
#define DX8_FVF_H
#define SORTING_RENDERER_H
#define VERTMATERIAL_H
#define MESHGEOMETRY_H

#include "always.h"
#include "refcount.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "matrix4.h"
#include "vector3i.h"
#include "shader.h"
#include "w3d_file.h" /* upstream reaches it through meshgeometry.h's includes */

class SphereClass;
class RenderInfoClass;

/* meshgeometry.h */
typedef Vector3i16 TriIndex;

/* ww3d.h */
class WW3D
{
public:
	static unsigned int Get_Sync_Time(void);
};

/* texture.h */
class TextureClass
{
public:
	void Add_Ref(void) {}
	void Release_Ref(void) {}
	const char *Get_Texture_Name(void) { return "segline"; }
};

/* vertmaterial.h */
class VertexMaterialClass
{
public:
	enum PresetType { PRELIT_DIFFUSE = 0, PRELIT_NODIFFUSE, PRESET_COUNT };
	int Preset;
	static VertexMaterialClass *Get_Preset(PresetType type);
	void Release_Ref(void) {}
};

/* dx8fvf.h */
struct VertexFormatXYZDUV1
{
	float x;
	float y;
	float z;
	unsigned int diffuse;
	float u1;
	float v1;
};
class FVFInfoClass
{
public:
	unsigned Get_Location_Offset() const { return 0; }
	unsigned Get_Diffuse_Offset() const { return 24; }
	unsigned Get_Tex_Offset(unsigned) const { return 28; }
	unsigned Get_FVF_Size() const { return 44; }
};

/* dx8vertexbuffer.h, dx8indexbuffer.h */
enum { BUFFER_TYPE_DX8, BUFFER_TYPE_SORTING, BUFFER_TYPE_DYNAMIC_DX8, BUFFER_TYPE_DYNAMIC_SORTING };
const unsigned dynamic_fvf_type = 0x1 | 0x2 | 0x4 | 0x8; /* stands for D3DFVF_XYZ|NORMAL|TEX2|DIFFUSE */
class DynamicVBAccessClass
{
public:
	DynamicVBAccessClass(unsigned type, unsigned fvf, unsigned short vertex_count);
	const FVFInfoClass &FVF_Info() const { return Info; }
	class WriteLockClass
	{
	public:
		WriteLockClass(DynamicVBAccessClass *vb) : VB(vb) {}
		unsigned char *Get_Formatted_Vertex_Array();
		DynamicVBAccessClass *VB;
	};
	unsigned Type;
	unsigned short Count;
	FVFInfoClass Info;
};
class DynamicIBAccessClass
{
public:
	DynamicIBAccessClass(unsigned short type, unsigned short index_count);
	class WriteLockClass
	{
	public:
		WriteLockClass(DynamicIBAccessClass *ib) : IB(ib) {}
		unsigned short *Get_Index_Array();
		DynamicIBAccessClass *IB;
	};
	unsigned Type;
	unsigned short Count;
};
void IndexBufferExceptionFunc(void);

/* dx8wrapper.h */
enum D3DTRANSFORMSTATETYPE { D3DTS_VIEW = 2, D3DTS_PROJECTION = 3, D3DTS_WORLD = 256 };
class DX8Wrapper
{
public:
	static void Get_Transform(D3DTRANSFORMSTATETYPE transform, Matrix4x4 &m);
	static void Set_Transform(D3DTRANSFORMSTATETYPE transform, const Matrix4x4 &m);
	static unsigned int Convert_Color(const Vector4 &color);
	static unsigned int Convert_Color(const Vector3 &color, const float alpha);
	static void Set_Index_Buffer(const DynamicIBAccessClass &ib, unsigned short index_base_offset);
	static void Set_Vertex_Buffer(const DynamicVBAccessClass &vb);
	static void Set_Material(const VertexMaterialClass *material);
	static void Set_Texture(unsigned stage, TextureClass *texture);
	static void Set_Shader(const ShaderClass &shader);
	static void Draw_Triangles(unsigned short start_index, unsigned short polygon_count, unsigned short min_vertex_index,
		unsigned short vertex_count);
};

/* sortingrenderer.h */
class SortingRendererClass
{
public:
	static void Insert_Triangles(const SphereClass &bounding_sphere, unsigned short start_index, unsigned short polygon_count,
		unsigned short min_vertex_index, unsigned short vertex_count);
};

#endif
