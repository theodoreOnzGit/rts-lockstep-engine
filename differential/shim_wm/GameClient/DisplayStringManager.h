/* Shim for GameEngine/Include/GameClient/DisplayStringManager.h, differential/wrap_wm.cpp only (#70): display strings
** are the test's (wrap_wm.cpp's TestDisplayString). */
#ifndef SHIM_WM_DISPLAYSTRINGMANAGER_H
#define SHIM_WM_DISPLAYSTRINGMANAGER_H
#include "GameClient/DisplayString.h"
class DisplayStringManager
{
public:
	DisplayString *newDisplayString(void);
	void freeDisplayString(DisplayString *string);
};
extern DisplayStringManager *TheDisplayStringManager;
#endif
