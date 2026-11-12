/* Differential-test wrapper for the popup message (GitHub #151, upstream commit 0a05454d), cut verbatim by build.rs into
** OUT_DIR/popup_message_gen:
**   pm_ui.inc    GameEngine/Source/GameClient/InGameUI.cpp, both InGameUI::popupMessage and clearPopupMessageData (from
**                the six-argument popupMessage to the three rules of dashes after clearPopupMessageData)
**   pm_cb.inc    GameEngine/Source/GameClient/GUI/GUICallbacks/InGamePopupMessage.cpp, from the statics (`static
**                NameKeyType parentID = NAMEKEY_INVALID;`) to `// PRIVATE FUNCTIONS`: InGamePopupMessageInit,
**                InGamePopupMessageInput, InGamePopupMessageSystem
** No edit. Compiled inside namespace pm on stand-ins that record every call (the same model as the Rust test's host,
** tests/differential_popup_message.rs):
** - InGameUI holds m_popupMessageData and m_popupMessageColor (its other members are not used); PopupMessageData is
**   InGameUI.h's members, newInstance / deleteInstance as new / delete;
** - the window manager holds InGamePopupMessage.wnd's three windows (0 the parent, 1 StaticTextMessage, 2 ButtonOk) once
**   winCreateLayout made the layout (a window the sequence leaves out is not found); winGetWindowFromId finds a window by
**   its key whatever the parent; the button's size and the static text's font (or none) are the sequence's; each
**   window's setter is recorded; winSendSystemMsg to the parent is recorded and given to InGamePopupMessageSystem (the
**   layout's system callback); runInit calls InGamePopupMessageInit (its init); NAMEKEY interns names in order of use;
** - DisplayString: getSize with setWordWrap(w): units * (font size / 2 + 1) wide (8 with no font), in lines of w
**   (at least one), each the font's size (12 with no font) high;
** - TheGameText->fetch: the label's bytes as units; TheDisplay: the sequence's size; setGamePaused,
**   UpdateDiplomacyBriefingText and TheMessageStream->appendMessage recorded.
**
** Entry point (extern "C"): ref_popup_message_seq(in, nin, out, cap). `in` is 8 words [display width, height,
** m_popupMessageColor, the windows present (bits 0..2), the static text's font size (0: none), the button's width,
** height, 0], then steps of 8 words [kind, a, b, c, d, e, f, g]:
**   kind 0  popupMessage(LABELS[a], x b, y c, width d, pause e, pauseMusic f)
**   kind 1  popupMessage(LABELS[a], x b, y c, width d, colour g, pause e, pauseMusic f)
**   kind 2  clearPopupMessageData()
**   kind 3  InGamePopupMessageInput(the parent, a ? 7 : GWM_CHAR, key b (0 KEY_ENTER, 1 KEY_ESC, 2 'A' 0x1E), state c)
**   kind 4  InGamePopupMessageSystem(the parent, msg a (0 GWM_CREATE, 1 GWM_DESTROY, 2 GWM_INPUT_FOCUS, 3 GBM_SELECTED,
**           4 other), the control: window b (0..2), mData1 c (GWM_INPUT_FOCUS))
** The calls, as they are made: [1, paused, music] setGamePaused, [2, label] UpdateDiplomacyBriefingText, [3]
** winCreateLayout, [4] runInit, [5] destroyWindows, [6, w, x, y] winSetPosition, [7, w, width, height] winSetSize, [8, w,
** colour, border] winSetEnabledTextColors, [9, w] winSetModal, [10, w] winSetFocus, [11, w, hide] winHide, [12, w]
** winBringToTop, [13, w, FNV-1a] GadgetStaticTextSetText, [14] MSG_CLEAR_INGAME_POPUP_MESSAGE appended, [15, w, control,
** id] winSendSystemMsg(GBM_SELECTED); a window is its index (-1 NULL). After each step [100, the data exists, then (if
** it does) x, y, width, colour, pause, pauseMusic, the layout exists, the message's FNV-1a; then pause, parentID,
** staticTextMessageID, buttonOkID, parent, staticTextMessage, buttonOk, the handled result (-1 none), the focus flag]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include <string>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/Debug.h"

namespace pm {

std::vector<int> g_out;
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }

typedef Int NameKeyType;
enum { NAMEKEY_INVALID = 0 };
typedef Int Color;
typedef size_t WindowMsgData;
enum WindowMsgHandledType { MSG_IGNORED, MSG_HANDLED };
enum { GWM_CREATE = 1, GWM_DESTROY = 2, GWM_CHAR = 21, GWM_INPUT_FOCUS = 23, GBM_SELECTED = 16392 };
enum { KEY_ESC = 0x01, KEY_ENTER = 0x1C };
enum { KEY_STATE_UP = 0x0001 };
#define BitTest( x, i ) ( ( (x) & (i) ) != 0 )
#ifndef INT_TO_REAL
#define INT_TO_REAL(x) ((Real)(x))
#endif

static Int fnv( const UnicodeString &t )
{
	UnsignedInt h = 2166136261u;
	for (Int i = 0; i < t.getLength(); ++i) {
		h ^= (UnsignedInt)(unsigned short)t.getCharAt(i);
		h *= 16777619u;
	}
	return (Int)h;
}

/* NAMEKEY: each new name the next key */
std::vector<std::string> g_keys;
struct NameKeyGeneratorStub
{
	NameKeyType nameToKey( const AsciiString &n )
	{
		for (size_t i = 0; i < g_keys.size(); ++i)
			if (g_keys[i] == n.str())
				return (NameKeyType)i + 1;
		g_keys.push_back(n.str());
		return (NameKeyType)g_keys.size();
	}
};
NameKeyGeneratorStub g_nkg;
NameKeyGeneratorStub *TheNameKeyGenerator = &g_nkg;

