/* Differential-test wrapper for Animatable3DObjClass::Simple_Evaluate_Bone (both overloads) and Set_HTree (GitHub #215,
** upstream commit 0a05454d), group "anim", part 4: src/drafts/sweep_ww3d/animobj.rs.
**
** VERBATIM cuts by build.rs (`drafts_anim`, anchors asserted to occur once) into OUT_DIR/draft_anim_gen:
**   dao_simple.inc     animobj.cpp, both Simple_Evaluate_Bone (to Compute_Current_Frame's comment)
**   dao_frame.inc      animobj.cpp, Compute_Current_Frame (to Single_Anim_Progress's definition)
**   dao_sethtree.inc   animobj.cpp, Set_HTree (to the end of the file)
**   dao_members.inc    animobj.h:168-216 (IsTreeValid, HTree, the motion modes, CurMotionMode, the ModeAnim union)
**   dao_anim_mode.inc  rendobj.h:331-339 (RenderObjClass::AnimMode)
** Stand-ins: the class around them (Transform and Get_Transform as RenderObjClass's; Update_Sub_Object_Transforms,
** which the lazy hierarchy update stands for: it counts the call and runs HTree->Base_Update(Transform) when there is a
** hierarchy, so that a later Get_Transform reads something); WW3D::Get_Sync_Time (a fixed 0: the cases use ANIM_MODE_MANUAL,
** so Compute_Current_Frame returns ModeAnim.Frame in SINGLE_ANIM and 0 otherwise). The hierarchy and the single
** animation are real, loaded by the real loaders from chunks (the hook of wrap_w3d.cpp).
**
** Entry point (extern "C", tests/differential_draft_animobj.rs): ref_dao_run(in, nin, out, cap)
**   op 0 reset   op 1 add_tree(blob) -> [index]   op 2 add_raw(blob) -> [index]
**   op 3 setup(tree or 0xFFFFFFFF, mode, anim or 0xFFFFFFFF, frame, transform 12 floats): the object holds a COPY of the tree
**   op 4 simple_bone(bone, tm 12 floats in) -> [return, tm 12, Update_Sub_Object_Transforms calls]
**   op 5 simple_bone_frame(bone, frame, tm 12 floats in) -> [return, tm 12, calls]
**   op 6 set_htree(tree) -> [HTree != NULL, NumPivots, per pivot BaseTransform 12, Transform 12] */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
#define private public
#define protected public
#include "chunkio.h"
#include "w3d_file.h"
#include "htree.h"
#include "hanim.h"
#include "hrawanim.h"
#undef private
#undef protected
#include "matrix3d.h"

extern "C" {
void *ref_w3d_ramfile_open(const char *data, int len);
int ref_w3d_ramfile_read(void *f, void *buffer, int bytes);
int ref_w3d_ramfile_seek(void *f, int pos, int mode);
int ref_w3d_ramfile_size(void *f);
void ref_w3d_ramfile_close(void *f);
}
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

class WW3D
{
public:
	static unsigned int Get_Sync_Time(void) { return 0; }
};

class Animatable3DObjClass
{
public:
#include "dao_anim_mode.inc"
	Matrix3D Transform;
	int UpdateCalls;
	Animatable3DObjClass() : Transform(true), UpdateCalls(0), IsTreeValid(false), HTree(NULL), CurMotionMode(0) { memset(&ModeAnim, 0, sizeof(ModeAnim)); }
	const Matrix3D &Get_Transform(void) const { return Transform; }
	void Update_Sub_Object_Transforms(void)
	{
		++UpdateCalls;
		if (HTree) HTree->Base_Update(Transform);
	}
	bool Simple_Evaluate_Bone(int boneindex, Matrix3D *tm) const;
	bool Simple_Evaluate_Bone(int boneindex, float frame, Matrix3D *tm) const;
	float Compute_Current_Frame(float *newDirection = NULL) const;
	void Set_HTree(HTreeClass *new_htree);
#include "dao_members.inc"
};

#include "dao_simple.inc"
#include "dao_frame.inc"
#include "dao_sethtree.inc"

