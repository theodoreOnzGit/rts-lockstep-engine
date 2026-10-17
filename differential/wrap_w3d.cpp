/* Differential-test wrapper for the W3D readers (upstream commit 0a05454d):
**   WWLib/chunkio.cpp                    (compiled unchanged)
**   WW3D2/htree.cpp, WW3D2/motchan.cpp   (copies with MSVC-scope edits, see build.rs)
**   WW3D2/hrawanim.cpp, WW3D2/hcanim.cpp, WW3D2/pivot.cpp, WWMath/quat.cpp
**                                        (compiled unchanged)
** with differential/shim_w3d/w3d_prelude.h force-included (see that file for
** each stand-in) and the real WW3D2/w3d_file.h.
**
** The file: ChunkLoadClass reads a FileClass. In the game that is a
** GameFileClass (GameEngineDevice/.../W3DFileSystem.cpp) over the RAMFile
** a .big archive hands out. TestFile's Read and Seek are GameFileClass's,
** transcribed (two forwarding functions: W3DFileSystem.cpp cannot be
** compiled here), over the real RAMFile.cpp (differential/wrap_w3d_file.cpp).
**
** Reading the results: the loaded classes keep their data private, so this
** translation unit (only) is compiled with `private` and `protected` defined
** as `public` (after the standard headers); class layouts do not change.
** Array lengths the classes do not store are taken from a registry of the
** sizes passed to operator new[], which this file replaces.
**
** Allocation: the game replaces the global operator new and new[]
** (GameEngine/Source/Common/System/GameMemory.cpp) with its
** DynamicMemoryAllocator, which zero-fills every block, and the W3DMPO pools
** come from the same memory manager. Here, too, every operator new, new[]
** and pool allocation returns zeroed memory, so bytes the loaders never write
** (the tail of a name buffer after its terminator) have the game's value.
**
** Every result is appended to a byte log in a fixed order that the Rust test
** reproduces from its own structs (tests/differential_w3d.rs). */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <map>
#include <mutex>
#include <string>
#include <vector>
#include <algorithm>
#include <new>

#define private public
#define protected public
#include "chunkio.h"
#include "w3d_file.h"
#include "htree.h"
#include "hanim.h"
#include "hrawanim.h"
#include "hcanim.h"
#include "motchan.h"
#undef private
#undef protected

/* NodeCompressedMotionStruct is defined in hcanim.cpp, not in a header;
** this is that definition, copied (hcanim.cpp lines 63-98), so that the
** loaded channels can be read here. */
struct NodeCompressedMotionStruct
{
	NodeCompressedMotionStruct();
	~NodeCompressedMotionStruct();
	void SetFlavor(int flavor)  {Flavor = flavor;}
	int Flavor;
	union {
		struct {
			TimeCodedMotionChannelClass *		X;
			TimeCodedMotionChannelClass *		Y;
			TimeCodedMotionChannelClass *		Z;
			TimeCodedMotionChannelClass *		Q;
		} tc;
		struct {
			AdaptiveDeltaMotionChannelClass *		X;
			AdaptiveDeltaMotionChannelClass *		Y;
			AdaptiveDeltaMotionChannelClass *		Z;
			AdaptiveDeltaMotionChannelClass *		Q;
		} ad;
		struct {
			 void * X;
			 void * Y;
			 void * Z;
			 void * Q;
		} vd;
	};
	TimeCodedBitChannelClass *			Vis;
};

/* ---- operator new[] with a size registry ---------------------------- */
namespace {
std::mutex &reg_mutex() { static std::mutex m; return m; }
std::map<void *, size_t> &registry() { static std::map<void *, size_t> r; return r; }
bool g_track = false;
} /* namespace */

void *operator new(size_t n)
{
	void *p = calloc(1, n ? n : 1);
	if (!p) throw std::bad_alloc();
	return p;
}
void operator delete(void *p) noexcept { free(p); }
void operator delete(void *p, size_t) noexcept { free(p); }

