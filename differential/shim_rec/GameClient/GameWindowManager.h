/* Stand-in for GameClient/GameWindowManager.h, differential tests only (differential/wrap_recorder.cpp): winGetWindowFromId (no window), for initControls. */
#ifndef SHIM_REC_GAMEWINDOWMANAGER_H
#define SHIM_REC_GAMEWINDOWMANAGER_H
#include "GameClient/GameWindow.h"
#include "Common/NameKeyGenerator.h"
class GameWindowManager { public: GameWindow *winGetWindowFromId(GameWindow *parent, Int id); };
extern GameWindowManager *TheWindowManager;
#endif
