/* Differential-test program for the window manager and the gadgets (GitHub #70, upstream commit 0a05454d).
**
** build.rs copies these upstream files VERBATIM into OUT_DIR/wm_gen (each cut from its `#include "PreRTS.h"` to the
** end, the marker asserted once) and compiles them with this file into one program, `wm_ref`:
**   GameEngine/Source/GameClient/GUI/GameWindowManager.cpp, GameWindow.cpp, WinInstanceData.cpp, GameWindowGlobal.cpp,
**   and Gadget/Gadget{CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton, StaticText,
**   TabControl, TextEntry, VerticalSlider}.cpp.
** Five edits, each asserted to occur once, are what MSVC 6 accepts and g++ does not (no change of meaning):
**   GadgetListBox.cpp: `for(j = 0; j < TheKeyboard->MAX_KEY_STATES; ++j)` reuses the `j` of an earlier `for` (MSVC 6's
**   `for` scope): it declares its own `Int j`; GLM_SET_SELECTION's `for (Int i=0; ...)` is followed by a use of `i`:
**   `Int i;` is declared before the loop; GLM_SCROLL_BUFFER's second loop `for(i = 0; ...)` declares `Int i`;
**   `UnicodeString.TheEmptyString` (GadgetListBox.cpp, GadgetComboBox.cpp) is spelled `UnicodeString::TheEmptyString`.
** The headers are upstream's own (GameEngine/Include), except the stand-ins in differential/shim_wm (each says what it
** keeps) and differential/shim (the base types, the strings, the memory pool glue: NEW zero-fills, as the game's
** allocator does).
**
** 64-bit: upstream passes pointers in 32-bit message data (`(WindowMsgData)window`, `&addInfo`, `&wantsFocus`). The
** program is linked without PIE, the C library's heap kept in `brk` (mallopt), and the case run on a stack allocated
** from that heap (ucontext): every pointer upstream casts is below 4 GB and survives the round trip. -fpermissive
** turns the narrowing casts into warnings.
**
** Everything below is a STAND-IN, written for this test:
** - TheWindowManager is a TestWindowManager: allocateNewWindow makes a TestGameWindow, recorded in creation order (its
**   index is the window's number in the dumps, as the Rust's WinId); the draw function getters return a draw function
**   that draws nothing (drawing is not compared); TestGameWindow::winDrawBorder draws nothing.
** - DisplayString's out-of-line members (DisplayString.cpp: the constructor, setText, reset, removeLastChar,
**   appendChar), and TestDisplayString's size: `height` = the font's height (12 without a font), width = units *
**   max(height / 2, 1), word wrapped at the wrap width into ceil(width / wrap) lines (the Rust test host's measure).
** - TheFontLibrary: one font per (name, size, bold), height = point size. TheMappedImageCollection: `Img*` names are
**   images 5x10 (`*Mid` 7x10). TheGameText: `T:` + the label. TheNameKeyGenerator: keys 1, 2, ... in order of use.
** - TheAudio, TheMouse's setCursorTooltip, TheInGameUI, TheSelectionTranslator, TheTacticalView: recorded. TheKeyboard:
**   getPrintableKey(key, 1) is the key itself, the other states 0. TheIMEManager: never attached. timeGetTime: the
**   case's clock. GetDoubleClickTime: 500.
** - The host's own callbacks: `TestSystem` (handles everything, does not want the focus) and `TestInput` (ignores
**   everything) on the user windows; both record the message.
**
** #72 added: the combo box, text entry, tab control, progress bar and static text made by their gogoGadget* calls,
** every gadget made with `defaultVisual` as the operation says (the combo box's EntryData and ListboxData made as
** createGadget makes them, with NEW: gogoGadgetComboBox deletes both); the operations on them (GWM_IME_CHAR, the
** GadgetTextEntry / GadgetComboBox / GadgetTabControl functions); and in the dump every window's text colours, the
** draw data of the windows each `W` made, and the entries', combo boxes' and tab controls' data. Each call is made
** before its result is logged, so that the messages it sends come first in the log.
**
** Input (stdin): one operation per line (see tests/differential_wm.rs, which writes them and runs the same ones on the
** Rust). Output (stdout): after each operation, `#` and its number, the messages the host recorded, then a dump of the
** windows and the manager's state, in the format the Rust writes. A crash ends the output early (the Rust test
** reports it). */
#include "PreRTS.h"
#include <malloc.h>
#include <strings.h>
#include <ucontext.h>
#include <string>
#include <vector>
#include <map>
#include <sstream>
#include <iostream>
#include "GameClient/GameWindow.h"
#include "GameClient/GameWindowManager.h"
#include "GameClient/Gadget.h"
#include "GameClient/GadgetListBox.h"
#include "GameClient/GadgetSlider.h"
#include "GameClient/GadgetPushButton.h"
#include "GameClient/GadgetCheckBox.h"
#include "GameClient/GadgetRadioButton.h"
#include "GameClient/GadgetStaticText.h"
#include "GameClient/GadgetProgressBar.h"
#include "GameClient/GadgetTextEntry.h"
#include "GameClient/GadgetComboBox.h"
#include "GameClient/GadgetTabControl.h"
#include "GameClient/DisplayStringManager.h"
#include "GameClient/GameFont.h"
#include "GameClient/Image.h"
#include "GameClient/Display.h"
#include "GameClient/Mouse.h"
#include "GameClient/Keyboard.h"
#include "GameClient/IMEManager.h"
#include "GameClient/GameWindowTransitions.h"
#include "GameClient/GlobalLanguage.h"
#include "Common/NameKeyGenerator.h"
#include "Common/GameAudio.h"
#include "Common/Language.h"

