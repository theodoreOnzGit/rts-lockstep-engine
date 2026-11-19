/* Differential-test wrapper for the W3D dependency scanner draft (GitHub #215, upstream commit 0a05454d), group "ww3dpure":
** src/drafts/ww3d/w3d_dep.rs against WW3D2/w3d_dep.cpp.
**
** w3d_dep.cpp is #included WHOLE from the copy build.rs `drafts_ww3dpure` writes (byte for byte, no edits), so its file-static
** functions (Get_W3D_Name, Make_W3D_Filename, the Scan_* family) are callable from here. The real chunkio.cpp
** (ChunkLoadClass), ffactory.h, wwfile.h and w3d_file.h are used. `_TheFileFactory` is renamed on the command line
** (-D_TheFileFactory=DepTheFileFactory) so that this library's global does not clash with the one the texture and asset manager
** wrappers define. Stand-ins: a FileClass over a memory buffer and a FileFactoryClass that hands it out (or NULL, for a missing
** file); strupr and strlwr, MSVC CRT functions (differential/shim_draft_ww3dpure/dep_prelude.h, forced in), as is _MAX_PATH.
**
** Entry points (extern "C", tests/differential_draft_w3ddep.rs):
**   ref_dpw_w3d_name(filename, out)      Get_W3D_Name -> out (16 bytes, NUL padded)
**   ref_dpw_w3d_filename(name, out)      Make_W3D_Filename -> out (64 bytes)
**   ref_dpw_w3d_deps(data, len, filename, out, cap)   Get_W3D_Dependencies over `data` (len < 0: the factory has no such file):
**     -1 when it returns false, else the byte length of `out`, the file names joined by '\n', in list order. */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include "w3d_dep.cpp"

class MemFile : public FileClass
{
public:
	MemFile(const uint8_t *d, int n) : Data(d), Len(n), Pos(0), Opened(false) { Name[0] = 0; }
	virtual char const *File_Name(void) const { return Name; }
	virtual char const *Set_Name(char const *filename) { strncpy(Name, filename, 255); Name[255] = 0; return Name; }
	virtual int Create(void) { return 0; }
	virtual int Delete(void) { return 0; }
	virtual bool Is_Available(int = false) { return true; }
	virtual bool Is_Open(void) const { return Opened; }
	virtual int Open(char const *filename, int = READ) { Set_Name(filename); Opened = true; Pos = 0; return 1; }
	virtual int Open(int = READ) { Opened = true; Pos = 0; return 1; }
	virtual int Read(void *buffer, int size)
	{
		int n = size;
		if (n > Len - Pos) n = Len - Pos;
		if (n < 0) n = 0;
		memcpy(buffer, Data + Pos, n);
		Pos += n;
		return n;
	}
	virtual int Seek(int pos, int dir = SEEK_CUR)
	{
		int base = dir == SEEK_SET ? 0 : dir == SEEK_END ? Len : Pos;
		Pos = base + pos;
		if (Pos < 0) Pos = 0;
		if (Pos > Len) Pos = Len;
		return Pos;
	}
	virtual int Size(void) { return Len; }
	virtual int Write(void const *, int) { return 0; }
	virtual void Close(void) { Opened = false; }
private:
	const uint8_t *Data;
	int Len, Pos;
	bool Opened;
	char Name[256];
};

class MemFactory : public FileFactoryClass
{
public:
	const uint8_t *Data;
	int Len;
	MemFactory() : Data(NULL), Len(-1) {}
	virtual FileClass *Get_File(char const *) { return Len < 0 ? NULL : new MemFile(Data, Len); }
	virtual void Return_File(FileClass *file) { delete file; }
};

static MemFactory g_factory;
FileFactoryClass *_TheFileFactory = &g_factory;

extern "C" {

void ref_dpw_w3d_name(const char *filename, char *out) { Get_W3D_Name(filename, out); }

void ref_dpw_w3d_filename(const char *name, char *out) { strcpy(out, Make_W3D_Filename(name)); }

int ref_dpw_w3d_deps(const uint8_t *data, int len, const char *filename, char *out, int cap)
{
	g_factory.Data = data;
	g_factory.Len = len;
	StringList files;
	bool ok = Get_W3D_Dependencies(filename, files);
	if (!ok) return -1;
	int n = 0;
	bool first = true;
	for (StringList::const_iterator it = files.begin(); it != files.end(); ++it) {
		if (!first) { if (n < cap) out[n] = '\n'; n++; }
		first = false;
		for (size_t k = 0; k < it->size(); k++) { if (n < cap) out[n] = (*it)[k]; n++; }
	}
	return n;
}

}
