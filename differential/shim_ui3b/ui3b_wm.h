/* A small window manager for the ui3b wrappers whose cuts send messages to windows, set the focus, go modal or walk a window tree (#215, group ui3b).
** Included INSIDE the wrapper's namespace after <vector> <string> <map> <string.h> <stdint.h>, Lib/BaseType.h and these cuts (build.rs, drafts_ui3b):
**   ui3b_msgs.inc, ui3b_gwm.inc    GameWindow.h: WindowMsgHandledType, GameWindowMessage;
**   ui3b_status.inc, ui3b_winerr.inc    GameWindow.h: the WIN_STATUS_* bits, the WIN_ERR_* codes.
** The window manager's own functions are upstream's, cut verbatim and included at the end (wm_*.inc): GameWindow::winHide and winEnable (GameWindow.cpp);
** GameWindowManager::windowHiding, winGetWindowFromId, winSendSystemMsg, winSetFocus, winSetModal, winUnsetModal (GameWindowManager.cpp).
** Stand-ins here (the classes upstream's cuts are members of, cut down to the members they use; the window manager is verified against its C++ by tests/differential_wm.rs):
**   fnv / keyOf / NAMEKEY / TheNameKeyGenerator   NameKeyGenerator: FNV-1a of the name, 31 bits (the Rust hosts' function; NAMEKEY_INVALID is 0);
**   WindowMsgData                    upstream: UnsignedInt. Here pointer-sized, so that `(GameWindow *)mData1` and `*(Bool *)mData2` keep 64-bit pointers;
**   GameWindow                       winSetTooltip / winSetEnabledTextColors / winSetEnabledImage are logged (the observer bar's); id, status word, parent / first child / next sibling, system callback, layout, button data (a token), position and size, the decorated name;
**                                    winSetStatus / winClearStatus (GameWindow.cpp: OR in / clear the bits, returning the old status);
**   ModalWindow, newInstance         the modal stack's node (GameWindow.h: window, next; deleteInstance is delete);
**   WindowLayout                     the layout's windows (head first), hidden flag, deleted flag: hide(b) hides every window (WindowLayout::hide), destroyWindows() calls
**                                    winDestroy on each, deleteInstance() marks it, getFirstWindow();
**   GameWindowManager                m_windowList, m_keyboardFocus, m_modalHead, m_mouseCaptor (never set); winCreateLayout(file) logs [912] and makes a new layout of the case's windows, or NULL;
**                                    winDestroy(window): DESTROYED set, the focus, the modal top and the captor given up, the children destroyed, the window unlinked from its parent's list or
**                                    the window list (GameWindowManager::winDestroy and unlinkWindow / unlinkChildWindow without the doubly linked previous pointers, the destroy list and
**                                    processDestroyList's freeing; an unlinked child loses its parent reference, as unlinkChildWindow does);
**   TestSystem                       the system callback of a window with no callback of the unit's own: GWM_INPUT_FOCUS logs [900, id, mData1] and answers TRUE when mData1 is TRUE
**                                    (the Rust hosts' `focus_system`), everything else ignored.
** g_log (lg) is the call log every wrapper puts first in its output. */
typedef Int NameKeyType;
enum { NAMEKEY_INVALID = 0 };
typedef uintptr_t WindowMsgData;

static unsigned fnv(const char *s)
{
	unsigned h = 2166136261u;
	for (; *s; ++s) { h ^= (unsigned char)*s; h *= 16777619u; }
	return h;
}
static NameKeyType keyOf(const char *s) { return (NameKeyType)(fnv(s) & 0x7FFFFFFFu); }
#define NAMEKEY(name) keyOf(name)
struct NameKeyGenerator { NameKeyType nameToKey(const char *n) { return keyOf(n); } };
static NameKeyGenerator g_nkg;
static NameKeyGenerator *TheNameKeyGenerator = &g_nkg;

static std::vector<unsigned> g_log;
static void lg(unsigned a) { g_log.push_back(a); }

#define newInstance(T) (new T)
#define DEBUG_LOG(x)

class GameWindow;
class WindowLayout;
typedef WindowMsgHandledType (*GameWinSystemFunc)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData);

struct ModalWindow
{
	GameWindow *window;
	ModalWindow *next;
	void deleteInstance() { delete this; }
};

struct WinInstanceData
{
	struct Name { std::string s; const char *str() { return s.c_str(); } } m_decoratedNameString;
	GameWindow *owner;   /* set by GameWindow::winGetInstanceData */
	WinInstanceData() : owner(NULL) {}
	void setVideoBuffer(long nullBuffer);           /* NULL: logs [30, window id, 0] (the window video manager's wrapper) */
	template <class B> void setVideoBuffer(B *b);   /* logs [30, window id, buffer token or 0] */
};

