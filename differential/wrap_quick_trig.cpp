/* Differential-test wrapper for GameEngine/Include/Common/QuickTrig.h and
** GameEngine/Source/Common/System/QuickTrig.cpp (upstream commit 0a05454d).
**
** QuickTrig.cpp (the tables) is #included unchanged; it includes the
** unchanged QuickTrig.h. The header needs two things from the real
** Lib/BaseType.h, which the shim BaseType.h does not carry: PI, copied here
** from BaseType.h line 86, and REAL_TO_INT, bound to the asm transcription of
** fast_float2long_round(fast_float_trunc(x)) in wrap_base_type.cpp. */
#include <stdlib.h>
#include "Lib/BaseType.h"
extern "C" int ref_real_to_int(float x);
#define PI     3.14159265359f
#define REAL_TO_INT(x) ref_real_to_int(x)
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/QuickTrig.cpp"

extern "C" {

/* op: 0 QSin, 1 QCos, 2 QTan, 3 QCsc, 4 QSec, 5 QCot */
float ref_qtrig(int op, float x)
{
	switch (op) {
	case 0: return QSin(x);
	case 1: return QCos(x);
	case 2: return QTan(x);
	case 3: return QCsc(x);
	case 4: return QSec(x);
	case 5: return QCot(x);
	}
	abort();
}

float ref_qmag(float x, float y, float z) { return QMag(x, y, z); }

/* which: 0 TheQuickSinTable, 1 TheQuickTanTable; i < 129 */
float ref_qtrig_table(int which, int i) { return which == 0 ? TheQuickSinTable[i] : TheQuickTanTable[i]; }
float ref_qtrig_count(int which) { return which == 0 ? TheQuickSinTableCount : TheQuickTanTableCount; }

} /* extern "C" */