const UnicodeString UnicodeString::TheEmptyString;
const AsciiString AsciiString::TheEmptyString;

/* ---- the log */
static std::ostringstream g_log;
static unsigned int g_time = 0;
static std::vector<GameWindow *> g_windows;
/* the first window whose draw data the dump writes: the ones the last 'W' made */
static int g_dd_from = 1 << 30;
static std::vector<int> g_freed;

static int widx(const void *w)
{
	for (size_t i = 0; i < g_windows.size(); ++i)
		if (g_windows[i] == w) return (int)i;
	return -1;
}
/* a window as message data: its index + 1, 0 for NULL (the Rust's win_data) */
static unsigned int wdata(UnsignedInt d)
{
	if (d == 0) return 0;
	for (size_t i = 0; i < g_windows.size(); ++i)
		if (!g_freed[i] && (UnsignedInt)(uintptr_t)g_windows[i] == d) return (unsigned int)i + 1;
	return 0xFFFFFFFFu;
}

/* ---- Win32 */
unsigned int GetDoubleClickTime(void) { return 500; }
unsigned int timeGetTime(void) { return g_time; }

/* ---- the subsystems */
void AudioManager::addAudioEvent(const AudioEventRTS *e) { g_log << "audio " << e->m_name.str() << "\n"; }
static AudioManager s_audio;
AudioManager *TheAudio = &s_audio;

static std::map<std::string, NameKeyType> s_keys;
NameKeyType NameKeyGenerator::nameToKey(const AsciiString &name) { return nameToKey(name.str()); }
NameKeyType NameKeyGenerator::nameToKey(const char *name)
{
	std::map<std::string, NameKeyType>::iterator it = s_keys.find(name);
	if (it != s_keys.end()) return it->second;
	NameKeyType k = (NameKeyType)s_keys.size() + 1;
	s_keys[name] = k;
	return k;
}
static NameKeyGenerator s_nkg;
NameKeyGenerator *TheNameKeyGenerator = &s_nkg;

static Display s_display;
Display *TheDisplay = &s_display;

void Mouse::setCursorTooltip(UnicodeString tooltip, Int tooltipDelay, const RGBColor *, Real)
{
	m_tooltip = tooltip;
	m_delay = tooltipDelay;
}
static Mouse s_mouse;
Mouse *TheMouse = &s_mouse;
static Keyboard s_keyboard;
Keyboard *TheKeyboard = &s_keyboard;
void InGameUI::setSelecting(Bool) { g_log << "block\n"; }
void InGameUI::endAreaSelectHint(const GameMessage *) {}
static InGameUI s_igui;
InGameUI *TheInGameUI = &s_igui;
void SelectionTranslator::setDragSelecting(Bool) {}
void SelectionTranslator::setLeftMouseButton(Bool) {}
static SelectionTranslator s_sel;
SelectionTranslator *TheSelectionTranslator = &s_sel;
void View::setMouseLock(Bool) {}
static View s_view;
View *TheTacticalView = &s_view;
static IMEManagerInterface s_ime;
IMEManagerInterface *TheIMEManager = &s_ime;
static GameWindowTransitionsHandler s_trans;
GameWindowTransitionsHandler *TheTransitionHandler = &s_trans;
GlobalLanguage *TheGlobalLanguageData = NULL;
LanguageID OurLanguage = LANGUAGE_ID_US;

/* ---- display strings */
DisplayString::DisplayString(void) { m_font = NULL; m_next = NULL; m_prev = NULL; }
DisplayString::~DisplayString(void) {}
void DisplayString::setText(UnicodeString text)
{
	if (text == m_textString) return;
	m_textString = text;
	notifyTextChanged();
}
void DisplayString::reset(void) { m_textString.clear(); m_font = NULL; }
void DisplayString::removeLastChar(void)
{
	Int n = m_textString.getLength();
	if (n > 0)
	{
		std::vector<WideChar> u;
		for (Int i = 0; i + 1 < n; ++i) u.push_back(m_textString.getCharAt(i));
		m_textString.setUnits(u.empty() ? NULL : &u[0], (int)u.size());
	}
	notifyTextChanged();
}
void DisplayString::appendChar(WideChar c) { m_textString.concat(c); notifyTextChanged(); }

