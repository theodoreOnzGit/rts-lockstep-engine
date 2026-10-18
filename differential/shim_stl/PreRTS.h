/* Stand-in for Include/Precompiled/PreRTS.h, STLTypedefs build only (differential
** tests): what AsciiString.cpp needs from the precompiled header (the base
** types and Debug.h's macros). The shim PreRTS.h of differential/shim would
** bring the random-value headers, and the shim STLTypedefs.h, which would
** shadow the real one. */
#ifndef SHIM_STL_PRERTS_H
#define SHIM_STL_PRERTS_H
#include <stdio.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#endif
