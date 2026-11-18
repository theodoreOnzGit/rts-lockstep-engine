/* Differential-test wrapper for the morph animation authoring drafts (GitHub #215, upstream commit 0a05454d), group
** "anim", part 2: src/drafts/sweep_ww3d/hmorphanim.rs.
**
** VERBATIM cut by build.rs (`drafts_anim`, anchors asserted to occur once) into OUT_DIR/draft_anim_gen/dmorph_def.cpp,
** from WW3D2/hmorphanim.cpp: TimeCodedMorphKeysClass::Add_Key (to Get_Morph_Info), HMorphAnimClass::Resolve_Pivot_Channels
** (to Set_Name), Free_Morph and Create_New_Morph (to Load_W3D), Insert_Morph_Key and Release_Keys (to the end). The
** constructor, destructor, Free and the key class's other members are the ones already linked (upstream_w3d_morph).
** Stand-in: DPose, an HAnimClass (the pose animation a channel holds) with the name, frame rate, pivot count and
** per-pivot Is_Node_Motion_Present the case gives, and nothing else (the morph reads only those four of its poses).
** private/protected are public around hmorphanim.h to read the morph's arrays.
**
** Entry point (extern "C", tests/differential_draft_morph.rs): ref_dmo_run(in, nin, out, cap)
**   op 0 new()                         a fresh HMorphAnimClass (the old one released)
**   op 1 create(channels, null_anim, then per channel [rate bits, npivots, name length, name chars..., then npivots
**        words: node motion present]) -> [return value]; with null_anim=1 no pose data follows and anim is NULL.
**        Each pose is given to the morph with the reference it owns (Free releases it).
**   op 2 free_morph()
**   op 3 insert(channel, morph_frame, pose_frame)
**   op 4 release_keys()
**   op 5 dump() -> [ChannelCount, FrameCount, FrameRate, NumNodes, PoseData!=NULL, MorphKeyData!=NULL, PivotChannel!=NULL,
**        then NumNodes pivot channels if present, then per channel if PoseData: name length, chars; if MorphKeyData:
**        key count, (MorphFrame, PoseFrame)*, CachedIdx] */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <vector>
#include <string>
#include "dsimplevec_this.h"
#include "hanim.h"
#define private public
#define protected public
#include "hmorphanim.h"
#undef private
#undef protected
#include "w3d_file.h"

namespace {

class DPose : public HAnimClass
{
public:
	std::string name;
	float rate;
	int pivots;
	std::vector<int> present;
	virtual const char *Get_Name(void) const { return name.c_str(); }
	virtual const char *Get_HName(void) const { return ""; }
	virtual int Get_Num_Frames(void) { return 0; }
	virtual float Get_Frame_Rate() { return rate; }
	virtual float Get_Total_Time() { return 0.0f; }
	virtual void Get_Translation(Vector3 &, int, float) const {}
	virtual void Get_Orientation(Quaternion &, int, float) const {}
	virtual void Get_Transform(Matrix3D &, int, float) const {}
	virtual bool Get_Visibility(int, float) { return true; }
	virtual int Get_Num_Pivots(void) const { return pivots; }
	virtual bool Is_Node_Motion_Present(int p) { return p >= 0 && p < (int)present.size() && present[p] != 0; }
};

HMorphAnimClass *g_morph = NULL;
std::vector<uint32_t> g_out;
void W(uint32_t v) { g_out.push_back(v); }

}

extern "C" int ref_dmo_run(const uint32_t *in, int nin, uint32_t *out, int cap)
{
	g_out.clear();
	int i = 0;
	switch (in[i++]) {
	case 0:
		if (g_morph) g_morph->Release_Ref();
		g_morph = new HMorphAnimClass;
		break;
	case 1: {
		int channels = (int)in[i++];
		int null_anim = (int)in[i++];
		if (null_anim) {
			W((uint32_t)g_morph->Create_New_Morph(channels, NULL));
			break;
		}
		std::vector<HAnimClass *> poses;
		for (int c = 0; c < channels; ++c) {
			DPose *p = new DPose;
			uint32_t r = in[i++];
			memcpy(&p->rate, &r, 4);
			p->pivots = (int)in[i++];
			uint32_t nl = in[i++];
			for (uint32_t k = 0; k < nl; ++k) p->name.push_back((char)in[i++]);
			for (int k = 0; k < p->pivots; ++k) p->present.push_back((int)in[i++]);
			poses.push_back(p);
		}
		W((uint32_t)g_morph->Create_New_Morph(channels, poses.data()));
		break;
	}
	case 2:
		g_morph->Free_Morph();
		break;
	case 3: {
		int ch = (int)in[i++];
		uint32_t mf = in[i++], pf = in[i++];
		g_morph->Insert_Morph_Key(ch, mf, pf);
		break;
	}
	case 4:
		g_morph->Release_Keys();
		break;
	case 5: {
		HMorphAnimClass *m = g_morph;
		W((uint32_t)m->ChannelCount);
		W((uint32_t)m->FrameCount);
		uint32_t r; memcpy(&r, &m->FrameRate, 4); W(r);
		W((uint32_t)m->NumNodes);
		W(m->PoseData != NULL); W(m->MorphKeyData != NULL); W(m->PivotChannel != NULL);
		if (m->PivotChannel) for (int k = 0; k < m->NumNodes; ++k) W(m->PivotChannel[k]);
		for (int c = 0; c < m->ChannelCount; ++c) {
			if (m->PoseData) {
				const char *n = m->PoseData[c]->Get_Name();
				W((uint32_t)strlen(n));
				for (const char *q = n; *q; ++q) W((unsigned char)*q);
			}
			if (m->MorphKeyData) {
				TimeCodedMorphKeysClass &k = m->MorphKeyData[c];
				W((uint32_t)k.Keys.Count());
				for (int j = 0; j < k.Keys.Count(); ++j) { W(k.Keys[j].MorphFrame); W(k.Keys[j].PoseFrame); }
				W(k.CachedIdx);
			}
		}
		break;
	}
	default:
		return -1;
	}
	if ((int)g_out.size() > cap) return -1;
	for (size_t k = 0; k < g_out.size(); ++k) out[k] = g_out[k];
	return (int)g_out.size();
}