void *operator new[](size_t n)
{
	void *p = calloc(1, n ? n : 1);
	if (!p) throw std::bad_alloc();
	std::lock_guard<std::mutex> l(reg_mutex());
	if (g_track) registry()[p] = n;
	return p;
}
void operator delete[](void *p) noexcept
{
	if (!p) return;
	{
		std::lock_guard<std::mutex> l(reg_mutex());
		registry().erase(p);
	}
	free(p);
}
void operator delete[](void *p, size_t) noexcept { operator delete[](p); }

static size_t array_bytes(const void *p)
{
	std::lock_guard<std::mutex> l(reg_mutex());
	std::map<void *, size_t>::iterator it = registry().find((void *)p);
	if (it == registry().end()) abort();
	return it->second;
}

/* For differential/wrap_texture.cpp: the registry, switched on and read. */
extern "C" void ref_w3d_track_arrays(int on)
{
	std::lock_guard<std::mutex> l(reg_mutex());
	g_track = on != 0;
}
extern "C" size_t ref_w3d_array_bytes(const void *p) { return array_bytes(p); }

/* ---- the W3DMPO pools (always.h) on malloc/free --------------------- */
void *createW3DMemPool(const char *, int) { return (void *)1; }
void *allocateFromW3DMemPool(void *, int n) { return calloc(1, n); }
void *allocateFromW3DMemPool(void *, int n, const char *, int) { return calloc(1, n); }
void freeFromW3DMemPool(void *, void *p) { free(p); }

/* ---- stand-ins never reached by the loaders (they abort) ------------ */
HAnimClass *HAnimComboClass::Get_Motion(int) { abort(); }
float HAnimComboClass::Get_Weight(int) { abort(); }
HAnimClass *HAnimComboClass::Peek_Motion(int) { abort(); }
PivotMapClass *HAnimComboClass::Get_Pivot_Weight_Map(int) { abort(); }
float HAnimComboClass::Get_Frame(int) { abort(); }

/* ---- the file -------------------------------------------------------- */
extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
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
	/* GameFileClass::Read */
	virtual int Read(void *buffer, int len) { return ref_w3d_ramfile_read(m_theFile, buffer, len); }
	/* GameFileClass::Seek: SEEK_CUR (and anything else) -> CURRENT, SEEK_SET -> START, SEEK_END -> END */
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
/* a C string inside an array of n bytes: up to the first 0, or all n */
void putName(const char *s, int n)
{
	int k = 0;
	while (k < n && s[k]) ++k;
	put32((uint32_t)k);
	put(s, k);
}

int copy_log(unsigned char *log, int cap, int *log_len)
{
	int n = (int)g_log.size();
	*log_len = n;
	if (n > cap) n = cap;
	if (n > 0) memcpy(log, &g_log[0], n);
	return 0;
}

/* ---- the hierarchies the animation loaders find --------------------- */
std::vector<std::pair<std::string, HTreeClass *> > g_trees;

} /* namespace */

WW3DAssetManager *WW3DAssetManager::Get_Instance() { static WW3DAssetManager m; return &m; }

/* The stand-in for the asset manager's lookup: the 16 name bytes as the
** animation loader passes them (its zero-padded HierarchyName), exactly. */
HTreeClass *WW3DAssetManager::Get_HTree(const char *name)
{
	for (size_t i = 0; i < g_trees.size(); ++i)
		if (memcmp(g_trees[i].first.data(), name, W3D_NAME_LEN) == 0) return g_trees[i].second;
	return NULL;
}


namespace {

void log_matrix(const Matrix3D &m)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) putF(m[r][c]);
}

void log_motion_channel(const MotionChannelClass *c)
{
	if (!c) { put8(0); return; }
	put8(1);
	put32(c->PivotIdx); put32(c->Type); put32((uint32_t)c->VectorLen);
	put32((uint32_t)c->FirstFrame); put32((uint32_t)c->LastFrame);
	size_t n = array_bytes(c->Data) / 4;
	put32((uint32_t)n);
	put(c->Data, n * 4);
}

