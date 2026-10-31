/* Stand-in for GameClient/GameText.h, differential tests only (differential/wrap_recorder.cpp): fetch(label): the label's text. */
#ifndef SHIM_REC_GAMETEXT_H
#define SHIM_REC_GAMETEXT_H
#include "Common/UnicodeString.h"
class GameTextInterface { public: UnicodeString fetch(const char *label); };
extern GameTextInterface *TheGameText;
#endif
