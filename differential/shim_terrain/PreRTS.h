/* Forwarder, Terrain, Multiplayer and Crate build only (differential tests):
** every GameEngine .cpp of the build begins with `#include "PreRTS.h"`, which
** would otherwise be the shared shim's (see terrain_prelude.h for why that
** one is not used). What the real PreRTS.h brings is in terrain_prelude.h,
** and SubsystemInterface is the one this build's class needs. */
#include "terrain_prelude.h"
#include "Common/SubsystemInterface.h"
