/* TunnelTracker / CaveSystem tests only (differential/wrap_tunnel_tracker.cpp): the precompiled
** header the two upstream files include first. It stands in for the real PreRTS.h, which reaches
** the whole engine. */
#ifndef SHIM_TUNNEL_PRERTS_H
#define SHIM_TUNNEL_PRERTS_H
#include <stdio.h>
#include <string.h>
#include <algorithm>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#ifndef PI
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#endif
#include "Common/Debug.h"
#include "Common/Errors.h"
#include "Common/GameMemory.h"
#endif
