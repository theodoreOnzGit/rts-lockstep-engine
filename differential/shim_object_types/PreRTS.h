/* ObjectTypes tests only (differential/wrap_object_types.cpp): the precompiled
** header the upstream ObjectTypes.cpp includes first. It stands in for the real
** PreRTS.h, which reaches the whole engine; ObjectTypes needs only the basic
** types, the debug macros and the memory-pool glue (shim/Common/GameMemory.h). */
#ifndef SHIM_OBJECTTYPES_PRERTS_H
#define SHIM_OBJECTTYPES_PRERTS_H
#include <stdio.h>
#include <string.h>
#include <algorithm>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/GameMemory.h"

/* the real PreRTS.h brings the forward declarations ObjectTypes.h relies on */
class Player;
class ThingTemplate;
#endif
