/* Shim for GameEngine/Include/GameClient/FXList.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_FXLIST_H
#define SHIM_FXLIST_H
#include "Common/AsciiString.h"
class FXList;
class FXListStore { public: const FXList *findFXList(const char *name) const; };
extern FXListStore *TheFXListStore;
#endif