static const char *const NAMES[] = { "InGamePopupMessage.wnd:InGamePopupMessageParent", "InGamePopupMessage.wnd:StaticTextMessage", "InGamePopupMessage.wnd:ButtonOk" };

struct Config { Int width, height, color, present, fontSize, okWidth, okHeight; };
Config g_cfg;

struct GameFont { Int pointSize; Int height; };
GameFont g_font;

class GameWindow
{
public:
	Int m_index;
	void winSetPosition( Int x, Int y ) { rec({6, m_index, x, y}); }
	void winSetSize( Int w, Int h ) { rec({7, m_index, w, h}); }
	void winGetSize( Int *w, Int *h ) { *w = g_cfg.okWidth; *h = g_cfg.okHeight; }
	void winSetEnabledTextColors( Color c, Color b ) { rec({8, m_index, c, b}); }
	void winHide( Bool hide ) { rec({11, m_index, hide ? 1 : 0}); }
	void winBringToTop( void ) { rec({12, m_index}); }
	GameFont *winGetFont( void ) { return g_cfg.fontSize ? &g_font : NULL; }
	Int winGetWindowId( void ) { return TheNameKeyGenerator->nameToKey(AsciiString(NAMES[m_index])); }
};
static Int idx( GameWindow *w ) { return w ? w->m_index : -1; }
GameWindow g_windows[3];
Bool g_made = FALSE;

static void GadgetStaticTextSetText( GameWindow *w, UnicodeString text ) { rec({13, idx(w), fnv(text)}); }

