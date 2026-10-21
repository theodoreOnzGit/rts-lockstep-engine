/* Stand-in for GameEngine/Include/Common/PlayerList.h, Module build only (differential tests, p1w11-sonnet, special power batch H): the local
** player, which differential/l9_powers_h.inc answers. Renamed ModTestPlayerList by module_prelude.h. */
#ifndef SHIM_MODULE_PLAYERLIST_H
#define SHIM_MODULE_PLAYERLIST_H
#include "Lib/BaseType.h"
class Player;
class PlayerList
{
public:
	Player *getLocalPlayer();
	Int getPlayerCount();            /* special power batch H / update batch I: the three players of the test */
	Player *getNthPlayer(Int i);
};
extern PlayerList *ThePlayerList;
#endif
