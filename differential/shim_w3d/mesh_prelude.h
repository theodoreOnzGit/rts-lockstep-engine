/* Forced include (-include, after w3d_prelude.h) for the W3D mesh sources,
** differential tests only: WW3D2/meshmdlio.cpp, meshmatdesc.cpp,
** w3d_util.cpp, WWLib/realcrc.cpp (all compiled unchanged) and
** differential/wrap_w3d_mesh.cpp.
**
** The mesh loader is compiled with the real meshmdl.h, meshgeometry.h,
** meshmatdesc.h, shader.h, w3d_file.h and chunkio.h, so every class layout
** and inline function it uses is upstream's. The headers below are replaced
** (their include guards defined first) because they bring in Direct3D 8,
** whose SDK is not in the release, or the renderer:
**
**   dx8wrapper.h, dx8caps.h, dx8list.h, rinfo.h, ww3d.h: stand-ins that
**     declare only what the loader calls (WW3D's pre-lit mode and load
**     options; DX8Wrapper::Convert_Color, whose x87 asm is transcribed in
**     wrap_w3d_mesh.cpp, as BaseType.h's helpers are in wrap_base_type.cpp);
**   vertmaterial.h, texture.h, matinfo.h, aabtree.h: stand-in classes with
**     the members the loader and meshmatdesc.cpp use. Their loading
**     functions (VertexMaterialClass::Load_W3D, Load_Texture,
**     AABTreeClass::Load_W3D) are TRANSCRIBED in wrap_w3d_mesh.cpp from
**     vertmaterial.cpp, texture.cpp and aabtree.cpp, which cannot be
**     compiled here (Direct3D; the collision system), and they record what
**     they read so the test can compare it. The material methods that
**     meshmatdesc.cpp's post-load processing calls do nothing here: that
**     processing is not part of what is compared (see wrap_w3d_mesh.cpp).
**
** The MeshGeometryClass functions of meshgeometry.cpp the loader calls
** (Reset_Geometry, Set_Name, read_vertices, ...) are likewise transcribed in
** wrap_w3d_mesh.cpp; meshgeometry.cpp itself pulls in the render-object and
** collision systems. */
#ifndef SHIM_MESH_PRELUDE_H
#define SHIM_MESH_PRELUDE_H

#define DX8_WRAPPER_H
#define DX8CAPS_H
#define DX8LIST_H
#define RINFO_H
#define WW3D_H
#define VERTMATERIAL_H
#define TEXTURE_H
#define MATINFO_H
#define AABTREE_H

#include "always.h"
#include "refcount.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "w3d_file.h"
#include "w3derr.h"
#include "chunkio.h"
#include "shader.h"
#include "multilist.h"
#include "vector_this.h"
#include <string>
#include <vector>

/* MSVC <stdlib.h> */
#define _MAX_FNAME 256
#define _MAX_EXT 256

class MeshModelClass;
class MeshGeometryClass;
class RenderInfoClass;
class SpecialRenderInfoClass;

/* ---- ww3d.h --------------------------------------------------------- */
class WW3D
{
public:
	enum PrelitModeEnum { PRELIT_MODE_VERTEX, PRELIT_MODE_LIGHTMAP_MULTI_PASS, PRELIT_MODE_LIGHTMAP_MULTI_TEXTURE };
	static PrelitModeEnum PrelitMode;
	static PrelitModeEnum Get_Prelit_Mode() { return PrelitMode; }
	/* ww3d.cpp: MungeSortOnLoad = false, OverbrightModifyOnLoad = false */
	static bool Is_Munge_Sort_On_Load_Enabled() { return false; }
	static bool Is_Overbright_Modify_On_Load_Enabled() { return false; }
	static int Get_NPatches_Level() { return 1; }
};

/* ---- dx8wrapper.h, dx8caps.h ---------------------------------------- */
class DX8Caps
{
public:
	bool Support_Bump_Envmap() const { return false; }
	bool Support_NPatches() const { return false; }
};
class DX8Wrapper
{
public:
	static Vector4 Convert_Color(unsigned color);
	static unsigned int Convert_Color(const Vector3 &color, float alpha);
	static unsigned int Convert_Color(const Vector4 &color);
	static bool Is_Initted() { return false; }
	static DX8Caps *Get_Current_Caps() { abort(); }
};

