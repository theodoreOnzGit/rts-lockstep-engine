/* Differential-test wrapper for the six small WOL screens' drafts (#215, group ui3a, upstream commit 0a05454d):
**   src/drafts/ui/wol_message_window.rs, wol_status_menu.rs, wol_custom_score_screen.rs, wol_qm_score_screen.rs,
**   wol_ladder_screen.rs, wol_locale_select_popup.rs
** Cut verbatim by build.rs (fn drafts_ui3a) into OUT_DIR/draft_ui3a_gen, each from `// window ids -` to the end of the file:
**   sm_msg.inc  WOLMessageWindow.cpp;  sm_status.inc  WOLStatusMenu.cpp;  sm_custom.inc  WOLCustomScoreScreen.cpp;
**   sm_qm.inc  WOLQMScoreScreen.cpp;  sm_ladder.inc  WOLLadderScreen.cpp;  sm_locale.inc  WOLLocaleSelectPopup.cpp;
**   sm_loc.inc  PersistentStorageDefs.h, enum LocaleType (LOC_MIN, LOC_MAX);  sm_gscolor.inc  PeerDefs.h, enum GameSpyColors (to
**   GSCOLOR_MAX, which this wrapper adds);  and the window system's functions listed in ui3a_host.h.
** Each screen is compiled in a namespace of its own (their statics share names).
**
** Stand-ins (outside the cuts; each writes one line of `g_log` per call, and the Rust test's host writes the same line):
**  - TheShell: shutdownComplete(layout) logs `shutdownComplete` (when the op began with no push pending; the Rust side counts the calls
**    through the host's `shell()`, see the test), or, when a push is pending, `doPush Menus/Next.wnd` once; showShellMap(use) logs
**    `showShellMap 1|0`; pop() logs `pop` when a screen is on the stack, else nothing (Shell::pop returns on an empty stack);
**  - TheWebBrowser (WebBrowser.h): createBrowserWindow(name, window) logs `web.create <name> <window>`, closeBrowserWindow(window) logs
**    `web.close <window>`; NULL when the case has none;
**  - TheGameSpyInfo, TheGameSpyPSMessageQueue, GameSpyMiscPreferences, GameSpyCloseOverlay, CheckReOpenPlayerInfo, GameSpyColor: the
**    GameSpy layer (a service that is gone): each call logs a line (`ps.addRequest ...`, `closeOverlay`, `prefs.setLocale`, `ps.track`,
**    `cached.set`, `ps.addResponse`, `checkReopen`); findPlayerStatsByID answers the case's stats; getLocalProfileID, the email, nick
**    and password are the case's constants;
**  - the window system and the listbox: ui3a_host.h.
** Excluded inputs (upstream dereferences NULL or reads an unset variable): GBM_SELECTED with a NULL control sent to the ladder or the locale
** popup; the locale popup's OK pressed with no list box (`int selected` is then unset upstream).
**
** Entry point ref_u3a_small_run(in, n, out, cap): `in` is the case (see tests/differential_draft_wol_small.rs). Returns the log's length. */
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include <memory>
#include "Lib/BaseType.h"
#include "GameClient/KeyDefs.h"
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(x, y)   /* compiled out in the release build, as upstream's */

