/* Forwarder, BitFlags build only (differential tests): the REAL
** Common/BitFlagsIO.h, ahead of the stand-in of differential/shim/Common
** that the Xfer build uses.
**
** It includes the real Common/KindOf.h first. KindOf.h includes this file
** before its `enum KindOfType` (the guard below makes that inner include
** empty), and BitFlagsIO.h includes Xfer.h, whose `enum KindOfType;` is a
** forward declaration MSVC accepted but standard C++ only allows after the
** enum is declared. In the MSVC build the order did not matter. */
#ifndef SHIM_FLAGS_BITFLAGSIO_H
#define SHIM_FLAGS_BITFLAGSIO_H
#include "Common/KindOf.h"
#include "../../../upstream/GeneralsMD/Code/GameEngine/Include/Common/BitFlagsIO.h"
#endif
