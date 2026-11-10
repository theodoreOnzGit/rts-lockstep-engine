/* Shim for GameEngine/Include/GameClient/Mouse.h, differential/wrap_wm.cpp only (#70): setCursorTooltip, recorded. */
#ifndef SHIM_WM_MOUSE_H
#define SHIM_WM_MOUSE_H
#include "Common/UnicodeString.h"
#define MOUSE_EVENT_NONE 0x00
class RGBColor;
class Mouse
{
public:
	void setCursorTooltip(UnicodeString tooltip, Int tooltipDelay = -1, const RGBColor *color = NULL, Real width = 1.0f);
	UnicodeString m_tooltip;
	Int m_delay;
};
extern Mouse *TheMouse;
#endif
