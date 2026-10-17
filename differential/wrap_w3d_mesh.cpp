/* Differential-test wrapper for the W3D mesh loader (upstream commit
** 0a05454d): WW3D2/meshmdlio.cpp and WW3D2/meshmatdesc.cpp (copies with
** MSVC-scope edits, see build.rs), WW3D2/w3d_util.cpp and WWLib/realcrc.cpp
** (unchanged), with differential/shim_w3d/w3d_prelude.h and mesh_prelude.h
** force-included. The real meshmdl.h, meshgeometry.h, meshmatdesc.h and
** shader.h give every class layout and inline function.
**
** TRANSCRIBED here (the files cannot be compiled, see mesh_prelude.h), each
** from the upstream function named in its comment:
**   meshgeometry.cpp: the MeshGeometryClass constructor, Reset_Geometry,
**     Get_Name, Set_Name, read_vertices, read_vertex_normals,
**     read_triangles, read_user_text, read_vertex_influences,
**     read_vertex_shade_indices, read_aabtree, get_vert_normals,
**     get_planes (the scratch array is reset for each mesh, see below);
**   meshmdl.cpp: the MeshModelClass constructor and Reset;
**   vertmaterial.cpp: VertexMaterialClass::Load_W3D (the reading; the file
**     data is kept instead of being converted);
**   texture.cpp: Load_Texture;
**   aabtree.cpp: AABTreeClass::Load_W3D, Read_Poly_Indices, Read_Nodes,
**     Reset;
**   dx8wrapper.h: DX8Wrapper::Convert_Color, both directions; the Vector4
**     one is MSVC inline asm, transcribed to GNU asm (the same x87
**     instructions: fld, fmul, fistp with the rounding control set to chop).
** Generate_Culling_Tree (meshgeometry.cpp, it builds a tree from the
** geometry) only records that it was called.
**
** The snapshot: the Rust reader stops after reading the chunks (it does not
** translate install_materials and post_process). Upstream's first call after
** that is context->AlternateMatDesc.Is_Empty() (install_materials ->
** install_alternate_material_desc). meshmatdesc.cpp is compiled with
** -DIs_Empty=Is_Empty_Upstream so that this file can define
** MeshMatDescClass::Is_Empty: it logs the mesh and its loading context and
** throws, unwinding out of Load_W3D. (Past this point upstream's own code is
** undefined for some inputs the reading accepts, e.g. Post_Load_Process
** indexes the 4-entry pass arrays by a pass count read from the file.) */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <string>
#include <vector>

#define private public
#define protected public
#include "meshmdl.h"
#include "meshmatdesc.h"
#include "w3d_util.h"
#include "realcrc.h"
#include "assetmgr.h"
#include "mesh_load_context.h" /* MeshLoadContextClass, extracted from meshmdlio.cpp by build.rs */
#undef private
#undef protected

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
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
void putStr(const char *s, size_t n) { put32((uint32_t)n); put(s, n); }

MeshModelClass *g_mesh = NULL;
bool g_generated = false;
bool g_planes_used = false;
SimpleVecClass<Vector4> *g_planes = NULL;
int g_serial = 0;
struct Snapshot {};

} /* namespace */

WW3D::PrelitModeEnum WW3D::PrelitMode = WW3D::PRELIT_MODE_LIGHTMAP_MULTI_PASS;

/* ---- stand-ins never reached by what is compared (they abort) ------- */
/* StringClass (wwstring.cpp needs windows.h): only the empty string, which
** is all ddsfile.cpp makes (`StringClass tmp(0,true)`, then Format for an
** assert message the release build discards: Format keeps it empty). */
void StringClass::Free_String(void)
{
	if (m_Buffer != m_EmptyString) abort();
}
void StringClass::Get_String(int length, bool)
{
	if (length != 0) abort();
	m_Buffer = m_EmptyString;
}
int _cdecl StringClass::Format(const TCHAR *, ...) { return 0; }
TCHAR StringClass::m_NullChar = 0;
TCHAR *StringClass::m_EmptyString = &StringClass::m_NullChar;
void ShaderClass::Enable_Fog(const char *) { abort(); }
ShaderClass::StaticSortCategoryType ShaderClass::Get_SS_Category(void) const { abort(); }

