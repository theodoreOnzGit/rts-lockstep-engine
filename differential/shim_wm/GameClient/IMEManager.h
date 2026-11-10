/* Shim for GameEngine/Include/GameClient/IMEManager.h, differential/wrap_wm.cpp only (#70): no IME (out of scope):
** never attached, never composing. */
#ifndef SHIM_WM_IMEMANAGER_H
#define SHIM_WM_IMEMANAGER_H
#include "Lib/BaseType.h"
class GameWindow;
class IMEManagerInterface
{
public:
	void attach(GameWindow *) {}
	void detatch(void) {}
	Bool isAttachedTo(GameWindow *) { return FALSE; }
	Bool isComposing(void) { return FALSE; }
};
extern IMEManagerInterface *TheIMEManager;
#endif
