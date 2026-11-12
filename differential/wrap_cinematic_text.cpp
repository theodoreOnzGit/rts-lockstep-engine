/* Differential-test wrapper for the cinematic text (GitHub #146, upstream commit 0a05454d), cut verbatim by build.rs into
** OUT_DIR/cinematic_text_gen:
**   ct_setters.inc   GameEngine/Include/GameClient/Display.h, setCinematicText, setCinematicFont and
**                    setCinematicTextFrames (to getAverageFPS), inside the stand-in Display class;
**   ct_ctor.inc      GameEngine/Source/GameClient/Display.cpp, Display::Display's cinematic members (to m_movieHoldTime),
**                    inside the stand-in's constructor;
**   ct_draw.inc      GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp, W3DDisplay::draw's cinematic text
**                    block (from "display cinematicText over the black" to the debug display's `if`), as the body of
**                    the stand-in's drawCinematicText.
** No edit. GameMakeColor is upstream's GameClient/Color.h. The stand-ins: Display (m_width, m_height, getWidth and
** getHeight as Display.h's, the three members), TheDisplay (the same object), GameFont (a name, a point size, bold),
** TheDisplayStringManager (newDisplayString gives one display string, reused: upstream's leaks a new one each frame) and
** DisplayString (records setWordWrap, setWordWrapCentered, setText, setFont and draw; getWidth is the input's).
** AsciiString and UnicodeString are the shim's (UnicodeString::translate sign-extends each char, as upstream).
**
** Entry point (extern "C", tests/differential_cinematic_text.rs):
**   ref_cinematic_text_seq(in, nin, out, cap): `in` is [display width, height], then steps of 4 words [kind, a, b, c]:
**     kind 0  setCinematicText(string a)           kind 1  setCinematicFont(a < 0: NULL; else the font (name a, size b,
**     kind 2  setCinematicTextFrames(a)                    bold c))
**     kind 3  the display's size (a, b)
**     kind 4  the draw, getWidth() answering a  -> [4, wrap, centered, n, n units, font name (-1 NULL), size, bold, x,
**                                                  y, color, backColor] when it draws
**   After each step [100, m_cinematicTextFrames, the text's length]. The strings are tests/differential_cinematic_text.rs's
**   list, in its order. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "GameClient/Color.h"

namespace ctx {

std::vector<int> g_out;
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }

static const char *const STRINGS[] = { "", "Hello", "The base is under attack", "\xE9t\xE9 \x80 \xFF", "A much longer line of cinematic text that wraps across the screen", "Arial", "rial - Size:20al - Size:20 - Size:20", "Times New Roman" };
static const Int NSTRINGS = (Int)(sizeof STRINGS / sizeof STRINGS[0]);

class GameFont
{
public:
	Int m_name, m_size;
	Bool m_bold;
};
GameFont g_font;

class DisplayString
{
public:
	Int m_width;
	void setWordWrap( Int wordWrap ) { rec({wordWrap}); }
	void setWordWrapCentered( Bool isCentered ) { rec({isCentered ? 1 : 0}); }
	void setText( UnicodeString text )
	{
		g_out.push_back(text.getLength());
		for (Int i = 0; i < text.getLength(); ++i)
			g_out.push_back((Int)text.str()[i]);
	}
	void setFont( GameFont *font )
	{
		if (font)
			rec({font->m_name, font->m_size, font->m_bold ? 1 : 0});
		else
			rec({-1, 0, 0});
	}
	Int getWidth( Int charPos = -1 ) { return m_width; }
	void draw( Int x, Int y, Color color, Color dropColor ) { rec({x, y, color, dropColor}); }
};
DisplayString g_string;
struct DisplayStringManagerStub
{
	DisplayString *newDisplayString() { g_out.push_back(4); return &g_string; }
};
DisplayStringManagerStub g_dsm;
DisplayStringManagerStub *TheDisplayStringManager = &g_dsm;

class Display;
extern Display *TheDisplay;
class Display
{
public:
	UnsignedInt m_width, m_height;
	AsciiString m_cinematicText;
	GameFont *m_cinematicFont;
	Int m_cinematicTextFrames;
	Display()
	{
#include "ct_ctor.inc"
	}
	virtual ~Display() {}
	virtual UnsignedInt getWidth( void ) { return m_width; }
	virtual UnsignedInt getHeight( void ) { return m_height; }
#include "ct_setters.inc"
	void drawCinematicText()
	{
#include "ct_draw.inc"
	}
};
Display *TheDisplay = NULL;

} // namespace ctx

extern "C" int ref_cinematic_text_seq(const int *in, int nin, int *out, int cap)
{
	using namespace ctx;
	g_out.clear();
	Display d;
	TheDisplay = &d;
	d.m_width = (UnsignedInt)in[0];
	d.m_height = (UnsignedInt)in[1];
	for (int k = 2; k + 4 <= nin; k += 4) {
		const int *w = in + k;
		switch (w[0]) {
		case 0: d.setCinematicText(AsciiString(STRINGS[w[1] % NSTRINGS])); break;
		case 1:
			if (w[1] < 0) {
				d.setCinematicFont(NULL);
			} else {
				g_font.m_name = w[1];
				g_font.m_size = w[2];
				g_font.m_bold = w[3] != 0;
				d.setCinematicFont(&g_font);
			}
			break;
		case 2: d.setCinematicTextFrames(w[1]); break;
		case 3: d.m_width = (UnsignedInt)w[1]; d.m_height = (UnsignedInt)w[2]; break;
		case 4: g_string.m_width = w[1]; d.drawCinematicText(); break;
		}
		rec({100, d.m_cinematicTextFrames, d.m_cinematicText.getLength()});
	}
	TheDisplay = NULL;
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