/* ---- meshgeometry.cpp ------------------------------------------------ */
MeshGeometryClass::MeshGeometryClass(void) :
	MeshName(NULL), UserText(NULL), Flags(0), SortLevel(SORT_LEVEL_NONE), W3dAttributes(0), PolyCount(0), VertexCount(0),
	Poly(NULL), PolySurfaceType(NULL), Vertex(NULL), VertexNorm(NULL), PlaneEq(NULL), VertexShadeIdx(NULL),
	VertexBoneLink(NULL), BoundBoxMin(0, 0, 0), BoundBoxMax(1, 1, 1), BoundSphereCenter(0, 0, 0), BoundSphereRadius(1),
	CullTree(NULL)
{
}
MeshGeometryClass::~MeshGeometryClass(void) {}
WW3DErrorType MeshGeometryClass::Load_W3D(ChunkLoadClass &) { abort(); }
void MeshGeometryClass::Compute_Plane_Equations(Vector4 *) { abort(); }
void MeshGeometryClass::Compute_Vertex_Normals(Vector3 *) { abort(); }
void MeshGeometryClass::Compute_Bounds(Vector3 *) { abort(); }

void MeshGeometryClass::Reset_Geometry(int polycount, int vertcount)
{
	Flags = 0;
	PolyCount = 0;
	VertexCount = 0;
	SortLevel = SORT_LEVEL_NONE;
	REF_PTR_RELEASE(MeshName);
	REF_PTR_RELEASE(UserText);
	REF_PTR_RELEASE(Poly);
	REF_PTR_RELEASE(PolySurfaceType);
	REF_PTR_RELEASE(Vertex);
	REF_PTR_RELEASE(VertexNorm);
	REF_PTR_RELEASE(PlaneEq);
	REF_PTR_RELEASE(VertexShadeIdx);
	REF_PTR_RELEASE(VertexBoneLink);
	REF_PTR_RELEASE(CullTree);
	PolyCount = polycount;
	VertexCount = vertcount;
	if ((polycount != 0) && (vertcount != 0)) {
		Poly = NEW_REF(ShareBufferClass<TriIndex>, (PolyCount, "MeshGeometryClass::Poly"));
		PolySurfaceType = NEW_REF(ShareBufferClass<uint8>, (PolyCount, "MeshGeometryClass::PolySurfaceType"));
		Vertex = NEW_REF(ShareBufferClass<Vector3>, (VertexCount, "MeshGeometryClass::Vertex"));
		Poly->Clear();
		PolySurfaceType->Clear();
		Vertex->Clear();
		VertexNorm = NEW_REF(ShareBufferClass<Vector3>, (VertexCount, "MeshGeometryClass::VertexNorm"));
		VertexNorm->Clear();
	}
}

const char *MeshGeometryClass::Get_Name(void) const
{
	if (MeshName) return MeshName->Get_Array();
	return NULL;
}

void MeshGeometryClass::Set_Name(const char *newname)
{
	if (MeshName) MeshName->Release_Ref();
	if (newname) {
		MeshName = NEW_REF(ShareBufferClass<char>, (strlen(newname) + 1, "MeshGeometryClass::MeshName"));
		strcpy(MeshName->Get_Array(), newname);
	}
}

Vector3 *MeshGeometryClass::get_vert_normals(void)
{
	/* OPTIMIZE_VNORM_RAM is 0 */
	return VertexNorm->Get_Array();
}

Vector4 *MeshGeometryClass::get_planes(bool)
{
	/* OPTIMIZE_PLANEEQ_RAM is 1: a scratch array shared by all meshes
	** (static SimpleVecClass<Vector4> _PlaneEQArray(1024)); here one per load,
	** zero-filled, so that only this mesh's planes are logged. */
	g_planes_used = true;
	g_planes->Uninitialised_Grow(PolyCount);
	return &((*g_planes)[0]);
}

void MeshGeometryClass::Generate_Culling_Tree(void) { g_generated = true; }

WW3DErrorType MeshGeometryClass::read_vertices(ChunkLoadClass &cload)
{
	W3dVectorStruct vert;
	Vector3 *loc = Vertex->Get_Array();
	assert(loc);
	for (int i = 0; i < Get_Vertex_Count(); i++) {
		if (cload.Read(&vert, sizeof(W3dVectorStruct)) != sizeof(W3dVectorStruct)) {
			return WW3D_ERROR_LOAD_FAILED;
		}
		loc[i].X = vert.X;
		loc[i].Y = vert.Y;
		loc[i].Z = vert.Z;
	}
	return WW3D_ERROR_OK;
}