class DisplayString
{
public:
	UnicodeString m_text;
	GameFont *m_font;
	Int m_wrap;
	DisplayString() : m_font(NULL), m_wrap(0) {}
	void setText( UnicodeString t ) { m_text = t; }
	void setFont( GameFont *f ) { m_font = f; }
	void setWordWrap( Int w ) { m_wrap = w; }
	void getSize( Int *width, Int *height )
	{
		Int unit = m_font ? m_font->pointSize / 2 + 1 : 8;
		Int line = m_font ? m_font->height : 12;
		Int total = m_text.getLength() * unit;
		Int lines = 1;
		if (m_wrap > 0 && total > m_wrap) lines = (total + m_wrap - 1) / m_wrap;
		*width = (m_wrap > 0 && total > m_wrap) ? m_wrap : total;
		*height = lines * line;
	}
};
struct DisplayStringManagerStub
{
	DisplayString *newDisplayString( void ) { return new DisplayString; }
	void freeDisplayString( DisplayString *d ) { delete d; }
};
DisplayStringManagerStub g_dsm;
DisplayStringManagerStub *TheDisplayStringManager = &g_dsm;

class WindowLayout;
void InGamePopupMessageInit( WindowLayout *layout, void *userData );
WindowMsgHandledType InGamePopupMessageSystem( GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2 );
class WindowLayout
{
public:
	void runInit( void *userData = NULL ) { rec({4}); InGamePopupMessageInit(this, userData); }
	void destroyWindows( void ) { rec({5}); g_made = FALSE; }
	void deleteInstance( void ) { }
};
WindowLayout g_layout;
struct WindowManagerStub
{
	GameWindow *winGetWindowFromId( GameWindow *parent, NameKeyType key )
	{
		if (!g_made)
			return NULL;
		for (Int i = 0; i < 3; ++i)
			if ((g_cfg.present & (1 << i)) && TheNameKeyGenerator->nameToKey(AsciiString(NAMES[i])) == key)
				return &g_windows[i];
		return NULL;
	}
	WindowLayout *winCreateLayout( AsciiString name ) { rec({3}); g_made = TRUE; return &g_layout; }
	void winSetModal( GameWindow *w ) { rec({9, idx(w)}); }
	void winSetFocus( GameWindow *w ) { rec({10, idx(w)}); }
	Int winSendSystemMsg( GameWindow *w, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2 )
	{
		rec({15, idx(w), idx((GameWindow *)d1), (Int)d2});
		if (w == &g_windows[0])
			return (Int)InGamePopupMessageSystem(w, msg, d1, d2);
		return 0;
	}
};
WindowManagerStub g_wm;
WindowManagerStub *TheWindowManager = &g_wm;

