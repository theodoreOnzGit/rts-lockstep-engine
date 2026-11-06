/* Differential-test wrapper for the last W3D prototype loaders (GitHub #17;
** upstream commit 0a05454d):
**   WW3D2/hmdldef.cpp: HModelDefClass (constructor, destructor, Free,
**     Load_W3D, read_connection), COMPILED unchanged;
**   WW3D2/distlod.cpp: DistLODDefClass (constructors, destructor, Free,
**     Load_W3D, read_header, read_node), COMPILED from a verbatim segment,
**     with distlod.h's DistLODNodeDefStruct and DistLODDefClass (verbatim,
**     w3d_gen/distlod_def.h), and WWLib/nstrdup.cpp unchanged;
**   WW3D2/part_ldr.cpp: every ParticleEmitterDefClass function (Load_W3D,
**     Initialize_To_Ver2, Convert_To_Ver2, the Read_* functions,
**     Create_Randomizer, ...), COMPILED from a verbatim segment, with
**     part_ldr.h's class and part_emt.h's ParticlePropertyStruct (verbatim,
**     w3d_gen/part_ldr_def.h), shader.cpp's two sprite presets (verbatim),
**     WWMath/v3_rnd.cpp and WWLib/random.cpp unchanged, and w3d_util.cpp's
**     Convert_Shader (the mesh library's copy, unchanged).
** See build.rs for the segments. The real w3d_file.h, chunkio.h,
** hmdldef.H, snapPts.h, v3_rnd.h, shader.h and w3d_util.h give every layout.
**
** TRANSCRIBED (stand-ins):
**   - WW3DAssetManager::Get_Texture(filename) (renamed EmitAssetShim): the
**     first lines of assetmgr.cpp's Get_Texture (null for a null or empty
**     name); otherwise the name is recorded and a stand-in texture whose
**     format has alpha or not, as the test asks, is returned (the real one
**     depends on the renderer's state; see src/formats/w3d/part_ldr.rs).
**   - The loaders' Load_W3D (proto.cpp HModelLoaderClass, distlod.cpp
**     DistLODLoaderClass, part_ldr.cpp ParticleEmitterLoaderClass): a new
**     definition, its Load_W3D, the definition deleted on an error. The
**     prototypes they wrap need the render objects. The asset-manager
**     wrapper (wrap_w3d_assetmgr.cpp) calls ref_w3d_emit_load_for_assetmgr
**     for the same thing.
**   - malloc for part_ldr.cpp's text (renamed ref_emit_malloc, build.rs):
**     malloc, recording the size, so the user string's bytes can be read
**     back (its last byte is never written).
** Allocation otherwise is wrap_w3d.cpp's (zero-filled operator new and
** new[], as the game's allocator); the file is the same RAMFile. The log is
** reproduced by tests/differential_w3d.rs (section 15). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <map>
#include <string>
#include <vector>

#define private public
#define protected public
#include "hmdldef.h"
#include "snappts.h"
#include "distlod_def.h"
#include "part_ldr_def.h"
#include "w3d_file.h"
#include "chunkio.h"
#undef private
#undef protected
#include "assetmgr.h" /* the prelude's stand-in, renamed EmitAssetShim */

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}

/* ---- malloc with its size (part_ldr.cpp's text) ----------------------- */
static std::map<void *, size_t> &malloc_sizes()
{
	static std::map<void *, size_t> m;
	return m;
}
void *ref_emit_malloc(size_t n)
{
	void *p = malloc(n);
	if (p) malloc_sizes()[p] = n;
	return p;
}

