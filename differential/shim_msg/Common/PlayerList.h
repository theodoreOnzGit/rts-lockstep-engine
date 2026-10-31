/* Stand-in for Common/PlayerList.h, differential tests only (differential/wrap_message_stream.cpp, wrap_recorder.cpp): the
** local player and, for the recorder, up to eight players by index. */
#ifndef SHIM_MSG_PLAYERLIST_H
#define SHIM_MSG_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList
{
public:
	Player m_local;
	Player m_players[8];
	Int m_count;
	Player *getLocalPlayer() { return &m_local; }
	Player *getNthPlayer(Int i) { return (i >= 0 && i < m_count) ? &m_players[i] : NULL; }
};
extern PlayerList *ThePlayerList;
#endif
