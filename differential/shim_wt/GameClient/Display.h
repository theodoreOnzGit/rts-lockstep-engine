/* Shim for GameEngine/Include/GameClient/Display.h, differential/wrap_wt.cpp only (#77): the size, and the drawing
** calls, which wrap_wt.cpp records (the transitions' draws are compared). The clip region is W3DDisplay's: set by
** setClipRegion, cleared by enableClipping(FALSE). A NULL image draws nothing (W3DDisplay::drawImage). */
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
	void setClipRegion(IRegion2D *r);
	void enableClipping(Bool on);
	Bool isClippingEnabled() { return m_clip; }
	void drawImage(const Image *image, Int x0, Int y0, Int x1, Int y1, Color color = 0xFFFFFFFF, Int mode = 0);
	void drawOpenRect(Int x, Int y, Int w, Int h, Real width, Color color);
	void drawFillRect(Int x, Int y, Int w, Int h, Color color);
	void drawLine(Int, Int, Int, Int, Real, Color) {}
	Int m_width, m_height;
	Bool m_clip;
	IRegion2D m_clipRegion;
};
extern Display *TheDisplay;
#endif