class TestDisplayString : public DisplayString
{
public:
	Int m_wrap;
	TestDisplayString() : m_wrap(0) {}
	virtual void setWordWrap(Int w) { m_wrap = w; }
	virtual void setWordWrapCentered(Bool) {}
	virtual void draw(Int, Int, Color, Color) {}
	virtual void draw(Int, Int, Color, Color, Int, Int) {}
	virtual void getSize(Int *width, Int *height)
	{
		Int h = m_font ? (m_font->height > 1 ? m_font->height : 1) : 12;
		Int half = h / 2 > 1 ? h / 2 : 1;
		Int w = m_textString.getLength() * half;
		if (m_wrap > 0 && w > m_wrap) { if (width) *width = m_wrap; if (height) *height = h * ((w + m_wrap - 1) / m_wrap); return; }
		if (width) *width = w;
		if (height) *height = h;
	}
	virtual Int getWidth(Int) { Int w; getSize(&w, NULL); return w; }
	virtual void setUseHotkey(Bool, Color) {}
};
DisplayString *DisplayStringManager::newDisplayString(void) { return new TestDisplayString; }
void DisplayStringManager::freeDisplayString(DisplayString *s) { if (s) s->deleteInstance(); }
static DisplayStringManager s_dsm;
DisplayStringManager *TheDisplayStringManager = &s_dsm;

/* ---- fonts, images (Image.cpp's and GameFont.cpp's members the headers declare: stand-ins) */
FontLibrary::FontLibrary(void) { m_fontList = NULL; m_count = 0; }
FontLibrary::~FontLibrary(void) {}
void FontLibrary::init(void) {}
void FontLibrary::reset(void) {}
GameFont *FontLibrary::getFont(AsciiString name, Int pointSize, Bool bold)
{
	for (GameFont *f = m_fontList; f; f = f->next)
		if (f->nameString.compare(name.str()) == 0 && f->pointSize == pointSize && f->bold == bold) return f;
	GameFont *f = newInstance(GameFont);
	f->nameString = name;
	f->pointSize = pointSize;
	f->height = pointSize;
	f->fontData = NULL;
	f->bold = bold;
	f->next = m_fontList;
	m_fontList = f;
	++m_count;
	return f;
}
class TestFontLibrary : public FontLibrary
{
	virtual Bool loadFontData(GameFont *) { return TRUE; }
};
static TestFontLibrary s_fonts;
FontLibrary *TheFontLibrary = &s_fonts;

Image::Image(void) {}
Image::~Image(void) {}
ImageCollection::ImageCollection(void) {}
ImageCollection::~ImageCollection(void) {}
/* `Img*` names are images, 5x10 (`*Mid` 7x10), as the Rust test host's */
const Image *ImageCollection::findImageByName(const AsciiString &name)
{
	const char *n = name.str();
	if (strncasecmp(n, "img", 3) != 0) return NULL;
	for (std::map<unsigned, Image *>::iterator i = m_imageMap.begin(); i != m_imageMap.end(); ++i)
		if (strcmp(i->second->getName().str(), n) == 0) return i->second;
	Image *im = newInstance(Image);
	im->setName(name);
	size_t len = strlen(n);
	ICoord2D sz = { (len >= 3 && strcmp(n + len - 3, "Mid") == 0) ? 7 : 5, 10 };
	im->setImageSize(&sz);
	unsigned k = (unsigned)m_imageMap.size();
	m_imageMap[k] = im;
	return im;
}
static ImageCollection s_images;
ImageCollection *TheMappedImageCollection = &s_images;

void GameGetColorComponents(Color color, UnsignedByte *red, UnsignedByte *green, UnsignedByte *blue, UnsignedByte *alpha)
{
	*alpha = (color & 0xFF000000) >> 24;
	*red = (color & 0x00FF0000) >> 16;
	*green = (color & 0x0000FF00) >> 8;
	*blue = (color & 0x000000FF);
}

/* WindowLayout.cpp: no layout is made here */
void WindowLayout::addWindow(GameWindow *) { abort(); }
void WindowLayout::removeWindow(GameWindow *) { abort(); }

/* ---- Color.cpp's GameDarkenColor, transcribed (GameWindow.cpp's grey and darkened images) */
Color GameDarkenColor(Color color, Int percent)
{
	if (percent >= 90 || percent <= 0)
		return color;
	UnsignedByte r, g, b, a;
	GameGetColorComponents(color, &r, &g, &b, &a);
	r -= (r * percent / 100);
	g -= (g * percent / 100);
	b -= (b * percent / 100);
	return GameMakeColor(r, g, b, a);
}

/* ---- GameWindowManagerScript.cpp's members (the reader is #12's differential; never reached here) */
void GameWindowManager::freeStaticStrings(void) {}
GameWindow *GameWindowManager::winCreateFromScript(AsciiString, WindowLayoutInfo *) { abort(); }
WindowLayout *GameWindowManager::winCreateLayout(AsciiString) { abort(); }

