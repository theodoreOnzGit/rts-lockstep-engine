/* Thing and ObjectIter tests only (differential/wrap_thing.cpp,
** wrap_object_iter.cpp): the precompiled header the two .cpp files include
** first. It brings the stand-ins of this directory (each Common/ and
** GameLogic/ header says what it stands in for) in place of the layer-5 to
** layer-8 headers that the real PreRTS.h reaches. normalizeAngle is declared
** as GameCommon.h declares it; its definition is GameCommon.cpp's, linked in
** the INI build (wrap_game_types.cpp). */
#ifndef SHIM_THING_PRERTS_H
#define SHIM_THING_PRERTS_H
#include <stdio.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
extern Real normalizeAngle(Real angle);
#endif