WW3DErrorType MeshGeometryClass::read_vertex_normals(ChunkLoadClass &cload)
{
	W3dVectorStruct norm;
	Vector3 *mdlnorms = get_vert_normals();
	for (int i = 0; i < VertexCount; i++) {
		if (cload.Read(&norm, sizeof(W3dVectorStruct)) != sizeof(W3dVectorStruct)) {
			return WW3D_ERROR_LOAD_FAILED;
		}
		mdlnorms[i].Set(norm.X, norm.Y, norm.Z);
	}
	return WW3D_ERROR_OK;
}

WW3DErrorType MeshGeometryClass::read_triangles(ChunkLoadClass &cload)
{
	W3dTriStruct tri;
	TriIndex *vi = get_polys();
	Set_Flag(DIRTY_PLANES, false);
	Vector4 *peq = get_planes();
	uint8 *surface_types = Get_Poly_Surface_Type_Array();
	for (int i = 0; i < Get_Polygon_Count(); i++) {
		if (cload.Read(&tri, sizeof(W3dTriStruct)) != sizeof(W3dTriStruct)) {
			return WW3D_ERROR_LOAD_FAILED;
		}
		vi[i].I = tri.Vindex[0];
		vi[i].J = tri.Vindex[1];
		vi[i].K = tri.Vindex[2];
		peq[i].X = tri.Normal.X;
		peq[i].Y = tri.Normal.Y;
		peq[i].Z = tri.Normal.Z;
		peq[i].W = -tri.Dist;
		surface_types[i] = (uint8)(tri.Attributes);
	}
	return WW3D_ERROR_OK;
}

WW3DErrorType MeshGeometryClass::read_user_text(ChunkLoadClass &cload)
{
	unsigned int textlen = cload.Cur_Chunk_Length();
	if (UserText != NULL) {
		return WW3D_ERROR_OK;
	}
	UserText = NEW_REF(ShareBufferClass<char>, (textlen, "MeshGeometryClass::UserText"));
	if (cload.Read(UserText->Get_Array(), textlen) != textlen) {
		return WW3D_ERROR_LOAD_FAILED;
	}
	return WW3D_ERROR_OK;
}

WW3DErrorType MeshGeometryClass::read_vertex_influences(ChunkLoadClass &cload)
{
	W3dVertInfStruct vinf;
	uint16 *links = get_bone_links(true);
	for (int i = 0; i < Get_Vertex_Count(); i++) {
		if (cload.Read(&vinf, sizeof(W3dVertInfStruct)) != sizeof(W3dVertInfStruct)) {
			return WW3D_ERROR_LOAD_FAILED;
		}
		links[i] = vinf.BoneIdx;
	}
	Set_Flag(SKIN, true);
	return WW3D_ERROR_OK;
}

WW3DErrorType MeshGeometryClass::read_vertex_shade_indices(ChunkLoadClass &cload)
{
	uint32 *shade_index = get_shade_indices(true);
	uint32 si;
	for (int i = 0; i < Get_Vertex_Count(); i++) {
		if (cload.Read(&si, sizeof(uint32)) != sizeof(uint32)) {
			return WW3D_ERROR_LOAD_FAILED;
		}
		shade_index[i] = si;
	}
	return WW3D_ERROR_OK;
}

WW3DErrorType MeshGeometryClass::read_aabtree(ChunkLoadClass &cload)
{
	REF_PTR_RELEASE(CullTree);
	CullTree = NEW_REF(AABTreeClass, ());
	CullTree->Load_W3D(cload);
	CullTree->Set_Mesh(this);
	return (WW3D_ERROR_OK);
}

/* ---- meshmdl.cpp ----------------------------------------------------- */
MeshModelClass::MeshModelClass(void) :
	DefMatDesc(NULL), AlternateMatDesc(NULL), CurMatDesc(NULL), MatInfo(NULL), GapFiller(NULL)
{
	Set_Flag(DIRTY_BOUNDS, true);
	DefMatDesc = W3DNEW MeshMatDescClass;
	CurMatDesc = DefMatDesc;
	MatInfo = NEW_REF(MaterialInfoClass, ());
}
MeshModelClass::~MeshModelClass(void) {}

