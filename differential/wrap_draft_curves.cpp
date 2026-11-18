/* Differential-test wrapper for the curve and spline drafts (#215, upstream commit 0a05454d), src/drafts/wwlib/curve.rs,
** hermite_spline.rs, cardinal_spline.rs, catmull_rom_spline.rs and tcb_spline.rs:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/curve.cpp (Curve3DClass, LinearCurve3DClass, Curve1DClass,
**   LinearCurve1DClass), hermitespline.cpp, cardinalspline.cpp, catmullromspline.cpp, tcbspline.cpp, all #included UNCHANGED, with
**   their headers (curve.h ... tcbspline.h, vector3.h, WWLib/Vector.H, WWSaveLoad/persist.h) and the MSVC-compatible build of
**   the other wrappers: clang++ with -fms-extensions -fdelayed-template-parsing.
** Stand-ins (build.rs's drafts_codec lists them): shim_draft_codec/persistfactory.h (SimplePersistFactoryClass and the chunk
** classes as empty inline classes: Save and Load are compiled but NOT exercised here); shim_draft_curves/always.h (the real
** always.h with _OPERATOR_NEW_DEFINED_, so the C++ runtime's operator new serves); a copy of WWLib/Vector.H with five members of
** the dependent base named through `this->` (the same mechanical edit, each count asserted, the other W3D wrappers use; build.rs);
** lower-case forwarders for MSVC's case-insensitive file names. Protected members (Keys, Tangents, Tightness, Params) are read by
** derived classes (Dump3, Dump1), not by a cast.
**
** LookupTableClass (src/drafts/wwlib/lookup_table.rs) is in this file because it needs the curve classes (a second copy of them in
** another library would define the same symbols twice). Its code is VERBATIM, cut by build.rs into OUT_DIR/draft_codec_gen from
** WWMath/lookuptable.cpp (LookupTableClass::LookupTableClass, ~LookupTableClass and Init, to LookupTableMgrClass::Init) and
** lookuptable.h (the inline Get_Value and Get_Value_Quick, to the comment of LookupTableMgrClass); lookuptable.h itself is not
** included, because its RefCountClass, MultiListObjectClass, StringClass and SimpleVecClass bases drag in the memory, string and
** list systems. The stand-in is the class declaration below: the same members (Name, MinInputValue, MaxInputValue,
** OOMaxMinusMin, OutputSamples), public, with Name a minimal string and OutputSamples a minimal array with Length() and
** operator[] as SimpleVecClass has them. LookupTableMgrClass (a registry over the file system and RefMultiListClass) is not cut;
** its Init is replayed in ref_lookup_default (the same calls on the same classes).
**
** Entry point (extern "C", tests/differential_draft_curves.rs):
**   ref_curve_seq(kind, in, nin, out, cap) runs one object of class `kind` through the operations in `in` (8 words each:
**   [op, i, w0 .. w5], floats as bits) and writes, for every operation, [op, its results...] then the object's state: the keys
**   (count, then per key point, time and for 1D the extra), is-looping, and for the Hermite family the tangents and the
**   tangents-dirty flag, for Cardinal the tightnesses, for TCB the parameters. Returns the number of words written.
**   kind: 0 LinearCurve3D, 1 HermiteSpline3D, 2 CardinalSpline3D, 3 CatmullRomSpline3D, 4 TCBSpline3D,
**         5 LinearCurve1D, 6 HermiteSpline1D, 7 CardinalSpline1D, 8 CatmullRomSpline1D
**   op: 0 Add_Key(point w0..w2, time w3; 1D: point w0, time w1, extra i)    -> [index]
**       1 Set_Key(i, point w0..w2; 1D: point w0, extra w1)
**       2 Remove_Key(i)     3 Clear_Keys     4 Set_Looping(w0)
**       5 Evaluate(time w0) -> [value]       6 Evaluate_Derivative(time w0) (HermiteSpline3D family) -> [value]
**       7 Set_Tangents(i, in w0..w2, out w3..w5; 1D: in w0, out w1)     8 Get_Tangents(i) -> [in, out]
**       9 Set_Tightness(i, w0) / Set_TCB_Params(i, w0, w1, w2)          10 Get_Tightness(i) / Get_TCB_Params(i) -> [..]
**       11 Key_Count, Is_Looping, Get_Start_Time, Get_End_Time -> [4 words]
**       12 Get_Key(i) -> [point, time, (1D) extra] */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <type_traits>
#include "curve.cpp"
#include "hermitespline.cpp"
#include "cardinalspline.cpp"
#include "catmullromspline.cpp"
#include "tcbspline.cpp"

