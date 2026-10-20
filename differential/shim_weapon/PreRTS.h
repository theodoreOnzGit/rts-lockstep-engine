/* The Weapon firing build's copy of differential/shim/PreRTS.h (p1w8-sonnet): the same, but "Lib/BaseType.h" is found next to it, i.e. the
** build's own BaseType.h (shim_weapon/Lib/BaseType.h), whose Coord3D is the real one. */
/* Shim for GameEngine/Include/Precompiled/PreRTS.h (upstream commit 0a05454d),
** differential tests only. The real precompiled header pulls in windows.h and
** most of the engine. RandomValue.cpp relies on it for <time.h> and for the
** declarations of the Logic/Client/Audio random-value headers, which are
** included here unchanged from upstream. */
#ifndef SHIM_PRERTS_H
#define SHIM_PRERTS_H
#include <stdio.h>   /* the Xfer headers use FILE (the real PreRTS.h brings stdio) */
#include <time.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "GameLogic/LogicRandomValue.h"
#include "GameClient/ClientRandomValue.h"
#include "Common/AudioRandomValue.h"
#endif
