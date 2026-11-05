/* Differential-test wrapper for the W3D collection and aggregate definition
** loaders (upstream commit 0a05454d): CollectionDefClass (constructor,
** destructor, Free, Get_Name, Load) from WW3D2/collect.cpp,
** AggregateDefClass (default constructor, destructor, Free_Subobject_List,
** Load_W3D, Read_Header, Read_Info, Read_Subobject, Add_Subobject,
** Read_Class_Info) from WW3D2/agg_def.cpp, both COMPILED from verbatim
** segments of upstream's text that build.rs copies into OUT_DIR, and
** WW3D2/snappts.cpp (SnapPointsClass::Load_W3D), compiled unchanged. The
** real collect.h, agg_def.h, proxy.h, snapPts.h, w3d_file.h and chunkio.h
** give every class layout; differential/shim_w3d/w3d_prelude.h and
** mesh_prelude.h are force-included.
**
** TRANSCRIBED here, from WWLib/wwstring.cpp (which casts pointers to a
** 32-bit `unsigned` and cannot be compiled for x86_64): the out-of-line
** StringClass members that ProxyClass's name needs: the static
** m_NullChar/m_EmptyString, Get_String and Free_String (their non-temporary
** paths: nothing compiled here asks for a temporary string; the temporary
** paths abort), and Uninitialised_Grow (verbatim). The library is compiled
** with StringClass renamed to CollectionStringClass (build.rs), so these do
** not clash with the mesh wrapper's empty-only stand-ins.
**
** Stand-ins never reached by the loading (they abort): the AggregateDefClass
** virtual functions agg_def.cpp defines outside the copied segments (the
** class's vtable, emitted with its destructor, names them): Save_W3D and its
** helpers, Attach_Subobjects, Find_Subobject, Load_Assets,
** Create_Render_Object, Is_Object_In_List, Build_Subobject_List.
**
** One known difference of this build from the Win32 game, outside what the
** tests feed it: SnapPointsClass::Load_W3D divides the int chunk length by
** `sizeof`, a 32-bit unsigned on Win32 and 64-bit here, so for a points
** chunk length of 2^31 or more the count differs. On Win32 every such length
** but 0x80000000-0x80000007 is an array the game cannot allocate (the Rust
** reports it undefined); the random tests reach those 8 lengths with
** negligible probability.
**
** Allocation, the file and the array-size registry are wrap_w3d.cpp's
** (zero-filled operator new / new[], as the game's allocator; GameFileClass's
** Read and Seek over the real RAMFile). The log is reproduced by
** tests/differential_w3d.rs from the Rust structs. */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

/* agg_def.h's inline Set_Name calls _strdup (windows.h: MSVC's name for strdup) */
#define _strdup strdup

#define private public
#define protected public
#include "collection_def.h" /* CollectionDefClass, extracted from collect.cpp by build.rs */
#include "agg_def.h"
#include "w3d_file.h"
#include "chunkio.h"
#undef private
#undef protected

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
void ref_w3d_track_arrays(int on);
size_t ref_w3d_array_bytes(const void *p);
}

/* ---- wwstring.cpp (transcribed, see above) --------------------------- */
TCHAR StringClass::m_NullChar = 0;
TCHAR *StringClass::m_EmptyString = &m_NullChar;

/* StringClass::Get_String: the non-temporary path. */
void StringClass::Get_String(int length, bool is_temp)
{
	if (!is_temp && length == 0) {
		m_Buffer = m_EmptyString;
		return;
	}
	if (is_temp) abort();
	if (length > 0) {
		Set_Buffer_And_Allocated_Length(Allocate_Buffer(length), length);
	} else {
		Free_String();
	}
}

/* StringClass::Free_String: the non-temporary path (no temporary buffers
** are handed out here). */
void StringClass::Free_String(void)
{
	if (m_Buffer != m_EmptyString) {
		char *buffer = ((char *)m_Buffer) - sizeof(StringClass::_HEADER);
		delete[] buffer;
		m_Buffer = m_EmptyString;
	}
}

/* StringClass::Uninitialised_Grow, verbatim (WWMEMLOG is empty). */
void StringClass::Uninitialised_Grow(int new_len)
{
	int allocated_len = Get_Allocated_Length();
	if (new_len > allocated_len) {
		TCHAR *new_buffer = Allocate_Buffer(new_len);
		Set_Buffer_And_Allocated_Length(new_buffer, new_len);
	}
	Store_Length(0);
	return;
}

