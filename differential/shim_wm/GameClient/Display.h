/* Shim for GameEngine/Include/GameClient/Display.h, differential/wrap_wm.cpp only (#70): the size, and the drawing
** calls GameWindowManager.cpp and GameWindow.cpp make (drawing is not compared: they do nothing). */
#ifndef SHIM_WM_DISPLAY_H
#define SHIM_WM_DISPLAY_H
#include "Lib/BaseType.h"
#include "GameClient/Color.h"
class Image;
class Display
{
public:
	Int getWidth() { return m_width; }
	Int getHeight() { return m_height; }
	void setClipRegion(IRegion2D *) {}
	void enableClipping(Bool) {}
	Bool isClippingEnabled() { return FALSE; }
	void drawImage(const Image *, Int, Int, Int, Int, Color = 0xFFFFFFFF, Int = 0) {}
	void drawOpenRect(Int, Int, Int, Int, Real, Color) {}
	void drawFillRect(Int, Int, Int, Int, Color) {}
	void drawLine(Int, Int, Int, Int, Real, Color) {}
	Int m_width, m_height;
};
extern Display *TheDisplay;
#endif