namespace dc {

static int bitsOf(float f) { int i; memcpy(&i, &f, sizeof i); return i; }
static float realOf(int i) { float f; memcpy(&f, &i, sizeof f); return f; }

struct Out
{
	int *p;
	int cap, n;
	void put(int v) { if (n < cap) p[n] = v; n++; }
	void putf(float f) { put(bitsOf(f)); }
	void put3(const Vector3 &v) { putf(v.X); putf(v.Y); putf(v.Z); }
};

/* ---- the objects: each upstream class with read access to its protected state ---- */
template <class C> struct Dump3 : public C
{
	void dumpKeys(Out &o) { o.put(this->Keys.Count()); for (int i = 0; i < this->Keys.Count(); i++) { o.put3(this->Keys[i].Point); o.putf(this->Keys[i].Time); } o.put(this->IsLooping ? 1 : 0); }
};
template <class C> struct Dump1 : public C
{
	void dumpKeys(Out &o) { o.put(this->Keys.Count()); for (int i = 0; i < this->Keys.Count(); i++) { o.putf(this->Keys[i].Point); o.putf(this->Keys[i].Time); o.put((int)this->Keys[i].Extra); } o.put(this->IsLooping ? 1 : 0); }
};
template <class C> struct Herm3 : public Dump3<C>
{
	void dumpMore(Out &o) { o.put(this->Tangents.Count()); for (int i = 0; i < this->Tangents.Count(); i++) { o.put3(this->Tangents[i].InTangent); o.put3(this->Tangents[i].OutTangent); } o.put(this->TangentsDirty ? 1 : 0); }
};
template <class C> struct Herm1 : public Dump1<C>
{
	void dumpMore(Out &o) { o.put(this->Tangents.Count()); for (int i = 0; i < this->Tangents.Count(); i++) { o.putf(this->Tangents[i].InTangent); o.putf(this->Tangents[i].OutTangent); } o.put(this->TangentsDirty ? 1 : 0); }
};

struct Lin3 : public Dump3<LinearCurve3DClass> { void dumpMore(Out &) {} };
struct Lin1 : public Dump1<LinearCurve1DClass> { void dumpMore(Out &) {} };
struct Her3 : public Herm3<HermiteSpline3DClass> {};
struct Her1 : public Herm1<HermiteSpline1DClass> {};
struct Car3 : public Herm3<CardinalSpline3DClass> { void dumpMore(Out &o) { Herm3<CardinalSpline3DClass>::dumpMore(o); o.put(Tightness.Count()); for (int i = 0; i < Tightness.Count(); i++) o.putf(Tightness[i]); } };
struct Car1 : public Herm1<CardinalSpline1DClass> { void dumpMore(Out &o) { Herm1<CardinalSpline1DClass>::dumpMore(o); o.put(Tightness.Count()); for (int i = 0; i < Tightness.Count(); i++) o.putf(Tightness[i]); } };
struct Cat3 : public Herm3<CatmullRomSpline3DClass> {};
struct Cat1 : public Herm1<CatmullRomSpline1DClass> {};
struct Tcb3 : public Herm3<TCBSpline3DClass> { void dumpMore(Out &o) { Herm3<TCBSpline3DClass>::dumpMore(o); o.put(Params.Count()); for (int i = 0; i < Params.Count(); i++) { o.putf(Params[i].Tension); o.putf(Params[i].Continuity); o.putf(Params[i].Bias); } } };

template <class C> struct Is3 : std::false_type {};
template <> struct Is3<Lin3> : std::true_type {};
template <> struct Is3<Her3> : std::true_type {};
template <> struct Is3<Car3> : std::true_type {};
template <> struct Is3<Cat3> : std::true_type {};
template <> struct Is3<Tcb3> : std::true_type {};

template <class C> struct IsHerm3 : std::false_type {};
template <> struct IsHerm3<Her3> : std::true_type {};
template <> struct IsHerm3<Car3> : std::true_type {};
template <> struct IsHerm3<Cat3> : std::true_type {};
template <> struct IsHerm3<Tcb3> : std::true_type {};

template <class C> struct HasTangents : std::false_type {};
template <> struct HasTangents<Her3> : std::true_type {};
template <> struct HasTangents<Car3> : std::true_type {};
template <> struct HasTangents<Cat3> : std::true_type {};
template <> struct HasTangents<Tcb3> : std::true_type {};
template <> struct HasTangents<Her1> : std::true_type {};
template <> struct HasTangents<Car1> : std::true_type {};
template <> struct HasTangents<Cat1> : std::true_type {};

template <class C> void dumpState(C &c, Out &o) { c.dumpKeys(o); c.dumpMore(o); }

template <class C> void op3(C &c, const int *w, Out &o)
{
	int op = w[0], i = w[1];
	Vector3 p(realOf(w[2]), realOf(w[3]), realOf(w[4]));
	switch (op) {
		case 0: o.put(c.Add_Key(p, realOf(w[5]))); break;
		case 1: c.Set_Key(i, p); break;
		case 2: c.Remove_Key(i); break;
		case 3: c.Clear_Keys(); break;
		case 4: c.Set_Looping(w[2] != 0); break;
		case 5: { Vector3 v; c.Evaluate(realOf(w[2]), &v); o.put3(v); break; }
		case 6: if constexpr (IsHerm3<C>::value) { Vector3 v; c.Evaluate_Derivative(realOf(w[2]), &v); o.put3(v); } break;
		case 7: if constexpr (HasTangents<C>::value) { c.Set_Tangents(i, p, Vector3(realOf(w[5]), realOf(w[6]), realOf(w[7]))); } break;
		case 8: if constexpr (HasTangents<C>::value) { Vector3 a, b; c.Get_Tangents(i, &a, &b); o.put3(a); o.put3(b); } break;
		case 9: if constexpr (std::is_same<C, Car3>::value) { c.Set_Tightness(i, realOf(w[2])); } else if constexpr (std::is_same<C, Tcb3>::value) { c.Set_TCB_Params(i, realOf(w[2]), realOf(w[3]), realOf(w[4])); } break;
		case 10: if constexpr (std::is_same<C, Car3>::value) { o.putf(c.Get_Tightness(i)); } else if constexpr (std::is_same<C, Tcb3>::value) { float a, b, d; c.Get_TCB_Params(i, &a, &b, &d); o.putf(a); o.putf(b); o.putf(d); } break;
		case 11: o.put(c.Key_Count()); o.put(c.Is_Looping() ? 1 : 0); o.putf(c.Get_Start_Time()); o.putf(c.Get_End_Time()); break;
		case 12: { Vector3 v; float t; c.Get_Key(i, &v, &t); o.put3(v); o.putf(t); break; }
	}
}

template <class C> void op1(C &c, const int *w, Out &o)
{
	int op = w[0], i = w[1];
	switch (op) {
		case 0: if constexpr (std::is_same<C, Car1>::value) { o.put(c.Add_Key(realOf(w[2]), realOf(w[3]))); } else { o.put(c.Add_Key(realOf(w[2]), realOf(w[3]), (unsigned)i)); } break;
		case 1: c.Set_Key(i, realOf(w[2]), (unsigned)w[3]); break;
		case 2: c.Remove_Key(i); break;
		case 3: c.Clear_Keys(); break;
		case 4: c.Set_Looping(w[2] != 0); break;
		case 5: { float v; c.Evaluate(realOf(w[2]), &v); o.putf(v); break; }
		case 7: if constexpr (HasTangents<C>::value) { c.Set_Tangents(i, realOf(w[2]), realOf(w[3])); } break;
		case 8: if constexpr (HasTangents<C>::value) { float a, b; c.Get_Tangents(i, &a, &b); o.putf(a); o.putf(b); } break;
		case 9: if constexpr (std::is_same<C, Car1>::value) { c.Set_Tightness(i, realOf(w[2])); } break;
		case 10: if constexpr (std::is_same<C, Car1>::value) { o.putf(c.Get_Tightness(i)); } break;
		case 11: o.put(c.Key_Count()); o.put(c.Is_Looping() ? 1 : 0); o.putf(c.Get_Start_Time()); o.putf(c.Get_End_Time()); break;
		case 12: { float v, t; unsigned e; c.Get_Key(i, &v, &t, &e); o.putf(v); o.putf(t); o.put((int)e); break; }
	}
}

template <class C> int run(const int *in, int nin, int *out, int cap)
{
	C c;
	Out o = {out, cap, 0};
	for (int k = 0; k + 8 <= nin; k += 8) {
		o.put(in[k]);
		if constexpr (Is3<C>::value) op3(c, in + k, o); else op1(c, in + k, o);
		dumpState(c, o);
	}
	return o.n;
}

}

