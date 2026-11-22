/* The reference PROGRAM for src/drafts/ui/disconnect_menu.rs and disconnect_window.rs (#215, group ui4, upstream commit 0a05454d): upstream's DisconnectMenu.cpp
** and DisconnectWindow.cpp on upstream's OWN window manager and gadgets.
**
** build.rs (`drafts_ui4`) compiles, into non-PIE programs, the files of #70's `wm_ref` (GameWindowManager.cpp, GameWindow.cpp, WinInstanceData.cpp,
** GameWindowGlobal.cpp and the eleven Gadget*.cpp, cut and edited exactly as there; see differential/wrap_wm.cpp), and this file, which INCLUDES
** differential/wrap_wm.cpp (the generated copy `ui4_wm_base.inc`: its `main` renamed `wm_ref_main`, and its three aborting stand-ins for
** WindowLayout::addWindow, WindowLayout::removeWindow and GameWindowManager::winCreateLayout removed). So every stand-in that file lists (the window manager,
** the display strings, the fonts, the images, the name keys: 1, 2, ... in order of first use, the clock, the dump of the windows) is the same here, and so is
** its 64-bit scheme (the heap below 4 GB, the case run on a stack allocated from it: upstream's `(WindowMsgData)window` keeps its pointers). The shared
** stand-ins (layouts, game text, the operations, the main) are in differential/ui4_host.h and ui4_main.h. The path is `UI4_DISCONNECT_EXE`.
**
** Upstream text, cut VERBATIM by build.rs (each anchor asserted once):
**   disconnect_window.inc   GUICallbacks/Menus/DisconnectWindow.cpp, from `// Private Data` to the end;
**   disconnect_menu.inc     DisconnectMenu/DisconnectMenu.cpp, from the first table to the end, ONE edit: `text.format(fmt, playerName.str())`
**                           becomes `ui4_format_ls(text, fmt, playerName)` (the shim's UnicodeString::format is glibc's vswprintf, whose `%ls` reads
**                           32-bit units, not the 16-bit WideChar the game's; see ui4_format_ls below).
**
** Stand-ins of this file:
**   - TheNetwork (NetworkInterface.h's Network): `sendDisconnectChat(text)`, `quitGame()`, `voteForPlayerDisconnect(slot)` log `net chat [units]`, `net quit`,
**     `net vote <slot>`. HideDiplomacy, HideInGameChat, HideQuitMenu (GUICallbacks.h's, Diplomacy.cpp, InGameChat.cpp, QuitMenu.cpp) log `hide diplomacy`, ...
**   - `itoa(Int, char *, 10)`: the C runtime's (MSVC), over a 32-bit int: DisconnectMenu's `time_t` is 32-bit in the game (VC6), so the value is truncated to Int.
**   - the DisconnectManager class (shim_ui4/GameNetwork/DisconnectManager.h): only the pointer type and MAX_SLOTS.
**
** Operations of this program (besides ui4_main.h's):
**   @ dm <fn> ...   DisconnectMenu's methods (init, attach <0|1>, show, hide, name <slot> <text>, timeout <slot> <t>, showctl <slot>, hidectl <slot>,
**                   routershow, routerhide, routertime <t>, chat <text>, showchat <text>, quit, remove <slot> <text>, vote <slot>, votes <slot> <n>)
**   @ dw <fn>       DisconnectWindow's: show, hide
** The dump adds `dm attached <0|1> visible <0|1>` (once a `dm` operation has run) and the window's statics. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/DisconnectMenu.h"

/* the `%ls` of GameText's `Network:PlayerLeftGame`: its first occurrence replaced by the name (UnicodeString::format's _vsnwprintf does the same for a
** format with that one specifier) */
static void ui4_format_ls(UnicodeString &out, UnicodeString fmt, UnicodeString name)
{
	UnicodeString r;
	bool done = false;
	for (Int i = 0; i < fmt.getLength(); ++i)
	{
		if (!done && fmt.getCharAt(i) == '%' && i + 2 < fmt.getLength() + 0 && fmt.getCharAt(i + 1) == 'l' && fmt.getCharAt(i + 2) == 's')
		{
			for (Int k = 0; k < name.getLength(); ++k) r.concat(name.getCharAt(k));
			i += 2;
			done = true;
		}
		else
			r.concat(fmt.getCharAt(i));
	}
	out = r;
}

