/* Shim for GameEngine/Include/GameLogic/Armor.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_ARMOR_H
#define SHIM_ARMOR_H
#include "Common/AsciiString.h"
#include "GameLogic/Damage.h"
class ArmorTemplate;
class ArmorStore { public: const ArmorTemplate *findArmorTemplate(AsciiString name) const; };
extern ArmorStore *TheArmorStore;
#endif
