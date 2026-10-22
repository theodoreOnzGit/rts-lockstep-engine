/* Stand-in for GameEngine/Include/Common/GameState.h, Module build only (differential tests, p1w13-sonnet, batch P): the shared shim (differential/shim/Common/
** GameState.h) and the SaveCode enum ActiveBody::xfer throws SC_INVALID_DATA of (GameState.h lines 128-138, verbatim). Not in the shared shim, which the tunnel
** tracker's build defines SaveCode against already. */
#ifndef SHIM_MODULE_GAMESTATE_H
#define SHIM_MODULE_GAMESTATE_H
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
