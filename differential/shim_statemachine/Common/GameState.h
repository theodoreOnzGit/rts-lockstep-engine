/* GameEngine/Include/Common/GameState.h for the StateMachine build, differential
** tests only: the Xfer build's stand-in (the post-process counter) plus the
** SaveCode enum, which build.rs cuts verbatim out of the real header
** (StateMachine.cpp throws SC_INVALID_DATA). */
#ifndef SM_SHIM_GAMESTATE_H
#define SM_SHIM_GAMESTATE_H
#include "../../shim/Common/GameState.h"
#include "save_code.inc"
#endif
