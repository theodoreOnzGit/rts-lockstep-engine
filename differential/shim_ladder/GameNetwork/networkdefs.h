/* Stand-in for GameNetwork/networkdefs.h (upstream's is NetworkDefs.h, whose message stream headers need the engine), differential
** tests only (differential/wrap_draft_user.cpp, #215): User.h needs of it the base types and the memory pool object. */
#ifndef SHIM_LADDER_NETWORKDEFS_H
#define SHIM_LADDER_NETWORKDEFS_H
#include "Lib/BaseType.h"
#include "Common/GameMemory.h"
#endif
