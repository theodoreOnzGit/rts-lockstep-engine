/* Differential-test wrapper for the wwshade drafts (GitHub #215, group "ww3d7", upstream commit 0a05454d):
**   src/drafts/ww3d/{shddef, shddeffactory, shddefmanager, shdinterface, shdchunk (class ids), shdlib, shdloader, shdforcelinks}.rs
**
** From upstream, cut VERBATIM by build.rs (`drafts_ww3d7`, each anchor asserted to occur once) into OUT_DIR/draft_ww3d7_gen:
**   shddef.h           from `enum SHDVER` to the end of the class (w7_shddef_h.inc);  shddef.cpp from the constructor to the end (w7_shddef_cpp.inc)
**   shddeffactory.h    from `class ShdDefFactoryClass` (w7_shdfactory_h.inc);          shddeffactory.cpp from the constructor (w7_shdfactory_cpp.inc)
**   shddefmanager.h    the class (w7_shdmanager_h.inc);  shddefmanager.cpp from `_FactoryListHead` to the end (w7_shdmanager_cpp.inc)
**   shdinterface.h     from SHD_MAX_PASSES (w7_shdinterface_h.inc);  shdinterface.cpp from the constructor (w7_shdinterface_cpp.inc)
**   dx8wrapper.h       the one line `const unsigned MAX_TEXTURE_STAGES=8;` (w7_maxtexstages.inc)
**   shdclassids.h      whole;  shdlib.cpp from SHD_Init (w7_shdlib_cpp.inc);  shdloader.h/.cpp (w7_shdloader_*.inc);  shdforcelinks.cpp from SHD_Force_Links
**   the REGISTER_SHDDEF line of each of the six shader .cpp files (w7_register_shddef.inc)
** chunkio.h / chunkio.cpp (ChunkLoadClass, the READ_MICRO_CHUNK macros), refcount.h, w3d_file.h, vector2/3/4.h are the real ones.
**
** Stand-ins, each for something the files call outside themselves:
**   EditableClass, DECLARE_EDITABLE: the level editor's property system (editable.h), which the game never reads. ENUM_PARAM: records the
**       (name, value) pairs its argument list holds (enum_param_list), which is all the macro does with them.
**   StringClass: the NAME of a shader definition: a growable byte buffer with the members the file calls: operator=(const char*), operator const char*,
**       Get_Buffer(n) (keeps the buffer when n fits, as StringClass::Uninitialised_Grow does, else a fresh one of n bytes). The wwstring.cpp
**       itself is not compiled (it casts pointers to 32-bit); see the test for the inputs this excludes (an empty or NUL-less name chunk).
**   TestFile: GameFileClass's Read and Seek over the RAMFile (as the light wrapper's).
**   REGISTER_SHDDEF: records (id, name); FORCE_LINK: records the module name (wwhack.h's declares and calls _Force_Link_<module>).
**   shdlib.cpp's callees (ShdRendererClass, WW3DAssetManager::Register_Prototype_Loader), shdloader.cpp's (ShdMeshClass, MeshClass,
**       PrimitivePrototypeClass, NEW_REF's target): scripted classes that log the call.
**
** Entry point (extern "C", tests/differential_draft_ww3d7_shd.rs): ref_dw7_shd(in, nin, out, cap) returns the words written (-1 if out is too small).
**   op 0 definition: [0, class id, n ops, per op (kind, ...)]; kinds 0 Set_Name(len, bytes), 1 Reset, 2 Set_Surface_Type(v), 3 Load(blob len, blob
**        bytes one per word) ; per op the state: [class id, surface type, name length, name bytes] and, for a Load, the result first
**   op 1 manager: [1, table (0 unique ids, 1 repeated ids), n ops, per op (kind, a)]; 0 register factory a, 1 unregister a, 2 Find_Factory(id a),
**        3 Find_Factory(name a), 4 enumerate (Get_First, Get_Next ...), 5 Get_Next(NULL), 6 Create_ShdDefClass_Instance(id a); factories are
**        0..7; results: factory index + 1 (0 = NULL); 4: count then indices; 6: 0 or class id, name digit
**   op 2 Load_Shader: [2, blob len, blob] with the eight unique-id factories registered -> [class id, surface type, name length, name bytes]
**   op 3 interface: [3, has definition, definition class id, interface class id] -> [definition class id + 1 or 0, Get_Class_ID, Greater_Than,
**        Similar_Enough, Pass_Selection, SHD_MAX_PASSES, MAX_TEXTURE_STAGES]
**   op 4 the ENUM_PARAM list of ShdDefClass::ShdDefClass: -> [n, per pair (name length, bytes, value)]
**   op 5 class ids: -> SHDDEF_CLASSID_DUMMY.._LAST (8), SHD_CLASSID_DUMMY, SHD_CLASSID_LAST
**   op 6 the registered shader definitions: -> [n, per (id, name length, bytes)] then SHD_Force_Links: [n, per module (length, bytes)]
**   op 7 shdlib: -> per function (SHD_Init, SHD_Shutdown, SHD_Init_Shaders, SHD_Shutdown_Shaders, SHD_Flush, SHD_Register_Loader): [n, call codes]
**   op 9 READ_MICRO_CHUNK: [9, blob len, blob] -> a Vector3 (initially 7, 8, 9), a Vector2 (5, 6) and an int (77), each read by READ_MICRO_CHUNK from the
**        micro-chunks 1, 2, 3 of the blob's one chunk: 3 + 2 + 1 words (float bits)
**   op 10 shader version choice: [10, pixel shader major, minor, Support_Dot3] -> ShdBumpDiffDefClass::Init's Version.Get(), ShdBumpSpecDefClass::Init's, then
**        the per-version Init each called (8, 7, 6; a diffuse call is the code, a specular one is the code + 10)
**   op 8 shdloader: [8, kind (0 ShdMeshLoader, 1 legacy), load fails] -> [prototype made, calls...] (2 Load_W3D, 6 Init_From_Legacy_Mesh, 4 PrimitivePrototype) */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <stdarg.h>
#include <strings.h>
#include <vector>
#include <string>

