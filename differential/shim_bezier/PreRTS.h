/* Bezier tests only: the precompiled header the Bezier .cpp files include
** first. It is this directory's own copy so that `Lib/BaseType.h` resolves
** here (a quoted include searches the including file's directory first, and
** differential/shim/PreRTS.h would find the shim's BaseType.h without the
** Coord3D methods). Needs only what the two files use. */
#ifndef SHIM_BEZIER_PRERTS_H
#define SHIM_BEZIER_PRERTS_H
#include <stdio.h>
#include <string.h>
#include "Lib/BaseType.h"
#endif
