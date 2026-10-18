/* ResourceGatheringManager tests only (differential/wrap_resource_gathering.cpp): the precompiled header the
** upstream file includes first, standing in for the real PreRTS.h. */
#ifndef SHIM_RGM_PRERTS_H
#define SHIM_RGM_PRERTS_H
#include <stdio.h>
#include <string.h>
#include <float.h>
#include <algorithm>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#ifndef PI
#define PI     3.14159265359f
#define TWO_PI 6.28318530718f
#endif
#include "Common/Debug.h"
#include "Common/GameMemory.h"
#endif
