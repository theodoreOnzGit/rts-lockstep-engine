/* Shim for GameEngine/Include/GameClient/GameWindowTransitions.h, differential/wrap_wm.cpp only (#70): the window
** transitions do nothing here. */
#ifndef SHIM_WM_GAMEWINDOWTRANSITIONS_H
#define SHIM_WM_GAMEWINDOWTRANSITIONS_H
class GameWindowTransitionsHandler
{
public:
	void init() {}
	void load() {}
	void reset() {}
	void update() {}
	void draw() {}
};
extern GameWindowTransitionsHandler *TheTransitionHandler;
#endif
