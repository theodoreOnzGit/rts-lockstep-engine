/* Shim, differential tests only: Trig.cpp includes "Lib/Trig.h" but the file
** on disk is Libraries/Include/Lib/trig.h (Windows file systems are
** case-insensitive). This forwards to the unchanged upstream header. */
#include "Lib/trig.h"