/* ---- the manager and windows */
static WindowMsgHandledType TestSystem(GameWindow *window, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2);
static WindowMsgHandledType TestInput(GameWindow *window, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2);
static void NoDraw(GameWindow *, WinInstanceData *) {}

class TestGameWindow : public GameWindow
{
public:
	virtual void winDrawBorder(void) {}
	virtual ~TestGameWindow()
	{
		int i = widx(this);
		if (i >= 0) g_freed[i] = 1;
	}
};

class TestWindowManager : public GameWindowManager
{
public:
	virtual GameWindow *allocateNewWindow(void)
	{
		GameWindow *w = newInstance(TestGameWindow);
		g_windows.push_back(w);
		g_freed.push_back(0);
		return w;
	}
#define D(n) virtual GameWinDrawFunc n(void) { return NoDraw; }
	D(getPushButtonImageDrawFunc) D(getPushButtonDrawFunc) D(getCheckBoxImageDrawFunc) D(getCheckBoxDrawFunc)
	D(getRadioButtonImageDrawFunc) D(getRadioButtonDrawFunc) D(getTabControlImageDrawFunc) D(getTabControlDrawFunc)
	D(getListBoxImageDrawFunc) D(getListBoxDrawFunc) D(getComboBoxImageDrawFunc) D(getComboBoxDrawFunc)
	D(getHorizontalSliderImageDrawFunc) D(getHorizontalSliderDrawFunc) D(getVerticalSliderImageDrawFunc)
	D(getVerticalSliderDrawFunc) D(getProgressBarImageDrawFunc) D(getProgressBarDrawFunc) D(getStaticTextImageDrawFunc)
	D(getStaticTextDrawFunc) D(getTextEntryImageDrawFunc) D(getTextEntryDrawFunc)
#undef D
	GameWindow *focus() { return m_keyboardFocus; }
	GameWindow *captor() { return m_mouseCaptor; }
	GameWindow *grab() { return m_grabWindow; }
	GameWindow *rgn() { return m_currMouseRgn; }
	GameWindow *lone() { return m_loneWindow; }
	ModalWindow *modal() { return m_modalHead; }
	GameWindow *list() { return m_windowList; }
};

/* which messages carry a window as mData1 (logged as its index + 1) */
static bool window_in_d1(UnsignedInt msg)
{
	switch (msg)
	{
	case GGM_LEFT_DRAG: case GBM_MOUSE_ENTERING: case GBM_MOUSE_LEAVING: case GBM_SELECTED: case GBM_SELECTED_RIGHT:
	case GSM_SLIDER_TRACK: case GSM_SLIDER_DONE: case GLM_SELECTED: case GLM_DOUBLE_CLICKED: case GLM_RIGHT_CLICKED:
	case GEM_EDIT_DONE: case GEM_UPDATE_TEXT: case GCM_SELECTED: case GCM_EDIT_DONE: case GCM_UPDATE_TEXT:
		return true;
	default:
		return false;
	}
}

static WindowMsgHandledType TestSystem(GameWindow *window, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2)
{
	g_log << "sys " << widx(window) << " " << msg << " " << (window_in_d1(msg) ? wdata(d1) : d1);
	if (msg == GLM_RIGHT_CLICKED)
	{
		RightClickStruct *rc = (RightClickStruct *)(uintptr_t)d2;
		g_log << " rc " << rc->pos << " " << rc->mouseX << " " << rc->mouseY;
	}
	else if (msg == GGM_FOCUS_CHANGE || msg == GSM_SLIDER_TRACK || msg == GSM_SLIDER_DONE || msg == GLM_SELECTED || msg == GLM_DOUBLE_CLICKED)
		g_log << " " << d2;
	g_log << "\n";
	return MSG_HANDLED;
}
static WindowMsgHandledType TestInput(GameWindow *window, UnsignedInt msg, WindowMsgData d1, WindowMsgData)
{
	if (msg == GWM_MOUSE_POS) return MSG_IGNORED;
	g_log << "inp " << widx(window) << " " << msg << " " << d1 << "\n";
	return MSG_IGNORED;
}

/* ---- the dump */
static void dump_text(std::ostringstream &o, const UnicodeString &t)
{
	o << " [";
	for (Int i = 0; i < t.getLength(); ++i) o << (i ? "," : "") << (unsigned)t.getCharAt(i);
	o << "]";
}

