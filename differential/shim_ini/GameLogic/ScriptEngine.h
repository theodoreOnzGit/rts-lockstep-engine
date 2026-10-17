/* Shim for GameEngine/Include/GameLogic/ScriptEngine.h (upstream commit 0a05454d), INI
** differential tests only. INI.cpp includes it for one lookup used by a
** field parser that needs a game system not yet translated; only that
** lookup is declared. It is defined in differential/wrap_ini.cpp to abort:
** the tests never reach it. */
#ifndef SHIM_SCRIPTENGINE_H
#define SHIM_SCRIPTENGINE_H
#include "Common/AsciiString.h"
class INI;
class ScriptEngine { public: static void parseScriptAction(INI *ini); static void parseScriptCondition(INI *ini); };
#endif