struct DisplayStub
{
	UnsignedInt getWidth( void ) { return (UnsignedInt)g_cfg.width; }
	UnsignedInt getHeight( void ) { return (UnsignedInt)g_cfg.height; }
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;
struct GameLogicStub { void setGamePaused( Bool paused, Bool pauseMusic = TRUE ) { rec({1, paused ? 1 : 0, pauseMusic ? 1 : 0}); } };
GameLogicStub g_logic;
GameLogicStub *TheGameLogic = &g_logic;
struct GameTextStub { UnicodeString fetch( AsciiString label ) { UnicodeString u; u.translate(label); return u; } };
GameTextStub g_text;
GameTextStub *TheGameText = &g_text;
struct GameMessage { enum Type { MSG_CLEAR_INGAME_POPUP_MESSAGE = 1088 }; };
struct MessageStreamStub { void appendMessage( GameMessage::Type ) { rec({14}); } };
MessageStreamStub g_stream;
MessageStreamStub *TheMessageStream = &g_stream;

static const char *const LABELS[] = { "SCRIPT:Pop", "SCRIPT:Brief1", "", "A much longer popup message that wraps over several lines of the box", "MISSING" };
static const Int NLABELS = 5;
static void UpdateDiplomacyBriefingText( AsciiString label, Bool clear )
{
	Int li = -1;
	for (Int i = 0; i < NLABELS; ++i)
		if (label == AsciiString(LABELS[i]))
			li = i;
	rec({2, li});
}

/* InGameUI.h's PopupMessageData, its members */
class PopupMessageData
{
public:
	UnicodeString		message;
	Int							x;
	Int							y;
	Int							width;
	Color						textColor;
	Bool						pause;
	Bool						pauseMusic;
	WindowLayout*	layout;
	void deleteInstance( void ) { delete this; }
};
#define newInstance( T ) new T

class InGameUI
{
public:
	PopupMessageData *m_popupMessageData;
	Color m_popupMessageColor;
	PopupMessageData *getPopupMessageData( void ) { return m_popupMessageData; }
	void clearPopupMessageData( void );
	void popupMessage( const AsciiString& message, Int x, Int y, Int width, Bool pause, Bool pauseMusic );
	void popupMessage( const AsciiString& identifier, Int x, Int y, Int width, Color textColor, Bool pause, Bool pauseMusic );
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

#include "pm_ui.inc"

#include "pm_cb.inc"

} /* namespace pm */

extern "C" int ref_popup_message_seq(const int *in, int nin, int *out, int cap)
{
	using namespace pm;
	g_out.clear();
	g_keys.clear();
	g_cfg.width = in[0]; g_cfg.height = in[1]; g_cfg.color = in[2]; g_cfg.present = in[3]; g_cfg.fontSize = in[4];
	g_cfg.okWidth = in[5]; g_cfg.okHeight = in[6];
	g_font.pointSize = in[4]; g_font.height = in[4];
	for (Int i = 0; i < 3; ++i)
		g_windows[i].m_index = i;
	g_made = FALSE;
	/* a new program: the statics and InGameUI's members as they start */
	g_ui.m_popupMessageData = NULL;
	g_ui.m_popupMessageColor = in[2];
	parentID = staticTextMessageID = buttonOkID = NAMEKEY_INVALID;
	parent = staticTextMessage = buttonOk = NULL;
	pm::pause = FALSE;
	for (int k = 8; k + 8 <= nin; k += 8) {
		const int *w = in + k + 1;
		Int handled = -1, focus = 1;
		switch (in[k]) {
		case 0: g_ui.popupMessage(AsciiString(LABELS[w[0] % NLABELS]), w[1], w[2], w[3], w[4] != 0, w[5] != 0); break;
		case 1: g_ui.popupMessage(AsciiString(LABELS[w[0] % NLABELS]), w[1], w[2], w[3], (Color)w[6], w[4] != 0, w[5] != 0); break;
		case 2: g_ui.clearPopupMessageData(); break;
		case 3: {
			static const Int KEYS[] = { KEY_ENTER, KEY_ESC, 0x1E };
			handled = (Int)InGamePopupMessageInput(&g_windows[0], w[0] ? 7 : GWM_CHAR, (WindowMsgData)KEYS[w[1] % 3], (WindowMsgData)w[2]);
			break;
		}
		case 4: {
			static const UnsignedInt MSGS[] = { GWM_CREATE, GWM_DESTROY, GWM_INPUT_FOCUS, GBM_SELECTED, 7 };
			UnsignedInt msg = MSGS[w[0] % 5];
			Bool f = FALSE;
			WindowMsgData d1 = msg == GBM_SELECTED ? (WindowMsgData)&g_windows[w[1] % 3] : (WindowMsgData)(w[2] != 0 ? 1 : 0);
			handled = (Int)InGamePopupMessageSystem(&g_windows[0], msg, d1, msg == GWM_INPUT_FOCUS ? (WindowMsgData)&f : 0);
			focus = f ? 1 : 0;
			break;
		}
		}
		PopupMessageData *d = g_ui.m_popupMessageData;
		rec({100, d ? 1 : 0});
		if (d)
			rec({d->x, d->y, d->width, d->textColor, d->pause ? 1 : 0, d->pauseMusic ? 1 : 0, d->layout ? 1 : 0, fnv(d->message)});
		rec({pm::pause ? 1 : 0, parentID, staticTextMessageID, buttonOkID, idx(parent), idx(staticTextMessage), idx(buttonOk), handled, focus});
	}
	if (g_ui.m_popupMessageData) {
		delete g_ui.m_popupMessageData;
		g_ui.m_popupMessageData = NULL;
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
