/* Stand-in for Include/Precompiled/PreRTS.h, SubsystemInterface build only
** (differential tests): what SubsystemInterface.cpp and its header need from
** the precompiled header (stdio, the base types, Debug.h's macros, and
** STLTypedefs.h through the shim tree). The shim PreRTS.h of differential/shim
** also brings the random-value headers, which this unit does not use. */
#ifndef SHIM_SUBSYS_PRERTS_H
#define SHIM_SUBSYS_PRERTS_H
#include <stdio.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/STLTypedefs.h"
#endif