void log_bit_channel(const BitChannelClass *c)
{
	if (!c) { put8(0); return; }
	put8(1);
	put32(c->PivotIdx); put32(c->Type); put32((uint32_t)c->DefaultVal);
	put32((uint32_t)c->FirstFrame); put32((uint32_t)c->LastFrame);
	size_t n = array_bytes(c->Bits);
	put32((uint32_t)n);
	put(c->Bits, n);
}

void log_tc_channel(const TimeCodedMotionChannelClass *c)
{
	if (!c) { put8(0); return; }
	put8(1);
	put32(c->PivotIdx); put32(c->Type); put32((uint32_t)c->VectorLen); put32(c->PacketSize);
	put32(c->NumTimeCodes); put32(c->LastTimeCodeIdx); put32(c->CachedIdx);
	size_t n = array_bytes(c->Data) / 4;
	put32((uint32_t)n);
	put(c->Data, n * 4);
}

void log_ad_channel(const AdaptiveDeltaMotionChannelClass *c)
{
	if (!c) { put8(0); return; }
	put8(2);
	put32(c->PivotIdx); put32(c->Type); put32((uint32_t)c->VectorLen); put32(c->NumFrames);
	putF(c->Scale); put32(c->CacheFrame);
	put32((uint32_t)(array_bytes(c->CacheData) / 4));
	size_t n = array_bytes(c->Data) / 4;
	put32((uint32_t)n);
	put(c->Data, n * 4);
}

void log_tc_bit_channel(const TimeCodedBitChannelClass *c)
{
	if (!c) { put8(0); return; }
	put8(1);
	put32(c->PivotIdx); put32(c->Type); put32((uint32_t)c->DefaultVal); put32(c->NumTimeCodes);
	put32(c->CachedIdx);
	size_t n = array_bytes(c->Bits) / 4;
	put32((uint32_t)n);
	put(c->Bits, n * 4);
}

void log_state(ChunkLoadClass &cload, FileClass &file)
{
	put8('s');
	put32((uint32_t)cload.Cur_Chunk_Depth());
	put32((uint32_t)file.Tell());
}

} /* namespace */

