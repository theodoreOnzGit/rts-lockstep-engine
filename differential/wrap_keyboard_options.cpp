/* Differential-test wrapper for the keyboard options screen (GitHub #160, a part of #100; upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/keyboard_options_gen:
**   keyboard_options_cut.inc  KeyboardOptionsMenu.cpp from `// PRIVATE DATA` to the end (the whole file but its
**                             includes: the statics and globals, populateCategoryBox, setKeyDown, fillCommandListBox,
**                             doKeyUp, doKeyDown, KeyboardOptionsMenuInit, Shutdown, Update, Input, System and
**                             KeyboardTextEntryInput);
**   keyboard_meta_header.inc  MetaEvent.h, `enum MappableKeyCategories` to `class MetaMapRec` (the categories,
**                             CategoryListName, the key enum and KeyNames);
** compiled inside namespace kbo, with the REAL GameClient/KeyDefs.h (the key codes and states, through the shared shim's
** dinput.h), on the stand-ins below (each written for this test):
** - AsciiString and UnicodeString with the real classes' meaning for what is used (content compare, concat, getCharAt,
**   removeLastChar, translate, format through vsnprintf); LookupListRec copied from Common/INI.h lines 105-109;
** - DisplayString: setText returns early when the text is the same (DisplayString.cpp lines 98-108), appendChar,
**   removeLastChar, getText, getTextLength;
** - the windows of the synthetic KeyboardOptionsMenu.wnd (the parent with the screen's callbacks; Back, Reset All and
**   Assign buttons; the category combo box; the command list; the two static texts; the hotkey entry with its
**   EntryData): enabled flag, instance state (hilite) and style (mouse track), owner (the parent), user data, input
**   function (recorded);
** - the gadgets, as upstream's gadget code behaves for these calls (GadgetComboBox.cpp, GadgetListBox.cpp): a combo box's
**   reset empties its list and selection; SetSelectedPos(i) on its list: out of the list's length the selection is -1 and
**   nothing reaches the screen, past the entries nothing happens, else the selection is set and the combo box sends its
**   owner GCM_SELECTED; a plain list's SetSelected(i) sends its owner GLM_SELECTED with the selection (-1 out of its
**   length; nothing past the entries); AddEntryText of an empty text adds " "; GetText of -1 or past the entries is
**   empty; GadgetStaticTextSetText of NULL does nothing;
** - TheWindowManager: winGetWindowFromId by id, winSendSystemMsg to the parent runs KeyboardOptionsMenuSystem (each
**   message the screen is told logged: `sys <msg> <control>`), winSetFocus and the tab moves do nothing (not compared),
**   winIsDigit / winIsAlNum / winIsAscii as iswdigit / iswalnum / iswascii (C locale, as the Rust's);
** - TheGameText: the case's table, else `T:` and the label; TheMetaMap: the case's records in order; TheIMEManager NULL
**   (the Rust's manager never composes); TheShell: pop() logs `pop` and runs the top's shutdown
**   (KeyboardOptionsMenuShutdown), shutdownComplete does nothing; WindowLayout::hide records the layout hidden.
**
** Entry point (extern "C"):
**   ref_keyboard_options_run(in, out, cap)  runs one case (tests/differential_keyboard_options.rs's text); the dump's
**                                           length. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <wctype.h>
#include <string>
#include <vector>
#include <map>
#include "PreRTS.h"
#include "GameClient/KeyDefs.h"

namespace kbo {

typedef uintptr_t WindowMsgData;
typedef Int NameKeyType;
typedef Int Color;
enum { NAMEKEY_INVALID = 0 };
enum WindowMsgHandledType { MSG_IGNORED, MSG_HANDLED };
enum
{
	GWM_CREATE = 1, GWM_DESTROY, GWM_LEFT_DOWN, GWM_LEFT_DRAG, GWM_MOUSE_ENTERING, GWM_MOUSE_LEAVING, GWM_CHAR, GWM_INPUT_FOCUS,
	GWM_IME_CHAR, GWM_OTHER,
	GGM_LEFT_DRAG = 100, GBM_MOUSE_ENTERING, GBM_MOUSE_LEAVING, GBM_SELECTED, GLM_SELECTED, GCM_SELECTED, GEM_EDIT_DONE,
	GEM_UPDATE_TEXT,
};
enum { GWS_MOUSE_TRACK = 0x1, WIN_STATE_HILITED = 0x2 };
enum { VK_RETURN = 0x0D };
static inline Color GameMakeColor(Int r, Int g, Int b, Int a) { return (Color)(((a & 0xFF) << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)); }

std::string g_out;
void outf(const char *fmt, ...)
{
	char b[4096];
	va_list a;
	va_start(a, fmt);
	vsnprintf(b, sizeof b, fmt, a);
	va_end(a);
	g_out += b;
}

/* ---- the strings ---- */
class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	void format(const char *fmt, ...)
	{
		char b[1024];
		va_list a;
		va_start(a, fmt);
		vsnprintf(b, sizeof b, fmt, a);
		va_end(a);
		s = b;
	}
	const char *str() const { return s.c_str(); }
};
class UnicodeString
{
public:
	std::basic_string<WideChar> s;
	static const UnicodeString TheEmptyString;
	UnicodeString() {}
	Bool operator==(const UnicodeString &o) const { return s == o.s; }
	void concat(const UnicodeString &o) { s += o.s; }
	void concat(WideChar c) { s += c; }
	Int getLength() const { return (Int)s.size(); }
	/* (getCharAt(-1) of an empty text is never reached here: the Rust cuts the case before it) */
	WideChar getCharAt(Int i) const { return (i >= 0 && i < (Int)s.size()) ? s[i] : 0; }
	Bool isEmpty() const { return s.empty(); }
	void removeLastChar() { if (!s.empty()) s.erase(s.size() - 1); }
	void translate(const AsciiString &a) { s.clear(); for (size_t i = 0; i < a.s.size(); ++i) s += (WideChar)(unsigned char)a.s[i]; }
};
const UnicodeString UnicodeString::TheEmptyString;
std::string narrow(const UnicodeString &u)
{
	std::string r;
	for (size_t i = 0; i < u.s.size(); ++i)
	{
		char b[16];
		if (u.s[i] >= 0x20 && u.s[i] < 0x7F && u.s[i] != '\\') r += (char)u.s[i];
		else { snprintf(b, sizeof b, "\\u%04x", (unsigned)u.s[i]); r += b; }
	}
	return r;
}
UnicodeString widen(const std::string &h)
{
	UnicodeString u;
	if (h == "-") return u;
	for (size_t i = 0; i + 4 <= h.size(); i += 4) { unsigned v; sscanf(h.c_str() + i, "%4x", &v); u.s += (WideChar)v; }
	return u;
}

