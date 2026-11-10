/* Shim for GameEngine/Include/GameClient/Keyboard.h, differential/wrap_wm.cpp only (#70): getPrintableKey (the test's
** table: state 1 gives the key code itself, the others nothing) and the modifier flags (none). */
#ifndef SHIM_WM_KEYBOARD_H
#define SHIM_WM_KEYBOARD_H
#include "Lib/BaseType.h"
class Keyboard
{
public:
	enum { MAX_KEY_STATES = 3 };
	WideChar getPrintableKey(UnsignedByte key, Int state) { return state == 1 ? (WideChar)key : 0; }
	Int getModifierFlags() { return 0; }
};
extern Keyboard *TheKeyboard;
#endif
