/* The shared stand-in host of the group ui3a differential tests (#215): a window tree, TheWindowManager, WindowLayout, name keys,
** the text strings and the game text that the WOL menu callbacks (upstream GUICallbacks/Menus/WOL*.cpp, commit 0a05454d) call.
** Included INSIDE a wrapper's own namespace (so no global is shared between two wrappers in one test binary), after the system
** headers, "Lib/BaseType.h" and "GameClient/KeyDefs.h", and after the verbatim cuts the wrapper's build.rs function wrote:
**   gw_status.inc   GameWindow.h, the WIN_STATUS_* enum;     gw_err.inc  GameWindow.h, the WIN_ERR_* enum;
**   gwm_find.inc    GameWindowManager.cpp winGetWindowFromId;
**   gwm_msg.inc     GameWindowManager.cpp winSendSystemMsg, winSendInputMsg, winGetFocus, winSetFocus;
**   gwm_hiding.inc  GameWindowManager.cpp windowHiding;  gwm_capture.inc  winCapture;  gwm_modal.inc  winSetModal, winUnsetModal;
**   gw_hide.inc     GameWindow.cpp GameWindow::winHide;   wl_hide.inc  WindowLayout.cpp WindowLayout::hide.
** Stand-ins (not upstream's text): the classes' data members (only what those functions and the callbacks read), AsciiString and
** UnicodeString (as the keyboard options wrapper's), the NameKeyGenerator (keys numbered from 1 in the order first asked, as the Rust
** shared table), TheGameText (the case's table, else "T:" and the label), a listbox of text rows (GadgetListBox.cpp's
** GLM_ADD_ENTRY, GLM_SET_SELECTION and GLM_GET_SELECTION for a single-select list: add appends; a selection out of the list's length
** sets -1 and tells the owner GLM_SELECTED; a row with no entry changes nothing; else the selection is set and the owner told), a
** log (`g_log`) every external call writes to, and the dump of the window state. */

typedef uintptr_t WindowMsgData;
typedef Int NameKeyType;
typedef Int Color;
enum { NAMEKEY_INVALID = 0 };
enum WindowMsgHandledType { MSG_IGNORED, MSG_HANDLED };
enum { GWM_CREATE = 1, GWM_DESTROY, GWM_CHAR, GWM_INPUT_FOCUS, GWM_LEFT_DOWN, GBM_SELECTED = 100, GLM_SELECTED, GEM_EDIT_DONE };
#include "gw_status.inc"
#include "gw_err.inc"

std::string g_log;
void u3aLog(const char *fmt, ...)
{
	char b[1 << 16];
	va_list a;
	va_start(a, fmt);
	vsnprintf(b, sizeof b, fmt, a);
	va_end(a);
	g_log += b;
	g_log += "\n";
}