static void dump(std::ostringstream &o, TestWindowManager *wm)
{
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) { o << "w" << i << " freed\n"; continue; }
		GameWindow *w = g_windows[i];
		Int x, y, sx, sy;
		w->winGetPosition(&x, &y);
		w->winGetSize(&sx, &sy);
		WinInstanceData *d = w->winGetInstanceData();
		o << "w" << i << " st " << w->winGetStatus() << " state " << d->getState() << " style " << d->getStyle() << " at " << x << " " << y << " size " << sx << " " << sy;
		o << " parent " << widx(w->winGetParent()) << " owner " << widx(w->winGetOwner()) << " kids";
		for (GameWindow *c = w->winGetChild(); c; c = c->winGetNext()) o << " " << widx(c);
		dump_text(o, w->winGetText());
		o << " tx " << w->winGetEnabledTextColor() << "/" << w->winGetEnabledTextBorderColor() << " " << w->winGetDisabledTextColor() << "/" << w->winGetDisabledTextBorderColor() << " " << w->winGetHiliteTextColor() << "/" << w->winGetHiliteTextBorderColor();
		o << "\n";
		if ((int)i >= g_dd_from)
		{
			/* the draw data of the windows the operation made (#72: assignDefaultGadgetLook) */
			o << " dd";
			for (int st = 0; st < 3; ++st)
				for (int k = 0; k < MAX_DRAW_DATA; ++k)
				{
					const WinDrawData *dd = st == 0 ? &d->m_enabledDrawData[k] : st == 1 ? &d->m_disabledDrawData[k] : &d->m_hiliteDrawData[k];
					o << " " << (dd->image ? dd->image->getName().str() : "-") << "/" << dd->color << "/" << dd->borderColor;
				}
			o << "\n";
		}
		Int style = d->getStyle();
		void *ud = w->winGetUserData();
		if (BitTest(style, GWS_SCROLL_LISTBOX) && ud)
		{
			ListboxData *l = (ListboxData *)ud;
			o << " lb len " << l->listLength << " cols " << l->columns << " cw";
			for (Int c = 0; c < l->columns && l->columnWidth; ++c) o << " " << l->columnWidth[c];
			o << " end " << l->endPos << " ins " << l->insertPos << " sel " << l->selectPos << " dh " << l->displayHeight << " dp " << l->displayPos << " th " << l->totalHeight;
			o << " up " << widx(l->upButton) << " down " << widx(l->downButton) << " slider " << widx(l->slider) << " multi " << (int)l->multiSelect << " sels";
			if (l->multiSelect && l->selections)
				for (Int s = 0; s < l->listLength && l->selections[s] >= 0; ++s) o << " " << l->selections[s];
			o << "\n";
			for (Int r = 0; r < l->listLength && l->listData; ++r)
			{
				ListEntryRow *row = &l->listData[r];
				o << "  r" << r << " lh " << row->listHeight << " h " << (int)row->height << " c " << (row->cell ? 1 : 0);
				if (row->cell && r < l->endPos)
					for (Int c = 0; c < l->columns; ++c)
					{
						ListEntryCell *cell = &row->cell[c];
						o << " {" << cell->cellType << " " << cell->color << " " << cell->width << " " << cell->height;
						if (cell->cellType == LISTBOX_TEXT && cell->data)
						{
							TestDisplayString *ds = (TestDisplayString *)cell->data;
							dump_text(o, ds->getText());
							o << " wrap " << ds->m_wrap;
						}
						else if (cell->cellType == LISTBOX_IMAGE && cell->data)
							o << " img " << ((Image *)cell->data)->getName().str();
						o << "}";
					}
				o << "\n";
			}
		}
		else if (BitTest(style, GWS_VERT_SLIDER | GWS_HORZ_SLIDER) && ud)
		{
			SliderData *s = (SliderData *)ud;
			UnsignedInt bits;
			memcpy(&bits, &s->numTicks, 4);
			o << " sl " << s->minVal << " " << s->maxVal << " " << bits << " " << s->position << "\n";
		}
		else if (BitTest(style, GWS_ENTRY_FIELD) && ud)
		{
			EntryData *e = (EntryData *)ud;
			o << " te";
			dump_text(o, e->text->getText());
			dump_text(o, e->sText->getText());
			o << " " << e->charPos << " " << e->maxTextLen << " " << (int)e->secretText << (int)e->numericalOnly << (int)e->alphaNumericalOnly << (int)e->aSCIIOnly << (int)e->drawTextFromStart << (int)e->receivedUnichar << " " << e->conCharPos << "\n";
		}
		else if (BitTest(style, GWS_COMBO_BOX) && ud)
		{
			ComboBoxData *c = (ComboBoxData *)ud;
			o << " cb " << (int)c->isEditable << " " << c->maxDisplay << " " << c->maxChars << " " << (int)c->asciiOnly << (int)c->lettersAndNumbersOnly << (int)c->dontHide << " " << c->entryCount << " " << widx(c->dropDownButton) << " " << widx(c->editBox) << " " << widx(c->listBox) << "\n";
		}
		else if (BitTest(style, GWS_TAB_CONTROL) && ud)
		{
			TabControlData *t = (TabControlData *)ud;
			o << " tb " << t->tabOrientation << " " << t->tabEdge << " " << t->tabWidth << " " << t->tabHeight << " " << t->tabCount << " " << t->paneBorder << " " << t->activeTab << " " << t->tabsLeftLimit << " " << t->tabsRightLimit << " " << t->tabsTopLimit << " " << t->tabsBottomLimit << " panes";
			for (int k = 0; k < NUM_TAB_PANES; ++k) o << " " << widx(t->subPanes[k]);
			o << " dis";
			for (int k = 0; k < NUM_TAB_PANES; ++k) o << " " << (int)t->subPaneDisabled[k];
			o << "\n";
		}
	}
	o << "mgr focus " << widx(wm->focus()) << " cap " << widx(wm->captor()) << " grab " << widx(wm->grab()) << " rgn " << widx(wm->rgn()) << " lone " << widx(wm->lone()) << " modal";
	for (ModalWindow *m = wm->modal(); m; m = m->next) o << " " << widx(m->window);
	o << " list";
	for (GameWindow *w = wm->list(); w; w = w->winGetNext()) o << " " << widx(w);
	o << " tip";
	dump_text(o, TheMouse->m_tooltip);
	o << "\n";
}

