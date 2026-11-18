/* Stand-in for Common/PlayerList.h, differential tests only (differential/wrap_draft_netpkt.cpp, #215): every name
** resolves to one player. Renamed by the wrapper's macros. */
#ifndef SHIM_NETPKT_PLAYERLIST_H
#define SHIM_NETPKT_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList
{
public:
	Player m_player;
	Player *getNthPlayer(Int) { return &m_player; }
	Player *findPlayerWithNameKey(NameKeyType) { return &m_player; }
};
extern PlayerList *ThePlayerList;
#endif
