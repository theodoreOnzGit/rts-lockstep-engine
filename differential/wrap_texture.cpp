/* Differential-test wrapper for the texture readers (upstream commit
** 0a05454d):
**   WW3D2/ddsfile.cpp, WW3D2/formconv.cpp (copies with MSVC-dialect edits,
**     see build.rs), WW3D2/ww3dformat.cpp (unchanged);
**   WWLib/TARGA.CPP (a copy with `long` as `int` and its _swapBytes
**     assembly transcribed to GNU asm, see build.rs);
** with differential/shim_w3d/w3d_prelude.h and mesh_prelude.h force-included
** (the Direct3D stand-ins, and DX8Wrapper::Convert_Color, which Recolor
** uses, transcribed in wrap_w3d_mesh.cpp) and the real bitmaphandler.h and
** colorspace.h.
**
** The file: both readers open their file through the file factory
** (_TheFileFactory). In the game that is W3DFileSystem, which hands out a
** GameFileClass over the RAMFile a .big archive gives; here a factory hands
** out the same TestFile as wrap_w3d.cpp (GameFileClass's Read and Seek,
** transcribed, over the real RAMFile.cpp), a fresh one for every
** Get_File. file_auto_ptr's constructor and destructor (ffactory.cpp, which
** pulls in the raw file classes) are transcribed below.
**
** Allocation: as in wrap_w3d.cpp, operator new[] returns zeroed memory (the
** game's allocator zero-fills) and records sizes, which give DDSMemory's
** length. Targa allocates with malloc, whose bytes are uninitialised; its
** buffers are only logged after a successful load, which writes all of
** them (the palette only when its length is positive: see ref_tga_run).
**
** Reading the results: the classes keep their data private, so this
** translation unit (only) is compiled with `private` and `protected` defined
** as `public` (after the standard headers), and reads ddsfile.h, whose
** members are private by default, with `class` defined as `struct`; class
** layouts and names do not change.
** Every result is appended to a byte log in a fixed order that the Rust
** test reproduces (tests/differential_texture.rs). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <malloc.h>
#include <memory.h>
#include <vector>

#include "wwfile.h"
#include "ffactory.h"
#include "bitmaphandler.h"
#define private public
#define protected public
#define class struct /* DDSFileClass's members are private by default */
#include "ddsfile.h"
#undef class
#include "targa.h"
#undef private
#undef protected
#include "ww3dformat.h"
#include "formconv.h"
#include "colorspace.h"

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
void ref_w3d_track_arrays(int on);
size_t ref_w3d_array_bytes(const void *p);
}