/* ---- vertmaterial.h ------------------------------------------------- */
class VertexMaterialClass : public W3DMPO, public RefCountClass
{
	W3DMPO_GLUE(VertexMaterialClass)
public:
	enum ColorSourceType { MATERIAL = 0, COLOR1, COLOR2 };
	enum PresetType { PRELIT_DIFFUSE = 0, PRELIT_NODIFFUSE, PRESET_COUNT };
	VertexMaterialClass();
	VertexMaterialClass(const VertexMaterialClass &that);
	~VertexMaterialClass();
	WW3DErrorType Load_W3D(ChunkLoadClass &cload);
	WW3DErrorType Save_W3D(ChunkSaveClass &) { abort(); }
	void Set_Name(const char *) { abort(); }
	void Init_From_Material3(const W3dMaterial3Struct &) { abort(); }
	unsigned long Get_CRC(void) const { abort(); }
	/* used by meshmatdesc.cpp's post-load processing (not compared): no-ops */
	void Get_Ambient(Vector3 *set) const { *set = Vector3(0, 0, 0); }
	void Get_Diffuse(Vector3 *set) const { *set = Vector3(0, 0, 0); }
	void Get_Specular(Vector3 *set) const { *set = Vector3(0, 0, 0); }
	void Get_Emissive(Vector3 *set) const { *set = Vector3(0, 0, 0); }
	float Get_Opacity(void) const { return 1.0f; }
	void Set_Ambient(const Vector3 &) {}
	void Set_Diffuse(const Vector3 &) {}
	void Set_Specular(const Vector3 &) {}
	void Set_Emissive(const Vector3 &) {}
	void Set_Ambient(float, float, float) {}
	void Set_Diffuse(float, float, float) {}
	void Set_Specular(float, float, float) {}
	void Set_Emissive(float, float, float) {}
	void Set_Lighting(bool) {}
	void Set_Ambient_Color_Source(ColorSourceType) {}
	void Set_Diffuse_Color_Source(ColorSourceType) {}
	void Set_Emissive_Color_Source(ColorSourceType) {}
	void Set_UV_Source(int, int) {}
	int Get_UV_Source(int) { return 0; }
	bool Do_Mappers_Need_Normals(void) const { return false; }

	/* what Load_W3D read */
	int Serial;
	bool HasName;
	std::string Name;
	W3dVertexMaterialStruct Info;
	bool HasArgs[2];
	std::vector<char> Args[2];
};

/* ---- texture.h ------------------------------------------------------ */
class TextureFilterClass
{
public:
	enum FilterType { FILTER_TYPE_NONE, FILTER_TYPE_FAST, FILTER_TYPE_BEST, FILTER_TYPE_DEFAULT, FILTER_TYPE_COUNT };
	enum TxtAddrMode { TEXTURE_ADDRESS_REPEAT = 0, TEXTURE_ADDRESS_CLAMP };
	void Set_Mip_Mapping(FilterType f) { MipMapping = f; }
	void Set_U_Addr_Mode(TxtAddrMode m) { UAddr = m; }
	void Set_V_Addr_Mode(TxtAddrMode m) { VAddr = m; }
	int MipMapping = -1, UAddr = -1, VAddr = -1;
};
enum MipCountType { MIP_LEVELS_ALL = 0, MIP_LEVELS_1, MIP_LEVELS_2, MIP_LEVELS_3, MIP_LEVELS_4 };
enum WW3DFormat { WW3D_FORMAT_UNKNOWN = 0, WW3D_FORMAT_U8V8, WW3D_FORMAT_L6V5U5, WW3D_FORMAT_X8L8V8U8 };
class TextureClass : public W3DMPO, public RefCountClass
{
	W3DMPO_GLUE(TextureClass)
public:
	TextureClass(const char *name, MipCountType mips, WW3DFormat format);
	const char *Get_Texture_Name(void) const { return Name.c_str(); }
	TextureFilterClass &Get_Filter() { return Filter; }
	int Serial;
	std::string Name;
	int MipCount;
	int Format;
	TextureFilterClass Filter;
};
TextureClass *Load_Texture(ChunkLoadClass &cload);

/* ---- matinfo.h ------------------------------------------------------ */
class MaterialInfoClass : public W3DMPO, public RefCountClass
{
	W3DMPO_GLUE(MaterialInfoClass)
public:
	void Reset(void) {}
	void Add_Texture(TextureClass *) {}
	void Add_Vertex_Material(VertexMaterialClass *) {}
};
class MaterialCollectorClass {};
class MaterialRemapperClass
{
public:
	MaterialRemapperClass(MaterialInfoClass *, MaterialInfoClass *) { abort(); }
	void Remap_Mesh(void *, void *) { abort(); }
};

/* ---- aabtree.h ------------------------------------------------------ */
class AABTreeClass : public W3DMPO, public RefCountClass
{
	W3DMPO_GLUE(AABTreeClass)
public:
	struct CullNodeStruct
	{
		Vector3 Min;
		Vector3 Max;
		uint32 FrontOrPoly0;
		uint32 BackOrPolyCount;
	};
	AABTreeClass(void);
	~AABTreeClass(void);
	void Reset(void);
	void Load_W3D(ChunkLoadClass &cload);
	void Set_Mesh(MeshGeometryClass *) {}
	int NodeCount;
	CullNodeStruct *Nodes;
	int PolyCount;
	uint32 *PolyIndices;
};

/* ---- rinfo.h, dx8list.h: only named in meshmdl.h -------------------- */
class DX8PolygonRendererClass;
class DX8FVFCategoryContainer;
class DecalGeneratorClass;
class MeshClass;
class GapFillerClass;
typedef MultiListClass<DX8PolygonRendererClass> DX8PolygonRendererList;

#endif
