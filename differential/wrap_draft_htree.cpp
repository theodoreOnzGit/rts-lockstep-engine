/* Differential-test wrapper for the W3D hierarchy and animation drafts (GitHub #215, upstream commit 0a05454d), group
** "anim", part 1: src/drafts/sweep_ww3d/{htree,hrawanim,hcanim,quat,vp}.rs.
**
** NO CUT of anything that already links: the functions under test are upstream's own, compiled elsewhere in the
** differential build and only CALLED here:
**   WW3D2/htree.cpp      HTreeClass::Alter_Avatar_HTree, Create_Morphed, both Create_Interpolated, Base_Update, Load_W3D
**                        (upstream_w3d, a copy with two MSVC-scope edits, see build.rs)
**   WW3D2/hrawanim.cpp   HRawAnimClass::Has_X/Y/Z_Translation, Has_Rotation, Has_Visibility, Load_W3D
**   WW3D2/hcanim.cpp     HCompressedAnimClass::Has_*, Load_W3D
**   WWMath/quat.cpp      Quaternion::Normalize, Make_Closest, Randomize
** and the one cut this library adds, VERBATIM (build.rs, `drafts_anim`, anchors asserted to occur once):
**   dvp_copy_indexed.cpp WWMath/vp.cpp, VectorProcessorClass::Prefetch (to `static Vector4 lastrow`) and the six
**                        CopyIndexed overloads (to `VectorProcessorClass::Clamp`).
**
** Stand-ins, each for something the files call outside themselves:
**   TestFile             GameFileClass's Read and Seek over the RAMFile (as wrap_w3d.cpp's, transcribed);
**   the hierarchy lookup the animation loaders make (WW3DAssetManager::Get_HTree) is wrap_w3d.cpp's `g_w3d_htree_hook`,
**                        set to this file's registry of loaded hierarchies (16 bytes compared, as wrap_w3d.cpp);
**   private/protected    defined as public around the upstream headers (after the standard ones), to read the pivots.
**
** Entry point (extern "C", tests/differential_draft_anim.rs):
**   ref_dah_run(in, nin, out, cap): `in` is [op, operands...]; a blob is [byte count, then the bytes, one per word];
**   floats are their bits; `out` receives the words of the op's answer; returns the count (-1: out too small).
**   op 0  reset (frees every hierarchy and animation)
**   op 1  add_tree(blob)  -> [index, or 0xFFFFFFFF when Load_W3D fails]     (a W3D_CHUNK_HIERARCHY)
**   op 2  add_raw(blob)   -> [index, or 0xFFFFFFFF]                          (a W3D_CHUNK_ANIMATION)
**   op 3  add_comp(blob)  -> [index, or 0xFFFFFFFF]                          (a W3D_CHUNK_COMPRESSED_ANIMATION)
**   op 4  base_update(tree, root: 12 floats row-major 3x4)
**   op 5  alter(tree, scale x y z)  -> [new tree index]       HTreeClass::Alter_Avatar_HTree
**   op 6  morphed(n, then n x (tree, weight)) -> [new index]  HTreeClass::Create_Morphed
**   op 7  interp4(a0b0, a0b1, a1b0, a1b1, lerp_a, lerp_b) -> [new index]   Create_Interpolated (bilinear)
**   op 8  interp3(base, a, b, a_scale, b_scale) -> [new index]             Create_Interpolated (two targets)
**   op 9  dump_tree(tree) -> [NumPivots, ScaleFactor, then per pivot: parent index (0xFFFFFFFF none), Index,
**                             BaseTransform 12 floats, Transform 12 floats]
**   op 10 has_raw(anim, pivot)  -> [X, Y, Z, Rotation, Visibility]  (0/1)
**   op 11 has_comp(anim, pivot) -> [X, Y, Z, Rotation, Visibility]
**   op 12 qnormalize(x y z w) -> [x y z w]          Quaternion::Normalize
**   op 13 qclosest(x y z w, tx ty tz tw) -> [x y z w]   Make_Closest(qto)
**   op 14 qrandomize(seed) -> [x y z w]             srand(seed); Randomize()
**   op 15 copy_indexed(type, nsrc, ndst, count, nidx, src words, dst words, index words) -> dst words
**         type 0 unsigned, 1 Vector2, 2 Vector3, 3 Vector4, 4 unsigned char (one byte per word), 5 float
**   op 16 prefetch() -> [] */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
#include <string>
#include <new>

#define private public
#define protected public
#include "chunkio.h"
#include "w3d_file.h"
#include "htree.h"
#include "hanim.h"
#include "hrawanim.h"
#include "hcanim.h"
#undef private
#undef protected
#include "quat.h"
#include "matrix3d.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "vp.h"

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}

/* wrap_w3d.cpp's lookup hook of WW3DAssetManager::Get_HTree */
extern HTreeClass *(*g_w3d_htree_hook)(const char *);

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

std::vector<HTreeClass *> g_trees;
std::vector<HRawAnimClass *> g_raw;
std::vector<HCompressedAnimClass *> g_comp;
std::vector<uint32_t> g_out;

