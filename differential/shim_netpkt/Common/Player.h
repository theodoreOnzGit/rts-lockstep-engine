/* Stand-in for Common/Player.h, differential tests only (differential/wrap_draft_netpkt.cpp, #215): NetCommandMsg.cpp's
** constructGameMessage reads a player's index (it goes into the GameMessage, never onto the wire) and its unused
** indexFromMask reads the mask. Renamed by the wrapper's macros. */
#ifndef SHIM_NETPKT_PLAYER_H
#define SHIM_NETPKT_PLAYER_H
#include "Common/AsciiString.h"
#define MAX_PLAYER_COUNT 16
typedef int NameKeyType;
class Player
{
public:
	Int m_index;
	UnsignedInt m_mask;
	Int getPlayerIndex() const { return m_index; }
	UnsignedInt getPlayerMask() const { return m_mask; }
};
class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &) { return 1; }
};
extern NameKeyGenerator *TheNameKeyGenerator;
#endif
