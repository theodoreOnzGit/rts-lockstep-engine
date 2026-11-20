/* Forced include (-include, after w3d_prelude.h) for the streak renderer library, differential tests only (GitHub #215, group ww3d6; see build.rs
** `drafts_ww3d6` and differential/wrap_draft_w6_streak.cpp). A copy of shim_w3d/segline_prelude.h (the segmented-line renderer's recorder device) with the classes
** renamed RefStreak* so that they cannot collide with that library's, and what WW3D2/streakRender.cpp needs beyond seglinerenderer.cpp:
**   - VertexFormatXYZUV1 (dx8fvf.h's struct: x, y, z, u1, v1), the vertex type of getVertexBuffer;
**   - DX8Wrapper::Convert_Color_Clamp and Clamp_Color, declared here and defined VERBATIM in the wrapper (dx8wrapper.h / dx8wrapper.cpp);
**   - DEBUG_ASSERTCRASH, empty (the release build).
** streakRender.cpp includes "streakrender.h" (the file is streakRender.h upstream: build.rs writes the lower-case copy). */
#ifndef SHIM_STREAK_PRELUDE_H
#define SHIM_STREAK_PRELUDE_H

#define DX8Wrapper RefStreakDX8Wrapper
#define WW3D RefStreakWW3D
#define TextureClass RefStreakTextureClass
#define VertexMaterialClass RefStreakVertexMaterialClass
#define SortingRendererClass RefStreakSortingRendererClass
#define DynamicVBAccessClass RefStreakDynamicVBAccessClass
#define DynamicIBAccessClass RefStreakDynamicIBAccessClass
#define FVFInfoClass RefStreakFVFInfoClass
#define RenderInfoClass RefStreakRenderInfoClass

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
	/* #169: what SegmentedLineClass::Render reads (declared here, defined by wrap_segline_obj.cpp; the renderer reads
	** none of them) */
	static bool Is_Sorting_Enabled(void);
	static bool Are_Static_Sort_Lists_Enabled(void);
	static void Add_To_Static_Sort_List(void *robj, unsigned int sort_level); /* a RenderObjClass * */
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
	static void Clamp_Color(Vector4 &color);
	static unsigned int Convert_Color_Clamp(const Vector4 &color);
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


/* dx8fvf.h */
struct VertexFormatXYZUV1
{
	float x;
	float y;
	float z;
	float u1;
	float v1;
};

#ifndef DEBUG_ASSERTCRASH
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#endif

#endif
