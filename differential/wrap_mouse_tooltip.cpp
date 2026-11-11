/* Differential-test wrapper for the mouse's tooltip (GitHub #124, upstream commit 0a05454d), cut verbatim by build.rs
** into OUT_DIR/mouse_tooltip_gen:
**   mt_color.inc     GameEngine/Include/GameClient/Color.h, `GameMakeColor` (to `GameGetColorComponents`);
**   mt_ctor.inc      GameEngine/Source/GameClient/Input/Mouse.cpp, `Mouse::Mouse( void )` (to `Mouse::~Mouse`);
**   mt_init.inc      Mouse.cpp, `Mouse::init` and `mouseNotifyResolutionChange` (to `Mouse::reset`);
**   mt_stream.inc    Mouse.cpp, `Mouse::createStreamMessages` and `setCursorTooltip` (to `Mouse::setMouseText`);
**   mt_draw.inc      Mouse.cpp, `Mouse::resetTooltipDelay` and `drawTooltip` (to `Mouse::drawCursorText`).
** No edit. They are compiled inside namespace mt on a stand-in `Mouse` declaring the members they use (Mouse.h's names
** and types), with stand-ins for the engine: `timeGetTime()` is the step's clock; the message stream counts its
** messages; `processMouseEvent(i)` puts the step's i-th event's movement in `m_currMouse.deltaPos`; the display string
** keeps its text, word wrap, font and clip region, measures itself as the Rust test does (6 pixels a character, 13 a
** line, wrapped at the word wrap) and records its draws; `TheDisplay` has the step's width and records its rectangles;
** `TheFontLibrary->getFont` and `TheGlobalLanguageData->adjustFontSize` hand back what they are given;
** `TheScriptEngine->getFade()` is the step's.
**
** Entry point (extern "C", tests/differential_mouse_tooltip.rs):
**   ref_mouse_tooltip_seq(in, nin, out, cap) runs one sequence on a new mouse and writes its words to out.
**   `in` is the setup, 16 words [font size, bold, animate, fill time, delay time, text colour, highlight colour, shadow
**   colour, background colour, border colour (r | g << 8 | b << 16 | a << 24), tooltip width (bits), use alt text, use
**   alt back, adjust alt, the language font (0 none, else its size), display width], then steps of 8 words
**   [kind, now, a, b, c, d, e, f]:
**     kind 0  setCursorTooltip(text a, delay b, c ? &rgb : NULL, width (bits e)); rgb from d (c 1: setFromInt(d),
**             c 2: d's three bytes as -1..2 reals (b - 85) / 85)
**     kind 1  resetTooltipDelay()
**     kind 2  createStreamMessages() with a events, the i-th moving when bit i of b is set (script debug c)
**     kind 3  drawTooltip() with the mouse at (a, b), m_maxX/Y (c, d), fade e
**     kind 4  mouseNotifyResolutionChange()
**     kind 5  the display's width a
**   texts: 0 "", 1 "Tank", 2 "Supply Warehouse: $1000\nPlayer 1", 3 sixty characters, 4 "A", 5 "Tank" again.
** After the setup and after each step: [200, m_isTooltipEmpty, m_tooltipDelay, m_lastTooltipWidth bits, m_tooltipTextColor,
** m_tooltipBackColor, m_stillTime, m_displayTooltip], [201, m_highlightPos, m_highlightUpdateStart, word wrap, text hash,
** text length, font size, font bold]; each draw: [210, 0 fill | 1 open, x, y, w, h, colour, 0],
** [211, x, y, colour, drop colour, clip lo.x, lo.y, hi.x], [212, clip hi.y, 0...]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <new>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"

extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
}
/* BaseType.h line 231 (the shim leaves the asm helpers to differential/wrap_base_type.cpp) */
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))