class UnicodeString;
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
	/* (the members below follow AsciiString.cpp: set copies through a temporary, so `a = a.str() + 1` shifts the text left) */
	AsciiString &operator=(const char *c) { std::string t(c); s = t; return *this; }
	Bool isEmpty() const { return s.empty(); }
	Int getLength() const { return (Int)s.size(); }
	char getCharAt(Int i) const { return s[i]; }
	void removeLastChar() { if (!s.empty()) s.erase(s.size() - 1); }
	void clear() { s.clear(); }
	Bool startsWith(const char *p) const { return s.compare(0, strlen(p), p) == 0 && s.size() >= strlen(p); }
	Int compare(const char *o) const { return strcmp(s.c_str(), o); }
	void trim()   /* AsciiString::trim: isspace at both ends */
	{
		size_t b = 0;
		while (b < s.size() && isspace((unsigned char)s[b])) ++b;
		s.erase(0, b);
		while (!s.empty() && isspace((unsigned char)s[s.size() - 1])) s.erase(s.size() - 1);
	}
	Bool nextToken(AsciiString *tok, const char *seps)   /* AsciiString::nextToken */
	{
		if (isEmpty() || tok == this) return false;
		if (seps == NULL) seps = " \n\r\t";
		size_t start = 0;
		while (start < s.size() && strchr(seps, s[start])) ++start;
		size_t end = start;
		while (end < s.size() && !strchr(seps, s[end])) ++end;
		if (end > start) { tok->s = s.substr(start, end - start); s = s.substr(end); return true; }
		s.clear();
		tok->s.clear();
		return false;
	}
};
inline Bool operator==(const AsciiString &a, const char *b) { return a.s == b; }
inline Bool operator==(const AsciiString &a, const AsciiString &b) { return a.s == b.s; }
inline Bool operator<(const AsciiString &a, const AsciiString &b) { return strcmp(a.s.c_str(), b.s.c_str()) < 0; }
static size_t wlen(const WideChar *p) { size_t n = 0; while (p[n]) ++n; return n; }
class UnicodeString
{
public:
	std::basic_string<WideChar> s;
	static const UnicodeString TheEmptyString;
	UnicodeString() {}
	UnicodeString(const WideChar *w) : s(w) {}
	UnicodeString(const wchar_t *w) { for (; *w; ++w) s += (WideChar)*w; }   /* the L"..." literals: 16-bit units, as MSVC's */
	/* UnicodeString::format(fmt, ...): the case's formats are ASCII with `%d` and `%%` only (what _vsnwprintf does with them) */
	void formatv(const UnicodeString &fmt, va_list args)
	{
		std::string f;
		for (size_t i = 0; i < fmt.s.size(); ++i) f += (char)fmt.s[i];
		char b[4096];
		vsnprintf(b, sizeof b, f.c_str(), args);
		s.clear();
		for (const char *c = b; *c; ++c) s += (WideChar)(unsigned char)*c;
	}
	void format(UnicodeString fmt, ...) { va_list a; va_start(a, fmt); formatv(fmt, a); va_end(a); }
	Bool isEmpty() const { return s.empty(); }
	Int getLength() const { return (Int)s.size(); }
	WideChar getCharAt(Int i) const { return s[i]; }
	void removeLastChar() { if (!s.empty()) s.erase(s.size() - 1); }
	void clear() { s.clear(); }
	void translate(const AsciiString &a) { s.clear(); for (size_t i = 0; i < a.s.size(); ++i) s += (WideChar)(unsigned char)a.s[i]; }
	/* UnicodeString::trim: iswspace (the C locale's: space and \t \n \v \f \r) at both ends */
	static Bool isWSpace(WideChar c) { return c == 0x20 || (c >= 9 && c <= 13); }
	void trim()
	{
		size_t b = 0;
		while (b < s.size() && isWSpace(s[b])) ++b;
		s.erase(0, b);
		while (!s.empty() && isWSpace(s[s.size() - 1])) s.erase(s.size() - 1);
	}
	Bool nextToken(UnicodeString *tok, UnicodeString delimiters)   /* UnicodeString::nextToken */
	{
		if (isEmpty() || tok == this) return false;
		if (delimiters.isEmpty()) { static const WideChar d[] = { ' ', '\t', '\n', '\r', 0 }; delimiters = UnicodeString(d); }
		size_t start = 0;
		while (start < s.size() && delimiters.s.find(s[start]) != std::basic_string<WideChar>::npos) ++start;
		size_t end = start;
		while (end < s.size() && delimiters.s.find(s[end]) == std::basic_string<WideChar>::npos) ++end;
		if (end > start) { tok->s = s.substr(start, end - start); s = s.substr(end); return true; }
		s.clear();
		tok->s.clear();
		return false;
	}
};
inline Bool operator==(const UnicodeString &a, const UnicodeString &b) { return a.s == b.s; }
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

class GameWindow;
class GameWindowManager;
typedef WindowMsgHandledType (*GameWinSystemFunc)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData);
typedef GameWinSystemFunc GameWinInputFunc;
extern GameWindowManager *TheWindowManager;

class GameWindow
{
public:
	std::string name;
	Int id;
	UnsignedInt m_status;
	GameWindow *m_parent, *m_child, *m_next, *m_owner, *m_nextLayout;
	GameWinSystemFunc m_system, m_input;
	Int winGetWindowId() { return id; }
	GameWindow *winGetParent() { return m_parent; }
	GameWindow *winGetChild() { return m_child; }
	GameWindow *winGetNext() { return m_next; }
	GameWindow *winGetOwner() { return m_owner; }
	GameWindow *winGetNextInLayout() { return m_nextLayout; }
	UnsignedInt winGetStatus() { return m_status; }
	Bool winIsHidden() { return BitTest( m_status, WIN_STATUS_HIDDEN ); }
	void freeImages() {}
	Int winHide( Bool hide );
	Int winEnable( Bool enable );
};

struct ModalWindow
{
	GameWindow *window;
	ModalWindow *next;
	void deleteInstance() { delete this; }
};
#define newInstance(T) new T

