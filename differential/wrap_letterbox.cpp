/* Differential-test wrapper for the script letterbox (GitHub #144, upstream commit 0a05454d), cut verbatim by build.rs
** into OUT_DIR/letterbox_gen:
**   lb_render.inc  GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp, from `#define LETTER_BOX_FADE_TIME` through
**                  W3DDisplay::renderLetterBox, isLetterBoxFading and isLetterBoxed (to createLightPulse's comment);
**   lb_enable.inc  the same file, W3DDisplay::toggleLetterBox and enableLetterBox (to setTimeOfDay's comment);
**   lb_reset.inc   GameEngine/Source/GameClient/Display.cpp, Display::reset's letterbox lines (its comment to `stopMovie();`).
** No edit. SLIDE_LETTERBOX is not defined (as upstream builds it), so its branches are not compiled.
** The stand-ins: a W3DDisplay declaring the members the cuts use (Display.h's names and types: m_letterBoxFadeLevel,
** m_letterBoxEnabled, m_letterBoxFadeStartTime, m_width, m_height) with getHeight() and a recording drawFillRect (W3DDisplay.h's
** signature: Int startX, Int startY, Int width, Int height, UnsignedInt color); TheTacticalView, present or NULL, whose
** setZoomLimited is recorded; the clock (`timeGetTime`). Display::reset's lines are wrapped in a member, resetLetterBox.
**
** Entry point (extern "C", tests/differential_letterbox.rs):
**   ref_letterbox_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in` is [width, height, the clock's start, TheTacticalView present], then steps of 4 words [kind, a, b, 0]:
**     kind 0  enableLetterBox(a != 0)     kind 1  toggleLetterBox()       kind 2  renderLetterBox(timeGetTime())
**     kind 3  Display::reset's lines      kind 4  the clock advances by a  kind 5  TheTacticalView present = a
**     kind 6  the display's size becomes (a, b)
**   Records of 6 words: [1, x, y, width, height, color] a drawFillRect; [2, limit] a setZoomLimited;
**   after each step [100, m_letterBoxEnabled, the fade level's bits, m_letterBoxFadeStartTime, isLetterBoxFading(),
**   isLetterBoxed()]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"

namespace lbx {

std::vector<int> g_out;
void rec(Int a, Int b = 0, Int c = 0, Int d = 0, Int e = 0, Int f = 0)
{
	Int w[6] = { a, b, c, d, e, f };
	g_out.insert(g_out.end(), w, w + 6);
}

UnsignedInt g_now;
UnsignedInt timeGetTime( void ) { return g_now; }

class View
{
public:
	void setZoomLimited( Bool limit ) { rec(2, limit ? 1 : 0); }
};
View g_view;
View *TheTacticalView = &g_view;

class W3DDisplay
{
public:
	Real	m_letterBoxFadeLevel;
	Bool	m_letterBoxEnabled;
	UnsignedInt	m_letterBoxFadeStartTime;
	Int m_width, m_height;
	Int getHeight( void ) { return m_height; }
	void drawFillRect( Int startX, Int startY, Int width, Int height, UnsignedInt color )
	{
		rec(1, startX, startY, width, height, (Int)color);
	}
	void renderLetterBox(UnsignedInt currentTime);
	Bool isLetterBoxFading(void);
	Bool isLetterBoxed(void);
	void toggleLetterBox(void);
	void enableLetterBox(Bool enable);
	void resetLetterBox(void);
};

#include "lb_render.inc"
#include "lb_enable.inc"
void W3DDisplay::resetLetterBox(void)
{
#include "lb_reset.inc"
}

W3DDisplay g_display;

} // namespace lbx

extern "C" int ref_letterbox_seq(const int *in, int nin, int *out, int cap)
{
	using namespace lbx;
	g_out.clear();
	/* Display::Display() */
	g_display.m_letterBoxFadeLevel = 0;
	g_display.m_letterBoxEnabled = FALSE;
	g_display.m_letterBoxFadeStartTime = 0;
	g_display.m_width = in[0];
	g_display.m_height = in[1];
	g_now = (UnsignedInt)in[2];
	TheTacticalView = in[3] ? &g_view : NULL;
	for (int k = 4; k + 4 <= nin; k += 4) {
		const int *w = in + k;
		switch (w[0]) {
		case 0: g_display.enableLetterBox(w[1] != 0); break;
		case 1: g_display.toggleLetterBox(); break;
		case 2: g_display.renderLetterBox(timeGetTime()); break;
		case 3: g_display.resetLetterBox(); break;
		case 4: g_now += (UnsignedInt)w[1]; break;
		case 5: TheTacticalView = w[1] ? &g_view : NULL; break;
		case 6: g_display.m_width = w[1]; g_display.m_height = w[2]; break;
		}
		Int bits;
		memcpy(&bits, &g_display.m_letterBoxFadeLevel, 4);
		rec(100, g_display.m_letterBoxEnabled ? 1 : 0, bits, (Int)g_display.m_letterBoxFadeStartTime,
			g_display.isLetterBoxFading() ? 1 : 0, g_display.isLetterBoxed() ? 1 : 0);
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
