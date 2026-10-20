/* Stand-in for GameEngine/Include/Common/PlayerList.h (layer 7), create-module build only (p1l9-creates). Renamed to CrtPlayerList. */
#ifndef SHIM_CREATES_PLAYERLIST_H
#define SHIM_CREATES_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList
{
public:
	Int getPlayerCount() { return m_playerCount; }
	Player *getNthPlayer(Int i);
	Int m_playerCount;
	Player m_players[16];
};
extern PlayerList *ThePlayerList;
#endif
