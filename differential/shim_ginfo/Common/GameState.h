/* Stand-in for Common/GameState.h, differential tests only (differential/wrap_game_info.cpp): the two map-path conversions (renamed by the wrapper's macros apart from the Xfer build's stand-in). */
#ifndef SHIM_GINFO_GAMESTATE_H
#define SHIM_GINFO_GAMESTATE_H
#include "Common/AsciiString.h"
class GameState
{
public:
	AsciiString realMapPathToPortableMapPath(const AsciiString &in) const;
	AsciiString portableMapPathToRealMapPath(const AsciiString &in) const;
};
extern GameState *TheGameState;
#endif
