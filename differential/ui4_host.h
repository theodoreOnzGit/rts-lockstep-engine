/* The shared stand-ins of group ui4's reference programs (#215), included by each differential/wrap_draft_ui4_<menu>.cpp right after the generated
** `ui4_wm_base.inc` (differential/wrap_wm.cpp's text; see build.rs `drafts_ui4`). What they are:
**   - `WindowLayoutInfo::WindowLayoutInfo()` (GameWindowManagerScript.cpp's): zero members; WindowLayout::load, its only user here, is never called.
**   - `GameWindowManager::winCreateLayout(filename)`: GameWindowManagerScript.cpp's reader (never run here): a new WindowLayout holding the windows the
**     case's last `@ lay` operation named (head first, as the `addWindow` calls in reverse order make them), the call logged as `layout <file>`.
**     WindowLayout.cpp itself is upstream's text (wl_cpp.inc, from `WindowLayout::WindowLayout` on).
**   - TheGameText: GameText.cpp's string file: `fetch(label)` is `T:` + the label, except `Network:PlayerLeftGame`, which is `T:Network:PlayerLeftGame:%ls:`.
**   - UnicodeString::trim() and set(const UnicodeString &) (shim_ui4/Common/UnicodeString.h: the shim's class with those two members, transcribed from
**     UnicodeString.cpp).
** A text on the program's input is its UTF-16 units in hex, comma separated; `-` is the empty text. */
#include <list>
#include "GameClient/WindowLayout.h"
#include "GameClient/GameText.h"

/* ---- the layouts */
static std::vector<int> u4_layout_members;
static std::vector<WindowLayout *> u4_layouts;
WindowLayout *GameWindowManager::winCreateLayout(AsciiString filename)
{
	WindowLayout *l = new WindowLayout;
	g_log << "layout " << filename.str() << "\n";
	for (size_t i = u4_layout_members.size(); i-- > 0;)
	{
		GameWindow *w = (u4_layout_members[i] >= 0 && u4_layout_members[i] < (int)g_windows.size() && !g_freed[u4_layout_members[i]]) ? g_windows[u4_layout_members[i]] : NULL;
		if (w) l->addWindow(w);
	}
	u4_layouts.push_back(l);
	return l;
}
WindowLayoutInfo::WindowLayoutInfo() : version(0), init(NULL), update(NULL), shutdown(NULL) {}
#include "wl_cpp.inc"

/* ---- text */
static UnicodeString u4_text(const std::string &t)
{
	UnicodeString u;
	if (t == "-") return u;
	size_t p = 0;
	while (p < t.size())
	{
		size_t c = t.find(',', p);
		if (c == std::string::npos) c = t.size();
		u.concat((WideChar)strtoul(t.substr(p, c - p).c_str(), NULL, 16));
		p = c + 1;
	}
	return u;
}
class U4GameText : public GameTextInterface
{
public:
	virtual void init() {}
	virtual void reset() {}
	virtual void update() {}
	virtual UnicodeString fetch(const Char *label, Bool *exists = NULL) { return fetch(AsciiString(label), exists); }
	virtual UnicodeString fetch(AsciiString label, Bool *exists = NULL)
	{
		std::string s = std::string("T:") + label.str();
		if (s == "T:Network:PlayerLeftGame") s += ":%ls:";
		UnicodeString u;
		for (size_t i = 0; i < s.size(); ++i) u.concat((WideChar)(unsigned char)s[i]);
		if (exists) *exists = TRUE;
		return u;
	}
	virtual AsciiStringVec &getStringsWithLabelPrefix(AsciiString) { static AsciiStringVec v; return v; }
	virtual void initMapStringFile(const AsciiString &) {}
};
static U4GameText u4_game_text;
GameTextInterface *TheGameText = &u4_game_text;

struct U4Menu
{
	const char *name;
	GameWinSystemFunc sys;
	GameWinInputFunc inp;
};

static int u4_li(std::istringstream &in) { int a = 0; in >> a; return a; }
static GameWindow *u4_win(int i) { return (i >= 0 && i < (int)g_windows.size() && !g_freed[i]) ? g_windows[i] : NULL; }
static int u4_lay_index(WindowLayout *l)
{
	for (size_t i = 0; i < u4_layouts.size(); ++i)
		if (u4_layouts[i] == l) return (int)i;
	return -1;
}


