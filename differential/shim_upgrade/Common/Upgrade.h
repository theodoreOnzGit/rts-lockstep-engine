/* Forwarder to the REAL GameEngine/Include/Common/Upgrade.h (upstream commit
** 0a05454d), for the Upgrade/SpecialPower build only (differential tests,
** p1w2-upgrade). differential/shim/Common/Upgrade.h is the Xfer build's
** stand-in and sits earlier on the other builds' include paths; this build
** puts this directory first, so that every `#include "Common/Upgrade.h"` here
** reaches the real header. */
#include "../../../upstream/GeneralsMD/Code/GameEngine/Include/Common/Upgrade.h"