void MeshModelClass::Reset(int polycount, int vertcount, int passcount)
{
	/* (the gap filler and the DX8 mesh renderer registration are not part of
	** loading) */
	Reset_Geometry(polycount, vertcount);
	MatInfo->Reset();
	DefMatDesc->Reset(polycount, vertcount, passcount);
	if (AlternateMatDesc != NULL) {
		delete AlternateMatDesc;
		AlternateMatDesc = NULL;
	}
	CurMatDesc = DefMatDesc;
}

/* ---- vertmaterial.cpp ------------------------------------------------ */
VertexMaterialClass::VertexMaterialClass() : Serial(g_serial++), HasName(false)
{
	memset(&Info, 0, sizeof(Info));
	HasArgs[0] = HasArgs[1] = false;
	Add_Ref(); /* kept alive past the context's release, for logging */
}
VertexMaterialClass::VertexMaterialClass(const VertexMaterialClass &) { abort(); }
VertexMaterialClass::~VertexMaterialClass() {}

/* VertexMaterialClass::Load_W3D, recording the file data where upstream
** calls Set_Name / Parse_W3dVertexMaterialStruct / Parse_Mapping_Args. */
WW3DErrorType VertexMaterialClass::Load_W3D(ChunkLoadClass &cload)
{
	char name[256];
	W3dVertexMaterialStruct vmat;
	bool hasname = false;
	char *mapping0_arg_buffer = NULL;
	char *mapping1_arg_buffer = NULL;
	unsigned int mapping0_arg_len = 0U;
	unsigned int mapping1_arg_len = 0U;
	while (cload.Open_Chunk()) {
		switch (cload.Cur_Chunk_ID()) {
		case W3D_CHUNK_VERTEX_MATERIAL_NAME:
			cload.Read(&name, cload.Cur_Chunk_Length());
			hasname = true;
			break;
		case W3D_CHUNK_VERTEX_MATERIAL_INFO:
			if (cload.Read(&vmat, sizeof(vmat)) != sizeof(vmat)) {
				return WW3D_ERROR_LOAD_FAILED;
			}
			break;
		case W3D_CHUNK_VERTEX_MAPPER_ARGS0:
			mapping0_arg_len = cload.Cur_Chunk_Length();
			mapping0_arg_buffer = MSGW3DNEWARRAY("VertexMaterialClassTemp") char[mapping0_arg_len];
			if (cload.Read(mapping0_arg_buffer, mapping0_arg_len) != mapping0_arg_len) {
				return WW3D_ERROR_LOAD_FAILED;
			}
			break;
		case W3D_CHUNK_VERTEX_MAPPER_ARGS1:
			mapping1_arg_len = cload.Cur_Chunk_Length();
			mapping1_arg_buffer = MSGW3DNEWARRAY("VertexMaterialClassTemp") char[mapping1_arg_len];
			if (cload.Read(mapping1_arg_buffer, mapping1_arg_len) != mapping1_arg_len) {
				return WW3D_ERROR_LOAD_FAILED;
			}
			break;
		};
		cload.Close_Chunk();
	}
	if (hasname) {
		/* Set_Name(name) */
		HasName = true;
		Name = name;
	}
	/* Parse_W3dVertexMaterialStruct(vmat) */
	Info = vmat;
	/* Parse_Mapping_Args(vmat, mapping0_arg_buffer, mapping1_arg_buffer) */
	if (mapping0_arg_buffer) { HasArgs[0] = true; Args[0].assign(mapping0_arg_buffer, mapping0_arg_buffer + mapping0_arg_len); }
	if (mapping1_arg_buffer) { HasArgs[1] = true; Args[1].assign(mapping1_arg_buffer, mapping1_arg_buffer + mapping1_arg_len); }
	delete[] mapping0_arg_buffer;
	delete[] mapping1_arg_buffer;
	return WW3D_ERROR_OK;
}

/* ---- texture.cpp ----------------------------------------------------- */
TextureClass::TextureClass(const char *name, MipCountType mips, WW3DFormat format) :
	Serial(g_serial++), Name(name), MipCount(mips), Format(format)
{
	Add_Ref(); /* kept alive past the context's release, for logging */
}

/* The asset manager's texture lookups: a new texture object per call. */
TextureClass *WW3DAssetManager::Get_Texture(const char *filename)
{
	return NEW_REF(TextureClass, (filename, MIP_LEVELS_ALL, WW3D_FORMAT_UNKNOWN));
}
TextureClass *WW3DAssetManager::Get_Texture(const char *filename, int mip_level_count, int texture_format)
{
	return NEW_REF(TextureClass, (filename, (MipCountType)mip_level_count, (WW3DFormat)texture_format));
}

