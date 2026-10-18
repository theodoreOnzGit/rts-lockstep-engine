/* Shim for GameEngine/Include/GameNetwork/GameInfo.h, for the Terrain,
** Multiplayer and Crate build only (differential tests): the player-template
** constants MultiplayerSettings.cpp uses, copied from lines 46-54 of the real
** header. The rest of that header (LAN and GameSpy game info) is not compiled. */
#ifndef SHIM_TERRAIN_GAMEINFO_H
#define SHIM_TERRAIN_GAMEINFO_H
enum
{
	PLAYERTEMPLATE_RANDOM = -1,
	PLAYERTEMPLATE_OBSERVER = -2,
	PLAYERTEMPLATE_MIN = PLAYERTEMPLATE_OBSERVER
};
#endif
