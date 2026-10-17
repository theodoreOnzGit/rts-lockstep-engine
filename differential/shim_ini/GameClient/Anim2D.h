/* Shim for GameEngine/Include/GameClient/Anim2D.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_ANIM2D_H
#define SHIM_ANIM2D_H
#include "Common/AsciiString.h"
class Anim2DTemplate;
class Anim2DCollection { public: Anim2DTemplate *findTemplate(const AsciiString &name); };
extern Anim2DCollection *TheAnim2DCollection;
#endif
