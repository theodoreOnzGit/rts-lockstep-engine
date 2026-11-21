/* The reference PROGRAM for src/drafts/ui/network_direct_connect.rs (#215, group ui5, upstream commit 0a05454d): upstream's NetworkDirectConnect.cpp on upstream's OWN
** window manager and gadgets (the program of group ui4, see differential/wrap_draft_ui4_disconnect.cpp for how it is built; the shared stand-ins are in ui4_host.h and
** ui4_main.h), built by build.rs `drafts_ui5` with differential/shim_ui5 ahead of the shim directories. The path is `UI5_NDC_EXE`.
**
** Upstream text, cut VERBATIM by build.rs (each anchor asserted once):
**   ndc.inc          GUICallbacks/Menus/NetworkDirectConnect.cpp, from `extern Bool LANbuttonPushed;` to the end. THREE edits, all for the 16-bit WideChar of the game against
**                    the shim's 16-bit strings: the three `translate` calls become `ui5_translate(<out>, <in>)` (the shim's AsciiString::translate is the Win32 one, but
**                    through a template the cut cannot name; ui5_translate is upstream's `concat((char)c)`: a unit whose low byte is 0 appends nothing), and
**                    `swscanf` is `ui5_swscanf` (below). Nothing else.
**   lan_user.inc     GUICallbacks/Menus/LanLobbyMenu.cpp, `LANPreferences::getUserName` (to `getPreferredColor`);
**   lan_ips.inc      the same file, `LANPreferences::getNumRemoteIPs` and `getRemoteIPEntry` (to `superweaponRestrictionKey`);
**   qp_to.inc        Common/System/QuotedPrintable.cpp, MAGIC_CHAR, the two hex digit helpers and UnicodeStringToQuotedPrintable;
**   qp_from.inc      the same file, QuotedPrintableToUnicodeString.
**
** Stand-ins of this file (what the cuts call outside themselves):
**   - TheLAN (LANAPI): `new`, `init`, `reset`, `SetLocalIP`, `GetLocalIP`, `RequestSetName`, `RequestGameCreate`, `RequestGameJoinDirectConnect`, `RequestLobbyLeave`, `delete`
**     log `lan new`, `lan init`, ... and keep the local IP (0 at `new`: LANAPI.cpp:85). Network I/O: nothing else of LANAPI is run.
**   - LANPreferences / OptionPreferences / UserPreferences: the std::map<AsciiString, AsciiString> of UserPreferences.h (PreferenceMap) loaded from, and written to, an in-memory
**     "Network.ini" (the case's `@ nd prefs`); `write()` logs `prefs write`; getUserName, getNumRemoteIPs and getRemoteIPEntry are upstream's (cut). File I/O: nothing else.
**     OptionPreferences::getOnlineIPAddress answers the case's value. IPEnumeration: the case's address list and the machine name `MACHINE`.
**   - TheShell: showShellMap(b), pop(), shutdownComplete(layout) log `shell map <b>`, `shell pop`, `shell complete <layout>`; reverseAnimatewindow() and isAnimFinished() model
**     the verified Shell's animate manager (src/client/shell.rs, which matches Shell.cpp:731-757 with m_animateWindows on): `needsUpdate` (the case's `@ nd anim`), `reverse`;
**     reverse sets both, shutdownComplete's reset clears both; isAnimFinished is transitions-finished AND NOT needsUpdate.
**   - TheTransitionHandler: setGroup(name) and reverse(name) of the ONE group `NetworkDirectConnectFade`, modelled as the verified Rust handler does (src/client/window_transitions.rs
**     `set_group`, `reverse` for a group with no windows): `cur`, `pend`, `dir`; isFinished() is TRUE (a group with no windows, or none current). The Rust side reads the same three from its handler.
**   - LANbuttonPushed, LANisShuttingDown (LANAPICallbacks.cpp's globals), g_lanPlayerNameLength (12), IsLANAPI-free.
**   - TheGameText: ui4_host.h's (`T:` + the label).
**   - swscanf: glibc's vswscanf over the 16-bit units widened (MSVC's scanf conforms to the C standard for `%d.%d.%d.%d` on the inputs tested: no overflow).
**
** Operations (besides ui4_main.h's): `@ nd layout <n> <win>...` (the menu's layout, held by the program), `init`, `shutdown <popImmediate>`, `update`, `ips <n> <ip>...` (the
** enumerated addresses), `online <ip>`, `prefs <key> <value|->` (the ini file's entry), `lan <0 delete | 1 create | 2 setip> [ip]`, `anim <needsUpdate>`, `cbtext <combo> <hex>`, `cbadd <combo> <hex> <color>` (GadgetComboBoxSetText / AddEntry).
** The dump adds the statics, TheLAN, the ini file, the transition and animate models. */
#define LANbuttonPushed u5_LANbuttonPushed
#define LANisShuttingDown u5_LANisShuttingDown
#define g_lanPlayerNameLength u5_g_lanPlayerNameLength
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "GameClient/KeyDefs.h"
#include <stdarg.h>
#include <wchar.h>

