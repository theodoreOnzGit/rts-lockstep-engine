/* Stand-in for GameEngine/Include/Common/PlayerList.h (layer 7), academy tests
** only: the local player, the neutral player (the advice reads their mines and
** vehicles sniped), and getNthPlayer. */
#ifndef SHIM_ACADEMY_PLAYERLIST_H
#define SHIM_ACADEMY_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList
{
public:
	Player *getLocalPlayer() const;
	Player *getNeutralPlayer() const;
	Player *getNthPlayer(Int i) const;
};
extern PlayerList *ThePlayerList;
#endif