/* ---- Common/INI.h lines 105-109 (copied) ---- */
struct LookupListRec
{
	const char* name;
	Int value;
};
#include "keyboard_meta_header.inc"

class DisplayString
{
public:
	UnicodeString m_textString;
	void setText(UnicodeString text) { if (text == m_textString) return; m_textString = text; }
	UnicodeString getText(void) { return m_textString; }
	Int getTextLength(void) { return m_textString.getLength(); }
	void appendChar(WideChar c) { m_textString.concat(c); }
	void removeLastChar(void) { m_textString.removeLastChar(); }
};
struct EntryData
{
	DisplayString *text, *sText;
	Bool numericalOnly, alphaNumericalOnly, aSCIIOnly;
	UnsignedShort charPos, conCharPos;
};
class WinInstanceData
{
public:
	Int m_state;
	UnsignedInt m_style;
	UnsignedInt getStyle() const { return m_style; }
};
class GameWindow;
typedef WindowMsgHandledType (*GameWinInputFunc)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData);
class GameWindow
{
public:
	std::string name;
	Int id;
	Bool enabled;
	WinInstanceData inst;
	void *userData;
	GameWindow *owner;
	GameWinInputFunc input;
	Int winGetWindowId() { return id; }
	void winSetInputFunc(GameWinInputFunc f) { input = f; }
	void winEnable(Bool e) { enabled = e; }
	void *winGetUserData() { return userData; }
	WinInstanceData *winGetInstanceData() { return &inst; }
	GameWindow *winGetOwner() { return owner; }
	void winNextTab() {}
	void winPrevTab() {}
};
class WindowLayout
{
public:
	Bool hidden;
	void hide(Bool h) { hidden = h; }
};