/* MSVC's 16-bit swscanf for `%d.%d.%d.%d`: glibc's over the units widened */
static int ui5_swscanf(const WideChar *s, const wchar_t *fmt, ...)
{
	std::wstring w;
	for (; *s; ++s) w += (wchar_t)*s;
	va_list a;
	va_start(a, fmt);
	int r = vswscanf(w.c_str(), fmt, a);
	va_end(a);
	return r;
}
#define swscanf ui5_swscanf
/* upstream's AsciiString::translate(const UnicodeString &): each unit's low byte appended, a 0 appending nothing */
static void ui5_translate(AsciiString &out, const UnicodeString &src)
{
	out.clear();
	for (Int i = 0; i < src.getLength(); ++i)
	{
		char c = (char)src.getCharAt(i);
		if (c != 0) out.concat(c);
	}
}

const Int g_lanPlayerNameLength = 12;
Bool LANbuttonPushed = false;
Bool LANisShuttingDown = false;

/* ---- the transition handler's one group (see the header comment) */
static bool u5_tr_cur = false, u5_tr_pend = false;
static int u5_tr_dir = 0;
void GameWindowTransitionsHandler::setGroup(AsciiString groupName, Bool immidiate)
{
	if (u5_tr_cur)
	{
		if (u5_tr_dir != -1) u5_tr_dir = -1;
		u5_tr_pend = true;
		u5_tr_dir = 1;
		return;
	}
	u5_tr_cur = true;
	u5_tr_dir = 1;
}
void GameWindowTransitionsHandler::reverse(AsciiString groupName)
{
	if (u5_tr_cur)
	{
		u5_tr_dir = -1;
		return;
	}
	u5_tr_cur = true;
	u5_tr_dir = -1;
	u5_tr_pend = false;
}
Bool GameWindowTransitionsHandler::isFinished(void) { return TRUE; }

/* ---- the shell */
static bool u5_anim_needs_update = false, u5_anim_reverse = false;
class Shell
{
public:
	void showShellMap(Bool b) { g_log << "shell map " << (int)b << "\n"; }
	void pop(void) { g_log << "shell pop\n"; }
	void shutdownComplete(WindowLayout *l, Bool impendingPush = FALSE)
	{
		g_log << "shell complete " << u4_lay_index(l) << "\n";
		u5_anim_needs_update = false;
		u5_anim_reverse = false;
	}
	void reverseAnimatewindow(void)
	{
		u5_anim_reverse = true;
		u5_anim_needs_update = true;
	}
	Bool isAnimFinished(void)
	{
		if (!TheTransitionHandler->isFinished()) return FALSE;
		return u5_anim_needs_update ? FALSE : TRUE;
	}
};
static Shell u5_shell;
static Shell *TheShell = &u5_shell;

/* ---- the LAN API */
class LANAPI
{
public:
	UnsignedInt m_localIP;
	LANAPI() : m_localIP(0) { g_log << "lan new\n"; }
	~LANAPI() { g_log << "lan delete\n"; }
	void init(void) { g_log << "lan init\n"; }
	void reset(void) { g_log << "lan reset\n"; }
	UnsignedInt GetLocalIP(void) { return m_localIP; }
	void SetLocalIP(UnsignedInt ip) { m_localIP = ip; g_log << "lan setip " << ip << "\n"; }
	void RequestSetName(UnicodeString name) { g_log << "lan name"; dump_text(g_log, name); g_log << "\n"; }
	void RequestGameCreate(UnicodeString game, Bool direct) { g_log << "lan create"; dump_text(g_log, game); g_log << " " << (int)direct << "\n"; }
	void RequestGameJoinDirectConnect(UnsignedInt ip) { g_log << "lan join " << ip << "\n"; }
	void RequestLobbyLeave(Bool forced) { g_log << "lan leave " << (int)forced << "\n"; }
};
static LANAPI *TheLAN = NULL;

