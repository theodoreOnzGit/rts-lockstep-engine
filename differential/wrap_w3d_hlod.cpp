/* Differential-test wrapper for the W3D HLOD definition loader (upstream
** commit 0a05454d): HLodDefClass::Load_W3D, read_header, read_proxy_array,
** Free and SubObjectArrayClass (constructor, Reset, Load_W3D), with
** ProxyRecordClass and ProxyArrayClass, all COMPILED from WW3D2/hlod.cpp's
** own text: build.rs copies the two segments of hlod.cpp that hold them,
** verbatim, into OUT_DIR (hlod_def.cpp, w3d_gen/hlod_proxy.h). The rest of
** hlod.cpp (HLodClass, the render object; HLodLoaderClass and
** HLodPrototypeClass, which construct one) needs the renderer and is not
** compiled. The real hlod.h, w3d_file.h and chunkio.h give every class
** layout; differential/shim_w3d/w3d_prelude.h and mesh_prelude.h are
** force-included (the latter for the render-object headers hlod.h pulls in).
**
** Nothing is transcribed. Defined here: lstrcpyn (windows.h; called only by
** SubObjectArrayClass::Save_W3D, which the tests do not run: it aborts) and
** the file, as in wrap_w3d.cpp (GameFileClass's Read and Seek over the real
** RAMFile). Allocation is wrap_w3d.cpp's operator new / new[] (zero-filled,
** as the game's DynamicMemoryAllocator) and its W3DMPO pools, which this
** library links against.
**
** The log (reproduced by tests/differential_w3d.rs from the Rust structs):
** 'O' and whether the first chunk opened; then, if it did, 'H' and the
** definition when Load_W3D returns WW3D_ERROR_OK, or 'e' and the error code;
** then 's', the chunk depth and the file position (Tell). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>

#define private public
#define protected public
#include "hlod.h"
#include "w3d_file.h"
#include "chunkio.h"
#include "hlod_proxy.h" /* ProxyRecordClass, ProxyArrayClass, extracted from hlod.cpp by build.rs */
#undef private
#undef protected

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}

/* windows.h; only HLodDefClass's Save functions call it. */
char *lstrcpyn(char *, const char *, int) { abort(); }

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
/* a C string: its length (strlen), then its bytes; 0xFFFFFFFF for NULL */
void putStr(const char *s)
{
	if (!s) { put32(0xFFFFFFFFu); return; }
	size_t n = strlen(s);
	put32((uint32_t)n);
	put(s, n);
}

/* SubObjectArrayClass: MaxScreenSize (bits), ModelCount, then each entry's
** name (ModelName[i], NULL for an entry not read) and BoneIndex[i]. */
void log_array(const HLodDefClass::SubObjectArrayClass &a)
{
	putF(a.MaxScreenSize);
	put32((uint32_t)a.ModelCount);
	for (int i = 0; i < a.ModelCount; ++i) {
		putStr(a.ModelName ? a.ModelName[i] : NULL);
		put32(a.BoneIndex ? (uint32_t)a.BoneIndex[i] : 0xEEEEEEEEu);
	}
}

} /* namespace */

extern "C" {

int ref_w3d_load_hlod(const char *data, int len, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	TestFile file(data, len);
	ChunkLoadClass cload(&file);
	put8('O');
	if (cload.Open_Chunk()) {
		put8(1);
		HLodDefClass *def = new HLodDefClass;
		WW3DErrorType r = def->Load_W3D(cload);
		if (r == WW3D_ERROR_OK) {
			put8('H');
			putStr(def->Name);
			putStr(def->HierarchyTreeName);
			put32((uint32_t)def->LodCount);
			for (int i = 0; i < def->LodCount; ++i) log_array(def->Lod[i]);
			log_array(def->Aggregates);
			if (def->ProxyArray) {
				put8(1);
				put32((uint32_t)def->ProxyArray->Length());
				for (int i = 0; i < def->ProxyArray->Length(); ++i) {
					ProxyRecordClass &p = (*def->ProxyArray)[i];
					put32((uint32_t)p.BoneIndex);
					put(p.Name, sizeof(p.Name));
				}
			} else {
				put8(0);
			}
		} else {
			put8('e');
			put8(r);
		}
		/* as HLodLoaderClass::Load_W3D on a failure (and the prototype's
		** destructor otherwise) */
		delete def;
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

} /* extern "C" */