/* ---- the case's windows ---- */
enum { W_PARENT, W_BACK, W_COMBO, W_LIST, W_DESC, W_HOTKEY, W_RESET, W_ENTRY, W_ASSIGN, W_COUNT };
const char *g_names[W_COUNT] = { "ParentKeyboardOptionsMenu", "ButtonBack", "ComboBoxCategoryList", "ListBoxCommandList", "StaticTextDescription",
	"StaticTextCurrentHotkey", "ButtonResetAll", "TextEntryAssignHotkey", "ButtonAssign" };
GameWindow g_win[W_COUNT];
Bool g_present[W_COUNT];
DisplayString g_text, g_sText;
EntryData g_entry;
WindowLayout g_layout;
std::map<std::string, UnicodeString> g_static;   /* static texts by window name */
/* the combo box's list and the command list */
struct List { std::vector<UnicodeString> rows; Int selectPos; Int listLength; };
List g_comboList, g_cmdList;
Bool g_popped;

class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &n)
	{
		const char *p = n.str();
		const char *pre = "KeyboardOptionsMenu.wnd:";
		if (!strncmp(p, pre, strlen(pre))) p += strlen(pre);
		for (int i = 0; i < W_COUNT; ++i) if (!strcmp(p, g_names[i])) return 1000 + i;
		return 2000;
	}
	NameKeyType nameToKey(const char *n) { return nameToKey(AsciiString(n)); }
};
NameKeyGenerator g_keys;
NameKeyGenerator *TheNameKeyGenerator = &g_keys;