#include "always.h"
#include "wwdebug.h"
#include "refcount.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "w3d_file.h"
#include "w3derr.h"
#include "chunkio.h"

#define stricmp strcasecmp

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}

typedef uint32_t W;
static std::vector<W> g_out;
static void put(W w) { g_out.push_back(w); }
static void put_str(const char *s)
{
	W n = (W)strlen(s);
	put(n);
	for (W k = 0; k < n; ++k) put((unsigned char)s[k]);
}

namespace {

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

}

/* ---------------- stand-ins for the editor ---------------- */
class EditableClass { public: virtual ~EditableClass(void) {} };
#define DECLARE_EDITABLE(a, b)
static std::vector<std::pair<std::string, int> > g_enum;
static void enum_param_list(const char *first, ...)
{
	va_list ap;
	va_start(ap, first);
	std::vector<std::pair<std::string, int> > v;
	const char *n = first;
	while (n) {
		int val = va_arg(ap, int);
		v.push_back(std::make_pair(std::string(n), val));
		n = va_arg(ap, const char *);
	}
	va_end(ap);
	g_enum = v;
}
#define ENUM_PARAM(cls, var, list) enum_param_list list

typedef char TCHAR;
class StringClass {
public:
	StringClass(const char *s) { assign(s); }
	int Get_Length(void) const { return (int)strlen(&b[0]); }
	StringClass &operator=(const char *s) { assign(s); return *this; }
	char *Get_Buffer(int n) { if ((size_t)n > b.size()) b.assign((size_t)n, '\0'); return &b[0]; }
	operator const char *(void) const { return &b[0]; }
private:
	void assign(const char *s) { b.assign(s, s + strlen(s) + 1); }
	std::vector<char> b;
};

