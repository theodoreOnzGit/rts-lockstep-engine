/* Stand-in for GameNetwork/GameSpy/PeerDefs.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): of the real one
** (the GameSpy peer layer's types) LadderDefs.cpp uses `MAX_SLOTS`, which NetworkDefs.h defines as MAX_PLAYER+1 with MAX_PLAYER 7
** (NetworkDefs.h line 50 and 54); NetworkDefs.h itself needs the message stream's headers. */
#ifndef SHIM_LADDER_PEERDEFS_H
#define SHIM_LADDER_PEERDEFS_H
#include "Lib/BaseType.h"
static const Int MAX_SLOTS = 7 + 1;
#endif
