/* p1w12-sonnet: CostModifierUpgrade.cpp includes "Common/player.h" (a Windows file system does not care about the case). This forwards to the
** stand-in Player.h of this directory; a file system that folds case cannot hold both names, and the differential build needs a case-sensitive one. */
#include "Common/Player.h"