/* ---- the texture lookup of Convert_To_Ver2 ---------------------------- */
static int g_alpha = 0;
static std::vector<std::string> g_textures_asked;
static TextureClass g_texture;
WW3DAssetManager *WW3DAssetManager::Get_Instance()
{
	static WW3DAssetManager mgr;
	return &mgr;
}
TextureClass *WW3DAssetManager::Get_Texture(const char *filename)
{
	/* assetmgr.cpp: "Bail if the user isn't really asking for anything" */
	if ((filename == NULL) || (strlen(filename) == 0)) return NULL;
	g_textures_asked.push_back(filename);
	g_texture.Format = g_alpha ? WW3D_FORMAT_A8R8G8B8 : WW3D_FORMAT_R8G8B8;
	return &g_texture;
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
/* a C string: strlen, then its bytes; 0xFFFFFFFF for NULL */
void putStr(const char *s)
{
	if (!s) { put32(0xFFFFFFFFu); return; }
	size_t n = strlen(s);
	put32((uint32_t)n);
	put(s, n);
}

/* HModelDefClass: Name, ModelName and BasePoseName (16 bytes each),
** SubObjectCount, each node's RenderObjName (C string) and PivotID, the
** snap points (none, or their count and coordinates). */
void log_hmodel(const HModelDefClass &d)
{
	putStr(d.Name);
	put(d.ModelName, W3D_NAME_LEN);
	put(d.BasePoseName, W3D_NAME_LEN);
	put32((uint32_t)d.SubObjectCount);
	for (int i = 0; i < d.SubObjectCount; ++i) {
		putStr(d.SubObjects[i].RenderObjName);
		put32((uint32_t)d.SubObjects[i].PivotID);
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

/* DistLODDefClass: Name, LodCount, each LOD's Name, ResDownDist, ResUpDist. */
void log_distlod(const DistLODDefClass &d)
{
	putStr(d.Name);
	put32((uint32_t)d.LodCount);
	for (int i = 0; i < d.LodCount; ++i) {
		putStr(d.Lods[i].Name);
		putF(d.Lods[i].ResDownDist);
		putF(d.Lods[i].ResUpDist);
	}
}

template <class T> void log_props(const ParticlePropertyStruct<T> &p)
{
	put(&p.Start, sizeof(T));
	put(&p.Rand, sizeof(T));
	put32(p.NumKeyFrames);
	put8(p.KeyTimes != NULL);
	put8(p.Values != NULL);
	if (p.KeyTimes && p.Values) {
		for (unsigned int i = 0; i < p.NumKeyFrames; ++i) {
			putF(p.KeyTimes[i]);
			put(&p.Values[i], sizeof(T));
		}
	}
}

void log_randomizer(const Vector3Randomizer *r)
{
	if (!r) { put32(0xFFFFFFFFu); return; }
	put32(r->Class_ID());
	switch (r->Class_ID()) {
	case Vector3Randomizer::CLASSID_SOLIDBOX: {
		const Vector3 &e = ((const Vector3SolidBoxRandomizer *)r)->Get_Extents();
		putF(e.X); putF(e.Y); putF(e.Z);
		break;
	}
	case Vector3Randomizer::CLASSID_SOLIDSPHERE: putF(((const Vector3SolidSphereRandomizer *)r)->Get_Radius()); break;
	case Vector3Randomizer::CLASSID_HOLLOWSPHERE: putF(((const Vector3HollowSphereRandomizer *)r)->Get_Radius()); break;
	case Vector3Randomizer::CLASSID_SOLIDCYLINDER:
		putF(((const Vector3SolidCylinderRandomizer *)r)->Get_Height());
		putF(((const Vector3SolidCylinderRandomizer *)r)->Get_Radius());
		break;
	default: abort();
	}
}

/* ParticleEmitterDefClass: m_pName; m_pUserString (its malloc'd size minus
** the terminator's byte, then those bytes; 0xFFFFFFFF for NULL);
** m_iUserType, m_Version; m_Info, m_InfoV2, m_ExtraInfo, m_LineProperties
** (their bytes); the six keyframe sets (start, random range, count, which
** arrays exist, each key's time and value); m_InitialOrientationRandom;
** the two randomizers (class id and parameters). */
void log_emitter(const ParticleEmitterDefClass &d)
{
	putStr(d.m_pName);
	if (!d.m_pUserString) {
		put32(0xFFFFFFFFu);
	} else {
		size_t n = malloc_sizes().at(d.m_pUserString) - 1;
		put32((uint32_t)n);
		put(d.m_pUserString, n);
	}
	put32((uint32_t)d.m_iUserType);
	put32(d.m_Version);
	put(&d.m_Info, sizeof(d.m_Info));
	put(&d.m_InfoV2, sizeof(d.m_InfoV2));
	put(&d.m_ExtraInfo, sizeof(d.m_ExtraInfo));
	put(&d.m_LineProperties, sizeof(d.m_LineProperties));
	log_props(d.m_ColorKeyframes);
	log_props(d.m_OpacityKeyframes);
	log_props(d.m_SizeKeyframes);
	log_props(d.m_RotationKeyframes);
	putF(d.m_InitialOrientationRandom);
	log_props(d.m_FrameKeyframes);
	log_props(d.m_BlurTimeKeyframes);
	log_randomizer(d.m_pCreationVolume);
	log_randomizer(d.m_pVelocityRandomizer);
}

} /* namespace */

/* For wrap_w3d_assetmgr.cpp: the three loaders' Load_W3D, transcribed (see
** the top): kind 1 hmodel, 2 distance LOD, 3 particle emitter. True and the
** prototype's name if one is made. The texture lookup answers "no alpha". */
bool ref_w3d_emit_load_for_assetmgr(ChunkLoadClass &cload, int kind, std::string &name)
{
	g_alpha = 0;
	if (kind == 1) {
		HModelDefClass *hdef = new HModelDefClass;
		if (hdef->Load_W3D(cload) != HModelDefClass::OK) {
			delete hdef;
			return false;
		}
		name = hdef->Get_Name();
		delete hdef;
		return true;
	} else if (kind == 2) {
		DistLODDefClass *def = new DistLODDefClass;
		if (def->Load_W3D(cload) != WW3D_ERROR_OK) {
			delete def;
			return false;
		}
		name = def->Get_Name();
		delete def;
		return true;
	} else if (kind == 3) {
		ParticleEmitterDefClass *def = new ParticleEmitterDefClass;
		if (def->Load_W3D(cload) != WW3D_ERROR_OK) {
			delete def;
			return false;
		}
		name = def->Get_Name();
		delete def;
		return true;
	}
	abort();
}

extern "C" {

/* Load the first chunk of the file with the loader of `kind` (0 hmodel,
** 1 distance LOD, 2 particle emitter, as their prototype loaders call
** them; `alpha`: whether the emitter's texture has alpha) and log: 'O' and
** whether the chunk opened; then 'H' / 'D' / 'P' and the definition on
** success, or 'e' and the error code; then 's', the chunk depth and the
** file position (Tell); for the emitter, 't' and the texture names asked
** for. */
int ref_w3d_load_emit(const char *data, int len, int kind, int alpha, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	g_alpha = alpha;
	g_textures_asked.clear();
	{
		TestFile file(data, len);
		ChunkLoadClass cload(&file);
		put8('O');
		if (cload.Open_Chunk()) {
			put8(1);
			if (kind == 0) {
				HModelDefClass *def = new HModelDefClass;
				int r = def->Load_W3D(cload);
				if (r == HModelDefClass::OK) {
					put8('H');
					log_hmodel(*def);
				} else {
					put8('e');
					put8(r);
				}
				delete def;
			} else if (kind == 1) {
				DistLODDefClass *def = new DistLODDefClass;
				WW3DErrorType r = def->Load_W3D(cload);
				if (r == WW3D_ERROR_OK) {
					put8('D');
					log_distlod(*def);
				} else {
					put8('e');
					put8(r);
				}
				delete def;
			} else if (kind == 2) {
				ParticleEmitterDefClass *def = new ParticleEmitterDefClass;
				WW3DErrorType r = def->Load_W3D(cload);
				if (r == WW3D_ERROR_OK) {
					put8('P');
					log_emitter(*def);
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
	if (kind == 2) {
		put8('t');
		put32((uint32_t)g_textures_asked.size());
		for (size_t i = 0; i < g_textures_asked.size(); ++i) putStr(g_textures_asked[i].c_str());
	}
	int n = (int)g_log.size();
	*log_len = n;
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	return 0;
}

} /* extern "C" */
