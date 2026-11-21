/* Stand-ins the ui3b drafts' wrappers (#215, group ui3b) share. Included INSIDE each wrapper's own namespace, after the system headers
** and Lib/BaseType.h. Each says what it replaces; the wrapper that includes it lists the rest of its own.
**
**   fnv / keyOf / NAMEKEY / TheNameKeyGenerator   NameKeyGenerator: FNV-1a of the name, 31 bits (the Rust hosts' function; NAMEKEY_INVALID is 0).
**   WindowMsgData                                  upstream: UnsignedInt. Here a pointer-sized integer, so that the cut's `(GameWindow *)mData1` and
**                                                  `*(Bool *)mData2` keep a 64-bit pointer (upstream's are 32-bit pointers on Windows).
**   g_log / lg                                     the words a wrapper logs (calls on the stand-ins, in order).
**   GameWindow                                     a window: id, status bits (hidden, enabled, no-focus), position, size, decorated name, parent, first child,
**                                                  next sibling. winHide / winEnable / winSetStatus / winClearStatus change the status word only (the children, the image freeing and the modal stack of the
**                                                  real GameWindow.cpp are the window manager's, tested by tests/differential_wm.rs).
**   TheWindowManager                               winGetWindowFromId is upstream's own (ui3b_winget.inc, cut verbatim from GameWindowManager.cpp, over m_windowList / m_next /
**                                                  m_child), winSetFocus (a window with
**                                                  no-focus refuses; the old one is sent INPUT_FOCUS 0, the new one INPUT_FOCUS 1 and always wants it, as the Rust
**                                                  test windows' system callback answers), both logged; a hidden window loses the focus (windowHiding).
**   WindowLayout / winCreateLayout                 a layout: its first window; winCreateLayout answers the case's layout (or NULL); destroyWindows and
**                                                  deleteInstance are logged. */
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

class GameWindow;
static GameWindow *g_focus = NULL;
struct WinInstanceData
{
	struct Name { std::string s; const char *str() { return s.c_str(); } } m_decoratedNameString;
	GameWindow *owner;   /* set by GameWindow::winGetInstanceData */
	WinInstanceData() : owner(NULL) {}
	void setVideoBuffer(long nullBuffer);           /* NULL */
	template <class B> void setVideoBuffer(B *b);   /* logs [30, window id, buffer token or 0] (the window video manager's wrapper) */
};
class GameWindow
{
public:
	Int id;
	unsigned gdata;                    /* the button's data (GadgetButtonGetData / setControlCommand), a token; 0 is NULL */
	unsigned status;                   /* WIN_STATUS_* bits: 0x08 enabled, 0x10 hidden, 0x400 no-focus, the others as the case sets them */
	Int x, y, w, h;
	WinInstanceData inst;
	GameWindow *m_parent, *m_child, *m_next;
	GameWindow() : id(0), gdata(0), status(0x08), x(0), y(0), w(0), h(0), m_parent(NULL), m_child(NULL), m_next(NULL) {}
	Int winGetWindowId() { return id; }
	WinInstanceData *winGetInstanceData() { inst.owner = this; return &inst; }
	/* windowHiding: a window that is hidden loses the keyboard focus (the modal stack and capture are not used by these wrappers) */
	void winHide(Bool hide) { if (hide) status |= 0x10; else status &= ~0x10u; if (hide && g_focus == this) g_focus = NULL; }
	Bool winIsHidden() { return (status & 0x10) != 0; }
	void winEnable(Bool en) { if (en) status |= 0x08; else status &= ~0x08u; }
	unsigned winSetStatus(unsigned bits) { unsigned old = status; status |= bits; return old; }
	unsigned winClearStatus(unsigned bits) { unsigned old = status; status &= ~bits; return old; }
	void winGetSize(Int *px, Int *py) { *px = w; *py = h; }
	void winGetPosition(Int *px, Int *py) { *px = x; *py = y; }
	GameWindow *winGetChild() { return m_child; }
	GameWindow *winGetNext() { return m_next; }
};

void WinInstanceData::setVideoBuffer(long) { lg(30); lg((unsigned)owner->id); lg(0); }
template <class B> void WinInstanceData::setVideoBuffer(B *b) { lg(30); lg((unsigned)owner->id); lg(b ? b->token : 0u); }

class WindowLayout
{
public:
	GameWindow *first;
	WindowLayout() : first(NULL) {}
	GameWindow *getFirstWindow() { return first; }
	void destroyWindows() { lg(910); }
	void deleteInstance() { lg(911); }
};

class GameWindowManager
{
public:
	std::vector<GameWindow *> table;   /* the top-level windows in the Rust window list's order */
	GameWindow *m_windowList;          /* the head of the list, linked through m_next */
	WindowLayout *layoutToReturn;       /* winCreateLayout's answer */
	GameWindowManager() : m_windowList(NULL), layoutToReturn(NULL) {}
	GameWindow *winGetWindowFromId(GameWindow *window, Int id);   /* upstream's own, cut verbatim into ui3b_winget.inc */
	WindowLayout *winCreateLayout(const char *) { lg(912); return layoutToReturn; }
	void winSetFocus(GameWindow *w)
	{
		if (w && (w->status & 0x400)) return;
		if (g_focus && g_focus != w) { lg(900); lg(g_focus->id); lg(0); }
		g_focus = w;
		if (g_focus) { lg(900); lg(g_focus->id); lg(1); }
	}
};
static GameWindowManager g_wm;
static GameWindowManager *TheWindowManager = &g_wm;

/* The scene block every wrapper reads: [nwin, nwin x [id, status word], focus index + 1 (0 none)], the windows in the Rust list's
** order (the C++ table is the same order; no parents or children). dumpScene appends every window's status word and the focus (index + 1), the words
** the Rust's `put_state` writes (tests/ui3b_common/mod.rs). */
static std::vector<GameWindow> g_wins;
static int readScene(const unsigned *in, int p)
{
	unsigned n = in[p++];
	g_wins.assign(n, GameWindow());
	g_wm.table.clear();
	for (unsigned i = 0; i < n; ++i) {
		g_wins[i].id = (Int)in[p++];
		g_wins[i].status = in[p++];
	}
	for (unsigned i = 0; i < n; ++i) g_wm.table.push_back(&g_wins[i]);
	for (unsigned i = 0; i + 1 < n; ++i) g_wins[i].m_next = &g_wins[i + 1];
	g_wm.m_windowList = n ? &g_wins[0] : NULL;
	unsigned f = in[p++];
	g_focus = f ? &g_wins[f - 1] : NULL;
	return p;
}
static void dumpScene(std::vector<unsigned> &out)
{
	for (size_t i = 0; i < g_wins.size(); ++i) out.push_back(g_wins[i].status);
	unsigned f = 0;
	for (size_t i = 0; i < g_wins.size(); ++i) if (g_focus == &g_wins[i]) f = (unsigned)i + 1;
	out.push_back(f);
}
