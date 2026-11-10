/* Shim for GameEngine/Include/GameClient/IMEManager.h, differential/wrap_wm.cpp only (#70): no IME (out of scope):
** never attached, never composing.
** #74: attach, detatch and isAttachedTo are transcribed from IMEManager.cpp (IMEManager::attach without its
** ImmAssociateContext / updateStatusWindow calls, which need Win32's IME), so that the text entry's own calls
** (GadgetTextEntry.cpp, compiled verbatim) decide the attached window; it is dumped. Never composing. */
#ifndef SHIM_WM_IMEMANAGER_H
#define SHIM_WM_IMEMANAGER_H
#include "Lib/BaseType.h"
class GameWindow;
class IMEManagerInterface
{
public:
	GameWindow *m_window;
	IMEManagerInterface() : m_window(NULL) {}
	void attach(GameWindow *window) { if (m_window != window) { detatch(); m_window = window; } }
	void detatch(void) { m_window = NULL; }
	Bool isAttachedTo(GameWindow *window) { return m_window == window; }
	Bool isComposing(void) { return FALSE; }
};
extern IMEManagerInterface *TheIMEManager;
#endif
