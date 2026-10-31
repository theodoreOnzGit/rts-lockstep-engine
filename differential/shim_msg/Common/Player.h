/* Stand-in for Common/Player.h, differential tests only: what MessageStream.cpp reads (differential/wrap_message_stream.cpp:
** GameMessage's constructor reads the local player's index) and what Recorder.cpp reads (differential/wrap_recorder.cpp:
** handleCRCMessage compares a player's name key). Renamed by the wrappers' macros. */
#ifndef SHIM_MSG_PLAYER_H
#define SHIM_MSG_PLAYER_H
class Player
{
public:
	Int m_index;
	UnsignedInt m_nameKey;
	Int getPlayerIndex() const { return m_index; }
	UnsignedInt getPlayerNameKey() const { return m_nameKey; }
};
#endif
