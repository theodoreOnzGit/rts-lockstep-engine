/* Forwarder, Science/RankInfo build only (differential tests): the REAL
** Common/Science.h, ahead of the stand-in of differential/shim/Common that
** the INI and Xfer builds use. The real header names `INI*` (in
** friend_parseScienceDefinition) before any header has declared it. */
#ifndef SHIM_SCIENCE_FORWARD_H
#define SHIM_SCIENCE_FORWARD_H
class INI;
#include "../../../upstream/GeneralsMD/Code/GameEngine/Include/Common/Science.h"
#endif