extern "C" {

/* ---- ChunkLoadClass, driven by a script -------------------------------
** ops (one byte each; 10 and 11 take a uint32 operand):
**   0 Open_Chunk  1 Close_Chunk  2 Cur_Chunk_ID  3 Cur_Chunk_Length
**   4 Cur_Chunk_Depth  5 Contains_Chunks  6 Open_Micro_Chunk
**   7 Close_Micro_Chunk  8 Cur_Micro_Chunk_ID  9 Cur_Micro_Chunk_Length
**   10 Read(n)  11 Seek(n)  12 the file's Tell()
** Ops 1, 2, 3, 5, 6, 10 and 11 are skipped (logged 'z') with no chunk
** open: they read HeaderStack[-1] (Open_Micro_Chunk through Read). Reads go into a 65536-byte buffer filled
** with 0xCD first; the first min(n, 64) bytes of it are logged. */
int ref_chunkio_run(const char *data, int len, const unsigned char *script, int slen, unsigned char *log, int cap,
	int *log_len)
{
	g_log.clear();
	TestFile file(data, len);
	ChunkLoadClass cload(&file);
	static unsigned char buf[65536];
	for (int i = 0; i < slen;) {
		unsigned char op = script[i++];
		uint32_t n = 0;
		if (op == 10 || op == 11) {
			memcpy(&n, script + i, 4);
			i += 4;
		}
		bool needs = op == 1 || op == 2 || op == 3 || op == 5 || op == 6 || op == 10 || op == 11;
		if (needs && cload.Cur_Chunk_Depth() == 0) { put8('z'); continue; }
		switch (op) {
		case 0: put8('O'); put8(cload.Open_Chunk()); break;
		case 1: put8('C'); put8(cload.Close_Chunk()); break;
		case 2: put8('I'); put32(cload.Cur_Chunk_ID()); break;
		case 3: put8('L'); put32(cload.Cur_Chunk_Length()); break;
		case 4: put8('D'); put32((uint32_t)cload.Cur_Chunk_Depth()); break;
		case 5: put8('H'); put32((uint32_t)cload.Contains_Chunks()); break;
		case 6: put8('o'); put8(cload.Open_Micro_Chunk()); break;
		case 7: put8('c'); put8(cload.Close_Micro_Chunk()); break;
		case 8: put8('i'); put32(cload.Cur_Micro_Chunk_ID()); break;
		case 9: put8('l'); put32(cload.Cur_Micro_Chunk_Length()); break;
		case 10: {
			memset(buf, 0xCD, sizeof(buf));
			put8('R');
			put32(cload.Read(buf, n));
			put(buf, n < 64 ? n : 64);
			break;
		}
		case 11: put8('S'); put32(cload.Seek(n)); break;
		case 12: put8('T'); put32((uint32_t)file.Tell()); break;
		default: abort();
		}
	}
	return copy_log(log, cap, log_len);
}

/* ---- the struct layouts of w3d_file.h --------------------------------
** ref_w3d_layouts: for every struct, its name, sizeof, and each member's
** name, offsetof and sizeof. ref_w3d_parse(i, bytes): memcpy the bytes into
** struct i (as cload.Read does) and log each member's bytes. */
#define W3D_STRUCTS(S) \
	S(W3dChunkHeader, F(ChunkType) F(ChunkSize)) \
	S(W3dVectorStruct, F(X) F(Y) F(Z)) \
	S(W3dQuaternionStruct, F(Q)) \
	S(W3dTexCoordStruct, F(U) F(V)) \
	S(W3dRGBStruct, F(R) F(G) F(B) F(pad)) \
	S(W3dRGBAStruct, F(R) F(G) F(B) F(A)) \
	S(W3dMaterialInfoStruct, F(PassCount) F(VertexMaterialCount) F(ShaderCount) F(TextureCount)) \
	S(W3dVertexMaterialStruct, F(Attributes) F(Ambient) F(Diffuse) F(Specular) F(Emissive) F(Shininess) F(Opacity) F(Translucency)) \
	S(W3dShaderStruct, F(DepthCompare) F(DepthMask) F(ColorMask) F(DestBlend) F(FogFunc) F(PriGradient) F(SecGradient) F(SrcBlend) \
		F(Texturing) F(DetailColorFunc) F(DetailAlphaFunc) F(ShaderPreset) F(AlphaTest) F(PostDetailColorFunc) F(PostDetailAlphaFunc) F(pad)) \
	S(W3dPS2ShaderStruct, F(DepthCompare) F(DepthMask) F(PriGradient) F(Texturing) F(AlphaTest) F(AParam) F(BParam) F(CParam) F(DParam) F(pad)) \
	S(W3dTextureInfoStruct, F(Attributes) F(AnimType) F(FrameCount) F(FrameRate)) \
	S(W3dTriStruct, F(Vindex) F(Attributes) F(Normal) F(Dist)) \
	S(W3dMeshHeader3Struct, F(Version) F(Attributes) F(MeshName) F(ContainerName) F(NumTris) F(NumVertices) F(NumMaterials) \
		F(NumDamageStages) F(SortLevel) F(PrelitVersion) F(FutureCounts) F(VertexChannels) F(FaceChannels) F(Min) F(Max) F(SphCenter) F(SphRadius)) \
	S(W3dVertInfStruct, F(BoneIdx) F(Pad)) \
	S(W3dMeshDeform, F(SetCount) F(AlphaPasses) F(reserved)) \
	S(W3dDeformSetInfo, F(KeyframeCount) F(flags) F(reserved)) \
	S(W3dDeformKeyframeInfo, F(DeformPercent) F(DataCount) F(reserved)) \
	S(W3dDeformData, F(VertexIndex) F(Position) F(Color) F(reserved)) \
	S(W3dMeshAABTreeHeader, F(NodeCount) F(PolyCount) F(Padding)) \
	S(W3dMeshAABTreeNode, F(Min) F(Max) F(FrontOrPoly0) F(BackOrPolyCount)) \
	S(W3dHierarchyStruct, F(Version) F(Name) F(NumPivots) F(Center)) \
	S(W3dPivotStruct, F(Name) F(ParentIdx) F(Translation) F(EulerAngles) F(Rotation)) \
	S(W3dPivotFixupStruct, F(TM)) \
	S(W3dAnimHeaderStruct, F(Version) F(Name) F(HierarchyName) F(NumFrames) F(FrameRate)) \
	S(W3dCompressedAnimHeaderStruct, F(Version) F(Name) F(HierarchyName) F(NumFrames) F(FrameRate) F(Flavor)) \
	S(W3dAnimChannelStruct, F(FirstFrame) F(LastFrame) F(VectorLen) F(Flags) F(Pivot) F(pad) F(Data)) \
	S(W3dBitChannelStruct, F(FirstFrame) F(LastFrame) F(Flags) F(Pivot) F(DefaultVal) F(Data)) \
	S(W3dTimeCodedAnimChannelStruct, F(NumTimeCodes) F(Pivot) F(VectorLen) F(Flags) F(Data)) \
	S(W3dTimeCodedBitChannelStruct, F(NumTimeCodes) F(Pivot) F(Flags) F(DefaultVal) F(Data)) \
	S(W3dAdaptiveDeltaAnimChannelStruct, F(NumFrames) F(Pivot) F(VectorLen) F(Flags) F(Scale) F(Data)) \
	S(W3dMorphAnimHeaderStruct, F(Version) F(Name) F(HierarchyName) F(FrameCount) F(FrameRate) F(ChannelCount)) \
	S(W3dMorphAnimKeyStruct, F(MorphFrame) F(PoseFrame)) \
	S(W3dHModelHeaderStruct, F(Version) F(Name) F(HierarchyName) F(NumConnections)) \
	S(W3dHModelNodeStruct, F(RenderObjName) F(PivotIdx)) \
	S(W3dLODModelHeaderStruct, F(Version) F(Name) F(NumLODs)) \
	S(W3dLODStruct, F(RenderObjName) F(LODMin) F(LODMax)) \
	S(W3dCollectionHeaderStruct, F(Version) F(Name) F(RenderObjectCount) F(pad)) \
	S(W3dPlaceholderStruct, F(version) F(transform) F(name_len)) \
	S(W3dTransformNodeStruct, F(version) F(transform) F(name_len)) \
	S(W3dLightStruct, F(Attributes) F(Unused) F(Ambient) F(Diffuse) F(Specular) F(Intensity)) \
	S(W3dSpotLightStruct, F(SpotDirection) F(SpotAngle) F(SpotExponent)) \
	S(W3dLightAttenuationStruct, F(Start) F(End)) \
	S(W3dLightTransformStruct, F(Transform)) \
	S(W3dEmitterHeaderStruct, F(Version) F(Name)) \
	S(W3dEmitterUserInfoStruct, F(Type) F(SizeofStringParam) F(StringParam)) \
	S(W3dEmitterInfoStruct, F(TextureFilename) F(StartSize) F(EndSize) F(Lifetime) F(EmissionRate) F(MaxEmissions) F(VelocityRandom) \
		F(PositionRandom) F(FadeTime) F(Gravity) F(Elasticity) F(Velocity) F(Acceleration) F(StartColor) F(EndColor)) \
	S(W3dEmitterExtraInfoStruct, F(FutureStartTime) F(Padding)) \
	S(W3dVolumeRandomizerStruct, F(ClassID) F(Value1) F(Value2) F(Value3) F(reserved)) \
	S(W3dEmitterInfoStructV2, F(BurstSize) F(CreationVolume) F(VelRandom) F(OutwardVel) F(VelInherit) F(Shader) F(RenderMode) \
		F(FrameMode) F(reserved)) \
	S(W3dEmitterPropertyStruct, F(ColorKeyframes) F(OpacityKeyframes) F(SizeKeyframes) F(ColorRandom) F(OpacityRandom) F(SizeRandom) F(reserved)) \
	S(W3dEmitterColorKeyframeStruct, F(Time) F(Color)) \
	S(W3dEmitterOpacityKeyframeStruct, F(Time) F(Opacity)) \
	S(W3dEmitterSizeKeyframeStruct, F(Time) F(Size)) \
	S(W3dEmitterRotationHeaderStruct, F(KeyframeCount) F(Random) F(OrientationRandom) F(Reserved)) \
	S(W3dEmitterRotationKeyframeStruct, F(Time) F(Rotation)) \
	S(W3dEmitterFrameHeaderStruct, F(KeyframeCount) F(Random) F(Reserved)) \
	S(W3dEmitterFrameKeyframeStruct, F(Time) F(Frame)) \
	S(W3dEmitterBlurTimeHeaderStruct, F(KeyframeCount) F(Random) F(Reserved)) \
	S(W3dEmitterBlurTimeKeyframeStruct, F(Time) F(BlurTime)) \
	S(W3dEmitterLinePropertiesStruct, F(Flags) F(SubdivisionLevel) F(NoiseAmplitude) F(MergeAbortFactor) F(TextureTileFactor) F(UPerSec) \
		F(VPerSec) F(Reserved)) \
	S(W3dAggregateHeaderStruct, F(Version) F(Name)) \
	S(W3dAggregateInfoStruct, F(BaseModelName) F(SubobjectCount)) \
	S(W3dAggregateSubobjectStruct, F(SubobjectName) F(BoneName)) \
	S(W3dTextureReplacerHeaderStruct, F(ReplacedTexturesCount)) \
	S(W3dTextureReplacerStruct, F(MeshPath) F(BonePath) F(OldTextureName) F(NewTextureName) F(TextureParams)) \
	S(W3dAggregateMiscInfo, F(OriginalClassID) F(Flags) F(reserved)) \
	S(W3dHLodHeaderStruct, F(Version) F(LodCount) F(Name) F(HierarchyName)) \
	S(W3dHLodArrayHeaderStruct, F(ModelCount) F(MaxScreenSize)) \
	S(W3dHLodSubObjectStruct, F(BoneIndex) F(Name)) \
	S(W3dBoxStruct, F(Version) F(Attributes) F(Name) F(Color) F(Center) F(Extent)) \
	S(W3dNullObjectStruct, F(Version) F(Attributes) F(pad) F(Name)) \
	S(W3dSoundRObjHeaderStruct, F(Version) F(Name) F(Flags) F(Padding)) \
	S(W3dShdMeshHeaderStruct, F(Version) F(Attributes) F(NumTris) F(NumVertices) F(NumSubMeshes) F(FutureCounts) F(BoxMin) F(BoxMax) \
		F(SphCenter) F(SphRadius)) \
	S(W3dShdSubMeshHeaderStruct, F(NumTris) F(NumVertices) F(FutureCounts) F(BoxMin) F(BoxMax) F(SphCenter) F(SphRadius)) \
	S(W3dMaterialStruct, F(MaterialName) F(PrimaryName) F(SecondaryName) F(RenderFlags) F(Red) F(Green) F(Blue)) \
	S(W3dMaterial2Struct, F(MaterialName) F(PrimaryName) F(SecondaryName) F(RenderFlags) F(Red) F(Green) F(Blue) F(Alpha) \
		F(PrimaryNumFrames) F(SecondaryNumFrames) F(Pad)) \
	S(W3dMaterial3Struct, F(Attributes) F(DiffuseColor) F(SpecularColor) F(EmissiveCoefficients) F(AmbientCoefficients) \
		F(DiffuseCoefficients) F(SpecularCoefficients) F(Shininess) F(Opacity) F(Translucency) F(FogCoeff)) \
	S(W3dMap3Struct, F(MappingType) F(FrameCount) F(FrameRate)) \
	S(W3dSurrenderTriStruct, F(Vindex) F(TexCoord) F(MaterialIdx) F(Normal) F(Attributes) F(Gouraud)) \
	S(W3dMeshHeaderStruct, F(Version) F(MeshName) F(Attributes) F(NumTris) F(NumQuads) F(NumSrTris) F(NumPovTris) F(NumPovQuads) \
		F(NumVertices) F(NumNormals) F(NumSrNormals) F(NumTexCoords) F(NumMaterials) F(NumVertColors) F(NumVertInfluences) \
		F(NumDamageStages) F(FutureCounts) F(LODMin) F(LODMax) F(Min) F(Max) F(SphCenter) F(SphRadius) F(Translation) F(Rotation) \
		F(MassCenter) F(Inertia) F(Volume) F(HierarchyTreeName) F(HierarchyModelName) F(FutureUse)) \
	S(W3dMeshDamageStruct, F(NumDamageMaterials) F(NumDamageVerts) F(NumDamageColors) F(DamageIndex) F(FutureUse)) \
	S(W3dMeshDamageVertexStruct, F(VertexIndex) F(NewVertex)) \
	S(W3dMeshDamageColorStruct, F(VertexIndex) F(NewColor)) \
	S(W3dHModelAuxDataStruct, F(Attributes) F(MeshCount) F(CollisionCount) F(SkinCount) F(ShadowCount) F(NullCount) F(FutureCounts) \
		F(LODMin) F(LODMax) F(FutureUse))

int ref_w3d_layouts(unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
#define F(m) putName(#m, 1000); put32((uint32_t)offsetof(T, m)); put32((uint32_t)sizeof(((T *)0)->m));
#define S(name, fields) { typedef name T; put8('S'); putName(#name, 1000); put32((uint32_t)sizeof(T)); fields }
	W3D_STRUCTS(S)
#undef S
#undef F
	return copy_log(log, cap, log_len);
}

int ref_w3d_parse(int index, const unsigned char *bytes, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	int i = 0;
#define F(m) put(&v.m, sizeof(v.m));
#define S(name, fields) if (i++ == index) { name v; memcpy(&v, bytes, sizeof(v)); fields return copy_log(log, cap, log_len); }
	W3D_STRUCTS(S)
#undef S
#undef F
	return -1;
}

/* ---- the loaders ----------------------------------------------------- */

/* Register a hierarchy for Get_HTree: loaded by the real HTreeClass from a
** header-only hierarchy chunk (version 4.1) with NumPivots = num_pivots, so
** its Num_Pivots() is num_pivots as an int. */
void ref_w3d_reset_htrees()
{
	for (size_t i = 0; i < g_trees.size(); ++i) delete g_trees[i].second;
	g_trees.clear();
}

int ref_w3d_add_htree(const char *name16, unsigned num_pivots)
{
	unsigned char d[8 + 8 + 36];
	uint32_t v;
	v = W3D_CHUNK_HIERARCHY; memcpy(d, &v, 4); v = 44; memcpy(d + 4, &v, 4);
	v = W3D_CHUNK_HIERARCHY_HEADER; memcpy(d + 8, &v, 4); v = 36; memcpy(d + 12, &v, 4);
	W3dHierarchyStruct h;
	memset(&h, 0, sizeof(h));
	h.Version = W3D_MAKE_VERSION(4, 1);
	memcpy(h.Name, name16, W3D_NAME_LEN);
	h.NumPivots = num_pivots;
	memcpy(d + 16, &h, 36);
	TestFile file((const char *)d, sizeof(d));
	ChunkLoadClass cload(&file);
	cload.Open_Chunk();
	HTreeClass *t = new HTreeClass;
	if (t->Load_W3D(cload) != HTreeClass::OK) abort();
	g_trees.push_back(std::make_pair(std::string(name16, W3D_NAME_LEN), t));
	return t->Num_Pivots();
}

/* Load one top-level chunk of the given kind with the real loader:
** 0 HTreeClass, 1 HRawAnimClass, 2 HCompressedAnimClass.
** The first chunk of the file is opened, the loader runs, and its result
** and everything it loaded are logged, then the chunk depth and the file
** position (Tell). Logged pivots are capped at the first 4096. */
int ref_w3d_load(const char *data, int len, int kind, unsigned char *log, int cap, int *log_len)
{
	g_log.clear();
	struct Track {
		Track() { std::lock_guard<std::mutex> l(reg_mutex()); g_track = true; }
		~Track() { std::lock_guard<std::mutex> l(reg_mutex()); g_track = false; }
	} track;
	TestFile file(data, len);
	ChunkLoadClass cload(&file);
	put8('O');
	if (!cload.Open_Chunk()) {
		put8(0);
		log_state(cload, file);
		return copy_log(log, cap, log_len);
	}
	put8(1);
	if (kind == 0) {
		HTreeClass t;
		int r = t.Load_W3D(cload);
		put8('h'); put8(r);
		if (r == HTreeClass::OK) {
			put(t.Name, W3D_NAME_LEN);
			put32((uint32_t)t.NumPivots);
			putF(t.ScaleFactor);
			int n = t.NumPivots < 4096 ? t.NumPivots : 4096;
			for (int i = 0; i < n; ++i) {
				const PivotClass &p = t.Pivot[i];
				putName(p.Name, W3D_NAME_LEN);
				put32(p.Parent ? (uint32_t)(p.Parent - t.Pivot) : 0xFFFFFFFFu);
				log_matrix(p.BaseTransform);
				put32((uint32_t)p.Index);
			}
		}
	} else if (kind == 1) {
		HRawAnimClass *a = new HRawAnimClass;
		int r = a->Load_W3D(cload);
		put8('r'); put8(r);
		if (r == HRawAnimClass::OK) {
			put(a->Name, 2 * W3D_NAME_LEN);
			put(a->HierarchyName, W3D_NAME_LEN);
			put32((uint32_t)a->NumFrames); put32((uint32_t)a->NumNodes); putF(a->FrameRate);
			for (int i = 0; i < a->NumNodes; ++i) {
				const NodeMotionStruct &m = a->NodeMotion[i];
				if (!m.X && !m.Y && !m.Z && !m.XR && !m.YR && !m.ZR && !m.Q && !m.Vis) continue;
				put8('n'); put32((uint32_t)i);
				log_motion_channel(m.X); log_motion_channel(m.Y); log_motion_channel(m.Z);
				log_motion_channel(m.XR); log_motion_channel(m.YR); log_motion_channel(m.ZR);
				log_motion_channel(m.Q); log_bit_channel(m.Vis);
			}
		}
		a->Release_Ref();
	} else if (kind == 2) {
		HCompressedAnimClass *a = new HCompressedAnimClass;
		int r = a->Load_W3D(cload);
		put8('c'); put8(r);
		if (r == HCompressedAnimClass::OK) {
			put(a->Name, 2 * W3D_NAME_LEN);
			put(a->HierarchyName, W3D_NAME_LEN);
			put32((uint32_t)a->NumFrames); put32((uint32_t)a->NumNodes); putF(a->FrameRate);
			put32((uint32_t)a->Flavor);
			for (int i = 0; i < a->NumNodes; ++i) {
				const NodeCompressedMotionStruct &m = a->NodeMotion[i];
				if (!m.vd.X && !m.vd.Y && !m.vd.Z && !m.vd.Q && !m.Vis) continue;
				put8('n'); put32((uint32_t)i);
				if (a->Flavor == ANIM_FLAVOR_ADAPTIVE_DELTA) {
					log_ad_channel(m.ad.X); log_ad_channel(m.ad.Y); log_ad_channel(m.ad.Z); log_ad_channel(m.ad.Q);
				} else {
					log_tc_channel(m.tc.X); log_tc_channel(m.tc.Y); log_tc_channel(m.tc.Z); log_tc_channel(m.tc.Q);
				}
				log_tc_bit_channel(m.Vis);
			}
		}
		a->Release_Ref();
	} else {
		abort();
	}
	log_state(cload, file);
	return copy_log(log, cap, log_len);
}

} /* extern "C" */
