/* Shim for GameEngine/Include/GameClient/Color.h, for the Terrain, Multiplayer
** and Crate build only (differential tests): the one type MultiplayerSettings.h
** uses. The real header (line 63) also says `typedef Int Color;`; the client
** header itself is not compiled here. */
#ifndef SHIM_TERRAIN_COLOR_H
#define SHIM_TERRAIN_COLOR_H
#include "Lib/BaseType.h"
typedef Int Color;
#endif
