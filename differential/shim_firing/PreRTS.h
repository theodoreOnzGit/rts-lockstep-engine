/* FiringTracker tests only (differential/wrap_firing_tracker.cpp): the precompiled header
** FiringTracker.cpp includes first. It stands in for the real PreRTS.h, which reaches the
** whole engine; the tracker needs the basic types, the debug macros, the memory-pool glue
** and the algorithm headers. */
#ifndef SHIM_FIRING_PRERTS_H
#define SHIM_FIRING_PRERTS_H
#include <stdio.h>
#include <string.h>
#include <algorithm>
#include <vector>
#include <bitset>
#include <strings.h>
#include "Lib/BaseType.h"
/* BitFlags.h (reached through ObjectStatusTypes.h) calls stricmp; the real BaseType.h defines PI, which GameCommon.h uses */
#define stricmp strcasecmp
#ifndef PI
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#endif
#include "Common/Debug.h"
#include "Common/GameMemory.h"
#include "Common/GameCommon.h"
#endif