WindowMsgHandledType KeyboardOptionsMenuSystem( GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2 );
const char *msgName(UnsignedInt m)
{
	switch (m)
	{
	case GBM_SELECTED: return "GBM_SELECTED"; case GLM_SELECTED: return "GLM_SELECTED"; case GCM_SELECTED: return "GCM_SELECTED";
	case GEM_EDIT_DONE: return "GEM_EDIT_DONE"; case GEM_UPDATE_TEXT: return "GEM_UPDATE_TEXT"; case GBM_MOUSE_ENTERING: return "GBM_MOUSE_ENTERING";
	case GBM_MOUSE_LEAVING: return "GBM_MOUSE_LEAVING"; case GGM_LEFT_DRAG: return "GGM_LEFT_DRAG";
	}
	return 0;
}
class GameWindowManager
{
public:
	GameWindow *winGetWindowFromId(GameWindow *, Int id)
	{
		for (int i = 0; i < W_COUNT; ++i) if (g_present[i] && g_win[i].id == id) return &g_win[i];
		return NULL;
	}
	WindowMsgHandledType winSendSystemMsg(GameWindow *w, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2)
	{
		if (w != &g_win[W_PARENT]) return MSG_IGNORED;
		if (msgName(msg))
		{
			GameWindow *c = (GameWindow *)d1;
			outf("sys %s %s\n", msgName(msg), c ? c->name.c_str() : "-");
		}
		return KeyboardOptionsMenuSystem(w, msg, d1, d2);
	}
	void winSetFocus(GameWindow *) {}
	Int winIsDigit(Int c) { return iswdigit(c); }
	Int winIsAlNum(Int c) { return iswalnum(c); }
	Int winIsAscii(Int c) { return c >= 0 && c < 0x80; }   /* iswascii (MSVC: c < 0x80) */
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;

std::map<std::string, UnicodeString> g_gameText;
class GameText
{
public:
	UnicodeString fetch(const AsciiString &l)
	{
		std::map<std::string, UnicodeString>::iterator it = g_gameText.find(l.s);
		if (it != g_gameText.end()) return it->second;
		UnicodeString u;
		u.translate(AsciiString(("T:" + l.s).c_str()));
		return u;
	}
	UnicodeString fetch(const char *l) { return fetch(AsciiString(l)); }
};
GameText g_gameTextObj;
GameText *TheGameText = &g_gameTextObj;

class MetaMapRec
{
public:
	MetaMapRec *m_next;
	MappableKeyType m_key;
	MappableKeyCategories m_category;
	UnicodeString m_description, m_displayName;
};
std::vector<MetaMapRec> g_recs;
class MetaMap
{
public:
	const MetaMapRec *getFirstMetaMapRec() const { return g_recs.empty() ? NULL : &g_recs[0]; }
};
MetaMap g_metaMap;
MetaMap *TheMetaMap = &g_metaMap;

class IMEManager
{
public:
	Bool isAttachedTo(GameWindow *) { return FALSE; }
	Bool isComposing() { return FALSE; }
};
IMEManager *TheIMEManager = NULL;

void KeyboardOptionsMenuShutdown( WindowLayout *layout, void *userData );
class Shell
{
public:
	void pop() { outf("pop\n"); g_popped = TRUE; KeyboardOptionsMenuShutdown(&g_layout, NULL); }
	void shutdownComplete(WindowLayout *) {}
};
Shell g_shell;
Shell *TheShell = &g_shell;

/* ---- the gadgets ---- */
void listSelect(GameWindow *lw, List &l, Int i);
void GadgetComboBoxReset(GameWindow *c) { if (!c) return; g_comboList.rows.clear(); g_comboList.selectPos = -1; }
Int GadgetComboBoxAddEntry(GameWindow *c, UnicodeString text, Color)
{
	if (!c) return -1;
	if (text.isEmpty()) { text.s.clear(); text.s += (WideChar)' '; }
	g_comboList.rows.push_back(text);
	return (Int)g_comboList.rows.size() - 1;
}
void GadgetComboBoxSetSelectedPos(GameWindow *c, Int i, Bool dontHide = FALSE)
{
	(void)dontHide;
	if (!c) return;
	if (i < 0 || g_comboList.listLength <= i) { g_comboList.selectPos = -1; return; }
	if (i >= (Int)g_comboList.rows.size()) return;
	g_comboList.selectPos = i;
	TheWindowManager->winSendSystemMsg(c->winGetOwner(), GCM_SELECTED, (WindowMsgData)c, 0);
}
void GadgetComboBoxGetSelectedPos(GameWindow *c, Int *sel) { if (!c) return; *sel = g_comboList.selectPos; }
void GadgetListBoxReset(GameWindow *lw) { if (!lw) return; g_cmdList.rows.clear(); g_cmdList.selectPos = -1; }
Int GadgetListBoxAddEntryText(GameWindow *lw, UnicodeString text, Color, Int, Int = -1, Bool = TRUE)
{
	if (!lw) return -1;
	if (text.isEmpty()) { text.s.clear(); text.s += (WideChar)' '; }
	g_cmdList.rows.push_back(text);
	return (Int)g_cmdList.rows.size() - 1;
}
void GadgetListBoxSetSelected(GameWindow *lw, Int i)
{
	if (i < 0 || g_cmdList.listLength <= i)
	{
		g_cmdList.selectPos = -1;
		TheWindowManager->winSendSystemMsg(lw->winGetOwner(), GLM_SELECTED, (WindowMsgData)lw, (WindowMsgData)(Int)-1);
		return;
	}
	if (i >= (Int)g_cmdList.rows.size()) return;
	g_cmdList.selectPos = i;
	TheWindowManager->winSendSystemMsg(lw->winGetOwner(), GLM_SELECTED, (WindowMsgData)lw, (WindowMsgData)i);
}
void GadgetListBoxGetSelected(GameWindow *lw, Int *sel) { if (!lw) return; *sel = g_cmdList.selectPos; }
UnicodeString GadgetListBoxGetText(GameWindow *lw, Int row, Int column = 0)
{
	(void)column;
	if (!lw || row < 0 || row >= (Int)g_cmdList.rows.size()) return UnicodeString::TheEmptyString;
	return g_cmdList.rows[row];
}
void GadgetStaticTextSetText(GameWindow *w, UnicodeString text) { if (!w) return; g_static[w->name] = text; }

/* ---- upstream ---- */
#include "keyboard_options_cut.inc"

void dump()
{
	outf("combo %d %d\n", (Int)g_comboList.rows.size(), g_comboList.selectPos);
	outf("list %d %d", (Int)g_cmdList.rows.size(), g_cmdList.selectPos);
	for (size_t i = 0; i < g_cmdList.rows.size(); ++i) outf(" [%s]", narrow(g_cmdList.rows[i]).c_str());
	outf("\n");
	outf("desc [%s]\nhot [%s]\n", narrow(g_static[g_names[W_DESC]]).c_str(), narrow(g_static[g_names[W_HOTKEY]]).c_str());
	if (g_present[W_ENTRY])
		outf("entry [%s] [%s] %d %d %d %d\n", narrow(g_text.m_textString).c_str(), narrow(g_sText.m_textString).c_str(), g_entry.charPos,
			g_win[W_ENTRY].enabled, BitTest(g_win[W_ENTRY].inst.m_state, WIN_STATE_HILITED), g_win[W_ENTRY].input ? 1 : 0);
	outf("flags %d %d %d %d [%s] [%s] [%s] %d\n", shiftDown, altDown, ctrlDown, absolute, narrow(alt).c_str(), narrow(ctrl).c_str(), narrow(shift).c_str(),
		g_layout.hidden);
}

} /* namespace kbo */

