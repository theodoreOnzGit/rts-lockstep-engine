/* Stand-in for Common/PlayerList.h, differential tests only (differential/wrap_message_stream.cpp). */
#ifndef SHIM_MSG_PLAYERLIST_H
#define SHIM_MSG_PLAYERLIST_H
#include "Common/Player.h"
class PlayerList { public: Player m_local; Player *getLocalPlayer() { return &m_local; } };
extern PlayerList *ThePlayerList;
#endif