std::vector<HTreeClass *> g_trees;
std::vector<HRawAnimClass *> g_raw;
Animatable3DObjClass *g_obj = NULL;
std::vector<uint32_t> g_out;
void W(uint32_t v) { g_out.push_back(v); }
void WF(float f) { uint32_t v; memcpy(&v, &f, 4); g_out.push_back(v); }
void WM(const Matrix3D &m)
{
	for (int r = 0; r < 3; ++r)
		for (int c = 0; c < 4; ++c) WF(m[r][c]);
}

HTreeClass *lookup_tree(const char *name)
{
	for (size_t i = 0; i < g_trees.size(); ++i)
		if (memcmp(g_trees[i]->Name, name, W3D_NAME_LEN) == 0) return g_trees[i];
	return NULL;
}

struct Reader {
	const uint32_t *p;
	int i;
	uint32_t u() { return p[i++]; }
	float f() { uint32_t v = p[i++]; float x; memcpy(&x, &v, 4); return x; }
	Matrix3D m()
	{
		float a[12];
		for (int k = 0; k < 12; ++k) a[k] = f();
		return Matrix3D(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9], a[10], a[11]);
	}
};

void dump_tree(HTreeClass *t)
{
	W(t != NULL);
	if (!t) return;
	W((uint32_t)t->NumPivots);
	for (int i = 0; i < t->NumPivots; ++i) {
		WM(t->Pivot[i].BaseTransform);
		WM(t->Pivot[i].Transform);
	}
}

}

extern "C" int ref_dao_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	g_w3d_htree_hook = lookup_tree;
	g_out.clear();
	Reader rd = { in, 0 };
	switch (rd.u()) {
	case 0:
		for (size_t i = 0; i < g_trees.size(); ++i) delete g_trees[i];
		g_trees.clear();
		for (size_t i = 0; i < g_raw.size(); ++i) g_raw[i]->Release_Ref();
		g_raw.clear();
		delete g_obj;
		g_obj = NULL;
		break;
	case 1:
	case 2: {
		int kind = (int)in[0];
		rd.i = 1;
		uint32_t len = rd.u();
		std::vector<char> b(len);
		for (uint32_t k = 0; k < len; ++k) b[k] = (char)rd.u();
		TestFile file(b.data(), (int)len);
		ChunkLoadClass cload(&file);
		if (!cload.Open_Chunk()) abort();
		if (kind == 1) {
			HTreeClass *t = new HTreeClass;
			if (t->Load_W3D(cload) != HTreeClass::OK) abort();
			g_trees.push_back(t);
			W((uint32_t)(g_trees.size() - 1));
		} else {
			HRawAnimClass *a = new HRawAnimClass;
			if (a->Load_W3D(cload) != HRawAnimClass::OK) abort();
			g_raw.push_back(a);
			W((uint32_t)(g_raw.size() - 1));
		}
		break;
	}
	case 3: {
		delete g_obj;
		g_obj = new Animatable3DObjClass;
		uint32_t t = rd.u();
		if (t != 0xFFFFFFFFu) g_obj->HTree = new HTreeClass(*g_trees[t]);
		g_obj->CurMotionMode = (int)rd.u();
		uint32_t a = rd.u();
		g_obj->ModeAnim.Motion = (a == 0xFFFFFFFFu) ? NULL : g_raw[a];
		g_obj->ModeAnim.Frame = rd.f();
		g_obj->ModeAnim.AnimMode = Animatable3DObjClass::ANIM_MODE_MANUAL;
		g_obj->ModeAnim.animDirection = 1.0f;
		g_obj->ModeAnim.frameRateMultiplier = 1.0f;
		g_obj->Transform = rd.m();
		break;
	}
	case 4:
	case 5: {
		bool with_frame = in[0] == 5;
		rd.i = 1;
		int bone = (int)rd.u();
		float frame = with_frame ? rd.f() : 0.0f;
		Matrix3D tm = rd.m();
		bool r = with_frame ? g_obj->Simple_Evaluate_Bone(bone, frame, &tm) : g_obj->Simple_Evaluate_Bone(bone, &tm);
		W(r ? 1 : 0);
		WM(tm);
		W((uint32_t)g_obj->UpdateCalls);
		break;
	}
	case 6:
		g_obj->Set_HTree(g_trees[rd.u()]);
		dump_tree(g_obj->HTree);
		break;
	default:
		return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
