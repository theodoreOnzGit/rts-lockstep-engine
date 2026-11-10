/* Shim for GameEngine/Include/Precompiled/PreRTS.h, differential/wrap_wt.cpp only (#77): the window manager's
** (differential/shim_wm/PreRTS.h), with what the transitions' files take from the precompiled header besides: the
** containers, the INI reader's declarations (this directory's Common/INI.h) and SubsystemInterface. */
#ifndef SHIM_WT_PRERTS_H
#define SHIM_WT_PRERTS_H
#include <list>
#include <stddef.h>
#include "../shim_wm/PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/INI.h"
#include "Common/SubsystemInterface.h"
#include "Common/NameKeyGenerator.h"
/* MSVC's <math.h> in C++ gives cos and sin of a float their float overloads (cosf, sinf); glibc's C header, which
** g++ reaches here, has only the double ones, so ProcessAnimateWindow.cpp's spiral would compute in double. These are
** those overloads (the float functions are wrap_wt.cpp's: petir's, as the translation's). */
#include <math.h>
static inline float cos(float x) { return cosf(x); }
static inline float sin(float x) { return sinf(x); }
/* WWLib/always.h's MIN (the precompiled header's WWLib), which GameWindowTransitionsStyles.cpp uses */
#ifndef MIN
#define MIN(a,b)            (((a) < (b)) ? (a) : (b))
#endif
#endif