class GameWindowManager
{
public:
	GameWindow *m_windowList, *m_keyboardFocus, *m_mouseCaptor;
	ModalWindow *m_modalHead;
	GameWindowManager() : m_windowList(NULL), m_keyboardFocus(NULL), m_mouseCaptor(NULL), m_modalHead(NULL) {}
	GameWindow *winGetWindowFromId( GameWindow *window, Int id );
	WindowMsgHandledType winSendSystemMsg( GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2 );
	WindowMsgHandledType winSendInputMsg( GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2 );
	GameWindow *winGetFocus( void );
	Int winSetFocus( GameWindow *window );
	void windowHiding( GameWindow *window );
	Int winCapture( GameWindow *window );
	Int winSetModal( GameWindow *window );
	Int winUnsetModal( GameWindow *window );
};
#include "gwm_find.inc"
#include "gwm_msg.inc"
#include "gwm_hiding.inc"
#include "gwm_capture.inc"
#include "gwm_modal.inc"
#include "gw_hide.inc"
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;

class WindowLayout
{
public:
	GameWindow *m_windowList;
	Bool m_hidden;
	void hide( Bool hide );
};
#include "wl_hide.inc"
WindowLayout g_layout;

/* ---- name keys: numbered from 1 in the order first asked ---- */
std::map<std::string, Int> g_keys;
std::map<Int, std::string> g_keyNames;
class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const char *c) { return nameToKey(AsciiString(c)); }
	NameKeyType nameToKey(const AsciiString &n)
	{
		std::map<std::string, Int>::iterator it = g_keys.find(n.s);
		if (it != g_keys.end()) return it->second;
		Int k = (Int)g_keys.size() + 1;
		g_keys[n.s] = k;
		g_keyNames[k] = n.s;
		return k;
	}
};
NameKeyGenerator g_nameKeys;
NameKeyGenerator *TheNameKeyGenerator = &g_nameKeys;

/* ---- the game text ---- */
std::map<std::string, std::string> g_gameTexts;   /* the case's table (ASCII), else `T:` and the label */
class GameText
{
public:
	UnicodeString fetch(const char *l)
	{
		UnicodeString u;
		std::map<std::string, std::string>::iterator it = g_gameTexts.find(l);
		AsciiString a(it != g_gameTexts.end() ? it->second.c_str() : (std::string("T:") + l).c_str());
		u.translate(a);
		return u;
	}
};
GameText g_gameText;
GameText *TheGameText = &g_gameText;

/* ---- the windows of a case ---- */
std::vector<GameWindow *> g_allWindows;    /* every window made, in creation order (the parent first) */
std::string shortName(const std::string &full)
{
	size_t c = full.find(':');
	return c == std::string::npos ? full : full.substr(c + 1);
}
std::string winText(GameWindow *w) { return w ? shortName(w->name) : std::string("-"); }
std::string idText(Int id)
{
	std::map<Int, std::string>::iterator it = g_keyNames.find(id);
	if (it != g_keyNames.end()) return shortName(it->second);
	char b[32];
	snprintf(b, sizeof b, "?%d", id);
	return b;
}
GameWindow *makeWindow(const std::string &fullName, GameWindow *parent, UnsignedInt status, GameWinSystemFunc sys, GameWinInputFunc inp)
{
	GameWindow *w = new GameWindow();
	w->name = fullName;
	w->id = g_nameKeys.nameToKey(AsciiString(fullName.c_str()));
	w->m_status = status;
	w->m_parent = parent;
	w->m_child = NULL;
	w->m_next = NULL;
	w->m_nextLayout = NULL;
	w->m_owner = parent ? parent : w;   /* winSetOwner( NULL ) makes the window its own owner */
	w->m_system = sys;
	w->m_input = inp;
	g_allWindows.push_back(w);
	if (parent)
	{
		GameWindow **p = &parent->m_child;
		while (*p) p = &(*p)->m_next;
		*p = w;
	}
	else
	{
		GameWindow **p = &g_wm.m_windowList;
		while (*p) p = &(*p)->m_next;
		*p = w;
		GameWindow **q = &g_layout.m_windowList;
		while (*q) q = &(*q)->m_nextLayout;
		*q = w;
	}
	return w;
}
void resetWindows()
{
	for (size_t i = 0; i < g_allWindows.size(); ++i) delete g_allWindows[i];
	g_allWindows.clear();
	while (g_wm.m_modalHead) { ModalWindow *n = g_wm.m_modalHead->next; delete g_wm.m_modalHead; g_wm.m_modalHead = n; }
	g_wm = GameWindowManager();
	g_layout.m_windowList = NULL;
	g_layout.m_hidden = FALSE;
	g_keys.clear();
	g_keyNames.clear();
	g_gameTexts.clear();
	g_log.clear();
}
const char *msgName(UnsignedInt m)
{
	switch (m)
	{
	case GWM_CREATE: return "CREATE"; case GWM_DESTROY: return "DESTROY"; case GWM_CHAR: return "CHAR"; case GWM_INPUT_FOCUS: return "INPUT_FOCUS";
	case GWM_LEFT_DOWN: return "LEFT_DOWN"; case GBM_SELECTED: return "GBM_SELECTED"; case GLM_SELECTED: return "GLM_SELECTED";
	case GEM_EDIT_DONE: return "GEM_EDIT_DONE";
	}
	return "OTHER";
}
std::string d1Text(UnsignedInt msg, WindowMsgData d1)
{
	char b[32];
	if (msg == GBM_SELECTED || msg == GLM_SELECTED) return winText((GameWindow *)d1);
	snprintf(b, sizeof b, "%u", (unsigned)d1);
	return b;
}
std::string d2Text(UnsignedInt msg, WindowMsgData d2)
{
	char b[32];
	if (msg == GBM_SELECTED) return idText((Int)d2);
	if (msg == GLM_SELECTED) { snprintf(b, sizeof b, "%d", (Int)d2); return b; }
	if (msg == GWM_INPUT_FOCUS) return "-";
	snprintf(b, sizeof b, "%u", (unsigned)d2);
	return b;
}
/* the log of a callback entered: `sys <window> <msg> <d1> <d2>` and `= <result>` after it */
WindowMsgHandledType loggedSystem(WindowMsgHandledType (*f)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData), GameWindow *w, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2)
{
	char head[256];
	snprintf(head, sizeof head, "sys %s %s %s %s", winText(w).c_str(), msgName(msg), d1Text(msg, d1).c_str(), d2Text(msg, d2).c_str());
	g_log += head;
	g_log += "\n";
	WindowMsgHandledType r = f(w, msg, d1, d2);
	u3aLog("= %s %d", msgName(msg), (int)r);
	return r;
}
WindowMsgHandledType loggedInput(WindowMsgHandledType (*f)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData), GameWindow *w, UnsignedInt msg, WindowMsgData d1, WindowMsgData d2)
{
	u3aLog("inp %s %s %u %u", winText(w).c_str(), msgName(msg), (unsigned)d1, (unsigned)d2);
	WindowMsgHandledType r = f(w, msg, d1, d2);
	u3aLog("= %s %d", msgName(msg), (int)r);
	return r;
}

