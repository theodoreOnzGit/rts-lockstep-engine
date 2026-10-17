/* Shim for GameEngine/Include/GameLogic/ObjectCreationList.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_OCL_H
#define SHIM_OCL_H
#include "Common/AsciiString.h"
class ObjectCreationList;
class ObjectCreationListStore { public: const ObjectCreationList *findObjectCreationList(const char *name) const; };
extern ObjectCreationListStore *TheObjectCreationListStore;
#endif