class GameWindow
{
public:
	Int m_id;
	unsigned m_status;
	GameWindow *m_parent, *m_child, *m_next;
	GameWinSystemFunc m_system;
	WindowLayout *m_layout;
	unsigned gdata;
	Int x, y, w, h;
	WinInstanceData inst;
	GameWindow() : m_id(0), m_status(WIN_STATUS_ENABLED), m_parent(NULL), m_child(NULL), m_next(NULL), m_system(NULL), m_layout(NULL), gdata(0), x(0), y(0), w(0), h(0) {}
	Int winGetWindowId() { return m_id; }
	unsigned winGetStatus() { return m_status; }
	GameWindow *winGetParent() { return m_parent; }
	GameWindow *winGetChild() { return m_child; }
	GameWindow *winGetNext() { return m_next; }
	WindowLayout *winGetLayout() { return m_layout; }
	WinInstanceData *winGetInstanceData() { inst.owner = this; return &inst; }
	void winGetSize(Int *px, Int *py) { *px = w; *py = h; }
	void winGetPosition(Int *px, Int *py) { *px = x; *py = y; }
	unsigned winSetStatus(unsigned bits) { unsigned old = m_status; m_status |= bits; return old; }
	unsigned winClearStatus(unsigned bits) { unsigned old = m_status; m_status &= ~bits; return old; }
	Int winHide(Bool hide);
	Bool winIsHidden() { return (m_status & WIN_STATUS_HIDDEN) != 0; }
	Int winEnable(Bool enable);
	void freeImages() {}
	/* logged window setters (the observer bar's wrapper): [6, id, length, units] tooltip, [7, id, color, back color], [8, id, image token] */
	template <class T> void winSetTooltip(T t) { lg(6); lg((unsigned)m_id); lg((unsigned)t.w.size()); for (size_t i = 0; i < t.w.size(); ++i) lg(t.w[i]); }
	void winSetEnabledTextColors(Int c, Int b) { lg(7); lg((unsigned)m_id); lg((unsigned)c); lg((unsigned)b); }
	template <class I> void winSetEnabledImage(Int, I *img) { lg(8); lg((unsigned)m_id); lg(img ? img->token : 0u); }
};

void WinInstanceData::setVideoBuffer(long) { lg(30); lg((unsigned)owner->m_id); lg(0); }
template <class B> void WinInstanceData::setVideoBuffer(B *b) { lg(30); lg((unsigned)owner->m_id); lg(b ? b->token : 0u); }

class GameWindowManager
{
public:
	GameWindow *m_windowList;
	GameWindow *m_keyboardFocus;
	GameWindow *m_mouseCaptor;
	ModalWindow *m_modalHead;
	bool layoutLoads;                       /* winCreateLayout: whether the file loads, and the windows (head first) it makes the layout of */
	std::vector<GameWindow *> layoutMembers;
	GameWindowManager() : m_windowList(NULL), m_keyboardFocus(NULL), m_mouseCaptor(NULL), m_modalHead(NULL), layoutLoads(false) {}
	void reset() { m_windowList = NULL; m_keyboardFocus = NULL; m_mouseCaptor = NULL; while (m_modalHead) { ModalWindow *n = m_modalHead->next; delete m_modalHead; m_modalHead = n; } layoutLoads = false; layoutMembers.clear(); }
	void windowHiding(GameWindow *window);
	GameWindow *winGetWindowFromId(GameWindow *window, Int id);
	WindowMsgHandledType winSendSystemMsg(GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2);
	Int winSetFocus(GameWindow *window);
	Int winSetModal(GameWindow *window);
	Int winUnsetModal(GameWindow *window);
	Int winCapture(GameWindow *) { return WIN_ERR_OK; }
	Int winDestroy(GameWindow *w);
	WindowLayout *winCreateLayout(const char *);
};
static GameWindowManager g_wm;
static GameWindowManager *TheWindowManager = &g_wm;

class WindowLayout
{
public:
	std::vector<GameWindow *> windows;   /* head first */
	bool hidden, deleted;
	WindowLayout() : hidden(false), deleted(false) {}
	void hide(Bool h) { for (size_t i = 0; i < windows.size(); ++i) windows[i]->winHide(h); hidden = h; }
	GameWindow *getFirstWindow() { return windows.empty() ? NULL : windows[0]; }
	void destroyWindows() { while (!windows.empty()) { GameWindow *w = windows[0]; windows.erase(windows.begin()); w->m_layout = NULL; TheWindowManager->winDestroy(w); } }
	void deleteInstance() { deleted = true; }
};