/* ---- the listbox ---- */
struct ListCell { UnicodeString text; Color color; };
struct ListState { std::vector<ListCell> rows; Int selectPos; Int listLength; GameWindow *win; };
ListState g_list;
Int GadgetListBoxAddEntryText( GameWindow *listbox, UnicodeString text, Color color, Int row, Int column = -1, Bool overwrite = TRUE )
{
	if (!listbox) return -1;
	if ((Int)g_list.rows.size() >= g_list.listLength) return -1;   /* GLM_ADD_ENTRY: a full list without auto purge refuses the entry */
	if (text.isEmpty()) { AsciiString sp(" "); text.translate(sp); }   /* AddEntryText of an empty text adds " " */
	ListCell c;
	c.text = text;
	c.color = color;
	g_list.rows.push_back(c);
	return (Int)g_list.rows.size() - 1;
}
void GadgetListBoxSetSelected( GameWindow *listbox, Int selectIndex )
{
	if (!listbox) return;
	if (selectIndex < 0 || g_list.listLength <= selectIndex)
	{
		g_list.selectPos = -1;
		TheWindowManager->winSendSystemMsg( listbox->winGetOwner(), GLM_SELECTED, (WindowMsgData)listbox, g_list.selectPos );
		return;
	}
	if (selectIndex >= (Int)g_list.rows.size()) return;   /* no cell: nothing is selected */
	g_list.selectPos = selectIndex;
	TheWindowManager->winSendSystemMsg( listbox->winGetOwner(), GLM_SELECTED, (WindowMsgData)listbox, g_list.selectPos );
}
void GadgetListBoxGetSelected( GameWindow *listbox, Int *selectList )
{
	if (listbox == NULL) return;
	*selectList = g_list.selectPos;
}

void dumpWindows(const char **names, int n, const char *prefix)
{
	for (int i = 0; i < n; ++i)
	{
		GameWindow *w = NULL;
		for (size_t k = 0; k < g_allWindows.size(); ++k) if (shortName(g_allWindows[k]->name) == names[i]) w = g_allWindows[k];
		if (!w) { u3aLog("win %s absent", names[i]); continue; }
		u3aLog("win %s hidden %d enabled %d", names[i], w->winIsHidden() ? 1 : 0, BitTest(w->m_status, WIN_STATUS_ENABLED) ? 1 : 0);
	}
	u3aLog("focus %s modal %s layout %d", winText(g_wm.m_keyboardFocus).c_str(), winText(g_wm.m_modalHead ? g_wm.m_modalHead->window : NULL).c_str(), g_layout.m_hidden ? 1 : 0);
	(void)prefix;
}
