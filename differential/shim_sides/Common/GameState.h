/* Stand-in for Common/GameState.h in the sides/scripts build: the shim's
** GameState (differential/shim/Common/GameState.h) and the SaveCode enum of
** the real header (GameState.h lines 129-140), which SidesList.cpp's xfer
** code names (SC_INVALID_DATA). The xfer code is compiled but never run. */
#ifndef SHIM_SIDES_GAMESTATE_H
#define SHIM_SIDES_GAMESTATE_H
#include "../../shim/Common/GameState.h"
enum SaveCode
{
	SC_INVALID = -1,
	SC_OK,
	SC_NO_FILE_AVAILABLE,
	SC_FILE_NOT_FOUND,
	SC_UNABLE_TO_OPEN_FILE,
	SC_INVALID_XFER,
	SC_UNKNOWN_BLOCK,
	SC_INVALID_DATA,
	SC_ERROR,
};
#endif
