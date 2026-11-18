/* Stand-in for Common/GameState.h, differential tests only (differential/wrap_draft_netpkt.cpp, #215): NetFileCommandMsg's
** and NetFileAnnounceCommandMsg's real <-> portable map paths (defined in the wrapper; the wire carries the portable one). */
#ifndef SHIM_NETPKT_GAMESTATE_H
#define SHIM_NETPKT_GAMESTATE_H
#include "Common/AsciiString.h"
class GameState
{
public:
	AsciiString realMapPathToPortableMapPath(const AsciiString &in) const;
	AsciiString portableMapPathToRealMapPath(const AsciiString &in) const;
};
extern GameState *TheGameState;
#endif