class ShdInterfaceClass;
#include "shdclassids.h"
#include "w7_shddef_h.inc"
#include "w7_shdfactory_h.inc"
class ShdDefFactoryClass;
#include "w7_shdmanager_h.inc"
#include "w7_shddef_cpp.inc"
#include "w7_shdfactory_cpp.inc"
#include "w7_shdmanager_cpp.inc"

#include "w7_maxtexstages.inc"
class TextureClass;
class ShdMeshClass;
class RenderInfoClass;
#include "w7_shdinterface_h.inc"
#include "w7_shdinterface_cpp.inc"

/* ---------------- the test classes ---------------- */
class TestDef : public ShdDefClass {
public:
	TestDef(uint32 id) : ShdDefClass(id) {}
	virtual ShdDefClass *Clone(void) const { return new TestDef(*this); }
	virtual void Init() {}
	virtual void Shutdown() {}
	virtual ShdInterfaceClass *Create(void) const { return 0; }
};

static const uint32_t IDS_A[8] = { 1, 2, 3, 4, 5, 6, 7, 9 };
static const uint32_t IDS_B[8] = { 1, 1, 2, 2, 3, 3, 4, 4 };
static const char *const FNAMES[8] = { "Simple", "simple", "Bump", "BUMP", "Cube", "cube", "Gloss", "Legacy" };
static const char *const QN[12] = { "Simple", "simple", "Bump", "BUMP", "Cube", "cube", "Gloss", "Legacy", "SIMPLE", "bump", "CUBE", "legacy" };

class TestFactory : public ShdDefFactoryClass {
public:
	TestFactory(int i, const uint32_t *ids) : idx(i), ids_(ids) {}
	virtual ShdDefClass *Create(void) const
	{
		TestDef *d = new TestDef(ids_[idx]);
		char nm[3] = { 'F', (char)('0' + idx), 0 };
		d->Set_Name(nm);
		return d;
	}
	virtual const char *Get_Name(void) const { return FNAMES[idx]; }
	virtual uint32 Get_Class_ID(void) const { return ids_[idx]; }
	int idx;
private:
	const uint32_t *ids_;
};

class TestIf : public ShdInterfaceClass {
public:
	TestIf(const ShdDefClass *d, int id) : ShdInterfaceClass(d, id) {}
	virtual int Get_Pass_Count(void) { return 1; }
	virtual void Apply_Shared(int, RenderInfoClass &) {}
	virtual void Apply_Instance(int, RenderInfoClass &) {}
	virtual unsigned Get_Vertex_Stream_Count() const { return 0; }
	virtual unsigned Get_Vertex_Size(unsigned) const { return 0; }
	virtual bool Use_HW_Vertex_Processing() const { return false; }
	virtual int Get_Texture_Count() const { return 0; }
	virtual TextureClass *Peek_Texture(int) const { return 0; }
	virtual void Copy_Vertex_Stream(unsigned, void *, const VertexStreamStruct &, unsigned) {}
};

/* ---------------- REGISTER_SHDDEF and FORCE_LINK ---------------- */
static std::vector<std::pair<uint32_t, std::string> > g_reg;
static int record_reg(uint32_t id, const char *name) { g_reg.push_back(std::make_pair(id, std::string(name))); return 0; }
#undef REGISTER_SHDDEF
#define REGISTER_SHDDEF(T, ID, NAME) static int reg_##T = record_reg(ID, NAME)
#include "w7_register_shddef.inc"

static std::vector<std::string> g_force;
#define FORCE_LINK(module) g_force.push_back(#module)
#include "w7_shdforce_cpp.inc"

