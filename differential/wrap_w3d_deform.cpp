/* Differential-test wrapper for the skin deformation (upstream commit
** 0a05454d), GitHub issue #10, step 4:
**   WW3D2/meshgeometry.cpp  MeshGeometryClass::get_deformed_vertices, both
**                           overloads (vertices; vertices and normals)
**   WWMath/vp.cpp           VectorProcessorClass::Transform (Vector3 by a
**                           Matrix3D; the non-Intel-compiler path)
** COMPILED from verbatim segments (build.rs writes them to
** OUT_DIR/deform_def.cpp), with the real meshgeometry.h in the mesh
** library's setting (shim_w3d/mesh_prelude.h; the MeshGeometryClass
** constructor and Reset_Geometry are wrap_w3d_mesh.cpp's). The hierarchy is
** loaded by the real HTreeClass::Load_W3D and posed by the real Base_Update
** (htree.cpp in upstream_w3d). This file only fills the geometry's arrays
** (Vertex, VertexNorm, VertexBoneLink) and calls the two functions.
**
** ref_w3d_deform: `file` holds one W3D_CHUNK_HIERARCHY; `root` is the
** Base_Update root (12 floats, rows); `n` vertices with their normals and
** bone links. Writes the deformed vertices and normals (vertices and
** normals overload) to out_v, out_n, and the vertices-only overload's
** result to out_v2. Returns 0, or -1 if the hierarchy did not load. */
#include <stdlib.h>
#include <string.h>

#include "meshgeometry.h"
#include "htree.h"
#include "chunkio.h"
#include "matrix3d.h"

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}

namespace {

/* The same file as wrap_w3d_eval.cpp: GameFileClass's Read and Seek over RAMFile. */
class DeformTestFile : public FileClass
{
public:
	DeformTestFile(const char *data, int len) : m_theFile(ref_w3d_ramfile_open(data, len)) {}
	~DeformTestFile() { ref_w3d_ramfile_close(m_theFile); }
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

/* Access to the protected arrays and functions. */
class DeformGeometry : public MeshGeometryClass
{
	W3DMPO_GLUE(DeformGeometry)
public:
	void Fill(int n, const float *verts, const float *norms, const unsigned short *links)
	{
		Reset_Geometry(1, n);
		memcpy(Vertex->Get_Array(), verts, sizeof(float) * 3 * n);
		memcpy(VertexNorm->Get_Array(), norms, sizeof(float) * 3 * n);
		VertexBoneLink = NEW_REF(ShareBufferClass<uint16>, (n, "DeformGeometry::VertexBoneLink"));
		memcpy(VertexBoneLink->Get_Array(), links, sizeof(unsigned short) * n);
	}
	void Deform(Vector3 *v, Vector3 *nrm, const HTreeClass *t) { get_deformed_vertices(v, nrm, t); }
	void Deform(Vector3 *v, const HTreeClass *t) { get_deformed_vertices(v, t); }
};

} // namespace

extern "C" int ref_w3d_deform(const char *file, int len, const float *root, int n, const float *verts, const float *norms,
	const unsigned short *links, float *out_v, float *out_n, float *out_v2)
{
	DeformTestFile f(file, len);
	ChunkLoadClass cload(&f);
	if (!cload.Open_Chunk()) return -1;
	HTreeClass *tree = new HTreeClass;
	if (tree->Load_W3D(cload) != 0) return -1;
	cload.Close_Chunk();
	Matrix3D m(root[0], root[1], root[2], root[3], root[4], root[5], root[6], root[7], root[8], root[9], root[10], root[11]);
	tree->Base_Update(m);

	DeformGeometry *g = new DeformGeometry;
	g->Fill(n, verts, norms, links);
	g->Deform((Vector3 *)out_v, (Vector3 *)out_n, tree);
	g->Deform((Vector3 *)out_v2, tree);
	return 0;
}