/* Load_Texture (texture.cpp) */
TextureClass *Load_Texture(ChunkLoadClass &cload)
{
	TextureClass *newtex = NULL;
	char name[256];
	if (cload.Open_Chunk() && (cload.Cur_Chunk_ID() == W3D_CHUNK_TEXTURE)) {
		W3dTextureInfoStruct texinfo;
		bool hastexinfo = false;
		while (cload.Open_Chunk()) {
			switch (cload.Cur_Chunk_ID()) {
			case W3D_CHUNK_TEXTURE_NAME:
				cload.Read(&name, cload.Cur_Chunk_Length());
				break;
			case W3D_CHUNK_TEXTURE_INFO:
				cload.Read(&texinfo, sizeof(W3dTextureInfoStruct));
				hastexinfo = true;
				break;
			};
			cload.Close_Chunk();
		}
		cload.Close_Chunk();
		if (hastexinfo) {
			MipCountType mipcount;
			bool no_lod = ((texinfo.Attributes & W3DTEXTURE_NO_LOD) == W3DTEXTURE_NO_LOD);
			if (no_lod) {
				mipcount = MIP_LEVELS_1;
			} else {
				switch (texinfo.Attributes & W3DTEXTURE_MIP_LEVELS_MASK) {
				case W3DTEXTURE_MIP_LEVELS_ALL: mipcount = MIP_LEVELS_ALL; break;
				case W3DTEXTURE_MIP_LEVELS_2: mipcount = MIP_LEVELS_2; break;
				case W3DTEXTURE_MIP_LEVELS_3: mipcount = MIP_LEVELS_3; break;
				case W3DTEXTURE_MIP_LEVELS_4: mipcount = MIP_LEVELS_4; break;
				default: mipcount = MIP_LEVELS_ALL; break;
				}
			}
			WW3DFormat format = WW3D_FORMAT_UNKNOWN;
			switch (texinfo.Attributes & W3DTEXTURE_TYPE_MASK) {
			case W3DTEXTURE_TYPE_COLORMAP: break;
			case W3DTEXTURE_TYPE_BUMPMAP:
				if (DX8Wrapper::Is_Initted() && DX8Wrapper::Get_Current_Caps()->Support_Bump_Envmap()) abort();
				break;
			default: break;
			}
			newtex = WW3DAssetManager::Get_Instance()->Get_Texture(name, mipcount, format);
			if (no_lod) {
				newtex->Get_Filter().Set_Mip_Mapping(TextureFilterClass::FILTER_TYPE_NONE);
			}
			bool u_clamp = ((texinfo.Attributes & W3DTEXTURE_CLAMP_U) != 0);
			newtex->Get_Filter().Set_U_Addr_Mode(u_clamp ? TextureFilterClass::TEXTURE_ADDRESS_CLAMP : TextureFilterClass::TEXTURE_ADDRESS_REPEAT);
			bool v_clamp = ((texinfo.Attributes & W3DTEXTURE_CLAMP_V) != 0);
			newtex->Get_Filter().Set_V_Addr_Mode(v_clamp ? TextureFilterClass::TEXTURE_ADDRESS_CLAMP : TextureFilterClass::TEXTURE_ADDRESS_REPEAT);
		} else {
			newtex = WW3DAssetManager::Get_Instance()->Get_Texture(name);
		}
	}
	return newtex;
}