/* ---- the network and the screens the disconnect menu hides */
class U4Network
{
public:
	void sendDisconnectChat(UnicodeString t) { g_log << "net chat"; dump_text(g_log, t); g_log << "\n"; }
	void quitGame() { g_log << "net quit\n"; }
	void voteForPlayerDisconnect(Int slot) { g_log << "net vote " << slot << "\n"; }
};
static U4Network u4_network;
static U4Network *TheNetwork = &u4_network;
static void HideDiplomacy(void) { g_log << "hide diplomacy\n"; }
static void HideInGameChat(void) { g_log << "hide ingamechat\n"; }
static void HideQuitMenu(void) { g_log << "hide quitmenu\n"; }
#define itoa ui4_itoa
static char *ui4_itoa(Int v, char *buf, int radix) { sprintf(buf, "%d", v); return buf; }

/* ---- the disconnect menu and window */
void ShowDisconnectWindow(void);
void HideDisconnectWindow(void);
#include "disconnect_menu.inc"
#include "disconnect_window.inc"
class U4DisconnectMenu : public DisconnectMenu
{
public:
	DisconnectManager *manager() { return m_disconnectManager; }
	DisconnectMenuStateType state() { return m_menuState; }
};
static DisconnectManager u4_manager;


static bool u4_dm_used = false;
static const U4Menu u4_menus[] = {
	{ "disconnect", DisconnectControlSystem, DisconnectControlInput },
};
static void u4_menu_setup(void) { TheDisconnectMenu = new U4DisconnectMenu; }
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd == "dw") {
		std::string f;
		in >> f;
		if (f == "show") ShowDisconnectWindow();
		else if (f == "hide") HideDisconnectWindow();
		return true;
	}
	if (cmd != "dm") return false;
	{
		std::string f;
		in >> f;
		DisconnectMenu *m = TheDisconnectMenu;
		u4_dm_used = true;
		std::string t;
		int a;
		if (f == "init") m->init();
		else if (f == "attach") { a = u4_li(in); m->attachDisconnectManager(a ? &u4_manager : NULL); }
		else if (f == "show") m->showScreen();
		else if (f == "hide") m->hideScreen();
		else if (f == "name") { in >> a >> t; m->setPlayerName(a, u4_text(t)); }
		else if (f == "timeout") { int v; in >> a >> v; m->setPlayerTimeoutTime(a, (time_t)v); }
		else if (f == "showctl") m->showPlayerControls(u4_li(in));
		else if (f == "hidectl") m->hidePlayerControls(u4_li(in));
		else if (f == "routershow") m->showPacketRouterTimeout();
		else if (f == "routerhide") m->hidePacketRouterTimeout();
		else if (f == "routertime") { int v = u4_li(in); m->setPacketRouterTimeoutTime((time_t)v); }
		else if (f == "chat") { in >> t; m->sendChat(u4_text(t)); }
		else if (f == "showchat") { in >> t; m->showChat(u4_text(t)); }
		else if (f == "quit") m->quitGame();
		else if (f == "remove") { in >> a >> t; m->removePlayer(a, u4_text(t)); }
		else if (f == "vote") m->voteForPlayer(u4_li(in));
		else if (f == "votes") { int v; in >> a >> v; m->updateVotes(a, v); }
	}
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	U4DisconnectMenu *m = (U4DisconnectMenu *)TheDisconnectMenu;
	if (u4_dm_used) o << "dm attached " << (m->manager() ? 1 : 0) << " visible " << (int)m->isScreenVisible() << "\n";
	o << "dw layout " << u4_lay_index(disconnectMenuLayout) << " te " << widx(textEntryWindow) << " td " << widx(textDisplayWindow) << " quit " << widx(buttonQuitWindow) << " votes";
	GameWindow *vw[7] = { buttonVotePlayer1Window, buttonVotePlayer2Window, buttonVotePlayer3Window, buttonVotePlayer4Window, buttonVotePlayer5Window, buttonVotePlayer6Window, buttonVotePlayer7Window };
	for (int i = 0; i < 7; ++i) o << " " << widx(vw[i]);
	o << " ids " << (textEntryID == NAMEKEY_INVALID ? 0 : 1) << (textDisplayID == NAMEKEY_INVALID ? 0 : 1) << (buttonQuitID == NAMEKEY_INVALID ? 0 : 1) << "\n";
}
#include "ui4_main.h"