/* ---------------- shdlib ---------------- */
namespace lib {
static std::vector<W> log;
struct ShdRendererClass {
	static ShdRendererClass *Peek_Instance(void) { static ShdRendererClass r; return &r; }
	void Init(void) { log.push_back(1); }
	void Shutdown(void) { log.push_back(2); }
	void Flush(void) { log.push_back(5); }
	static void Init_Shaders(void) { log.push_back(3); }
	static void Shutdown_Shaders(void) { log.push_back(4); }
};
struct PrototypeLoaderClass { int code; };
static PrototypeLoaderClass _ShdMeshLoader = { 10 };
static PrototypeLoaderClass _MeshLoader = { 11 };
struct WW3DAssetManager {
	static WW3DAssetManager *Get_Instance(void) { static WW3DAssetManager a; return &a; }
	void Register_Prototype_Loader(PrototypeLoaderClass *l) { log.push_back((W)l->code); }
};
#include "w7_shdlib_cpp.inc"
}

/* ---------------- shdloader ---------------- */
namespace ldr {
static std::vector<W> log;
static bool g_fail = false;
class PrototypeClass {};
class PrototypeLoaderClass {
public:
	virtual ~PrototypeLoaderClass(void) {}
	virtual int Chunk_Type(void) = 0;
	virtual PrototypeClass *Load_W3D(ChunkLoadClass &cload) = 0;
};
class MeshClass : public RefCountClass {
public:
	int Load_W3D(ChunkLoadClass &) { log.push_back(2); return g_fail ? WW3D_ERROR_LOAD_FAILED : WW3D_ERROR_OK; }
};
class ShdMeshClass : public RefCountClass {
public:
	int Load_W3D(ChunkLoadClass &) { log.push_back(2); return g_fail ? WW3D_ERROR_LOAD_FAILED : WW3D_ERROR_OK; }
	void Init_From_Legacy_Mesh(MeshClass *) { log.push_back(6); }
};
class PrimitivePrototypeClass : public PrototypeClass {
public:
	template <class M> PrimitivePrototypeClass(M *) { log.push_back(4); }
};
#include "w7_shdloader_h.inc"
#include "w7_shdloader_globals.inc"
#include "w7_shdloader_cpp.inc"
}

/* ---------------- the shader version choice (ShdBumpDiffDefClass::Init, ShdBumpSpecDefClass::Init) ---------------- */
namespace bump {
static std::vector<W> log;
struct Caps {
	int major, minor;
	bool dot3;
	int Get_Pixel_Shader_Major_Version() const { return major; }
	int Get_Pixel_Shader_Minor_Version() const { return minor; }
	bool Support_Dot3() const { return dot3; }
};
static Caps g_caps;
struct DX8Wrapper { static Caps *Get_Current_Caps(void) { return &g_caps; } };
struct Shd8BumpDiffClass { static void Init() { log.push_back(8); } };
struct Shd7BumpDiffClass { static void Init() { log.push_back(7); } };
struct Shd6BumpDiffClass { static void Init() { log.push_back(6); } };
struct Shd8BumpSpecClass { static void Init() { log.push_back(18); } };
struct Shd7BumpSpecClass { static void Init() { log.push_back(17); } };
struct Shd6BumpSpecClass { static void Init() { log.push_back(16); } };
struct ShdBumpDiffDefClass { ShdVersion Version; void Init(); };
struct ShdBumpSpecDefClass { ShdVersion Version; void Init(); };
#include "w7_bumpdiff_init.inc"
#include "w7_bumpspec_init.inc"
}

static void state(ShdDefClass &d)
{
	put(d.Get_Class_ID());
	put((W)d.Get_Surface_Type());
	put_str(d.Get_Name());
}