/* ---- aabtree.cpp ----------------------------------------------------- */
AABTreeClass::AABTreeClass(void) : NodeCount(0), Nodes(NULL), PolyCount(0), PolyIndices(NULL) {}
AABTreeClass::~AABTreeClass(void) { Reset(); }
void AABTreeClass::Reset(void)
{
	NodeCount = 0;
	if (Nodes) { delete[] Nodes; Nodes = NULL; }
	PolyCount = 0;
	if (PolyIndices) { delete[] PolyIndices; PolyIndices = NULL; }
}
void AABTreeClass::Load_W3D(ChunkLoadClass &cload)
{
	Reset();
	W3dMeshAABTreeHeader header;
	cload.Open_Chunk();
	cload.Read(&header, sizeof(header));
	cload.Close_Chunk();
	NodeCount = header.NodeCount;
	PolyCount = header.PolyCount;
	Nodes = W3DNEWARRAY CullNodeStruct[NodeCount];
	PolyIndices = W3DNEWARRAY uint32[PolyCount];
	while (cload.Open_Chunk()) {
		switch (cload.Cur_Chunk_ID()) {
		case W3D_CHUNK_AABTREE_POLYINDICES:
			/* Read_Poly_Indices */
			cload.Read(PolyIndices, sizeof(uint32) * PolyCount);
			break;
		case W3D_CHUNK_AABTREE_NODES: {
			/* Read_Nodes */
			W3dMeshAABTreeNode w3dnode;
			for (int i = 0; i < NodeCount; i++) {
				cload.Read(&w3dnode, sizeof(w3dnode));
				Nodes[i].Min.X = w3dnode.Min.X;
				Nodes[i].Min.Y = w3dnode.Min.Y;
				Nodes[i].Min.Z = w3dnode.Min.Z;
				Nodes[i].Max.X = w3dnode.Max.X;
				Nodes[i].Max.Y = w3dnode.Max.Y;
				Nodes[i].Max.Z = w3dnode.Max.Z;
				Nodes[i].FrontOrPoly0 = w3dnode.FrontOrPoly0;
				Nodes[i].BackOrPolyCount = w3dnode.BackOrPolyCount;
			}
			break;
		}
		}
		cload.Close_Chunk();
	}
}

/* ---- dx8wrapper.h ---------------------------------------------------- */
Vector4 DX8Wrapper::Convert_Color(unsigned color)
{
	Vector4 col;
	col[3] = ((color & 0xff000000) >> 24) / 255.0f;
	col[0] = ((color & 0xff0000) >> 16) / 255.0f;
	col[1] = ((color & 0xff00) >> 8) / 255.0f;
	col[2] = ((color & 0xff) >> 0) / 255.0f;
	return col;
}

/* The MSVC asm of DX8Wrapper::Convert_Color(const Vector3 &, float), in GNU
** syntax: set the x87 rounding control to chop, load 255, r, g, b and a,
** multiply each by 255, fistp each to an int, restore the control word,
** then (a << 24) | (r << 16) | (g << 8) | b in 32-bit registers. */
unsigned int DX8Wrapper::Convert_Color(const Vector3 &color, float alpha)
{
	const float scale = 255.0f;
	int out[4];
	unsigned short cw, chop;
	__asm__ volatile(
		"fnstcw %[cw]\n\t"
		"movzwl %[cw], %%eax\n\t"
		"orl $0xC00, %%eax\n\t"
		"movw %%ax, %[chop]\n\t"
		"fldcw %[chop]\n\t"
		"flds %[scale]\n\t"
		"flds 0(%[c])\n\t"
		"flds 4(%[c])\n\t"
		"flds 8(%[c])\n\t"
		"flds %[alpha]\n\t"
		"fld %%st(4)\n\t"
		"fmul %%st, %%st(4)\n\t"
		"fmul %%st, %%st(3)\n\t"
		"fmul %%st, %%st(2)\n\t"
		"fmulp %%st, %%st(1)\n\t"
		"fistpl 0(%[out])\n\t"
		"fistpl 4(%[out])\n\t"
		"fistpl 8(%[out])\n\t"
		"fistpl 12(%[out])\n\t"
		"fstp %%st(0)\n\t"
		"fldcw %[cw]\n\t"
		: [cw] "+m"(cw), [chop] "+m"(chop)
		: [scale] "m"(scale), [c] "r"(&color.X), [alpha] "m"(alpha), [out] "r"(out)
		: "eax", "memory", "st", "st(1)", "st(2)", "st(3)", "st(4)", "st(5)", "st(6)", "st(7)");
	unsigned a = (unsigned)out[0], b = (unsigned)out[1], g = (unsigned)out[2], r = (unsigned)out[3];
	return (a << 24) | b | (r << 16) | (g << 8);
}

unsigned int DX8Wrapper::Convert_Color(const Vector4 &color)
{
	return Convert_Color(reinterpret_cast<const Vector3 &>(color), color[3]);
}

