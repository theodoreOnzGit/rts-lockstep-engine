/* Forwarder, Terrain, Multiplayer and Crate build only (differential tests):
** the real Libraries/Include/Lib/BaseType.h, which MultiplayerSettings uses
** for RGBColor::setFromInt and getAsInt. The shared shim's BaseType.h (the
** build's default, for the other builds) has no RGBColor methods. Found by
** relative path, so that this forwarder is not itself the file found. */
#include "../../../upstream/GeneralsMD/Code/Libraries/Include/Lib/BaseType.h"
