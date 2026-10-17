/* Differential-test wrapper for GameEngine/Source/Common/System/Trig.cpp and
** Libraries/Include/Lib/trig.h (upstream commit 0a05454d).
**
** The upstream .cpp is #included unchanged, so its tables (sinLookup,
** arcCosLookup) and file-static integer kernels (intSin, intArcCos, intTan,
** and the intCos macro) are compiled as they are and can be reached here.
** Trig.cpp itself `#define`s DEFAULT_TRIG, so its Sin/Cos/Tan/ACos are the
** sinf/cosf/tanf/acosf branch (the shipped behaviour): ref_trig_default.
**
** The table-based `#else` branches cannot be selected without editing the
** file. ref_trig_table reproduces those four one-line expressions verbatim
** (Trig.cpp lines 1645, 1654, 1663, 1672) around the unchanged kernels. */
#include <stdlib.h>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Trig.cpp"

extern "C" {

/* op: 0 Sin, 1 Cos, 2 Tan, 3 ACos, 4 ASin (the compiled DEFAULT_TRIG functions) */
float ref_trig_default(int op, float x)
{
	switch (op) {
	case 0: return Sin(x);
	case 1: return Cos(x);
	case 2: return Tan(x);
	case 3: return ACos(x);
	case 4: return ASin(x);
	}
	abort();
}

/* op: 0 Sin, 1 Cos, 2 Tan, 3 ACos: the #else branches, verbatim */
float ref_trig_table(int op, float x)
{
	switch (op) {
	case 0: return (Real)intSin((Int)((x) * INT_ONE)) / (Real)INT_ONE;
	case 1: return (Real)intCos((Int)((x) * INT_ONE)) / (Real)INT_ONE;
	case 2: return (Real)intTan((Int)((x) * INT_ONE)) / (Real)INT_ONE;
	case 3: return (Real)(intArcCos((Int)((x) * INT_ONE))) * TWOPI / (Real)INT_TWOPI;
	}
	abort();
}

/* op: 0 intSin, 1 intCos, 2 intTan, 3 intArcCos */
int ref_int_trig(int op, int v)
{
	switch (op) {
	case 0: return intSin(v);
	case 1: return intCos(v);
	case 2: return intTan(v);
	case 3: return intArcCos(v);
	}
	abort();
}

/* which: 0 sinLookup (TRIG_RES entries), 1 arcCosLookup (2*INT_ONE entries) */
int ref_trig_lookup(int which, int i)
{
	return which == 0 ? sinLookup[i] : arcCosLookup[i];
}

} /* extern "C" */
