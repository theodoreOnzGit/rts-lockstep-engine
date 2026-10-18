/* Differential-test wrapper for BitFlags and its instantiations (upstream
** commit 0a05454d):
**   GameEngine/Include/Common/BitFlags.h, BitFlagsIO.h (the templates)
**   GameEngine/Source/Common/BitFlags.cpp (ModelConditionFlags and
**     ArmorSetFlags names), Source/Common/System/KindOf.cpp,
**     ObjectStatusTypes.cpp, DisabledTypes.cpp
**   Include/Common/KindOf.h, ObjectStatusTypes.h, DisabledTypes.h,
**     ModelState.h, SpecialPowerMaskType.h, SpecialPowerType.h,
**     Include/GameLogic/ArmorSet.h, WeaponSetFlags.h, WeaponSetType.h
** The .cpp files are compiled unchanged beside this one (build.rs, clang++
** -fms-extensions for the headers' MSVC forward declarations of enums).
** WeaponSet.cpp and SpecialPower.cpp need most of the engine, so their
** s_bitNameList definitions are copied verbatim by build.rs into
** OUT_DIR/flags_gen/bit_name_lists.h (each located by its first line and
** the `NULL` that ends it, asserted to occur once) and included here.
** INI parsing runs the real templates on `strtok`, which INI's
** getNextToken/getNextTokenOrNull continue (INI.cpp, upstream_ini); xfer
** runs them on the real XferSave, XferLoad and XferCRC (upstream_xfer).
**
** `which` selects the instantiation: 0 KindOfMaskType, 1 ObjectStatusMaskType,
** 2 DisabledMaskType, 3 ModelConditionFlags, 4 ArmorSetFlags,
** 5 WeaponSetFlags, 6 SpecialPowerMaskType. Masks cross as 4 x 64-bit
** words, bit i in word i/64. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "Common/KindOf.h"
#include "Common/ObjectStatusTypes.h"
#include "Common/DisabledTypes.h"
#include "Common/ModelState.h"
#include "Common/SpecialPowerMaskType.h"
#include "GameLogic/ArmorSet.h"
#include "GameLogic/WeaponSetFlags.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/XferCRC.h"

#include "bit_name_lists.h"

namespace {

typedef unsigned long long W;

template <class F> F fromWords(const W *w)
{
	F f;
	for (int i = 0; i < f.size(); ++i)
		if ((w[i / 64] >> (i % 64)) & 1) f.set(i);
	return f;
}

template <class F> void toWords(const F &f, W *w)
{
	for (int k = 0; k < 4; ++k) w[k] = 0;
	for (int i = 0; i < f.size(); ++i)
		if (f.test(i)) w[i / 64] |= 1ULL << (i % 64);
}

/* op (a, b, c masks; i an index; result mask into out):
**  0 operator==(a,b)  1 operator!=  2 testForAny  3 testForAll  4 testForNone
**  5 count(a)  6 any(a)  7 flip(a)->out  8 clear(a)->out
**  9 countIntersection  10 countInverseIntersection  11 anyIntersectionWith
**  12 clear(a, b)->out  13 set(a, b)->out  14 clearAndSet(a: b, c)->out
**  15 testSetAndClear(a: b, c)  16 size  17 test(a, i)  18 set(a, i, c[0])->out */
template <class F> int op(int code, const W *a, const W *b, const W *c, int i, W *out)
{
	F fa = fromWords<F>(a), fb = fromWords<F>(b), fc = fromWords<F>(c);
	switch (code) {
	case 0: return fa == fb;
	case 1: return fa != fb;
	case 2: return fa.testForAny(fb);
	case 3: return fa.testForAll(fb);
	case 4: return fa.testForNone(fb);
	case 5: return fa.count();
	case 6: return fa.any();
	case 7: fa.flip(); toWords(fa, out); return 0;
	case 8: fa.clear(); toWords(fa, out); return 0;
	case 9: return fa.countIntersection(fb);
	case 10: return fa.countInverseIntersection(fb);
	case 11: return fa.anyIntersectionWith(fb);
	case 12: fa.clear(fb); toWords(fa, out); return 0;
	case 13: fa.set(fb); toWords(fa, out); return 0;
	case 14: fa.clearAndSet(fb, fc); toWords(fa, out); return 0;
	case 15: return fa.testSetAndClear(fb, fc);
	case 16: return fa.size();
	case 17: return fa.test(i);
	case 18: fa.set(i, (Int)c[0]); toWords(fa, out); return 0;
	}
	abort();
}

/* name ops: 0 getSingleBitFromName(token) -> index; 1 getNameFromSingleBit(i)
** -> name or NULL; 2 getBitNameIfSet(a, i) -> name or NULL;
** 3 setBitByName(a, token) -> bool, mask into out; 4 buildDescription(a)
** -> into name buffer */
template <class F> int names(int code, const W *a, const char *token, int i, W *out, char *name, int cap)
{
	F fa = fromWords<F>(a);
	const char *n = 0;
	switch (code) {
	case 0: return F::getSingleBitFromName(token);
	case 1: n = F::getNameFromSingleBit(i); break;
	case 2: n = fa.getBitNameIfSet(i); break;
	case 3: { Bool r = fa.setBitByName(token); toWords(fa, out); return r; }
	case 4: { AsciiString s; fa.buildDescription(&s); n = s.str(); if ((int)strlen(n) >= cap) return -2; strcpy(name, n); return 1; }
	default: abort();
	}
	if (!n) return 0;
	if ((int)strlen(n) >= cap) return -2;
	strcpy(name, n);
	return 1;
}

