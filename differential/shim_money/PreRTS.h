/* Money and Handicap tests only (differential/wrap_money.cpp,
** wrap_handicap.cpp): the precompiled header the two .cpp files include
** first. It brings the stand-ins of this directory (each Common/ header says
** what it stands in for), in place of the layer-5 to layer-7 headers that
** the real PreRTS.h reaches. */
#ifndef SHIM_MONEY_PRERTS_H
#define SHIM_MONEY_PRERTS_H
#include <stdio.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/NameKeyGenerator.h"
#endif
