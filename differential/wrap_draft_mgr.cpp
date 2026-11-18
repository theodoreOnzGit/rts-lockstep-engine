/* Differential-test wrapper for the animation and hierarchy managers' exclusion-list drafts (GitHub #215, upstream
** commit 0a05454d), group "anim", part 3: src/drafts/sweep_ww3d/{hanimmgr,htreemgr}.rs.
**
** Compiled WHOLE from upstream, with the classes renamed on the command line (build.rs, `drafts_anim`) so that this
** library's copies do not clash with the manager already linked for the asset manager tests, whose W3DExclusionListClass
** answers `abort()`: HTreeManagerClass -> DraftHTreeManagerClass, HAnimManagerClass -> DraftHAnimManagerClass,
** HAnimManagerIterator -> DraftHAnimManagerIterator, W3DExclusionListClass -> DraftW3DExclusionListClass, and
** StringClass -> ShimStringClass (the transcription of wrap_w3d_string.cpp). Files: WW3D2/htreemgr.cpp (a copy with the
** MSVC for-scope edit and the `_strlwr` line the asset manager build makes), hanimmgr.cpp (a copy with the `strchr`
** cast), w3dexclusionlist.cpp (unchanged). Stand-ins: DAnim, an HAnimClass that has a name; `_strlwr` is
** wrap_w3d_assetmgr.cpp's `ref_am_strlwr`; trees are loaded by the real Load_Tree from hierarchy chunks.
** The managers are never destroyed: Free_All_Trees_With_Exclusion_List leaves a moved tree in its old slot too, and the
** destructor would delete it twice (an upstream defect this test measures, see the Rust test).
**
** Entry point (extern "C", tests/differential_draft_mgr.rs): ref_dmg_run(in, nin, out, cap); strings are [length, chars]
**   op 0 new()                       fresh managers
**   op 1 add_tree(blob)              -> [Load_Tree result]
**   op 2 free_trees(n, names...)     Free_All_Trees_With_Exclusion_List -> [NumTrees, then for slots 0..32: 0 or length+1,
**                                    chars]
**   op 3 tree_hash(n, names...)      -> n words: TreeHash.Exists(name)
**   op 4 tree_name(idx)              -> [0 for NULL, else length+1, chars]
**   op 5 add_anim(name, extra_ref)   Add_Anim; extra_ref=0 leaves the table the only holder (Num_Refs 1), 1 keeps one more
**   op 6 free_anims(n, names...)     Free_All_Anims_With_Exclusion_List -> per added animation (in add order): 1 if still
**                                    in the table (Peek_Anim)
**   op 7 asset_list()                Create_Asset_List -> [count, then each string as length, chars] (table order) */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
#include <string>
#include "dvector_this.h"
#include "dsimplevec_this.h"
#include "hashtemplate.h"
#include "wwstring.h"
#include "hash.h"
#include "htree.h"
#include "hanim.h"
#include "hrawanim.h"
#include "hcanim.h"
#include "hmorphanim.h"
#include "animatedsoundmgr.h"
#define private public
#define protected public
#include "htreemgr.h"
#include "hanimmgr.h"
#undef private
#undef protected
#include "htree.h"
#include "hanim.h"
#include "chunkio.h"
#include "w3dexclusionlist.h"

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

class DAnim : public HAnimClass
{
public:
	std::string name;
	virtual const char *Get_Name(void) const { return name.c_str(); }
	virtual const char *Get_HName(void) const { return ""; }
	virtual int Get_Num_Frames(void) { return 0; }
	virtual float Get_Frame_Rate() { return 0.0f; }
	virtual float Get_Total_Time() { return 0.0f; }
	virtual void Get_Translation(Vector3 &, int, float) const {}
	virtual void Get_Orientation(Quaternion &, int, float) const {}
	virtual void Get_Transform(Matrix3D &, int, float) const {}
	virtual bool Get_Visibility(int, float) { return true; }
	virtual int Get_Num_Pivots(void) const { return 0; }
	virtual bool Is_Node_Motion_Present(int) { return true; }
};

HTreeManagerClass *g_tm = NULL;
HAnimManagerClass *g_am = NULL;
std::vector<std::string> g_anims;
std::vector<uint32_t> g_out;
void W(uint32_t v) { g_out.push_back(v); }

struct Reader {
	const uint32_t *p;
	int i;
	uint32_t u() { return p[i++]; }
	std::string str()
	{
		uint32_t n = u();
		std::string s;
		for (uint32_t k = 0; k < n; ++k) s.push_back((char)u());
		return s;
	}
};

void W_str(const char *s)
{
	W((uint32_t)strlen(s));
	for (; *s; ++s) W((unsigned char)*s);
}

void names_into(Reader &rd, DynamicVectorClass<StringClass> &v)
{
	uint32_t n = rd.u();
	for (uint32_t k = 0; k < n; ++k) v.Add(StringClass(rd.str().c_str()));
}

}

extern "C" int ref_dmg_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	g_out.clear();
	Reader rd = { in, 0 };
	switch (rd.u()) {
	case 0:
		g_tm = new HTreeManagerClass;
		g_am = new HAnimManagerClass;
		g_anims.clear();
		break;
	case 1: {
		uint32_t len = rd.u();
		std::vector<char> b(len);
		for (uint32_t k = 0; k < len; ++k) b[k] = (char)rd.u();
		TestFile file(b.data(), (int)len);
		ChunkLoadClass cload(&file);
		if (!cload.Open_Chunk()) { W(99); break; }
		W((uint32_t)g_tm->Load_Tree(cload));
		break;
	}
	case 2: {
		DynamicVectorClass<StringClass> names;
		names_into(rd, names);
		W3DExclusionListClass ex(names);
		g_tm->Free_All_Trees_With_Exclusion_List(ex);
		W((uint32_t)g_tm->NumTrees);
		for (int s = 0; s < 32; ++s) {
			if (g_tm->TreePtr[s]) {
				W((uint32_t)strlen(g_tm->TreePtr[s]->Get_Name()) + 1);
				for (const char *q = g_tm->TreePtr[s]->Get_Name(); *q; ++q) W((unsigned char)*q);
			} else {
				W(0);
			}
		}
		break;
	}
	case 3: {
		uint32_t n = rd.u();
		for (uint32_t k = 0; k < n; ++k) W(g_tm->TreeHash.Exists(StringClass(rd.str().c_str())) ? 1 : 0);
		break;
	}
	case 4: {
		char *n = g_tm->Get_Tree_Name((int)rd.u());
		if (!n) W(0);
		else {
			W((uint32_t)strlen(n) + 1);
			for (const char *q = n; *q; ++q) W((unsigned char)*q);
		}
		break;
	}
	case 5: {
		DAnim *a = new DAnim;
		a->name = rd.str();
		int extra = (int)rd.u();
		g_am->Add_Anim(a);
		if (!extra) a->Release_Ref();
		g_anims.push_back(a->name.empty() ? std::string() : std::string(a->Get_Name()));
		break;
	}
	case 6: {
		DynamicVectorClass<StringClass> names;
		names_into(rd, names);
		W3DExclusionListClass ex(names);
		g_am->Free_All_Anims_With_Exclusion_List(ex);
		for (size_t k = 0; k < g_anims.size(); ++k) W(g_am->Peek_Anim(g_anims[k].c_str()) ? 1 : 0);
		break;
	}
	case 7: {
		DynamicVectorClass<StringClass> list;
		g_am->Create_Asset_List(list);
		W((uint32_t)list.Count());
		for (int k = 0; k < list.Count(); ++k) W_str((const char *)list[k]);
		break;
	}
	default:
		return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
