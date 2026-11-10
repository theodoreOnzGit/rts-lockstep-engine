/* Shim for GameEngine/Include/GameClient/InGameUI.h, differential/wrap_wm.cpp only (#70): what GameWinBlockInput
** calls (recorded as one event by wrap_wm.cpp's stand-ins). */
#ifndef SHIM_WM_INGAMEUI_H
#define SHIM_WM_INGAMEUI_H
#include "Lib/BaseType.h"
class GameMessage;
class InGameUI
{
public:
	void setSelecting(Bool);
	void endAreaSelectHint(const GameMessage *);
};
extern InGameUI *TheInGameUI;
#endif
