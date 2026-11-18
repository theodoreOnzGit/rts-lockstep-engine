/* Stand-in for GameClient/GameText.h, differential tests only (differential/wrap_draft_ladder.cpp, #215): LadderDefs.cpp calls
** `TheGameText->fetch` only inside a DEBUG_LOG (compiled out in the release build). */
#ifndef SHIM_LADDER_GAMETEXT_H
#define SHIM_LADDER_GAMETEXT_H
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
class GameText { public: UnicodeString fetch(AsciiString) { return UnicodeString(); } };
extern GameText *TheGameText;
#endif
