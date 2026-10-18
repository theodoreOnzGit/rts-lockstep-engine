/* Shim for GameEngine/Include/Common/GameState.h, for the TerrainLogic build only
** (differential tests): the shared shim's GameState (the Xfer files use it), plus the
** REAL SaveCode enum (extracted by build.rs into terrain_logic_gen_defs.h), whose
** SC_INVALID_DATA TerrainLogic::xfer throws. */
#ifndef SHIM_TL_GAMESTATE_H
#define SHIM_TL_GAMESTATE_H
#include "../../shim/Common/GameState.h"
#include "terrain_logic_gen_defs.h"
#endif
