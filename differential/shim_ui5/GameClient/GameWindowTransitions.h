/* Shim for GameEngine/Include/GameClient/GameWindowTransitions.h, group ui5's programs only (#215): differential/shim_wm's class (the transitions do nothing there) with the three
** members NetworkDirectConnect.cpp and the other ui5 menus call, declared here and DEFINED in the wrappers (they log the call and keep the one group's state; see
** differential/wrap_draft_ui5_ndc.cpp). */
#ifndef SHIM_UI5_GAMEWINDOWTRANSITIONS_H
#define SHIM_UI5_GAMEWINDOWTRANSITIONS_H
#include "Common/AsciiString.h"
#include "Lib/BaseType.h"
class GameWindowTransitionsHandler
{
public:
	void init() {}
	void load() {}
	void reset() {}
	void update() {}
	void draw() {}
	void setGroup(AsciiString groupName, Bool immidiate = false);
	void reverse(AsciiString groupName);
	Bool isFinished(void);
};
extern GameWindowTransitionsHandler *TheTransitionHandler;
#endif
