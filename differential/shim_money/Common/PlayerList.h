/* Stand-in for GameEngine/Include/Common/PlayerList.h (layer 7), money tests
** only: ThePlayerList->getNthPlayer(i) (eight players, see wrap_money.cpp). */
#ifndef SHIM_MONEY_PLAYERLIST_H
#define SHIM_MONEY_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList
{
public:
	Player *getNthPlayer(Int i);
};
extern PlayerList *ThePlayerList;
#endif
