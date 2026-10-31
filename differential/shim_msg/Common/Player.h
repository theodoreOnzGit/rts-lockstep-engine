/* Stand-in for Common/Player.h, differential tests only (differential/wrap_message_stream.cpp): the one call
** MessageStream.cpp makes, GameMessage's constructor reading the local player's index. Renamed by the wrapper's macros. */
#ifndef SHIM_MSG_PLAYER_H
#define SHIM_MSG_PLAYER_H
class Player { public: Int m_index; Int getPlayerIndex() const { return m_index; } };
#endif
