/* Differential-test wrapper for src/drafts/sweep_ww3d/part_buf.rs (GitHub #215, group sweepww3d; upstream commit 0a05454d).
** VERBATIM cuts by build.rs (`drafts_sweepww3d`), from WW3D2/part_buf.cpp: ParticleBufferClass::Get_Color_Key_Frames,
** Get_Opacity_Key_Frames, Get_Size_Key_Frames, Get_Rotation_Key_Frames, Get_Frame_Key_Frames, Get_Blur_Time_Key_Frames,
** Get_Line_Texture_Mapping_Mode, Get_Subdivision_Level.
** Stand-ins: ParticleBufferClass has the fields of part_buf.h these read (same types: unsigned int counts, times and MaxAge,
** pointers for the arrays) and the eight members; ParticlePropertyStruct<T> is part_emt.h's (copied); SegLineRendererClass has
** the enum value UNIFORM_WIDTH_TEXTURE_MAP = 0 (seglinerenderer.h:75) and the two getters, returning stored values.
** Entry point: ref_dsww3d_pb_run(in, nin, out, cap): in = [kind 0 colour 1 opacity 2 size 3 rotation 4 frame 5 blur, n, has_deltas,
**   has_values, max_age, random (3 words for colour, else 1), times[n], values (3n or n), deltas (3n or n)]; out = Start, Rand,
**   NumKeyFrames, KeyTimes, Values.  kind 6: [6, has_renderer, mode, level] -> mode, level (the two line-renderer queries). */
#include <stdint.h>
#include <string.h>
#include <vector>
#include "always.h"
#include "wwdebug.h"
#include "vector3.h"


namespace {

static float f_of(uint32_t w) { float f; memcpy(&f, &w, 4); return f; }
static uint32_t w_of(float f) { uint32_t w; memcpy(&w, &f, 4); return w; }

template<class T> struct ParticlePropertyStruct {
	T					Start;
	T					Rand;
	unsigned int	NumKeyFrames;
	float *			KeyTimes;
	T *				Values;
};

class SegLineRendererClass
{
public:
	enum TextureMapMode { UNIFORM_WIDTH_TEXTURE_MAP = 0x00000000 };
	int mode; unsigned int level;
	TextureMapMode Get_Texture_Mapping_Mode(void) const { return (TextureMapMode)mode; }
	unsigned int Get_Current_Subdivision_Level(void) const { return level; }
};

class ParticleBufferClass
{
public:
	void Get_Color_Key_Frames (ParticlePropertyStruct<Vector3> &colors) const;
	void Get_Opacity_Key_Frames (ParticlePropertyStruct<float> &opacities) const;
	void Get_Size_Key_Frames (ParticlePropertyStruct<float> &sizes) const;
	void Get_Rotation_Key_Frames (ParticlePropertyStruct<float> &rotations) const;
	void Get_Frame_Key_Frames (ParticlePropertyStruct<float> &frames) const;
	void Get_Blur_Time_Key_Frames (ParticlePropertyStruct<float> &blurtimes) const;
	int Get_Line_Texture_Mapping_Mode(void) const;
	int Get_Subdivision_Level(void) const;

	unsigned int	MaxAge;
	unsigned int	NumColorKeyFrames;
	unsigned int *	ColorKeyFrameTimes;
	Vector3 *		ColorKeyFrameValues;
	Vector3 *		ColorKeyFrameDeltas;
	unsigned int	NumAlphaKeyFrames;
	unsigned int *	AlphaKeyFrameTimes;
	float *			AlphaKeyFrameValues;
	float *			AlphaKeyFrameDeltas;
	unsigned int	NumSizeKeyFrames;
	unsigned int *	SizeKeyFrameTimes;
	float *			SizeKeyFrameValues;
	float *			SizeKeyFrameDeltas;
	unsigned int	NumRotationKeyFrames;
	unsigned int *	RotationKeyFrameTimes;
	float *			RotationKeyFrameValues;
	float *			HalfRotationKeyFrameDeltas;
	unsigned int	NumFrameKeyFrames;
	unsigned int *	FrameKeyFrameTimes;
	float *			FrameKeyFrameValues;
	float *			FrameKeyFrameDeltas;
	unsigned int	NumBlurTimeKeyFrames;
	unsigned int *	BlurTimeKeyFrameTimes;
	float *			BlurTimeKeyFrameValues;
	float *			BlurTimeKeyFrameDeltas;
	Vector3			ColorRandom;
	float				OpacityRandom;
	float				SizeRandom;
	float				RotationRandom;
	float				FrameRandom;
	float				BlurTimeRandom;
	SegLineRendererClass * LineRenderer;
};

#include "pb_color.inc"
#include "pb_opac.inc"
#include "pb_size.inc"
#include "pb_rot.inc"
#include "pb_frame.inc"
#include "pb_blur.inc"
#include "pb_lmode.inc"
#include "pb_sub.inc"

template<class T> static void dump(std::vector<uint32_t> & o, ParticlePropertyStruct<T> & s);
static void put(std::vector<uint32_t> & o, float v) { o.push_back(w_of(v)); }
static void put(std::vector<uint32_t> & o, const Vector3 & v) { o.push_back(w_of(v.X)); o.push_back(w_of(v.Y)); o.push_back(w_of(v.Z)); }
template<class T> static void dump(std::vector<uint32_t> & o, ParticlePropertyStruct<T> & s)
{
	put(o, s.Start); put(o, s.Rand); o.push_back(s.NumKeyFrames);
	for (unsigned int i = 0; i < s.NumKeyFrames; i++) put(o, s.KeyTimes[i]);
	for (unsigned int i = 0; i < s.NumKeyFrames; i++) put(o, s.Values[i]);
	delete[] s.KeyTimes; delete[] s.Values;
}

}

