/* Shim for GameEngine/Include/Precompiled/PreRTS.h (upstream commit 0a05454d),
** differential tests only. The real precompiled header pulls in windows.h and
** most of the engine. RandomValue.cpp relies on it for <time.h> and for the
** declarations of the Logic/Client/Audio random-value headers, which are
** included here unchanged from upstream. */
#ifndef SHIM_PRERTS_H
#define SHIM_PRERTS_H
#include <time.h>
#include <string.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "GameLogic/LogicRandomValue.h"
#include "GameClient/ClientRandomValue.h"
#include "Common/AudioRandomValue.h"
#endif
