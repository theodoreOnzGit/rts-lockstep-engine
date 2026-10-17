/* Shim for GameEngine/Include/Common/DamageFX.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_DAMAGEFX_H
#define SHIM_DAMAGEFX_H
#include "Common/AsciiString.h"
class DamageFX;
class DamageFXStore { public: const DamageFX *findDamageFX(AsciiString name) const; };
extern DamageFXStore *TheDamageFXStore;
#endif