namespace ui3a_small {
#include "ui3a_host.h"

/* ---- the shell ---- */
Bool g_pendingPush, g_stackTop, g_countable;
class Shell
{
public:
	void shutdownComplete(WindowLayout *)
	{
		if (g_pendingPush) { u3aLog("doPush Menus/Next.wnd"); g_pendingPush = FALSE; }
		else if (g_countable) u3aLog("shutdownComplete");
	}
	void showShellMap(Bool use) { u3aLog("showShellMap %d", use ? 1 : 0); }
	void pop() { if (g_stackTop) u3aLog("pop"); }
};
Shell g_shell;
Shell *TheShell = &g_shell;

/* ---- the web browser ---- */
class WebBrowser
{
public:
	void createBrowserWindow(const char *name, GameWindow *w) { u3aLog("web.create %s %s", name, winText(w).c_str()); }
	void closeBrowserWindow(GameWindow *w) { u3aLog("web.close %s", winText(w).c_str()); }
};
WebBrowser g_web;
WebBrowser *TheWebBrowser = NULL;

/* ---- the GameSpy layer ---- */
#include "sm_loc.inc"
#include "sm_gscolor.inc"
	GSCOLOR_MAX
};
Color GameSpyColor[GSCOLOR_MAX];
enum { GSOVERLAY_LOCALESELECT = 7 };
struct PSPlayerStats { Int id; Int locale; };
struct PSRequest
{
	enum { PSREQUEST_UPDATEPLAYERLOCALE = 1 } requestType;
	PSPlayerStats player;
	const char *email, *nick, *password;
};
struct PSResponse
{
	enum { PSRESPONSE_PLAYERSTATS = 1 } responseType;
	PSPlayerStats player;
};
Int g_localId, g_statsKind, g_statsLocale;
PSPlayerStats g_cached;
class GameSpyInfo
{
public:
	AsciiString m_email, m_nick, m_password;
	GameSpyInfo() : m_email("me@example.org"), m_nick("Nick"), m_password("pw") {}
	AsciiString getLocalEmail() { return m_email; }
	AsciiString getLocalBaseName() { return m_nick; }
	AsciiString getLocalPassword() { return m_password; }
	Int getLocalProfileID() { return g_localId; }
	PSPlayerStats getCachedLocalPlayerStats() { return g_cached; }
	void setCachedLocalPlayerStats(PSPlayerStats s) { u3aLog("cached.set %d %d", s.id, s.locale); g_cached = s; }
};
GameSpyInfo g_info;
GameSpyInfo *TheGameSpyInfo = &g_info;
class PSMessageQueue
{
public:
	void addRequest(const PSRequest &r) { u3aLog("ps.addRequest %d locale %d id %d %s %s %s", (int)r.requestType, r.player.locale, r.player.id, r.email, r.nick, r.password); }
	PSPlayerStats findPlayerStatsByID(Int id)
	{
		(void)id;
		PSPlayerStats s;
		s.id = g_statsKind == 0 ? 0 : g_statsKind == 1 ? g_localId : 777;
		s.locale = g_statsLocale;
		return s;
	}
	void trackPlayerStats(PSPlayerStats s) { u3aLog("ps.track %d %d", s.id, s.locale); }
	void addResponse(const PSResponse &r) { u3aLog("ps.addResponse %d %d %d", (int)r.responseType, r.player.id, r.player.locale); }
};
PSMessageQueue g_psq;
PSMessageQueue *TheGameSpyPSMessageQueue = &g_psq;
void GameSpyCloseOverlay(Int which) { u3aLog("closeOverlay %d", which); }
class GameSpyMiscPreferences
{
public:
	void setLocale(Int l) { u3aLog("prefs.setLocale %d", l); }
	void write() { u3aLog("prefs.write"); }
};
void CheckReOpenPlayerInfo() { u3aLog("checkReopen"); }

/* ---- the six screens' cuts ---- */
namespace c_msg {
#include "sm_msg.inc"
}
namespace c_status {
#include "sm_status.inc"
}
namespace c_custom {
#include "sm_custom.inc"
}
namespace c_qm {
#include "sm_qm.inc"
}
namespace c_ladder {
#include "sm_ladder.inc"
}
namespace c_locale {
#include "sm_locale.inc"
}

