/* Shim for GameEngine/Include/GameLogic/Weapon.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_WEAPON_H
#define SHIM_WEAPON_H
#include "Common/AsciiString.h"
class WeaponTemplate;
class WeaponStore { public: const WeaponTemplate *findWeaponTemplate(AsciiString name) const; };
extern WeaponStore *TheWeaponStore;
#endif
