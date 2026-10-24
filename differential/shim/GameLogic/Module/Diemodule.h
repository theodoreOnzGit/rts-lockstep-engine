/* Shim, differential tests only: upstream includes "GameLogic/Module/Diemodule.h" (BridgeTowerBehavior.h) but the file on disk is
** GameLogic/Module/DieModule.h (Windows file systems are case-insensitive). This forwards to the header the build resolves for the right spelling. */
#include "GameLogic/Module/DieModule.h"