/* ---- trampolines: every callback entered is logged ---- */
WindowMsgHandledType defaultSystem(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
WindowMsgHandledType sysMsg(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedSystem(c_msg::WOLMessageWindowSystem, w, m, a, b); }
WindowMsgHandledType inpMsg(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedInput(c_msg::WOLMessageWindowInput, w, m, a, b); }
WindowMsgHandledType sysStatus(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedSystem(c_status::WOLStatusMenuSystem, w, m, a, b); }
WindowMsgHandledType inpStatus(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedInput(c_status::WOLStatusMenuInput, w, m, a, b); }
WindowMsgHandledType sysCustom(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedSystem(c_custom::WOLCustomScoreScreenSystem, w, m, a, b); }
WindowMsgHandledType inpCustom(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedInput(c_custom::WOLCustomScoreScreenInput, w, m, a, b); }
WindowMsgHandledType sysQm(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedSystem(c_qm::WOLQMScoreScreenSystem, w, m, a, b); }
WindowMsgHandledType inpQm(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedInput(c_qm::WOLQMScoreScreenInput, w, m, a, b); }
WindowMsgHandledType sysLadder(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedSystem(c_ladder::WOLLadderScreenSystem, w, m, a, b); }
WindowMsgHandledType inpLadder(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedInput(c_ladder::WOLLadderScreenInput, w, m, a, b); }
WindowMsgHandledType sysLocale(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedSystem(c_locale::WOLLocaleSelectSystem, w, m, a, b); }
WindowMsgHandledType inpLocale(GameWindow *w, UnsignedInt m, WindowMsgData a, WindowMsgData b) { return loggedInput(c_locale::WOLLocaleSelectInput, w, m, a, b); }

struct MenuSpec
{
	const char *file;
	const char *win[3];
	int count;
	GameWinSystemFunc sys;
	GameWinInputFunc inp;
};
const MenuSpec g_menus[6] = {
	{ "WOLMessageWindow.wnd", { "WOLMessageWindowParent", "ButtonCancel", "" }, 2, sysMsg, inpMsg },
	{ "WOLStatusMenu.wnd", { "WOLStatusMenuParent", "ButtonDisconnect", "ListboxStatus" }, 3, sysStatus, inpStatus },
	{ "WOLCustomScoreScreen.wnd", { "WOLCustomScoreScreenParent", "ButtonDisconnect", "ButtonLobby" }, 3, sysCustom, inpCustom },
	{ "WOLQMScoreScreen.wnd", { "WOLQMScoreScreenParent", "ButtonDisconnect", "ButtonQuickMatch" }, 3, sysQm, inpQm },
	{ "WOLLadderScreen.wnd", { "LadderParent", "ButtonBack", "WindowLadder" }, 3, sysLadder, inpLadder },
	{ "PopupLocaleSelect.wnd", { "ParentLocaleSelect", "ButtonOk", "ListBoxLocaleSelect" }, 3, sysLocale, inpLocale },
};
const UnsignedInt g_msgs[8] = { GWM_CREATE, GWM_DESTROY, GWM_CHAR, GWM_INPUT_FOCUS, GBM_SELECTED, GEM_EDIT_DONE, GLM_SELECTED, GWM_LEFT_DOWN };

void callInit(int menu, WindowLayout *l)
{
	switch (menu)
	{
	case 0: c_msg::WOLMessageWindowInit(l, NULL); break;
	case 1: c_status::WOLStatusMenuInit(l, NULL); break;
	case 2: c_custom::WOLCustomScoreScreenInit(l, NULL); break;
	case 3: c_qm::WOLQMScoreScreenInit(l, NULL); break;
	case 4: c_ladder::WOLLadderScreenInit(l, NULL); break;
	default: c_locale::WOLLocaleSelectInit(l, NULL); break;
	}
}
void callShutdown(int menu, WindowLayout *l)
{
	switch (menu)
	{
	case 0: c_msg::WOLMessageWindowShutdown(l, NULL); break;
	case 1: c_status::WOLStatusMenuShutdown(l, NULL); break;
	case 2: c_custom::WOLCustomScoreScreenShutdown(l, NULL); break;
	case 3: c_qm::WOLQMScoreScreenShutdown(l, NULL); break;
	case 4: c_ladder::WOLLadderScreenShutdown(l, NULL); break;
	default: c_locale::WOLLocaleSelectShutdown(l, NULL); break;
	}
}
void callUpdate(int menu, WindowLayout *l)
{
	switch (menu)
	{
	case 0: c_msg::WOLMessageWindowUpdate(l, NULL); break;
	case 1: c_status::WOLStatusMenuUpdate(l, NULL); break;
	case 2: c_custom::WOLCustomScoreScreenUpdate(l, NULL); break;
	case 3: c_qm::WOLQMScoreScreenUpdate(l, NULL); break;
	case 4: c_ladder::WOLLadderScreenUpdate(l, NULL); break;
	default: c_locale::WOLLocaleSelectUpdate(l, NULL); break;
	}
}

void dumpCase(const MenuSpec &m, bool list)
{
	const char *names[3] = { m.win[0], m.win[1], m.win[2] };
	dumpWindows(names, m.count, "");
	if (list)
	{
		std::string s;
		for (size_t i = 0; i < g_list.rows.size(); ++i)
		{
			char b[32];
			snprintf(b, sizeof b, "{%d}", (int)g_list.rows[i].color);
			s += " [" + narrow(g_list.rows[i].text) + "]" + b;
		}
		u3aLog("list %d sel %d%s", (int)g_list.rows.size(), g_list.selectPos, s.c_str());
	}
}

} /* namespace ui3a_small */

