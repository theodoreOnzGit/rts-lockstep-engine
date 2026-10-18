/* Forced include (-include) for the BitFlags build, differential tests only:
** the INI prelude (differential/shim_ini/ini_prelude.h: what the real
** PreRTS.h brings before INI.h), then Common/BitFlagsIO.h through its
** forwarder (shim_flags/Common/BitFlagsIO.h), so that every translation unit
** sees `enum KindOfType` declared before Xfer.h forward-declares it, whatever
** it includes first. */
#ifndef SHIM_FLAGS_PRELUDE_H
#define SHIM_FLAGS_PRELUDE_H
#include "../shim_ini/ini_prelude.h"
#include "Common/BitFlagsIO.h"
#endif
