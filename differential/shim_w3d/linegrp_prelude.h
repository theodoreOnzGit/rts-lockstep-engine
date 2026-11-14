/* Forced include (-include, after w3d_prelude.h) for the line-group library,
** differential tests only (GitHub #65, batch e; see build.rs and
** differential/wrap_linegrp.cpp).
**
** WW3D2/linegrp.cpp is compiled VERBATIM (w3d_gen/linegrp_def.cpp, from its
** first #include to the end). What it needs from the renderer is replaced
** here, the real headers' include guards defined first:
**   ww3d.h (WW3D_H): WW3D::Is_Sorting_Enabled, the driver's switch.
**   rinfo.h (RINFO_H), camera.h (CAMERA_H): RenderInfoClass holding a
**     CameraClass whose Get_Transform answers the driver's camera.
**   texture.h (TEXTURE_H): TextureClass, reference counting only (a stand-in
**     the driver sets or not; Render tests it for NULL and hands it on).
**   dx8wrapper.h (DX8_WRAPPER_H): DX8Wrapper's transforms (Get_Transform
**     answers the identity view; Set_Transform records whether the view was
**     set while drawing), Convert_Color (the Vector4 one through
**     ref_dx8_convert_color, the mesh library's transcription of the MSVC
**     asm), and the draw calls, which record what Render set: the vertex and
**     index buffers, the material, texture and shader, and whether the
**     triangles went to the sorting renderer.
**   dx8vertexbuffer.h, dx8indexbuffer.h, dx8fvf.h: DynamicVBAccessClass and
**     DynamicIBAccessClass over the recorder's arrays, VertexFormatXYZNDUV2
**     (dx8fvf.h's struct).
**   sortingrenderer.h (SORTING_RENDERER_H): the four-argument
**     Insert_Triangles records.
**   vertmaterial.h (VERTMATERIAL_H): VertexMaterialClass::Get_Preset answers
**     a stand-in material carrying the preset's number.
** The classes are renamed (RefLineGrp*) so that they cannot collide with the
** other libraries' stand-ins of the same names (part_prelude.h's
** LineGroupClass and CameraClass, segline_prelude.h's DX8Wrapper, ...). */
#ifndef SHIM_LINEGRP_PRELUDE_H
#define SHIM_LINEGRP_PRELUDE_H

#define LineGroupClass RefLineGrpLineGroupClass
#define DX8Wrapper RefLineGrpDX8Wrapper
#define WW3D RefLineGrpWW3D
#define TextureClass RefLineGrpTextureClass
#define VertexMaterialClass RefLineGrpVertexMaterialClass
#define SortingRendererClass RefLineGrpSortingRendererClass
#define DynamicVBAccessClass RefLineGrpDynamicVBAccessClass
#define DynamicIBAccessClass RefLineGrpDynamicIBAccessClass
#define RenderInfoClass RefLineGrpRenderInfoClass
#define CameraClass RefLineGrpCameraClass
#define VertexFormatXYZNDUV2 RefLineGrpVertexFormatXYZNDUV2
#define IndexBufferExceptionFunc RefLineGrpIndexBufferExceptionFunc

#define WW3D_H
#define RINFO_H
#define CAMERA_H
#define TEXTURE_H
#define DX8_WRAPPER_H
#define DX8VERTEXBUFFER_H
#define DX8INDEXBUFFER_H
#define DX8_FVF_H
#define SORTING_RENDERER_H
#define VERTMATERIAL_H

#include "always.h"
#include "refcount.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "matrix4.h"
#include "shader.h"

/* ww3d.h */
class WW3D
{
public:
	static bool Is_Sorting_Enabled(void);
};

/* camera.h, rinfo.h */
class CameraClass
{
public:
	const Matrix3D &Get_Transform(void) const { return Transform; }
	Matrix3D Transform;
};
class RenderInfoClass
{
public:
	RenderInfoClass(CameraClass &cam) : Camera(cam) {}
	CameraClass &Camera;
};

/* texture.h */
class TextureClass
{
public:
	void Add_Ref(void) {}
	void Release_Ref(void) {}
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
struct VertexFormatXYZNDUV2
{
	float x;
	float y;
	float z;
	float nx;
	float ny;
	float nz;
	unsigned diffuse;
	float u1;
	float v1;
	float u2;
	float v2;
};

/* dx8vertexbuffer.h, dx8indexbuffer.h */
enum { BUFFER_TYPE_DX8, BUFFER_TYPE_SORTING, BUFFER_TYPE_DYNAMIC_DX8, BUFFER_TYPE_DYNAMIC_SORTING };
const unsigned dynamic_fvf_type = 0x1 | 0x2 | 0x4 | 0x8; /* stands for D3DFVF_XYZ|NORMAL|TEX2|DIFFUSE */
class DynamicVBAccessClass
{
public:
	DynamicVBAccessClass(unsigned type, unsigned fvf, unsigned short vertex_count);
	class WriteLockClass
	{
	public:
		WriteLockClass(DynamicVBAccessClass *vb) : VB(vb) {}
		VertexFormatXYZNDUV2 *Get_Formatted_Vertex_Array();
		DynamicVBAccessClass *VB;
	};
	unsigned Type;
	unsigned short Count;
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
	static void Insert_Triangles(unsigned short start_index, unsigned short polygon_count, unsigned short min_vertex_index,
		unsigned short vertex_count);
};

#endif