HTreeClass *find_tree(const char *name)
{
	for (size_t i = 0; i < g_trees.size(); ++i)
		if (g_trees[i] && memcmp(g_trees[i]->Name, name, W3D_NAME_LEN) == 0) return g_trees[i];
	return NULL;
}

struct Reader {
	const uint32_t *p;
	int n;
	int i;
	uint32_t u() { return p[i++]; }
	float f() { uint32_t v = p[i++]; float x; memcpy(&x, &v, 4); return x; }
	std::vector<unsigned char> blob()
	{
		uint32_t len = u();
		std::vector<unsigned char> b(len);
		for (uint32_t k = 0; k < len; ++k) b[k] = (unsigned char)u();
		return b;
	}
	Matrix3D m()
	{
		float a[12];
		for (int k = 0; k < 12; ++k) a[k] = f();
		return Matrix3D(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]);
	}
};

void W(uint32_t v) { g_out.push_back(v); }
void WF(float f) { uint32_t v; memcpy(&v, &f, 4); g_out.push_back(v); }
void WM(const Matrix3D &m)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) WF(m[r][c]);
}

void reset_all()
{
	for (size_t i = 0; i < g_trees.size(); ++i) delete g_trees[i];
	g_trees.clear();
	for (size_t i = 0; i < g_raw.size(); ++i) g_raw[i]->Release_Ref();
	g_raw.clear();
	for (size_t i = 0; i < g_comp.size(); ++i) g_comp[i]->Release_Ref();
	g_comp.clear();
}

/* The hierarchy a loaded animation names (the stand-in of WW3DAssetManager::Get_HTree). */
HTreeClass *lookup_tree(const char *name)
{
	return find_tree(name);
}

uint32_t add_tree(const std::vector<unsigned char> &b)
{
	TestFile file((const char *)b.data(), (int)b.size());
	ChunkLoadClass cload(&file);
	if (!cload.Open_Chunk()) return 0xFFFFFFFFu;
	HTreeClass *t = new HTreeClass;
	if (t->Load_W3D(cload) != HTreeClass::OK) {
		delete t;
		return 0xFFFFFFFFu;
	}
	g_trees.push_back(t);
	return (uint32_t)(g_trees.size() - 1);
}

uint32_t add_raw(const std::vector<unsigned char> &b)
{
	TestFile file((const char *)b.data(), (int)b.size());
	ChunkLoadClass cload(&file);
	if (!cload.Open_Chunk()) return 0xFFFFFFFFu;
	HRawAnimClass *a = new HRawAnimClass;
	if (a->Load_W3D(cload) != HRawAnimClass::OK) {
		a->Release_Ref();
		return 0xFFFFFFFFu;
	}
	g_raw.push_back(a);
	return (uint32_t)(g_raw.size() - 1);
}

uint32_t add_comp(const std::vector<unsigned char> &b)
{
	TestFile file((const char *)b.data(), (int)b.size());
	ChunkLoadClass cload(&file);
	if (!cload.Open_Chunk()) return 0xFFFFFFFFu;
	HCompressedAnimClass *a = new HCompressedAnimClass;
	if (a->Load_W3D(cload) != HCompressedAnimClass::OK) {
		a->Release_Ref();
		return 0xFFFFFFFFu;
	}
	g_comp.push_back(a);
	return (uint32_t)(g_comp.size() - 1);
}

void has_all(HAnimClass *a, int pivot)
{
	W(a->Has_X_Translation(pivot) ? 1 : 0);
	W(a->Has_Y_Translation(pivot) ? 1 : 0);
	W(a->Has_Z_Translation(pivot) ? 1 : 0);
	W(a->Has_Rotation(pivot) ? 1 : 0);
	W(a->Has_Visibility(pivot) ? 1 : 0);
}

} /* namespace */