/* ---- the snapshot ---------------------------------------------------- */
namespace {

int vmat_index(MeshLoadContextClass *c, VertexMaterialClass *v)
{
	if (!v) return -1;
	for (int i = 0; i < c->VertexMaterials.Count(); ++i)
		if (c->VertexMaterials[i] == v) return i;
	abort();
}

int tex_index(MeshLoadContextClass *c, TextureClass *t)
{
	if (!t) return -1;
	for (int i = 0; i < c->Textures.Count(); ++i)
		if (c->Textures[i] == t) return i;
	abort();
}

void log_matdesc(MeshLoadContextClass *c, MeshMatDescClass *d)
{
	put32((uint32_t)d->PassCount);
	put32((uint32_t)d->VertexCount);
	put32((uint32_t)d->PolyCount);
	for (int pass = 0; pass < MeshMatDescClass::MAX_PASSES; ++pass) {
		put32((uint32_t)vmat_index(c, d->Material[pass]));
		if (d->MaterialArray[pass]) {
			put8(1);
			put32((uint32_t)d->MaterialArray[pass]->Get_Count());
			for (int i = 0; i < d->MaterialArray[pass]->Get_Count(); ++i)
				put32((uint32_t)vmat_index(c, d->MaterialArray[pass]->Peek_Element(i)));
		} else put8(0);
		put32(d->Shader[pass].Get_Bits());
		if (d->ShaderArray[pass]) {
			put8(1);
			put32((uint32_t)d->ShaderArray[pass]->Get_Count());
			for (int i = 0; i < d->ShaderArray[pass]->Get_Count(); ++i) put32(d->ShaderArray[pass]->Get_Element(i).Get_Bits());
		} else put8(0);
		for (int stage = 0; stage < MeshMatDescClass::MAX_TEX_STAGES; ++stage) {
			put32((uint32_t)tex_index(c, d->Texture[pass][stage]));
			if (d->TextureArray[pass][stage]) {
				put8(1);
				put32((uint32_t)d->TextureArray[pass][stage]->Get_Count());
				for (int i = 0; i < d->TextureArray[pass][stage]->Get_Count(); ++i)
					put32((uint32_t)tex_index(c, d->TextureArray[pass][stage]->Peek_Element(i)));
			} else put8(0);
			put32((uint32_t)d->UVSource[pass][stage]);
		}
		put32((uint32_t)d->DCGSource[pass]);
		put32((uint32_t)d->DIGSource[pass]);
	}
	int nuv = 0;
	while (nuv < MeshMatDescClass::MAX_UV_ARRAYS && d->UV[nuv]) ++nuv;
	put32((uint32_t)nuv);
	for (int i = 0; i < nuv; ++i) {
		put32((uint32_t)d->UV[i]->Get_Count());
		put32(d->UV[i]->Get_CRC());
		put(d->UV[i]->Get_Array(), d->UV[i]->Get_Count() * 8);
	}
	for (int a = 0; a < MeshMatDescClass::MAX_COLOR_ARRAYS; ++a) {
		if (d->ColorArray[a]) {
			put8(1);
			put32((uint32_t)d->ColorArray[a]->Get_Count());
			put(d->ColorArray[a]->Get_Array(), d->ColorArray[a]->Get_Count() * 4);
		} else put8(0);
	}
}

template <class T> void log_buf(ShareBufferClass<T> *b, int elems_bytes)
{
	if (!b) { put8(0); return; }
	put8(1);
	put32((uint32_t)b->Get_Count());
	put(b->Get_Array(), (size_t)b->Get_Count() * elems_bytes);
}

void snapshot(MeshLoadContextClass *c)
{
	MeshModelClass *m = g_mesh;
	put8('S');
	put(&c->Header, sizeof(c->Header));
	const char *name = m->Get_Name();
	putStr(name ? name : "", name ? strlen(name) : 0);
	put32((uint32_t)m->Flags);
	put32(m->W3dAttributes);
	put32((uint32_t)(int)m->SortLevel);
	put32((uint32_t)m->PolyCount);
	put32((uint32_t)m->VertexCount);
	putF(m->BoundBoxMin.X); putF(m->BoundBoxMin.Y); putF(m->BoundBoxMin.Z);
	putF(m->BoundBoxMax.X); putF(m->BoundBoxMax.Y); putF(m->BoundBoxMax.Z);
	putF(m->BoundSphereCenter.X); putF(m->BoundSphereCenter.Y); putF(m->BoundSphereCenter.Z);
	putF(m->BoundSphereRadius);
	log_buf(m->Vertex, 12);
	log_buf(m->VertexNorm, 12);
	log_buf(m->Poly, 6);
	log_buf(m->PolySurfaceType, 1);
	if (g_planes_used) {
		put8(1);
		put32((uint32_t)m->PolyCount);
		put(&((*g_planes)[0]), (size_t)m->PolyCount * 16);
	} else put8(0);
	log_buf(m->VertexShadeIdx, 4);
	log_buf(m->VertexBoneLink, 2);
	log_buf(m->UserText, 1);
	if (m->CullTree) {
		AABTreeClass *t = m->CullTree;
		put8(1);
		put32((uint32_t)t->NodeCount);
		put32((uint32_t)t->PolyCount);
		for (int i = 0; i < t->NodeCount; ++i) put(&t->Nodes[i], 32);
		put(t->PolyIndices, (size_t)t->PolyCount * 4);
	} else put8(0);
	put8(g_generated ? 1 : 0);
	put32(c->PrelitChunkID);
	put(&c->MatInfo, sizeof(c->MatInfo));
	put32((uint32_t)c->Shaders.Count());
	for (int i = 0; i < c->Shaders.Count(); ++i) put32(c->Shaders[i].Get_Bits());
	put32((uint32_t)c->VertexMaterials.Count());
	for (int i = 0; i < c->VertexMaterials.Count(); ++i) {
		VertexMaterialClass *v = c->VertexMaterials[i];
		put8(v->HasName ? 1 : 0);
		putStr(v->Name.data(), v->Name.size());
		put(&v->Info, sizeof(v->Info));
		for (int a = 0; a < 2; ++a) {
			put8(v->HasArgs[a] ? 1 : 0);
			putStr(v->Args[a].data(), v->Args[a].size());
		}
	}
	put32((uint32_t)c->Textures.Count());
	for (int i = 0; i < c->Textures.Count(); ++i) {
		TextureClass *t = c->Textures[i];
		putStr(t->Name.data(), t->Name.size());
		put32((uint32_t)t->MipCount);
		put32((uint32_t)t->Format);
		put32((uint32_t)t->Filter.MipMapping);
		put32((uint32_t)t->Filter.UAddr);
		put32((uint32_t)t->Filter.VAddr);
	}
	log_matdesc(c, m->DefMatDesc);
	log_matdesc(c, &c->AlternateMatDesc);
}

} /* namespace */