extern "C" int ref_dsww3d_pb_run(const uint32_t * in, int nin, uint32_t * out, int cap)
{
	std::vector<uint32_t> o;
	const uint32_t * p = in;
	uint32_t kind = *p++;
	ParticleBufferClass b;
	memset(&b, 0, sizeof(b));
	if (kind == 6) {
		SegLineRendererClass lr;
		uint32_t has = *p++; lr.mode = (int)*p++; lr.level = *p++;
		b.LineRenderer = has ? &lr : NULL;
		o.push_back((uint32_t)b.Get_Line_Texture_Mapping_Mode());
		o.push_back((uint32_t)b.Get_Subdivision_Level());
	} else {
		unsigned int n = *p++;
		bool has_deltas = *p++ != 0;
		bool has_values = *p++ != 0;
		b.MaxAge = *p++;
		float rnd[3];
		int dim = (kind == 0) ? 3 : 1;
		for (int i = 0; i < dim; i++) rnd[i] = f_of(*p++);
		std::vector<unsigned int> times(n ? n : 1);
		for (unsigned int i = 0; i < n; i++) times[i] = *p++;
		std::vector<float> vals((size_t)dim * (n ? n : 1)), dels((size_t)dim * (n ? n : 1));
		for (unsigned int i = 0; i < n * dim; i++) vals[i] = f_of(*p++);
		for (unsigned int i = 0; i < n * dim; i++) dels[i] = f_of(*p++);
		unsigned int * T = &times[0];
		float * V = has_values ? &vals[0] : NULL;
		float * D = has_deltas ? &dels[0] : NULL;
		switch (kind) {
		case 0: {
			b.NumColorKeyFrames = n; b.ColorKeyFrameTimes = T;
			b.ColorKeyFrameValues = (Vector3 *)V; b.ColorKeyFrameDeltas = (Vector3 *)D;
			b.ColorRandom = Vector3(rnd[0], rnd[1], rnd[2]);
			ParticlePropertyStruct<Vector3> s; b.Get_Color_Key_Frames(s); dump(o, s); break; }
		case 1: {
			b.NumAlphaKeyFrames = n; b.AlphaKeyFrameTimes = T; b.AlphaKeyFrameValues = V; b.AlphaKeyFrameDeltas = D; b.OpacityRandom = rnd[0];
			ParticlePropertyStruct<float> s; b.Get_Opacity_Key_Frames(s); dump(o, s); break; }
		case 2: {
			b.NumSizeKeyFrames = n; b.SizeKeyFrameTimes = T; b.SizeKeyFrameValues = V; b.SizeKeyFrameDeltas = D; b.SizeRandom = rnd[0];
			ParticlePropertyStruct<float> s; b.Get_Size_Key_Frames(s); dump(o, s); break; }
		case 3: {
			b.NumRotationKeyFrames = n; b.RotationKeyFrameTimes = T; b.RotationKeyFrameValues = V; b.HalfRotationKeyFrameDeltas = D; b.RotationRandom = rnd[0];
			ParticlePropertyStruct<float> s; b.Get_Rotation_Key_Frames(s); dump(o, s); break; }
		case 4: {
			b.NumFrameKeyFrames = n; b.FrameKeyFrameTimes = T; b.FrameKeyFrameValues = V; b.FrameKeyFrameDeltas = D; b.FrameRandom = rnd[0];
			ParticlePropertyStruct<float> s; b.Get_Frame_Key_Frames(s); dump(o, s); break; }
		case 5: {
			b.NumBlurTimeKeyFrames = n; b.BlurTimeKeyFrameTimes = T; b.BlurTimeKeyFrameValues = V; b.BlurTimeKeyFrameDeltas = D; b.BlurTimeRandom = rnd[0];
			ParticlePropertyStruct<float> s; b.Get_Blur_Time_Key_Frames(s); dump(o, s); break; }
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); i++) out[i] = o[i];
	return (int)o.size();
}