/* in: menu, mask, parentNoFocus, layoutHidden0, pendingPush, hasWeb, color, localId, statsKind, statsLocale, stackTop, listLength, nops,
** then nops ops of 5 ints: code, msgIdx, a, b, c (codes: 1 init, 2 shutdown, 3 update, 4 input to the parent, 5 system to the parent,
** 6 winSetFocus(parent) (a 0) or winSetFocus(NULL) (a -1)). */
extern "C" int ref_u3a_small_run(const int *in, int n, char *out, int cap)
{
	using namespace ui3a_small;
	(void)n;
	resetWindows();
	g_list = ListState();
	g_list.selectPos = -1;
	const int menu = in[0], mask = in[1];
	const MenuSpec &m = g_menus[menu];
	g_layout.m_hidden = in[3] != 0;
	g_pendingPush = in[4] != 0;
	g_stackTop = in[10] != 0;
	TheWebBrowser = in[5] ? &g_web : NULL;
	for (int i = 0; i < GSCOLOR_MAX; ++i) GameSpyColor[i] = 0;
	GameSpyColor[GSCOLOR_DEFAULT] = in[6];
	g_localId = in[7];
	g_statsKind = in[8];
	g_statsLocale = in[9];
	g_cached.id = 0;
	g_cached.locale = 0;
	g_list.listLength = in[11];
	const int nops = in[12];
	/* the windows: the parent (hidden, as the WND makes it) and its children, in the WND's order */
	GameWindow *parent = NULL;
	GameWindow *ws[3] = { NULL, NULL, NULL };
	std::string pre = std::string(m.file) + ":";
	if (mask & 1)
	{
		UnsignedInt st = WIN_STATUS_ENABLED | WIN_STATUS_HIDDEN | (in[2] ? WIN_STATUS_NO_FOCUS : 0);
		parent = makeWindow(pre + m.win[0], NULL, st, m.sys, m.inp);
		ws[0] = parent;
	}
	for (int i = 1; i < m.count; ++i)
	{
		if (!(mask & (1 << i))) continue;
		ws[i] = makeWindow(pre + m.win[i], parent, WIN_STATUS_ENABLED, defaultSystem, defaultSystem);
	}
	if ((menu == 1 || menu == 4 || menu == 5) && ws[2]) g_list.win = ws[2];
	const int *op = in + 13;
	for (int k = 0; k < nops; ++k, op += 5)
	{
		const int code = op[0];
		u3aLog("> %d %d %d %d %d", op[0], op[1], op[2], op[3], op[4]);
		g_countable = !g_pendingPush;
		const UnsignedInt msg = g_msgs[op[1] & 7];
		if (code == 1) callInit(menu, &g_layout);
		else if (code == 2) callShutdown(menu, &g_layout);
		else if (code == 3) callUpdate(menu, &g_layout);
		else if (code == 4)
		{
			WindowMsgHandledType r = g_wm.winSendInputMsg(parent, msg, (WindowMsgData)(unsigned)op[2], (WindowMsgData)(unsigned)op[3]);
			u3aLog("ret %d", (int)r);
		}
		else if (code == 5)
		{
			WindowMsgData d1 = (WindowMsgData)(unsigned)op[2], d2 = (WindowMsgData)(unsigned)op[3];
			if (msg == GBM_SELECTED || msg == GLM_SELECTED)
			{
				GameWindow *c = (op[2] >= 0 && op[2] < 3) ? ws[op[2]] : NULL;
				d1 = (WindowMsgData)c;
				if (msg == GBM_SELECTED) d2 = (WindowMsgData)g_nameKeys.nameToKey(AsciiString((pre + m.win[op[3] % 3]).c_str()));
				else d2 = (WindowMsgData)(Int)op[3];
			}
			Bool reply = FALSE;   /* GWM_INPUT_FOCUS's mData2 is a pointer to the answer */
			if (msg == GWM_INPUT_FOCUS) d2 = (WindowMsgData)&reply;
			WindowMsgHandledType r = g_wm.winSendSystemMsg(parent, msg, d1, d2);
			u3aLog("ret %d", (int)r);
			if (msg == GWM_INPUT_FOCUS) u3aLog("reply %d", reply ? 1 : 0);
		}
		else if (code == 6)
		{
			g_wm.winSetFocus(op[2] < 0 ? NULL : parent);
		}
		dumpCase(m, menu == 5);
	}
	int len = (int)g_log.size();
	if (len > cap) len = cap;
	memcpy(out, g_log.data(), len);
	return (int)g_log.size();
}