/* ---- the operations (tests/differential_wm.rs documents each) */
static GameWindow *win(int i) { return (i >= 0 && i < (int)g_windows.size() && !g_freed[i]) ? g_windows[i] : NULL; }

static void run_op(TestWindowManager *wm, std::istringstream &in, char op)
{
	int a = 0, b = 0, c = 0, d = 0, e = 0;
	switch (op)
	{
	case 'W':
	{
		int parent, type, x, y, w, h, visual;
		unsigned int status, style;
		in >> parent >> type >> x >> y >> w >> h >> status >> style >> visual;
		WinInstanceData inst;
		inst.init();
		inst.m_style = style;
		GameWindow *p = win(parent);
		inst.m_owner = p;
		switch (type)
		{
		case 0:
		{
			GameWindow *u = wm->winCreate(p, status, x, y, w, h, TestSystem, &inst);
			u->winSetInputFunc(TestInput);
			break;
		}
		case 1: wm->gogoGadgetPushButton(p, status, x, y, w, h, &inst, NULL, visual); break;
		case 2: wm->gogoGadgetCheckbox(p, status, x, y, w, h, &inst, NULL, visual); break;
		case 3:
		{
			RadioButtonData r;
			in >> r.group;
			r.screen = 1;
			wm->gogoGadgetRadioButton(p, status, x, y, w, h, &inst, &r, NULL, visual);
			break;
		}
		case 6:
		{
			static ListboxData l;
			memset(&l, 0, sizeof(l));
			int len, cols, as, ap, sb, ms, fs, se;
			in >> len >> cols >> as >> ap >> sb >> ms >> fs >> se;
			l.listLength = (Short)len;
			l.columns = (Short)cols;
			l.autoScroll = as; l.autoPurge = ap; l.scrollBar = sb; l.multiSelect = ms; l.forceSelect = fs; l.scrollIfAtEnd = se;
			if (cols > 1)
			{
				l.columnWidthPercentage = NEW Int[cols];
				for (int i = 0; i < cols; ++i) in >> l.columnWidthPercentage[i];
			}
			wm->gogoGadgetListBox(p, status, x, y, w, h, &inst, &l, NULL, visual);
			break;
		}
		case 7: case 8:
		{
			static SliderData s;
			memset(&s, 0, sizeof(s));
			in >> s.minVal >> s.maxVal;
			wm->gogoGadgetSlider(p, status, x, y, w, h, &inst, &s, NULL, visual);
			break;
		}
		case 4:
		{
			/* createGadget's combo box data: an EntryData and a ListboxData (NEW: gogoGadgetComboBox deletes both) */
			static ComboBoxData c;
			memset(&c, 0, sizeof(c));
			int editable, ascii, letters, len, sb, fsize;
			in >> editable >> c.maxDisplay >> c.maxChars >> ascii >> letters >> len >> sb >> fsize;
			c.isEditable = editable; c.asciiOnly = ascii; c.lettersAndNumbersOnly = letters;
			c.entryData = NEW EntryData;
			memset(c.entryData, 0, sizeof(EntryData));
			c.entryData->aSCIIOnly = c.asciiOnly;
			c.entryData->alphaNumericalOnly = c.lettersAndNumbersOnly;
			c.entryData->maxTextLen = c.maxChars;
			c.listboxData = NEW ListboxData;
			memset(c.listboxData, 0, sizeof(ListboxData));
			c.listboxData->listLength = (Short)len;
			c.listboxData->scrollBar = sb;
			c.listboxData->forceSelect = 1;
			c.listboxData->columns = 1;
			inst.m_font = fsize ? TheFontLibrary->getFont(AsciiString("F"), fsize, FALSE) : NULL;
			wm->gogoGadgetComboBox(p, status, x, y, w, h, &inst, &c, inst.m_font, visual);
			break;
		}
		case 5:
		{
			static TabControlData t;
			memset(&t, 0, sizeof(t));
			int mask;
			in >> t.tabOrientation >> t.tabEdge >> t.tabWidth >> t.tabHeight >> t.tabCount >> t.paneBorder >> mask;
			for (int k = 0; k < NUM_TAB_PANES; ++k) t.subPaneDisabled[k] = (mask >> k) & 1;
			wm->gogoGadgetTabControl(p, status, x, y, w, h, &inst, &t, NULL, visual);
			break;
		}
		case 9:
		{
			static EntryData e;
			memset(&e, 0, sizeof(e));
			int maxlen, secret, num, alnum, ascii;
			std::string label;
			in >> maxlen >> secret >> num >> alnum >> ascii >> label;
			e.maxTextLen = (Short)maxlen; e.secretText = secret; e.numericalOnly = num; e.alphaNumericalOnly = alnum; e.aSCIIOnly = ascii;
			if (label != "_") inst.m_textLabelString = label.c_str();
			wm->gogoGadgetTextEntry(p, status, x, y, w, h, &inst, &e, NULL, visual);
			break;
		}
		case 10: wm->gogoGadgetProgressBar(p, status, x, y, w, h, &inst, NULL, visual); break;
		case 11:
		{
			static TextData t;
			memset(&t, 0, sizeof(t));
			wm->gogoGadgetStaticText(p, status, x, y, w, h, &inst, &t, NULL, visual);
			break;
		}
		}
		break;
	}
	case 'M':
		in >> a >> b >> c >> d >> e;
		{
			ICoord2D pos = { b, c }, delta = { d, e };
			wm->winProcessMouseEvent((GameWindowMessage)a, &pos, (a == GWM_LEFT_DRAG || a == GWM_NONE) ? &delta : NULL);
		}
		break;
	case 'K': in >> a >> b; wm->winProcessKey((UnsignedByte)a, (UnsignedInt)b); break;
	case 't': in >> a; g_time = (unsigned int)a; break;
	case 'A':
	{
		std::string t;
		in >> a >> t >> b >> c >> d >> e;
		UnicodeString u;
		for (size_t i = 0; i < t.size(); ++i) if (t[i] != '_') u.concat((WideChar)t[i]);
		g_log << "added " << GadgetListBoxAddEntryText(win(a), u, b, c, d, e) << "\n";
		break;
	}
	case 'R': in >> a; GadgetListBoxReset(win(a)); break;
	case 'S':
	{
		in >> a >> b;
		std::vector<Int> v(b > 0 ? b : 1, -1);
		for (int i = 0; i < b; ++i) in >> v[i];
		GadgetListBoxSetSelected(win(a), &v[0], b);
		break;
	}
	case 'L': in >> a >> b; GadgetListBoxSetListLength(win(a), b); break;
	case 'D': in >> a >> b; wm->winSendSystemMsg(win(a), GLM_DEL_ENTRY, (WindowMsgData)b, 0); break;
	case 'B': in >> a >> b; wm->winSendSystemMsg(win(a), GLM_SCROLL_BUFFER, (WindowMsgData)b, 0); break;
	case 'U': in >> a >> b; wm->winSendSystemMsg(win(a), GLM_UPDATE_DISPLAY, (WindowMsgData)b, 0); break;
	case 'X': in >> a >> b; wm->winSendSystemMsg(win(a), GLM_TOGGLE_MULTI_SELECTION, (WindowMsgData)b, 0); break;
	case 'G':
	{
		in >> a >> b >> c;
		Color col;
		UnicodeString t = GadgetListBoxGetTextAndColor(win(a), &col, b, c);
		g_log << "text";
		dump_text(g_log, t);
		g_log << " " << col << "\n";
		break;
	}
	case 'V':
		in >> a >> b >> c;
		if (b) GadgetListBoxSetTopVisibleEntry(win(a), c); else GadgetListBoxSetBottomVisibleEntry(win(a), c);
		break;
	case 'P': in >> a >> b; GadgetSliderSetPosition(win(a), b); break;
	case 'Q': in >> a >> b >> c; wm->winSendSystemMsg(win(a), GSM_SET_MIN_MAX, (WindowMsgData)b, (WindowMsgData)c); break;
	case 'H': in >> a >> b; win(a)->winHide(b); break;
	case 'E': in >> a >> b; win(a)->winEnable(b); break;
	case 'F': in >> a; wm->winSetFocus(win(a)); break;
	case 'C': in >> a >> b; if (b) wm->winCapture(win(a)); else wm->winRelease(win(a)); break;
	case 'O': in >> a >> b; if (b) wm->winSetModal(win(a)); else wm->winUnsetModal(win(a)); break;
	case 'Z': in >> a; wm->winDestroy(win(a)); wm->update(); break;
	case 'Y': in >> a >> b >> c; win(a)->winSetSize(b, c); break;
	case 'T':
	{
		std::string t;
		in >> a >> t;
		UnicodeString u;
		for (size_t i = 0; i < t.size(); ++i) if (t[i] != '_') u.concat((WideChar)t[i]);
		win(a)->winSetText(u);
		break;
	}
	/* #72: the text entry, the combo box, the tab control */
	/* (each call is made before its result is logged: the messages it sends are logged first) */
	case 'I': { in >> a >> b; int r = (int)wm->winSendInputMsg(win(a), GWM_IME_CHAR, (WindowMsgData)b, 0); g_log << "ret " << r << "\n"; break; }
	case 'e': case 'y': case 'a':
	{
		std::string t;
		in >> a >> t;
		UnicodeString u;
		for (size_t i = 0; i < t.size(); ++i) if (t[i] != '_') u.concat((WideChar)t[i]);
		if (op == 'e') GadgetTextEntrySetText(win(a), u);
		else if (op == 'y') GadgetComboBoxSetText(win(a), u);
		else { in >> b; Int r = GadgetComboBoxAddEntry(win(a), u, b); g_log << "cadded " << r << "\n"; }
		break;
	}
	case 'g': { in >> a; UnicodeString t = GadgetTextEntryGetText(win(a)); g_log << "etext"; dump_text(g_log, t); g_log << "\n"; break; }
	case 'c': { in >> a; UnicodeString t = GadgetComboBoxGetText(win(a)); g_log << "ctext"; dump_text(g_log, t); g_log << "\n"; break; }
	case 'r': in >> a; GadgetComboBoxReset(win(a)); break;
	case 's': in >> a >> b >> c; GadgetComboBoxSetSelectedPos(win(a), b, c); break;
	case 'p': { in >> a; Int v = -7; GadgetComboBoxGetSelectedPos(win(a), &v); g_log << "csel " << v << "\n"; break; }
	case 'h': in >> a; GadgetComboBoxHideList(win(a)); break;
	case 'k':
		in >> a >> b >> c;
		switch (b)
		{
		case 0: GadgetComboBoxSetIsEditable(win(a), c); break;
		case 1: GadgetComboBoxSetMaxChars(win(a), c); break;
		case 2: GadgetComboBoxSetMaxDisplay(win(a), c); break;
		case 3: GadgetComboBoxSetAsciiOnly(win(a), c); break;
		default: GadgetComboBoxSetLettersAndNumbersOnly(win(a), c); break;
		}
		break;
	case 'j': in >> a >> b >> c; GadgetComboBoxSetItemData(win(a), b, (void *)(uintptr_t)(unsigned int)c); break;
	case 'J': { in >> a >> b; void *r = GadgetComboBoxGetItemData(win(a), b); g_log << "cdata " << (unsigned int)(uintptr_t)r << "\n"; break; }
	case 'n': in >> a >> b; GadgetTabControlShowSubPane(win(a), b); break;
	case 'q': in >> a; GadgetTabControlCreateSubPanes(win(a)); break;
	case 'u': in >> a; GadgetTabControlFixupSubPaneList(win(a)); break;
	}
}

