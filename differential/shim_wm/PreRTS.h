/* Shim for GameEngine/Include/Precompiled/PreRTS.h, differential/wrap_wm.cpp only (#70): the C library and the base
** types; nothing else of the precompiled header is used by the window manager and gadget files. */
#ifndef SHIM_WM_PRERTS_H
#define SHIM_WM_PRERTS_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "windows_wm.h"
#include "Common/GameAudio.h"
#include "GameClient/InGameUI.h"
#include "GameClient/SelectionXlat.h"
#endif