namespace mt {

std::vector<int> g_out;
void rec(Int tag, Int a = 0, Int b = 0, Int c = 0, Int d = 0, Int e = 0, Int f = 0, Int g = 0)
{
	Int w[8] = { tag, a, b, c, d, e, f, g };
	g_out.insert(g_out.end(), w, w + 8);
}

typedef Int Color;
#include "mt_color.inc"

template <class T> T min(T a, T b) { return a < b ? a : b; }

UnsignedInt g_now;
UnsignedInt timeGetTime() { return g_now; }

/* ---- the display string, the display, the fonts ---- */
struct GameFont { AsciiString name; Int size; Bool bold; };
GameFont g_font;
struct FontLibrary { GameFont *getFont(AsciiString name, Int size, Bool bold) { g_font.name = name; g_font.size = size; g_font.bold = bold; return &g_font; } };
FontLibrary g_fonts;
FontLibrary *TheFontLibrary = &g_fonts;
struct FontDesc { AsciiString name; Int size; Bool bold; };
struct GlobalLanguage { FontDesc m_tooltipFontName; Int adjustFontSize(Int s) { return s; } };
GlobalLanguage g_lang;
GlobalLanguage *TheGlobalLanguageData = &g_lang;

/* the Rust test's measure: 6 pixels a character, 13 a line, wrapped at the word wrap */
void measure(const UnicodeString &t, Int wrap, Int *w, Int *h)
{
	Int ww = t.getLength() * 6;
	*h = 13;
	if (wrap > 0 && ww > wrap) {
		*h = 13 * ((ww + wrap - 1) / wrap);
		ww = wrap;
	}
	*w = ww;
}

struct DisplayString {
	UnicodeString m_text;
	Int m_wrap;
	GameFont *m_font;
	IRegion2D m_clip;
	DisplayString() : m_wrap(0), m_font(NULL) { m_clip.lo.x = m_clip.lo.y = m_clip.hi.x = m_clip.hi.y = 0; }
	void setText(UnicodeString t) { m_text = t; }
	UnicodeString getText() { return m_text; }
	Int getTextLength() { return m_text.getLength(); }
	void setWordWrap(Int w) { m_wrap = w; }
	void setFont(GameFont *f) { m_font = f; }
	void getSize(Int *w, Int *h) { measure(m_text, m_wrap, w, h); }
	void setClipRegion(IRegion2D *r) { m_clip = *r; }
	void draw(Int x, Int y, Color c, Color d)
	{
		rec(211, x, y, c, d, m_clip.lo.x, m_clip.lo.y, m_clip.hi.x);
		rec(212, m_clip.hi.y);
	}
};
DisplayString g_strings[4];
Int g_nextString;
struct DisplayStringManager {
	DisplayString *newDisplayString() { DisplayString *s = &g_strings[g_nextString++ & 3]; new (s) DisplayString(); return s; }
	void freeDisplayString(DisplayString *) {}
};
DisplayStringManager g_dsm;
DisplayStringManager *TheDisplayStringManager = &g_dsm;

struct DisplayStub {
	UnsignedInt m_width;
	UnsignedInt getWidth() { return m_width; }
	UnsignedInt getHeight() { return 600; }
	void drawFillRect(Int x, Int y, Int w, Int h, Color c) { rec(210, 0, x, y, w, h, c); }
	void drawOpenRect(Int x, Int y, Int w, Int h, Real, Color c) { rec(210, 1, x, y, w, h, c); }
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;

struct ScriptEngine { enum { FADE_NONE = 0, FADE_SUBTRACT, FADE_ADD }; Int m_fade; Int getFade() { return m_fade; } };
ScriptEngine g_script;
ScriptEngine *TheScriptEngine = &g_script;

struct GlobalData { Bool m_winCursors; Bool m_scriptDebug; };
GlobalData g_gd;
GlobalData *TheGlobalData = &g_gd;

/* ---- the message stream (its messages are not compared: the tooltip timer is) ---- */
struct GameMessage {
	enum Type {
		MSG_RAW_MOUSE_POSITION, MSG_RAW_MOUSE_LEFT_BUTTON_DOWN, MSG_RAW_MOUSE_LEFT_DOUBLE_CLICK, MSG_RAW_MOUSE_LEFT_BUTTON_UP,
		MSG_RAW_MOUSE_LEFT_DRAG, MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN, MSG_RAW_MOUSE_MIDDLE_DOUBLE_CLICK,
		MSG_RAW_MOUSE_MIDDLE_BUTTON_UP, MSG_RAW_MOUSE_MIDDLE_DRAG, MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN,
		MSG_RAW_MOUSE_RIGHT_DOUBLE_CLICK, MSG_RAW_MOUSE_RIGHT_BUTTON_UP, MSG_RAW_MOUSE_RIGHT_DRAG, MSG_RAW_MOUSE_WHEEL
	};
	void appendPixelArgument(ICoord2D) {}
	void appendIntegerArgument(Int) {}
};
GameMessage g_msg;
struct MessageStream { GameMessage *appendMessage(GameMessage::Type) { return &g_msg; } };
MessageStream g_stream;
MessageStream *TheMessageStream = &g_stream;
struct Keyboard { Int getModifierFlags() { return 0; } };
Keyboard g_kb;
Keyboard *TheKeyboard = &g_kb;
enum { GWM_NONE = 0, GWM_LEFT_DOWN, GWM_LEFT_UP, GWM_LEFT_DOUBLE_CLICK, GWM_LEFT_DRAG, GWM_MIDDLE_DOWN, GWM_MIDDLE_UP,
	GWM_MIDDLE_DOUBLE_CLICK, GWM_MIDDLE_DRAG, GWM_RIGHT_DOWN, GWM_RIGHT_UP, GWM_RIGHT_DOUBLE_CLICK, GWM_RIGHT_DRAG };

/* Mouse.h's MouseButtonState and MouseIO */
enum MouseButtonState { MBS_Up = 0, MBS_Down, MBS_DoubleClick };
struct MouseIO {
	ICoord2D pos; UnsignedInt time; Int wheelPos; ICoord2D deltaPos;
	MouseButtonState leftState; Int leftEvent; Int leftFrame;
	MouseButtonState rightState; Int rightEvent; Int rightFrame;
	MouseButtonState middleState; Int middleEvent; Int middleFrame;
};

UnsignedInt g_moves;

class Mouse
{
public:
	enum MouseCursor { NONE = 0, ARROW = 2 };
	enum RedrawMode { RM_WINDOWS = 0, RM_W3D };
	Mouse( void );
	void init( void );
	void mouseNotifyResolutionChange( void );
	void createStreamMessages( void );
	void setCursorTooltip( UnicodeString tooltip, Int tooltipDelay = -1, const RGBColor *color = NULL, Real width = 1.0f );
	void resetTooltipDelay( void );
	void drawTooltip( void );
	/* the step's event i moves the mouse when bit i is set */
	void processMouseEvent( Int i ) { m_currMouse.deltaPos.x = (g_moves >> i) & 1; m_currMouse.deltaPos.y = 0; m_currMouse.leftEvent = m_currMouse.rightEvent = m_currMouse.middleEvent = GWM_NONE; m_currMouse.wheelPos = 0; }