/* ---- IPEnumeration and the preferences */
class EnumeratedIP
{
public:
	UnsignedInt m_ip;
	EnumeratedIP *m_next;
	UnsignedInt getIP(void) { return m_ip; }
	EnumeratedIP *getNext(void) { return m_next; }
};
static EnumeratedIP *u5_ips = NULL;
class IPEnumeration
{
public:
	EnumeratedIP *getAddresses(void) { return u5_ips; }
	AsciiString getMachineName(void) { return AsciiString("MACHINE"); }
};
static UnsignedInt u5_online_ip = 0;
class OptionPreferences
{
public:
	UnsignedInt getOnlineIPAddress(void) { return u5_online_ip; }
};
typedef std::map<AsciiString, AsciiString> PreferenceMap;
static std::map<std::string, std::string> u5_ini;
class LANPreferences : public PreferenceMap
{
public:
	LANPreferences()
	{
		for (std::map<std::string, std::string>::iterator it = u5_ini.begin(); it != u5_ini.end(); ++it) (*this)[AsciiString(it->first.c_str())] = AsciiString(it->second.c_str());
	}
	Bool write(void)
	{
		u5_ini.clear();
		for (PreferenceMap::const_iterator it = begin(); it != end(); ++it) u5_ini[it->first.str()] = it->second.str();
		g_log << "prefs write\n";
		return true;
	}
	UnicodeString getUserName(void);
	Int getNumRemoteIPs(void);
	UnicodeString getRemoteIPEntry(Int i);
};

#include "qp_to.inc"
#include "qp_from.inc"
#include "lan_user.inc"
#include "lan_ips.inc"
#include "ndc.inc"

static const U4Menu u4_menus[] = {
	{ "ndc", NetworkDirectConnectSystem, NetworkDirectConnectInput },
};
static WindowLayout *u5_layout = NULL;
static void u4_menu_setup(void) {}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "nd") return false;
	std::string f;
	in >> f;
	if (f == "layout")
	{
		int n = u4_li(in);
		u4_layout_members.clear();
		for (int i = 0; i < n; ++i) u4_layout_members.push_back(u4_li(in));
		u5_layout = TheWindowManager->winCreateLayout(AsciiString("NetworkDirectConnect.wnd"));
	}
	else if (f == "init") NetworkDirectConnectInit(u5_layout, NULL);
	else if (f == "shutdown") { Bool pi = u4_li(in) != 0; NetworkDirectConnectShutdown(u5_layout, &pi); }
	else if (f == "update") NetworkDirectConnectUpdate(u5_layout, NULL);
	else if (f == "ips")
	{
		int n = u4_li(in);
		u5_ips = NULL;
		EnumeratedIP **tail = &u5_ips;
		for (int i = 0; i < n; ++i)
		{
			EnumeratedIP *e = new EnumeratedIP;
			unsigned v = 0;
			in >> v;
			e->m_ip = v;
			e->m_next = NULL;
			*tail = e;
			tail = &e->m_next;
		}
	}
	else if (f == "online") { unsigned v = 0; in >> v; u5_online_ip = v; }
	else if (f == "prefs")
	{
		std::string k, v;
		in >> k >> v;
		if (v == "-") u5_ini.erase(k); else u5_ini[k] = v;
	}
	else if (f == "lan")
	{
		int what = u4_li(in);
		if (what == 0) { delete TheLAN; TheLAN = NULL; }
		else if (what == 1) { if (!TheLAN) TheLAN = NEW LANAPI(); }
		else { unsigned v = 0; in >> v; if (TheLAN) TheLAN->SetLocalIP(v); }
	}
	else if (f == "cbtext") { int a = u4_li(in); std::string t; in >> t; GadgetComboBoxSetText(u4_win(a), u4_text(t)); }
	else if (f == "cbadd") { int a = u4_li(in); std::string t; in >> t; int c = u4_li(in); GadgetComboBoxAddEntry(u4_win(a), u4_text(t), c); }
	else if (f == "anim") u5_anim_needs_update = u4_li(in) != 0;
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "nd shutting " << (int)isShuttingDown << " pushed " << (int)buttonPushed << " lanbp " << (int)LANbuttonPushed << " lansd " << (int)LANisShuttingDown << " lan " << (TheLAN ? 1 : 0) << " ip " << (TheLAN ? TheLAN->m_localIP : 0) << "\n";
	o << "nd ids " << buttonBackID << " " << buttonHostID << " " << buttonJoinID << " " << editPlayerNameID << " " << comboboxRemoteIPID << " " << staticLocalIPID << " wins " << widx(buttonBack) << " " << widx(buttonHost) << " " << widx(buttonJoin) << " " << widx(editPlayerName) << " " << widx(comboboxRemoteIP) << " " << widx(staticLocalIP) << "\n";
	o << "nd trans cur " << (int)u5_tr_cur << " pend " << (int)u5_tr_pend << " dir " << u5_tr_dir << " anim nu " << (int)u5_anim_needs_update << " rev " << (int)u5_anim_reverse << "\n";
	o << "nd ini";
	for (std::map<std::string, std::string>::iterator it = u5_ini.begin(); it != u5_ini.end(); ++it)
	{
		o << " " << it->first << "=";
		for (size_t i = 0; i < it->second.size(); ++i) { char b[8]; snprintf(b, sizeof b, "%02x", (unsigned)(unsigned char)it->second[i]); o << b; }
	}
	o << "\n";
}
#include "ui4_main.h"
