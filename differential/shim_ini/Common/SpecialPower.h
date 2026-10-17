/* Shim for GameEngine/Include/Common/SpecialPower.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_SPECIALPOWER_H
#define SHIM_SPECIALPOWER_H
#include "Common/AsciiString.h"
class SpecialPowerTemplate;
class SpecialPowerStore { public: const SpecialPowerTemplate *findSpecialPowerTemplate(AsciiString name) const; };
extern SpecialPowerStore *TheSpecialPowerStore;
#endif