extern "C" int ref_keyboard_options_run(const char *in, char *out, int cap)
{
	using namespace kbo;
	g_out.clear();
	g_gameText.clear();
	g_recs.clear();
	g_static.clear();
	g_comboList = List(); g_comboList.selectPos = -1; g_comboList.listLength = 10; /* (the list createGadget gives a combo box: 10 rows) */
	g_cmdList = List(); g_cmdList.selectPos = -1; g_cmdList.listLength = 100;
	g_text = DisplayString(); g_sText = DisplayString();
	memset(&g_entry, 0, sizeof g_entry);
	g_entry.text = &g_text; g_entry.sText = &g_sText;
	g_layout.hidden = TRUE;
	g_popped = FALSE;
	for (int i = 0; i < W_COUNT; ++i)
	{
		g_win[i] = GameWindow();
		g_win[i].name = g_names[i];
		g_win[i].id = 1000 + i;
		g_win[i].enabled = TRUE;
		g_win[i].inst.m_state = 0;
		g_win[i].inst.m_style = 0;
		g_win[i].userData = NULL;
		g_win[i].owner = i == W_PARENT ? NULL : &g_win[W_PARENT];
		g_win[i].input = NULL;
		g_present[i] = TRUE;
	}
	g_win[W_ENTRY].userData = &g_entry;
	const char *p = in;
	std::string line;
	while (*p && !g_popped)
	{
		size_t n = strcspn(p, "\n");
		line.assign(p, n);
		p += n;
		if (*p == '\n') ++p;
		char op[32] = "";
		sscanf(line.c_str(), "%31s", op);
		std::string cmd(op);
		const char *rest = line.c_str() + strlen(op);
		while (*rest == ' ') ++rest;
		if (cmd == "text")
		{
			char a[512], b[4096];
			sscanf(rest, "%511s %4095s", a, b);
			g_gameText[a] = widen(b);
			continue;
		}
		if (cmd == "rec")
		{
			int cat, key;
			char a[4096], b[4096];
			sscanf(rest, "%d %d %4095s %4095s", &cat, &key, a, b);
			MetaMapRec r;
			r.m_next = NULL; r.m_key = (MappableKeyType)key; r.m_category = (MappableKeyCategories)cat;
			r.m_displayName = widen(a); r.m_description = widen(b);
			g_recs.push_back(r);
			continue;
		}
		if (cmd == "entry")
		{
			int num, al, as, track;
			sscanf(rest, "%d %d %d %d", &num, &al, &as, &track);
			g_entry.numericalOnly = num; g_entry.alphaNumericalOnly = al; g_entry.aSCIIOnly = as;
			g_win[W_ENTRY].inst.m_style = track ? GWS_MOUSE_TRACK : 0;
			continue;
		}
		if (cmd == "globals")
		{
			/* the file's globals as an earlier visit left them (the Rust test keeps them as well) */
			int s, a, c, abs;
			char x[4096], y[4096], z[4096];
			sscanf(rest, "%d %d %d %d %4095s %4095s %4095s", &s, &a, &c, &abs, x, y, z);
			shiftDown = s; altDown = a; ctrlDown = c; absolute = abs; alt = widen(x); ctrl = widen(y); shift = widen(z);
			continue;
		}
		for (size_t i = 1; i < g_recs.size(); ++i) g_recs[i - 1].m_next = &g_recs[i];
		outf("> %s\n", line.c_str());
		int ret = -1;
		int a = 0, b = 0;
		GameWindow *parent = &g_win[W_PARENT], *entry = &g_win[W_ENTRY];
		if (cmd == "init") { KeyboardOptionsMenuInit(&g_layout, NULL); }
		else if (cmd == "cat") { sscanf(rest, "%d", &a); GadgetComboBoxSetSelectedPos(&g_win[W_COMBO], a); }
		else if (cmd == "pick") { sscanf(rest, "%d", &a); GadgetListBoxSetSelected(&g_win[W_LIST], a); }
		else if (cmd == "button")
		{
			sscanf(rest, "%d", &a);
			int w = a == 0 ? W_BACK : a == 1 ? W_RESET : W_ASSIGN;
			ret = TheWindowManager->winSendSystemMsg(parent, GBM_SELECTED, (WindowMsgData)&g_win[w], g_win[w].id);
		}
		else if (cmd == "menukey") { sscanf(rest, "%d %d", &a, &b); ret = KeyboardOptionsMenuInput(parent, GWM_CHAR, (WindowMsgData)a, (WindowMsgData)b); }
		else if (cmd == "key") { sscanf(rest, "%d %d", &a, &b); ret = entry->input ? entry->input(entry, GWM_CHAR, (WindowMsgData)a, (WindowMsgData)b) : -2; }
		else if (cmd == "ime") { sscanf(rest, "%d", &a); ret = entry->input ? entry->input(entry, GWM_IME_CHAR, (WindowMsgData)a, 0) : -2; }
		else if (cmd == "mouse")
		{
			sscanf(rest, "%d", &a);
			UnsignedInt m = a == 0 ? GWM_LEFT_DOWN : a == 1 ? GWM_MOUSE_ENTERING : a == 2 ? GWM_MOUSE_LEAVING : a == 3 ? GWM_LEFT_DRAG : GWM_OTHER;
			ret = entry->input ? entry->input(entry, m, 0, 0) : -2;
		}
		else if (cmd == "focus")
		{
			sscanf(rest, "%d", &a);
			Bool want = FALSE;
			ret = KeyboardOptionsMenuSystem(parent, GWM_INPUT_FOCUS, (WindowMsgData)a, (WindowMsgData)&want);
			outf("want %d\n", want);
		}
		else if (cmd == "sys") { sscanf(rest, "%d", &a); ret = KeyboardOptionsMenuSystem(parent, a == 0 ? GWM_CREATE : a == 1 ? GWM_DESTROY : GWM_OTHER, 0, 0); }
		else if (cmd == "update") { KeyboardOptionsMenuUpdate(&g_layout, NULL); }
		outf("= %d\n", ret);
		if (g_popped) { outf("popped\n"); break; }
		dump();
	}
	int len = (int)g_out.size();
	if (len > cap) len = cap;
	memcpy(out, g_out.data(), len);
	return (int)g_out.size();
}
