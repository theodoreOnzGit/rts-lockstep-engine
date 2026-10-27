/* Stand-in for GameEngine/Include/Common/AudioEventRTS.h, AI build only (differential tests, batch A4): AIStateMachine.h needs only
** AudioHandle; AudioEventRTS itself is the shared shim's Common/GameAudio.h stand-in. */
#ifndef SHIM_AI_AUDIOEVENTRTS_H
#define SHIM_AI_AUDIOEVENTRTS_H
#include "Common/GameAudio.h"
typedef UnsignedInt AudioHandle;
#endif