extern "C" int ref_dw7_shd(const W *in, int nin, W *out, int cap)
{
	(void)nin;
	g_out.clear();
	int i = 0;
	W op = in[i++];
	switch (op) {
	case 0: {
		TestDef def(in[i++]);
		W n = in[i++];
		for (W k = 0; k < n; ++k) {
			W kind = in[i++];
			if (kind == 0) {
				W len = in[i++];
				std::string s;
				for (W j = 0; j < len; ++j) s.push_back((char)in[i++]);
				def.Set_Name(s.c_str());
			} else if (kind == 1) {
				def.Reset();
			} else if (kind == 2) {
				def.Set_Surface_Type((int)in[i++]);
			} else {
				W len = in[i++];
				std::vector<char> blob(len);
				for (W j = 0; j < len; ++j) blob[j] = (char)in[i++];
				TestFile f(blob.empty() ? "" : &blob[0], (int)len);
				ChunkLoadClass cload(&f);
				put(def.Load(cload) ? 1 : 0);
			}
			state(def);
		}
		break;
	}
	case 1: {
		const uint32_t *ids = in[i++] == 0 ? IDS_A : IDS_B;
		TestFactory *fac[8] = { 0, 0, 0, 0, 0, 0, 0, 0 };
		W n = in[i++];
		for (W k = 0; k < n; ++k) {
			W kind = in[i++], a = in[i++];
			switch (kind) {
			case 0: if (!fac[a]) fac[a] = new TestFactory((int)a, ids); break;
			case 1: if (fac[a]) { delete fac[a]; fac[a] = 0; } break;
			case 2: { ShdDefFactoryClass *f = ShdDefManagerClass::Find_Factory((uint32)a); put(f ? (W)static_cast<TestFactory *>(f)->idx + 1 : 0); break; }
			case 3: { ShdDefFactoryClass *f = ShdDefManagerClass::Find_Factory(QN[a]); put(f ? (W)static_cast<TestFactory *>(f)->idx + 1 : 0); break; }
			case 4: {
				std::vector<W> seq;
				for (ShdDefFactoryClass *f = ShdDefManagerClass::Get_First(); f && seq.size() < 20; f = ShdDefManagerClass::Get_Next(f)) seq.push_back((W)static_cast<TestFactory *>(f)->idx);
				put((W)seq.size());
				for (size_t j = 0; j < seq.size(); ++j) put(seq[j]);
				break;
			}
			case 5: { ShdDefFactoryClass *f = ShdDefManagerClass::Get_Next(NULL); put(f ? (W)static_cast<TestFactory *>(f)->idx + 1 : 0); break; }
			default: {
				ShdDefClass *d = ShdDefManagerClass::Create_ShdDefClass_Instance((uint32)a);
				if (d) { put(d->Get_Class_ID()); put((W)(d->Get_Name()[1] - '0')); d->Release_Ref(); } else { put(0xFFFFFFFFu); }
				break;
			}
			}
		}
		for (int j = 0; j < 8; ++j) delete fac[j];
		break;
	}
	case 2: {
		TestFactory *fac[8];
		for (int j = 0; j < 8; ++j) fac[j] = new TestFactory(j, IDS_A);
		W len = in[i++];
		std::vector<char> blob(len);
		for (W j = 0; j < len; ++j) blob[j] = (char)in[i++];
		TestFile f(blob.empty() ? "" : &blob[0], (int)len);
		ChunkLoadClass cload(&f);
		ShdDefClass *def = 0;
		ShdDefManagerClass::Load_Shader(cload, &def);
		state(*def);
		def->Release_Ref();
		for (int j = 0; j < 8; ++j) delete fac[j];
		break;
	}
	case 3: {
		bool has = in[i++] != 0;
		uint32 did = in[i++];
		int cid = (int)in[i++];
		TestDef *d = new TestDef(did);
		TestIf *t = new TestIf(has ? (const ShdDefClass *)d : (const ShdDefClass *)0, cid);
		const ShdDefClass *pd = t->Peek_Definition();
		put(pd ? pd->Get_Class_ID() + 1 : 0);
		put((W)t->Get_Class_ID());
		put((W)t->Greater_Than(*t, 0));
		put((W)t->Similar_Enough(*t, 0));
		put((W)t->Pass_Selection(0, 0, 0));
		put((W)SHD_MAX_PASSES);
		put((W)MAX_TEXTURE_STAGES);
		t->Release_Ref();
		d->Release_Ref();
		break;
	}
	case 4: {
		TestDef d(0);
		put((W)g_enum.size());
		for (size_t k = 0; k < g_enum.size(); ++k) { put_str(g_enum[k].first.c_str()); put((W)g_enum[k].second); }
		break;
	}
	case 5: {
		W ids[10] = { SHDDEF_CLASSID_DUMMY, SHDDEF_CLASSID_SIMPLE, SHDDEF_CLASSID_GLOSSMASK, SHDDEF_CLASSID_BUMPSPEC, SHDDEF_CLASSID_BUMPDIFF,
			SHDDEF_CLASSID_CUBEMAP, SHDDEF_CLASSID_LEGACYW3D, SHDDEF_CLASSID_LAST, SHD_CLASSID_DUMMY, SHD_CLASSID_LAST };
		for (int k = 0; k < 10; ++k) put(ids[k]);
		break;
	}
	case 6: {
		put((W)g_reg.size());
		for (size_t k = 0; k < g_reg.size(); ++k) { put(g_reg[k].first); put_str(g_reg[k].second.c_str()); }
		g_force.clear();
		SHD_Force_Links();
		put((W)g_force.size());
		for (size_t k = 0; k < g_force.size(); ++k) put_str(g_force[k].c_str());
		break;
	}
	case 7: {
		void (*fns[6])() = { lib::SHD_Init, lib::SHD_Shutdown, lib::SHD_Init_Shaders, lib::SHD_Shutdown_Shaders, lib::SHD_Flush, lib::SHD_Register_Loader };
		for (int k = 0; k < 6; ++k) {
			lib::log.clear();
			fns[k]();
			put((W)lib::log.size());
			for (size_t j = 0; j < lib::log.size(); ++j) put(lib::log[j]);
		}
		break;
	}
	case 8: {
		W kind = in[i++];
		ldr::g_fail = in[i++] != 0;
		ldr::log.clear();
		TestFile f("", 0);
		ChunkLoadClass cload(&f);
		ldr::PrototypeClass *p;
		if (kind == 0) p = ldr::_ShdMeshLoader.Load_W3D(cload); else p = ldr::_ShdMeshLegacyLoader.Load_W3D(cload);
		put(p ? 1 : 0);
		for (size_t j = 0; j < ldr::log.size(); ++j) put(ldr::log[j]);
		break;
	}
	case 10: {
		bump::g_caps.major = (int)in[i++];
		bump::g_caps.minor = (int)in[i++];
		bump::g_caps.dot3 = in[i++] != 0;
		bump::log.clear();
		bump::ShdBumpDiffDefClass d;
		bump::ShdBumpSpecDefClass sp;
		d.Init();
		sp.Init();
		put((W)d.Version.Get());
		put((W)sp.Version.Get());
		for (size_t k = 0; k < bump::log.size(); ++k) put(bump::log[k]);
		break;
	}
	case 9: {
		W len = in[i++];
		std::vector<char> blob(len);
		for (W j = 0; j < len; ++j) blob[j] = (char)in[i++];
		TestFile f(blob.empty() ? "" : &blob[0], (int)len);
		ChunkLoadClass cload(&f);
		Vector3 v(7.0f, 8.0f, 9.0f);
		Vector2 w(5.0f, 6.0f);
		int surf = 77;
		cload.Open_Chunk();
		while (cload.Open_Micro_Chunk()) {
			switch (cload.Cur_Micro_Chunk_ID()) {
				READ_MICRO_CHUNK(cload, 1, v);
				READ_MICRO_CHUNK(cload, 2, w);
				READ_MICRO_CHUNK(cload, 3, surf);
			}
			cload.Close_Micro_Chunk();
		}
		W b[6];
		memcpy(&b[0], &v.X, 4); memcpy(&b[1], &v.Y, 4); memcpy(&b[2], &v.Z, 4); memcpy(&b[3], &w.U, 4); memcpy(&b[4], &w.V, 4); memcpy(&b[5], &surf, 4);
		for (int k = 0; k < 6; ++k) put(b[k]);
		break;
	}
	default: return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