/* ---- LookupTableClass: the stand-in declaration, then the verbatim cuts ---- */
namespace lt {

struct SimpleName
{
	char text[64];
	SimpleName &operator=(const char *s) { strncpy(text, s, 63); text[63] = 0; return *this; }
};

struct SimpleSamples
{
	float *v;
	int n;
	SimpleSamples(int count) : v((float *)calloc(count > 0 ? count : 1, sizeof(float))), n(count) {}
	~SimpleSamples() { free(v); }
	int Length(void) { return n; }
	float &operator[](int i) { return v[i]; }
};

class LookupTableClass
{
public:
	LookupTableClass(int sample_count = 256);
	virtual ~LookupTableClass(void);

	void Init(const char *name, Curve1DClass *curve);
	float Get_Value(float input);
	float Get_Value_Quick(float input);

	SimpleName Name;
	float MinInputValue;
	float MaxInputValue;
	float OOMaxMinusMin;
	SimpleSamples OutputSamples;
};

#include "lt_init.inc"
#include "lt_get_value.inc"

}

extern "C" {

int ref_curve_seq(int kind, const int *in, int nin, int *out, int cap)
{
	switch (kind) {
		case 0: return dc::run<dc::Lin3>(in, nin, out, cap);
		case 1: return dc::run<dc::Her3>(in, nin, out, cap);
		case 2: return dc::run<dc::Car3>(in, nin, out, cap);
		case 3: return dc::run<dc::Cat3>(in, nin, out, cap);
		case 4: return dc::run<dc::Tcb3>(in, nin, out, cap);
		case 5: return dc::run<dc::Lin1>(in, nin, out, cap);
		case 6: return dc::run<dc::Her1>(in, nin, out, cap);
		case 7: return dc::run<dc::Car1>(in, nin, out, cap);
		case 8: return dc::run<dc::Cat1>(in, nin, out, cap);
	}
	return -1;
}


/* Builds a 1D curve of class `kind` (5 LinearCurve1D, 6 HermiteSpline1D, 7 CardinalSpline1D, 8 CatmullRomSpline1D, numbered as
** ref_curve_seq) from `nkeys` keys (point, time, extra: 3 words each; the Hermite curve's tangents follow: 2 words per key), makes a
** LookupTableClass(nsamples) of it with Init and writes [min, max, oo (bits), the samples (bits)] then, for each of the `nq` query
** floats (bits), [Get_Value, Get_Value_Quick]. Input: [kind, looping, nsamples, nkeys, nq, keys..., tangents..., queries...]. */
int ref_lookup_case(const int *in, int *out, int cap)
{
	dc::Out o = {out, cap, 0};
	int kind = in[0], looping = in[1], nsamples = in[2], nkeys = in[3], nq = in[4];
	const int *k = in + 5;
	const int *tg = k + 3 * nkeys;
	const int *q = tg + (kind == 6 ? 2 * nkeys : 0);
	Curve1DClass *curve = 0;
	if (kind == 5) { dc::Lin1 *c = new dc::Lin1; for (int i = 0; i < nkeys; i++) c->Add_Key(dc::realOf(k[3 * i]), dc::realOf(k[3 * i + 1]), (unsigned)k[3 * i + 2]); curve = c; }
	if (kind == 6) { dc::Her1 *c = new dc::Her1; for (int i = 0; i < nkeys; i++) c->Add_Key(dc::realOf(k[3 * i]), dc::realOf(k[3 * i + 1]), (unsigned)k[3 * i + 2]); for (int i = 0; i < nkeys; i++) c->Set_Tangents(i, dc::realOf(tg[2 * i]), dc::realOf(tg[2 * i + 1])); curve = c; }
	if (kind == 7) { dc::Car1 *c = new dc::Car1; for (int i = 0; i < nkeys; i++) c->Add_Key(dc::realOf(k[3 * i]), dc::realOf(k[3 * i + 1])); curve = c; }
	if (kind == 8) { dc::Cat1 *c = new dc::Cat1; for (int i = 0; i < nkeys; i++) c->Add_Key(dc::realOf(k[3 * i]), dc::realOf(k[3 * i + 1]), (unsigned)k[3 * i + 2]); curve = c; }
	curve->Set_Looping(looping != 0);
	lt::LookupTableClass table(nsamples);
	table.Init("T", curve);
	o.putf(table.MinInputValue); o.putf(table.MaxInputValue); o.putf(table.OOMaxMinusMin);
	for (int i = 0; i < nsamples; i++) o.putf(table.OutputSamples[i]);
	for (int i = 0; i < nq; i++) {
		float x = dc::realOf(q[i]);
		o.putf(table.Get_Value(x));
		o.putf(table.Get_Value_Quick(x));
	}
	delete curve;
	return o.n;
}

/* LookupTableMgrClass::Init's default table: LookupTableClass(2), a LinearCurve1DClass with Add_Key(0.5, 0.0) and Add_Key(0.5, 1.0),
** Init("DefaultTable"); then Get_Value and Get_Value_Quick of the query floats. [min, max, oo, 2 samples, then 2 per query] */
int ref_lookup_default(const int *q, int nq, int *out, int cap)
{
	dc::Out o = {out, cap, 0};
	lt::LookupTableClass table(2);
	dc::Lin1 curve;
	curve.Add_Key(0.5f, 0.0f);
	curve.Add_Key(0.5f, 1.0f);
	table.Init("DefaultTable", &curve);
	o.putf(table.MinInputValue); o.putf(table.MaxInputValue); o.putf(table.OOMaxMinusMin);
	o.putf(table.OutputSamples[0]); o.putf(table.OutputSamples[1]);
	for (int i = 0; i < nq; i++) {
		float x = dc::realOf(q[i]);
		o.putf(table.Get_Value(x));
		o.putf(table.Get_Value_Quick(x));
	}
	return o.n;
}

}
