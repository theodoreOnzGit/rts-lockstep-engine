/* Stand-in for GameEngine/Include/Common/PlayerList.h (layer 7), energy and
** stats tests only: ThePlayerList->getNthPlayer(i), sixteen players (defined in
** the wrapper); NULL outside 0..15, as upstream. */
#ifndef SHIM_ENERGY_PLAYERLIST_H
#define SHIM_ENERGY_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList
{
public:
	Player *getNthPlayer(Int i);
};
extern PlayerList *ThePlayerList;
#endif