/* ---- agg_def.cpp functions the loading never reaches ----------------- */
WW3DErrorType AggregateDefClass::Save_W3D(ChunkSaveClass &) { abort(); }
WW3DErrorType AggregateDefClass::Save_Header(ChunkSaveClass &) { abort(); }
WW3DErrorType AggregateDefClass::Save_Info(ChunkSaveClass &) { abort(); }
WW3DErrorType AggregateDefClass::Save_Subobject(ChunkSaveClass &, W3dAggregateSubobjectStruct *) { abort(); }
WW3DErrorType AggregateDefClass::Save_Class_Info(ChunkSaveClass &) { abort(); }
void AggregateDefClass::Attach_Subobjects(RenderObjClass &) { abort(); }
RenderObjClass *AggregateDefClass::Find_Subobject(RenderObjClass &, const char[MESH_PATH_ENTRIES][MESH_PATH_ENTRY_LEN],
	const char[MESH_PATH_ENTRIES][MESH_PATH_ENTRY_LEN]) { abort(); }
bool AggregateDefClass::Load_Assets(const char *) { abort(); }
RenderObjClass *AggregateDefClass::Create_Render_Object(const char *) { abort(); }
bool AggregateDefClass::Is_Object_In_List(const char *, DynamicVectorClass<RenderObjClass *> &) { abort(); }
void AggregateDefClass::Build_Subobject_List(RenderObjClass &, RenderObjClass &) { abort(); }

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
/* a C string: strlen, then its bytes; 0xFFFFFFFF for NULL */
void putStr(const char *s)
{
	if (!s) { put32(0xFFFFFFFFu); return; }
	size_t n = strlen(s);
	put32((uint32_t)n);
	put(s, n);
}

/* CollectionDefClass: Name (16 bytes); each object name's array (its size
** from the new[] registry, then all its bytes); each proxy's name and
** transform (3 rows of 4); the snap points (none, or their count and
** coordinates). */
void log_collection(const CollectionDefClass &d)
{
	put(d.Name, W3D_NAME_LEN);
	put32((uint32_t)d.ObjectNames.Count());
	for (int i = 0; i < d.ObjectNames.Count(); ++i) {
		size_t n = ref_w3d_array_bytes(d.ObjectNames[i]);
		put32((uint32_t)n);
		put(d.ObjectNames[i], n);
	}
	put32((uint32_t)d.ProxyList.Count());
	for (int i = 0; i < d.ProxyList.Count(); ++i) {
		const ProxyClass &p = d.ProxyList[i];
		putStr(p.Get_Name());
		const Matrix3D &m = p.Get_Transform();
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c) putF(m[r][c]);
	}
	if (!d.SnapPoints) {
		put8(0);
	} else {
		put8(1);
		put32((uint32_t)d.SnapPoints->Count());
		for (int i = 0; i < d.SnapPoints->Count(); ++i) {
			const Vector3 &v = (*d.SnapPoints)[i];
			putF(v.X); putF(v.Y); putF(v.Z);
		}
	}
}

/* AggregateDefClass: m_pName, m_Version, m_Info and m_MiscInfo (their
** bytes), and each sub-object's two 32-byte names. */
void log_aggregate(const AggregateDefClass &d)
{
	putStr(d.m_pName);
	put32((uint32_t)d.m_Version);
	put(&d.m_Info, sizeof(d.m_Info));
	put(&d.m_MiscInfo, sizeof(d.m_MiscInfo));
	put32((uint32_t)d.m_SubobjectList.Count());
	for (int i = 0; i < d.m_SubobjectList.Count(); ++i) {
		put(d.m_SubobjectList[i]->SubobjectName, sizeof(d.m_SubobjectList[i]->SubobjectName));
		put(d.m_SubobjectList[i]->BoneName, sizeof(d.m_SubobjectList[i]->BoneName));
	}
}

} /* namespace */

extern "C" {

/* Load the first chunk of the file with CollectionDefClass::Load (kind 0)
** or AggregateDefClass::Load_W3D (kind 1), as their loaders call them, and
** log: 'O' and whether the chunk opened; then 'C' / 'A' and the definition
** on WW3D_ERROR_OK, or 'e' and the error code; then 's', the chunk depth and
** the file position (Tell). */
int ref_w3d_load_collect(const char *data, int len, int kind, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	ref_w3d_track_arrays(1);
	{
		TestFile file(data, len);
		ChunkLoadClass cload(&file);
		put8('O');
		if (cload.Open_Chunk()) {
			put8(1);
			if (kind == 0) {
				CollectionDefClass *def = new CollectionDefClass;
				WW3DErrorType r = def->Load(cload);
				if (r == WW3D_ERROR_OK) {
					put8('C');
					log_collection(*def);
				} else {
					put8('e');
					put8(r);
				}
				delete def;
			} else if (kind == 1) {
				AggregateDefClass *def = new AggregateDefClass;
				WW3DErrorType r = def->Load_W3D(cload);
				if (r == WW3D_ERROR_OK) {
					put8('A');
					log_aggregate(*def);
				} else {
					put8('e');
					put8(r);
				}
				delete def;
			} else {
				abort();
			}
		} else {
			put8(0);
		}
		put8('s');
		put32((uint32_t)cload.Cur_Chunk_Depth());
		put32((uint32_t)file.Tell());
	}
	ref_w3d_track_arrays(0);
	int n = (int)g_log.size();
	*log_len = n;
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	return 0;
}

} /* extern "C" */