WindowLayout *GameWindowManager::winCreateLayout(const char *)
{
	lg(912);
	if (!layoutLoads) return NULL;
	WindowLayout *l = new WindowLayout;
	l->windows = layoutMembers;
	for (size_t i = 0; i < l->windows.size(); ++i) l->windows[i]->m_layout = l;
	return l;
}

Int GameWindowManager::winDestroy(GameWindow *w)
{
	if (w == NULL) return WIN_ERR_INVALID_WINDOW;
	if (w->m_status & WIN_STATUS_DESTROYED) return WIN_ERR_OK;
	w->m_status |= WIN_STATUS_DESTROYED;
	if (m_mouseCaptor == w) m_mouseCaptor = NULL;
	if (m_keyboardFocus == w) m_keyboardFocus = NULL;
	if (m_modalHead && m_modalHead->window == w) winUnsetModal(w);
	for (GameWindow *c = w->m_child; c; ) { GameWindow *nx = c->m_next; winDestroy(c); c = nx; }
	if (w->m_parent == NULL) {
		if (m_windowList == w) m_windowList = w->m_next;
		else for (GameWindow *p = m_windowList; p; p = p->m_next) if (p->m_next == w) { p->m_next = w->m_next; break; }
	} else {
		if (w->m_parent->m_child == w) w->m_parent->m_child = w->m_next;
		else for (GameWindow *p = w->m_parent->m_child; p; p = p->m_next) if (p->m_next == w) { p->m_next = w->m_next; break; }
		w->m_parent = NULL;   /* unlinkChildWindow: "remove the parent reference from this window" */
	}
	w->m_next = NULL;
	return WIN_ERR_OK;
}

static WindowMsgHandledType TestSystem(GameWindow *window, UnsignedInt msg, WindowMsgData mData1, WindowMsgData mData2)
{
	if (msg == GWM_INPUT_FOCUS) {
		lg(900); lg((unsigned)window->m_id); lg((unsigned)mData1);
		if (mData1 == TRUE) *(Bool *)mData2 = TRUE;
		return MSG_HANDLED;
	}
	return MSG_IGNORED;
}

#include "wm_winhide.inc"
#include "wm_winenable.inc"
#include "wm_hiding.inc"
#include "ui3b_winget.inc"
#include "wm_sendsys.inc"
#include "wm_setfocus.inc"
#include "wm_setmodal.inc"
#include "wm_unsetmodal.inc"

/* The scene block every wrapper using this manager reads: [nwin, nwin x [id, status word, parent + 1, first child + 1, next sibling + 1, system (0 TestSystem, 1 the unit's)],
** window list head + 1, focus + 1]; the windows are indexed in the Rust test's creation order. dumpScene appends every window's status word, the focus (index + 1), the
** modal stack's depth and top (index + 1). */
static std::vector<GameWindow> g_wins;
static GameWinSystemFunc g_unitSystem = NULL;
static int readScene(const unsigned *in, int p)
{
	unsigned n = in[p++];
	g_wins.assign(n, GameWindow());
	std::vector<unsigned> par(n), chi(n), nxt(n);
	for (unsigned i = 0; i < n; ++i) {
		g_wins[i].m_id = (Int)in[p++];
		g_wins[i].m_status = in[p++];
		par[i] = in[p++]; chi[i] = in[p++]; nxt[i] = in[p++];
		g_wins[i].m_system = in[p++] ? g_unitSystem : TestSystem;
	}
	for (unsigned i = 0; i < n; ++i) {
		g_wins[i].m_parent = par[i] ? &g_wins[par[i] - 1] : NULL;
		g_wins[i].m_child = chi[i] ? &g_wins[chi[i] - 1] : NULL;
		g_wins[i].m_next = nxt[i] ? &g_wins[nxt[i] - 1] : NULL;
	}
	g_wm.reset();
	unsigned head = in[p++];
	g_wm.m_windowList = head ? &g_wins[head - 1] : NULL;
	unsigned f = in[p++];
	g_wm.m_keyboardFocus = f ? &g_wins[f - 1] : NULL;
	return p;
}
static unsigned winIndex(GameWindow *w) { for (size_t i = 0; i < g_wins.size(); ++i) if (&g_wins[i] == w) return (unsigned)i + 1; return 0; }
static void dumpScene(std::vector<unsigned> &out)
{
	for (size_t i = 0; i < g_wins.size(); ++i) out.push_back(g_wins[i].m_status);
	out.push_back(winIndex(g_wm.m_keyboardFocus));
	unsigned depth = 0;
	for (ModalWindow *m = g_wm.m_modalHead; m; m = m->next) ++depth;
	out.push_back(depth);
	out.push_back(g_wm.m_modalHead ? winIndex(g_wm.m_modalHead->window) : 0);
}