static std::vector<std::string> g_ops;
static std::string g_out;

static void run_case(void)
{
	TestWindowManager *wm = new TestWindowManager;
	TheWindowManager = wm;
	wm->init();
	TheDisplay->m_width = 800;
	TheDisplay->m_height = 600;
	for (size_t i = 0; i < g_ops.size(); ++i)
	{
		std::istringstream in(g_ops[i]);
		char op = 0;
		in >> op;
		size_t before = g_windows.size();
		run_op(wm, in, op);
		g_dd_from = op == 'W' ? (int)before : 1 << 30;
		std::ostringstream o;
		o << "#" << i << "\n" << g_log.str();
		g_log.str("");
		dump(o, wm);
		g_out += o.str();
		fputs(o.str().c_str(), stdout);
		fflush(stdout);
	}
}

static ucontext_t g_main_ctx, g_case_ctx;

int main(void)
{
	/* the heap in brk, below 4 GB in a non-PIE program */
	mallopt(M_MMAP_THRESHOLD, 1 << 30);
	mallopt(M_MMAP_MAX, 0);
	std::string line;
	while (std::getline(std::cin, line)) if (!line.empty()) g_ops.push_back(line);
	const size_t stack = 8 << 20;
	char *st = (char *)malloc(stack);
	if ((uintptr_t)st + stack > 0xFFFFFFFFu) { fprintf(stderr, "wm_ref: the heap is above 4 GB\n"); return 3; }
	getcontext(&g_case_ctx);
	g_case_ctx.uc_stack.ss_sp = st;
	g_case_ctx.uc_stack.ss_size = stack;
	g_case_ctx.uc_link = &g_main_ctx;
	makecontext(&g_case_ctx, run_case, 0);
	swapcontext(&g_main_ctx, &g_case_ctx);
	return 0;
}
