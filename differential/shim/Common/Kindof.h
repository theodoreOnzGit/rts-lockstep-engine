/* Shim, differential tests only: upstream includes "Common/Kindof.h" (SalvageCrateCollide.cpp) but the file on disk is Common/KindOf.h
** (Windows file systems are case-insensitive). This forwards to the header the build resolves for the right spelling. */
#include "Common/KindOf.h"