	AsciiString m_tooltipFontName; Int m_tooltipFontSize; Bool m_tooltipFontIsBold; Bool m_tooltipAnimateBackground;
	Int m_tooltipFillTime; Int m_tooltipDelayTime; Real m_tooltipWidth; Real m_lastTooltipWidth;
	RGBAColorInt m_tooltipColorText, m_tooltipColorHighlight, m_tooltipColorShadow, m_tooltipColorBackground, m_tooltipColorBorder;
	RedrawMode m_currentRedrawMode;
	Bool m_useTooltipAltTextColor; Bool m_useTooltipAltBackColor; Bool m_adjustTooltipAltColor;
	Bool m_orthoCamera; Real m_orthoZoom;
	UnsignedInt m_dragTolerance, m_dragTolerance3D, m_dragToleranceMS;
	UnsignedByte m_numButtons; UnsignedByte m_numAxes; Bool m_forceFeedback;
	UnicodeString m_tooltipString; DisplayString *m_tooltipDisplayString; Bool m_displayTooltip; Bool m_isTooltipEmpty;
	enum { NUM_MOUSE_EVENTS = 256 };
	MouseIO m_mouseEvents[ NUM_MOUSE_EVENTS ]; MouseIO m_currMouse; MouseIO m_prevMouse;
	Int m_minX, m_maxX, m_minY, m_maxY;
	UnsignedInt m_inputFrame, m_deadInputFrame; Bool m_inputMovesAbsolute; Bool m_visible;
	MouseCursor m_currentCursor;
	DisplayString *m_cursorTextDisplayString; RGBAColorInt m_cursorTextColor, m_cursorTextDropColor;
	Int m_tooltipDelay; Int m_highlightPos; UnsignedInt m_highlightUpdateStart; UnsignedInt m_stillTime;
	RGBAColorInt m_tooltipTextColor; RGBAColorInt m_tooltipBackColor;
	Int m_eventsThisFrame;
};

#include "mt_ctor.inc"
#include "mt_init.inc"
#include "mt_stream.inc"
#include "mt_draw.inc"

Int packColor(const RGBAColorInt &c) { return (Int)(c.red | (c.green << 8) | (c.blue << 16) | (c.alpha << 24)); }
RGBAColorInt unpackColor(Int w)
{
	RGBAColorInt c;
	c.red = w & 0xFF; c.green = (w >> 8) & 0xFF; c.blue = (w >> 16) & 0xFF; c.alpha = ((UnsignedInt)w >> 24) & 0xFF;
	return c;
}

const char *TEXTS[6] = { "", "Tank", "Supply Warehouse: $1000\nPlayer 1",
	"A tooltip of sixty characters, long enough to wrap twice!!!", "A", "Tank" };

void state(Mouse *m)
{
	Int bits;
	memcpy(&bits, &m->m_lastTooltipWidth, 4);
	rec(200, m->m_isTooltipEmpty, m->m_tooltipDelay, bits, packColor(m->m_tooltipTextColor), packColor(m->m_tooltipBackColor), (Int)m->m_stillTime, m->m_displayTooltip);
	DisplayString *s = m->m_tooltipDisplayString;
	UnsignedInt h = 2166136261u;
	for (const WideChar *c = s->m_text.str(); *c; ++c) {
		h ^= (UnsignedInt)*c;
		h *= 16777619u;
	}
	rec(201, m->m_highlightPos, (Int)m->m_highlightUpdateStart, s->m_wrap, (Int)h, s->m_text.getLength(), s->m_font ? s->m_font->size : -1, s->m_font ? (Int)s->m_font->bold : -1);
}

alignas(Mouse) unsigned char g_buf[sizeof(Mouse)];

}  // namespace mt

using namespace mt;

extern "C" int ref_mouse_tooltip_seq(const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	g_now = 0;
	g_nextString = 0;
	g_script.m_fade = ScriptEngine::FADE_NONE;
	g_gd.m_winCursors = TRUE;
	g_gd.m_scriptDebug = FALSE;
	if (nin < 16)
		return 0;
	g_display.m_width = (UnsignedInt)in[15];
	g_lang.m_tooltipFontName.name = in[14] ? "Language" : "";
	g_lang.m_tooltipFontName.size = in[14];
	g_lang.m_tooltipFontName.bold = TRUE;
	Mouse *m = new (g_buf) Mouse();
	/* parseIni's Mouse block */
	m->m_tooltipFontName = "Arial";
	m->m_tooltipFontSize = in[0];
	m->m_tooltipFontIsBold = in[1] != 0;
	m->m_tooltipAnimateBackground = in[2] != 0;
	m->m_tooltipFillTime = in[3];
	m->m_tooltipDelayTime = in[4];
	m->m_tooltipColorText = unpackColor(in[5]);
	m->m_tooltipColorHighlight = unpackColor(in[6]);
	m->m_tooltipColorShadow = unpackColor(in[7]);
	m->m_tooltipColorBackground = unpackColor(in[8]);
	m->m_tooltipColorBorder = unpackColor(in[9]);
	memcpy(&m->m_tooltipWidth, &in[10], 4);
	m->m_useTooltipAltTextColor = in[11] != 0;
	m->m_useTooltipAltBackColor = in[12] != 0;
	m->m_adjustTooltipAltColor = in[13] != 0;
	m->init();
	state(m);
	for (int s = 16; s + 8 <= nin; s += 8) {
		const int *w = in + s;
		g_now = (UnsignedInt)w[1];
		switch (w[0]) {
		case 0: {
			const char *t = TEXTS[w[2] % 6];
			std::vector<WideChar> u;
			for (const char *c = t; *c; ++c)
				u.push_back((WideChar)(unsigned char)*c);
			u.push_back(0);
			RGBColor rgb;
			if (w[4] == 1) {
				rgb.setFromInt(w[5]);
			} else {
				rgb.red = (Real)((w[5] & 0xFF) - 85) / 85.0f;
				rgb.green = (Real)(((w[5] >> 8) & 0xFF) - 85) / 85.0f;
				rgb.blue = (Real)(((w[5] >> 16) & 0xFF) - 85) / 85.0f;
			}
			Real width;
			memcpy(&width, &w[6], 4);
			m->setCursorTooltip(UnicodeString(&u[0]), w[3], w[4] ? &rgb : NULL, width);
			break;
		}
		case 1: m->resetTooltipDelay(); break;
		case 2:
			m->m_eventsThisFrame = w[2];
			g_moves = (UnsignedInt)w[3];
			g_gd.m_scriptDebug = w[4] != 0;
			m->createStreamMessages();
			g_gd.m_scriptDebug = FALSE;
			break;
		case 3:
			m->m_currMouse.pos.x = w[2];
			m->m_currMouse.pos.y = w[3];
			m->m_maxX = w[4];
			m->m_maxY = w[5];
			g_script.m_fade = w[6];
			m->drawTooltip();
			g_script.m_fade = ScriptEngine::FADE_NONE;
			break;
		case 4: m->mouseNotifyResolutionChange(); break;
		case 5: g_display.m_width = (UnsignedInt)w[2]; break;
		}
		state(m);
	}
	m->~Mouse();
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