/* Upstream's first call after the chunks are read (see the top). */
bool MeshMatDescClass::Is_Empty(void)
{
	if (g_mesh) {
		MeshLoadContextClass *c =
			(MeshLoadContextClass *)((char *)this - offsetof(MeshLoadContextClass, AlternateMatDesc));
		snapshot(c);
		throw Snapshot();
	}
	abort();
}

extern "C" {

/* Load the first chunk of the file as a mesh with the real
** MeshModelClass::Load_W3D, the pre-lit mode set to prelit_mode. Logs the
** snapshot (or the load error), then the chunk depth and the file position. */
int ref_w3d_load_mesh(const char *data, int len, int prelit_mode, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	WW3D::PrelitMode = (WW3D::PrelitModeEnum)prelit_mode;
	TestFile file(data, len);
	ChunkLoadClass cload(&file);
	put8('O');
	if (cload.Open_Chunk()) {
		put8(1);
		g_generated = false;
		g_planes_used = false;
		SimpleVecClass<Vector4> planes;
		g_planes = &planes;
		g_mesh = new MeshModelClass;
		try {
			WW3DErrorType r = g_mesh->Load_W3D(cload);
			put8('e');
			put8(r);
		} catch (Snapshot &) {
		}
		g_mesh = NULL;
		g_planes = NULL;
	} else {
		put8(0);
	}
	put8('s');
	put32((uint32_t)cload.Cur_Chunk_Depth());
	put32((uint32_t)file.Tell());
	int n = (int)g_log.size();
	*log_len = n;
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	return 0;
}

/* WWLib/realcrc.cpp */
unsigned ref_crc_memory(const unsigned char *data, int len, unsigned crc)
{
	return (unsigned)CRC_Memory(data, (unsigned long)len, crc);
}

/* dx8wrapper.h (the transcription above) */
unsigned ref_dx8_convert_color(const float *rgba)
{
	return DX8Wrapper::Convert_Color(Vector4(rgba[0], rgba[1], rgba[2], rgba[3]));
}

} /* extern "C" */
