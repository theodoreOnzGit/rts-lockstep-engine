/* Stand-in for GameClient/GameText.h, differential tests only (differential/wrap_game_info.cpp): fetch(label): the label's text from the wrapper. */
#ifndef SHIM_GINFO_GAMETEXT_H
#define SHIM_GINFO_GAMETEXT_H
#include "Common/UnicodeString.h"
class GameTextInterface { public: UnicodeString fetch(const char *label); };
extern GameTextInterface *TheGameText;
#endif
