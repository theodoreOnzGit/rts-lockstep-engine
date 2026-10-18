/* ExperienceTracker tests only (differential/wrap_experience_tracker.cpp): the precompiled
** header ExperienceTracker.cpp includes first. It stands in for the real PreRTS.h, which
** reaches the whole engine; the tracker needs the basic types, the debug macros, the
** memory-pool glue and the algorithm headers. The real GameCommon.h and GameType.h, which
** ExperienceTracker.h includes, compile unchanged (they reach only Lib/BaseType.h). */
#ifndef SHIM_EXPERIENCE_PRERTS_H
#define SHIM_EXPERIENCE_PRERTS_H
#include <stdio.h>
#include <string.h>
#include <algorithm>
#include <vector>
#include "Lib/BaseType.h"
/* the real BaseType.h defines PI (upstream Libraries/Include/Lib/BaseType.h:86), which GameCommon.h uses */
#ifndef PI
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#endif
#include "Common/Debug.h"
#include "Common/GameMemory.h"
#endif