namespace {

/* The same file as wrap_w3d.cpp: GameFileClass's Read and Seek over RAMFile. */
class TestFile : public FileClass
{
public:
	TestFile(const char *data, int len) : m_theFile(ref_w3d_ramfile_open(data, len)) {}
	~TestFile() { ref_w3d_ramfile_close(m_theFile); }
	virtual char const *File_Name(void) const { return "test"; }
	virtual char const *Set_Name(char const *) { return "test"; }
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

const char *g_data = NULL;
int g_len = 0;

class TestFileFactory : public FileFactoryClass
{
public:
	virtual FileClass *Get_File(char const *) { return new TestFile(g_data, g_len); }
	virtual void Return_File(FileClass *file) { delete file; }
};
TestFileFactory g_factory;

std::vector<unsigned char> g_log;
void put(const void *p, size_t n) { g_log.insert(g_log.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_log.push_back((unsigned char)v); }
void put32(uint32_t v) { put(&v, 4); }

int copy_log(unsigned char *log, int cap, int *log_len)
{
	int n = (int)g_log.size();
	*log_len = n;
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	return 0;
}

} /* namespace */

FileFactoryClass *_TheFileFactory = &g_factory;
RawFileFactoryClass *_TheWritingFileFactory = NULL;
/* Targa's write modes only (never called). */
RawFileClass *RawFileFactoryClass::Get_File(char const *) { abort(); }
void RawFileFactoryClass::Return_File(FileClass *) { abort(); }

/* ffactory.cpp, transcribed (a factory that returns no file gets a
** BufferedFileClass there; the test factory always returns one). */
file_auto_ptr::file_auto_ptr(FileFactoryClass *fac, const char *filename) : _Ptr(NULL), _Fac(fac)
{
	_Ptr = _Fac->Get_File(filename);
	if (_Ptr == NULL) abort();
}
file_auto_ptr::~file_auto_ptr() { _Fac->Return_File(_Ptr); }

/* bitmaphandler.cpp (not compiled: its mipmap and image-copy functions are
** not used here): the assert hook Write_B8G8R8A8 calls for formats it does
** not write, a WWASSERT, which is empty in the release build. */
void Bitmap_Assert(bool) {}

namespace {

void log_dds_header(DDSFileClass &d, unsigned max_levels)
{
	put32(d.Width);
	put32(d.Height);
	put32(d.Depth);
	put32(d.FullWidth);
	put32(d.FullHeight);
	put32(d.FullDepth);
	put32(d.MipLevels);
	put32((uint32_t)d.DateTime);
	put32(d.ReductionFactor);
	put32((uint32_t)d.Format);
	put32((uint32_t)d.Type);
	put32(d.CubeFaceSize);
	put(&d.SurfaceDesc, sizeof(LegacyDDSURFACEDESC2));
	put8(d.Is_Available());
	if (d.Is_Available() && d.MipLevels <= max_levels) {
		for (unsigned l = 0; l < d.MipLevels; ++l) {
			put32(d.LevelSizes[l]);
			put32(d.LevelOffsets[l]);
		}
	}
}

} /* namespace */

namespace {
/* The game fills the D3D-to-WW3D table when it starts Direct3D
** (DX8Wrapper::Init), before any texture is read. */
void init_formats()
{
	static bool done = false;
	if (!done) {
		Init_D3D_To_WW3_Conversion();
		done = true;
	}
}
} /* namespace */

extern "C" {

/* formconv.cpp, ww3dformat.cpp, bitmaphandler.h, colorspace.h. */
unsigned ref_d3d_to_ww3d(unsigned code)
{
	init_formats();
	return (unsigned)D3DFormat_To_WW3DFormat((D3DFORMAT)code);
}
unsigned ref_bytes_per_pixel(unsigned format) { return Get_Bytes_Per_Pixel((WW3DFormat)format); }
/* Write_B8G8R8A8 into out[0..8], which starts filled with 0xA5. */
void ref_write_b8g8r8a8(unsigned format, unsigned argb, unsigned char *out)
{
	memset(out, 0xA5, 8);
	BitmapHandlerClass::Write_B8G8R8A8(out, (WW3DFormat)format, argb);
}
unsigned ref_recolor(unsigned rgba, const float *hsv)
{
	Recolor(rgba, Vector3(hsv[0], hsv[1], hsv[2]));
	return rgba;
}

/* The layout the DDS file is read into. */
int ref_dds_surface_desc_size(void) { return (int)sizeof(LegacyDDSURFACEDESC2); }

/* DDSFileClass on the file `data`: the constructor with `reduction`, the
** header and level table logged (up to `max_levels` levels), then, if
** `do_load`, Load() and the payload, then the operations in `ops` (9
** words each: kind, level, x or face, y, dest format, width, height, pitch,
** buffer length; hsv: 3 floats each):
**   0 Get_Pixel(level, x, y)
**   1 Copy_Level_To_Surface(level, format, width, height, buffer, pitch, hsv)
**   2 Copy_CubeMap_Level_To_Surface(face, level, ...)
**   3 Get_4x4_Block(buffer, pitch, format, level, x, y, hsv)
**   4 Get_Width, Get_Height, Get_Depth (level)
**   5 Get_Level_Size(level), LevelOffsets[level], Get_CubeMap_Memory_Pointer
**     (face, level) as an offset
**   6 Copy_Volume_Level_To_Surface(level, 1, format, width, height, buffer,
**     pitch, pitch * height, hsv)
** Destination buffers start filled with 0xA5 and are logged whole. */
int ref_dds_run(const char *data, int len, unsigned reduction, int do_load, unsigned max_levels,
	const unsigned *ops, const float *hsv, int n_ops, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	g_data = data;
	g_len = len;
	init_formats();
	ref_w3d_track_arrays(1);
	DDSFileClass *d = new DDSFileClass("t.dds", reduction);
	log_dds_header(*d, max_levels);
	if (do_load) {
		put8('L');
		put8(d->Load());
		put8(d->DDSMemory != NULL);
		if (d->DDSMemory) {
			size_t size = ref_w3d_array_bytes(d->DDSMemory);
			put32((uint32_t)size);
			size_t shown = size < (size_t)len ? size : (size_t)len;
			put(d->DDSMemory, shown);
			int rest_zero = 1;
			for (size_t i = shown; i < size && i < ((size_t)1 << 24); ++i)
				if (d->DDSMemory[i]) rest_zero = 0;
			put8(rest_zero);
		}
	}
	for (int i = 0; i < n_ops; ++i) {
		const unsigned *o = ops + 9 * i;
		Vector3 h(hsv[3 * i], hsv[3 * i + 1], hsv[3 * i + 2]);
		unsigned kind = o[0], level = o[1], xf = o[2], y = o[3];
		WW3DFormat fmt = (WW3DFormat)o[4];
		unsigned w = o[5], ht = o[6], pitch = o[7], blen = o[8];
		put8('o');
		put8(kind);
		std::vector<unsigned char> buf(blen ? blen : 1, 0xA5);
		switch (kind) {
		case 0: put32(d->Get_Pixel(level, xf, y)); break;
		case 1: d->Copy_Level_To_Surface(level, fmt, w, ht, &buf[0], pitch, h); put(&buf[0], blen); break;
		case 2: d->Copy_CubeMap_Level_To_Surface(xf, level, fmt, w, ht, &buf[0], pitch, h); put(&buf[0], blen); break;
		case 3: put8(d->Get_4x4_Block(&buf[0], pitch, fmt, level, xf, y, h)); put(&buf[0], blen); break;
		case 4: put32(d->Get_Width(level)); put32(d->Get_Height(level)); put32(d->Get_Depth(level)); break;
		case 5:
			put32(d->Get_Level_Size(level));
			put32(d->LevelOffsets[level]);
			put32((uint32_t)(d->Get_CubeMap_Memory_Pointer(xf, level) - d->DDSMemory));
			break;
		case 6: d->Copy_Volume_Level_To_Surface(level, 1, fmt, w, ht, &buf[0], pitch, pitch * ht, h); put(&buf[0], blen); break;
		default: abort();
		}
	}
	delete d;
	ref_w3d_track_arrays(0);
	return copy_log(log, cap, log_len);
}

/* Targa on the file `data`.
** flow 0, the game's texture loader (textureloader.cpp,
**   Load_Uncompressed_Mipmap): Open; Header.ImageDescriptor ^= TGAIDF_YORIGIN;
**   Get_WW3D_Format; SetPalette(a 1024-byte buffer, zeroed here, stack
**   garbage in the game); Load(name, TGAF_IMAGE, false).
** flow 1, the tools' load: Load(name, TGAF_IMAGE | TGAF_PAL, true).
** After each step: the error code, the header, mFlags, the extension, and,
** after a successful Load, the image and the palette. */
int ref_tga_run(const char *data, int len, int flow, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	g_data = data;
	g_len = len;
	Targa *t = new Targa();
	char palette[1024];
	memset(palette, 0, sizeof(palette));
	struct L {
		static void state(Targa &t, int err)
		{
			put32((uint32_t)err);
			put(&t.Header, sizeof(TGAHeader));
			put32((uint32_t)t.mFlags);
			put(&t.mExtension, sizeof(TGA2Extension));
			put8(t.GetExtension() != NULL);
			put8(t.IsCompressed());
			put8(t.Is_File_Open());
		}
	};
	if (flow == 0) {
		int err = t->Open("t.tga", TGA_READMODE);
		put8('O');
		L::state(*t, err);
		if (!err) {
			t->Header.ImageDescriptor ^= TGAIDF_YORIGIN;
			WW3DFormat src;
			unsigned bpp;
			Get_WW3D_Format(src, bpp, *t);
			put32((uint32_t)src);
			put32(bpp);
			t->SetPalette(palette);
			err = t->Load("t.tga", TGAF_IMAGE, false);
			put8('L');
			L::state(*t, err);
			if (!err) {
				put8(t->mImage != NULL);
				if (t->mImage) {
					int size = (t->Header.Width * t->Header.Height) * TGA_BytesPerPixel(t->Header.PixelDepth);
					put(t->mImage, size);
				}
				put(palette, sizeof(palette));
			}
			t->SetPalette(NULL);
		}
	} else {
		int err = t->Load("t.tga", TGAF_IMAGE | TGAF_PAL, true);
		put8('L');
		L::state(*t, err);
		if (!err) {
			put8(t->mImage != NULL);
			if (t->mImage) {
				int size = (t->Header.Width * t->Header.Height) * TGA_BytesPerPixel(t->Header.PixelDepth);
				put(t->mImage, size);
			}
			put8(t->mPalette != NULL);
			/* read only for a positive length (else uninitialised) */
			if (t->mPalette && t->Header.CMapLength > 0) put(t->mPalette, t->Header.CMapLength * (t->Header.CMapDepth >> 3));
		}
	}
	delete t;
	return copy_log(log, cap, log_len);
}

} /* extern "C" */