extern "C" int ref_dah_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	g_w3d_htree_hook = lookup_tree;
	g_out.clear();
	Reader rd = { in, nin, 0 };
	switch (rd.u()) {
	case 0:
		reset_all();
		break;
	case 1:
		W(add_tree(rd.blob()));
		break;
	case 2:
		W(add_raw(rd.blob()));
		break;
	case 3:
		W(add_comp(rd.blob()));
		break;
	case 4: {
		HTreeClass *t = g_trees[rd.u()];
		Matrix3D root = rd.m();
		t->Base_Update(root);
		break;
	}
	case 5: {
		HTreeClass *t = g_trees[rd.u()];
		float x = rd.f(), y = rd.f(), z = rd.f();
		Vector3 scale(x, y, z);
		g_trees.push_back(HTreeClass::Alter_Avatar_HTree(t, scale));
		W((uint32_t)(g_trees.size() - 1));
		break;
	}
	case 6: {
		int n = (int)rd.u();
		std::vector<const HTreeClass *> trees;
		std::vector<float> weights;
		for (int k = 0; k < n; ++k) {
			trees.push_back(g_trees[rd.u()]);
			weights.push_back(rd.f());
		}
		g_trees.push_back(HTreeClass::Create_Morphed(n, weights.data(), trees.data()));
		W((uint32_t)(g_trees.size() - 1));
		break;
	}
	case 7: {
		const HTreeClass *a = g_trees[rd.u()];
		const HTreeClass *b = g_trees[rd.u()];
		const HTreeClass *c = g_trees[rd.u()];
		const HTreeClass *d = g_trees[rd.u()];
		float la = rd.f(), lb = rd.f();
		g_trees.push_back(HTreeClass::Create_Interpolated(a, b, c, d, la, lb));
		W((uint32_t)(g_trees.size() - 1));
		break;
	}
	case 8: {
		const HTreeClass *base = g_trees[rd.u()];
		const HTreeClass *a = g_trees[rd.u()];
		const HTreeClass *b = g_trees[rd.u()];
		float sa = rd.f(), sb = rd.f();
		g_trees.push_back(HTreeClass::Create_Interpolated(base, a, b, sa, sb));
		W((uint32_t)(g_trees.size() - 1));
		break;
	}
	case 9: {
		HTreeClass *t = g_trees[rd.u()];
		W((uint32_t)t->NumPivots);
		WF(t->ScaleFactor);
		for (int i = 0; i < t->NumPivots; ++i) {
			const PivotClass &p = t->Pivot[i];
			W(p.Parent ? (uint32_t)(p.Parent - t->Pivot) : 0xFFFFFFFFu);
			W((uint32_t)p.Index);
			WM(p.BaseTransform);
			WM(p.Transform);
		}
		break;
	}
	case 10: {
		HRawAnimClass *a = g_raw[rd.u()];
		has_all(a, (int)rd.u());
		break;
	}
	case 11: {
		HCompressedAnimClass *a = g_comp[rd.u()];
		has_all(a, (int)rd.u());
		break;
	}
	case 12: {
		float x = rd.f(), y = rd.f(), z = rd.f(), w = rd.f();
		Quaternion q(x, y, z, w);
		q.Normalize();
		WF(q.X); WF(q.Y); WF(q.Z); WF(q.W);
		break;
	}
	case 13: {
		float x = rd.f(), y = rd.f(), z = rd.f(), w = rd.f();
		float tx = rd.f(), ty = rd.f(), tz = rd.f(), tw = rd.f();
		Quaternion q(x, y, z, w);
		Quaternion to(tx, ty, tz, tw);
		q.Make_Closest(to);
		WF(q.X); WF(q.Y); WF(q.Z); WF(q.W);
		break;
	}
	case 14: {
		uint32_t seed = rd.u();
		srand(seed);
		Quaternion q(0.0f, 0.0f, 0.0f, 0.0f);
		q.Randomize();
		WF(q.X); WF(q.Y); WF(q.Z); WF(q.W);
		break;
	}
	case 15: {
		int type = (int)rd.u();
		int nsrc = (int)rd.u(), ndst = (int)rd.u(), count = (int)rd.u(), nidx = (int)rd.u();
		static const int WIDTH[6] = { 1, 2, 3, 4, 1, 1 };
		int w = WIDTH[type];
		std::vector<uint32_t> src(nsrc * w), dst(ndst * w), idx(nidx);
		for (size_t k = 0; k < src.size(); ++k) src[k] = rd.u();
		for (size_t k = 0; k < dst.size(); ++k) dst[k] = rd.u();
		for (size_t k = 0; k < idx.size(); ++k) idx[k] = rd.u();
		switch (type) {
		case 0:
			VectorProcessorClass::CopyIndexed((unsigned *)dst.data(), (const unsigned *)src.data(), idx.data(), count);
			break;
		case 1:
			VectorProcessorClass::CopyIndexed((Vector2 *)dst.data(), (const Vector2 *)src.data(), idx.data(), count);
			break;
		case 2:
			VectorProcessorClass::CopyIndexed((Vector3 *)dst.data(), (const Vector3 *)src.data(), idx.data(), count);
			break;
		case 3:
			VectorProcessorClass::CopyIndexed((Vector4 *)dst.data(), (const Vector4 *)src.data(), idx.data(), count);
			break;
		case 4: {
			std::vector<unsigned char> bs(src.size()), bd(dst.size());
			for (size_t k = 0; k < bs.size(); ++k) bs[k] = (unsigned char)src[k];
			for (size_t k = 0; k < bd.size(); ++k) bd[k] = (unsigned char)dst[k];
			VectorProcessorClass::CopyIndexed(bd.data(), bs.data(), idx.data(), count);
			for (size_t k = 0; k < bd.size(); ++k) dst[k] = bd[k];
			break;
		}
		default:
			VectorProcessorClass::CopyIndexed((float *)dst.data(), (float *)src.data(), idx.data(), count);
			break;
		}
		for (size_t k = 0; k < dst.size(); ++k) W(dst[k]);
		break;
	}
	case 16:
		VectorProcessorClass::Prefetch(NULL);
		break;
	default:
		return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
