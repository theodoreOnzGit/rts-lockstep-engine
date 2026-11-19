/* Differential-test wrapper for the W3D game window text drafts (#215, upstream commit 0a05454d), src/drafts/sweep_gui_logic/w3d_game_window.rs
** (W3dGameWindowText::get_text_size, set_text_loc), cut verbatim by build.rs (drafts_guilogic3) from W3DGameWindow.cpp into:
**   w3dwin_size.inc  W3DGameWindow::getTextSize (to getTextLoc's comment);   w3dwin_loc.inc  W3DGameWindow::setTextLoc (to drawText's comment).
** Stand-ins (outside the cuts): `W3DGameWindow` with m_textPos, m_newTextPos, m_textRenderer and m_instData.getText().str(); `Vector2` (X, Y floats);
** `Render2DSentenceClass::Get_Text_Extents(const char *)` (returns the extents the test sets, and records the text it was given: the real one measures
** the font), `Vector2` as WWMath's.
** Entry points (extern "C", tests/differential_draft_w3dwin.rs): ref_w3d_size(x_bits, y_bits, want_w, want_h, out) -> out = [width or the sentinel, height or the
** sentinel] (the sentinel -77777 marks an output the call did not write); ref_w3d_loc(in, n, out): in = (x, y).. applied in order from (0, 0) with m_newTextPos
** FALSE; out = after each call [x, y, newTextPos]. */
#include <vector>
#define NULL 0
typedef int Int;
typedef int Bool;
typedef int Color;
#define TRUE 1
#define FALSE 0
struct Vector2 { float X, Y; };
struct Render2DSentenceClass { Vector2 Get_Text_Extents(const char *text) { return m_ext; } Vector2 m_ext; };
struct InstText { const char *str() const { return "text"; } };
struct InstData { InstText getText() const { InstText t; return t; } };
struct ICoord2DStandIn { Int x, y; };
class W3DGameWindow
{
public:
	void getTextSize( Int *width, Int *height );
	void setTextLoc( Int x, Int y );
	Render2DSentenceClass m_textRenderer;
	InstData m_instData;
	ICoord2DStandIn m_textPos;
	Bool m_newTextPos;
};
#include "w3dwin_size.inc"
#include "w3dwin_loc.inc"

extern "C" int ref_w3d_size(int xbits, int ybits, int want_w, int want_h, int *out)
{
	W3DGameWindow w;
	union { float f; int i; } x, y;
	x.i = xbits; y.i = ybits;
	w.m_textRenderer.m_ext.X = x.f; w.m_textRenderer.m_ext.Y = y.f;
	Int width = -77777, height = -77777;
	w.getTextSize(want_w ? &width : (Int *)NULL, want_h ? &height : (Int *)NULL);
	out[0] = width; out[1] = height;
	return 2;
}

extern "C" int ref_w3d_loc(const int *in, int n, int *out)
{
	W3DGameWindow w;
	w.m_textPos.x = 0; w.m_textPos.y = 0; w.m_newTextPos = FALSE;
	int k = 0;
	for (int i = 0; i + 1 < n; i += 2)
	{
		w.setTextLoc(in[i], in[i + 1]);
		out[k++] = w.m_textPos.x; out[k++] = w.m_textPos.y; out[k++] = w.m_newTextPos;
		w.m_newTextPos = in[i] & 1 ? FALSE : w.m_newTextPos; /* the test clears the flag after some calls, as drawText does */
	}
	return k;
}