template <class F> const char *nameAt(int i) { return F::getBitNames()[i]; }

/* parse the rest of `line` (whose first token, the field name, is taken
** with strtok first, as INI::initFromINI does) into a mask starting at
** `start`; the description string into desc. Returns 1 if upstream threw. */
template <class F> int parse(char *line, const W *start, W *out, int single, int *index, char *desc, int cap)
{
	F f = fromWords<F>(start);
	AsciiString d;
	int threw = 0;
	INI ini;
	strtok(line, ini.getSeps());
	try {
		if (single) F::parseSingleBitFromINI(&ini, 0, index, 0);
		else f.parse(&ini, &d);
	} catch (...) {
		threw = 1;
	}
	toWords(f, out);
	int n = (int)strlen(d.str());
	if (n >= cap) n = cap - 1;
	memcpy(desc, d.str(), n);
	desc[n] = 0;
	return threw;
}

/* mode: 0 XferSave to path, 1 XferLoad from path (mask into out),
** 2 XferCRC (its getCRC() into out[0]). Returns 0, or the XferStatus
** thrown. (XferDeepCRC is not used: its mode is XFER_SAVE, so BitFlags
** takes the save branch there.) */
template <class F> int xfer(int mode, const char *path, const W *in, W *out)
{
	F f = fromWords<F>(in);
	XferSave save; XferLoad load; XferCRC crc;
	Xfer *x = mode == 0 ? (Xfer *)&save : mode == 1 ? (Xfer *)&load : (Xfer *)&crc;
	int status = 0;
	x->open(AsciiString(path));
	try {
		f.xfer(x);
	} catch (XferStatus s) {
		status = (int)s;
	}
	x->close();
	toWords(f, out);
	if (mode == 2) out[0] = crc.getCRC();
	return status;
}

} /* namespace */

#define DISPATCH(which, CALL)                                \
	switch (which) {                                         \
	case 0: { typedef KindOfMaskType F; return CALL; }       \
	case 1: { typedef ObjectStatusMaskType F; return CALL; } \
	case 2: { typedef DisabledMaskType F; return CALL; }     \
	case 3: { typedef ModelConditionFlags F; return CALL; }  \
	case 4: { typedef ArmorSetFlags F; return CALL; }        \
	case 5: { typedef WeaponSetFlags F; return CALL; }       \
	case 6: { typedef SpecialPowerMaskType F; return CALL; } \
	}                                                        \
	abort();

static const long long k_kindof[] = {
#define X(n) n,
#include "kind_of_list.h"
};
static const long long k_status[] = {
#include "object_status_list.h"
};
static const long long k_disabled[] = {
#include "disabled_list.h"
};
static const long long k_model[] = {
#include "model_condition_list.h"
};
static const long long k_armor[] = {
#include "armor_set_list.h"
#undef X
};

extern "C" {

int ref_flags_op(int which, int code, const W *a, const W *b, const W *c, int i, W *out)
{
	DISPATCH(which, op<F>(code, a, b, c, i, out))
}

int ref_flags_names(int which, int code, const W *a, const char *token, int i, W *out, char *name, int cap)
{
	DISPATCH(which, names<F>(code, a, token, i, out, name, cap))
}

/* s_bitNameList[i], through getBitNames(); NULL at the terminator */
const char *ref_flags_name_at(int which, int i)
{
	DISPATCH(which, nameAt<F>(i))
}

int ref_flags_parse(int which, char *line, const W *start, W *out, int single, int *index, char *desc, int cap)
{
	DISPATCH(which, parse<F>(line, start, out, single, index, desc, cap))
}

int ref_flags_xfer(int which, int mode, const char *path, const W *in, W *out)
{
	DISPATCH(which, xfer<F>(mode, path, in, out))
}

/* enumerators: 0 KindOfType, 1 ObjectStatusTypes, 2 DisabledType,
** 3 ModelConditionFlagType, 4 ArmorSetType */
int ref_flags_enum_values(int which, long long *out, int cap)
{
	const long long *t; int n;
#define T(a) t = a; n = (int)(sizeof(a) / sizeof(a[0])); break
	switch (which) {
	case 0: T(k_kindof);
	case 1: T(k_status);
	case 2: T(k_disabled);
	case 3: T(k_model);
	case 4: T(k_armor);
	default: abort();
	}
#undef T
	for (int i = 0; i < n && i < cap; ++i) out[i] = t[i];
	return n;
}

/* The two KindOfMaskType lookups Xfer::xferKindOf calls (wrap_xfer.cpp's
** stand-in class forwards to these). */
const char *ref_kindof_name_from_single_bit(int i) { return KindOfMaskType::getNameFromSingleBit(i); }
int ref_kindof_single_bit_from_name(const char *token) { return KindOfMaskType::getSingleBitFromName(token); }

/* KINDOFMASK_FS after initKindOfMasks(), DISABLEDMASK_ALL after
** initDisabledMasks(): which 0, 1 */
void ref_flags_globals(int which, W *out)
{
	if (which == 0) { initKindOfMasks(); toWords(KINDOFMASK_FS, out); }
	else { initDisabledMasks(); toWords(DISABLEDMASK_ALL, out); }
}

} /* extern "C" */
