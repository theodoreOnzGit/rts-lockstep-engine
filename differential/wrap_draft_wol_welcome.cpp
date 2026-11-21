/* Differential-test wrapper for the WOL welcome menu's statistics and message-of-the-day logic (#215, group ui3a, upstream commit
** 0a05454d): src/drafts/ui/wol_welcome_menu.rs (`update_server_display`, `enable_controls`, `grab_ubyte`, `update_num_players_online`,
** `handle_num_players_online`, `find_next_number`, `handle_overall_stats`, `update_overall_stats`). Cut verbatim by build.rs
** (fn drafts_ui3a) from GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLWelcomeMenu.cpp:
**   ws_statics.inc  `static Bool isShuttingDown` to `static UnicodeString gServerName;` (the file's statics);
**   ws_server.inc   `static UnicodeString gServerName;` and `updateServerDisplay`;
**   ws_enable.inc   `enableControls`;
**   ws_stats.inc    `lastNumPlayersOnline`, `grabUByte`, `updateNumPlayersOnline`, `HandleNumPlayersOnline`, `s_winStats`, `FindNextNumber`,
**                   `HandleOverallStats`, `updateOverallStats` (to the comment of `UpdateLocalPlayerStats`);
** and GameClient/Color.h's `GameMakeColor` (ws_color.inc), GameClient/GameWindow.h's WIN_STATUS_* and WIN_ERR_*, GameWindow.cpp's
** `GameWindow::winEnable` (gw_enable.inc) and the window system's functions that ui3a_host.h lists.
** Stand-ins (outside the cuts): the shared host (ui3a_host.h: AsciiString and UnicodeString copy `nextToken`, `trim`, `startsWith` from AsciiString.cpp and
** UnicodeString.cpp, with the C locale's whitespace); NAMEKEY(x) (the name key generator's nameToKey); TheGameText->fetch (the case's table by label,
** else `T:` and the label); UnicodeString::format(fmt, ...) (`%d` and `%%` of the case's ASCII formats: what _vsnwprintf does with them);
** TheGameSpyInfo->getMOTD() (the case's text; the case may have no GameSpyInfo, and then the list is never touched, as the cut tests the pointer
** with `listboxInfo && TheGameSpyInfo`: here the pointer is NULL); GameSpyColor[GSCOLOR_MOTD_HEADING] and [GSCOLOR_MOTD] (the case's colours);
** MultiByteToWideCharSingleLine (Windows' MultiByteToWideChar: each byte becomes the unit of that value); ThePlayerTemplateStore (the case's list
** of sides and playable flags); GadgetStaticTextSetText, GadgetCheckBoxSetText (the label kept per window, as GGM_SET_LABEL leaves it);
** GadgetListBoxReset and AddEntryText (the host's list); REAL_TO_INT (BaseType.h: `(Int)fast_float2long_round(fast_float_trunc(x))`, a truncation);
** `max` (windows.h's macro); atof, strstr, strchr, isdigit, strtol are the C library's; DEBUG_LOG compiled out.
** Excluded inputs (upstream reads past the text's end or is undefined): the generator puts a digit after every newline FindNextNumber seeks, the
** statistics text is ASCII (isdigit of a negative char is undefined), and no format has a directive but `%d` and `%%`.
**
** Entry point ref_u3a_welcome_run(in, out, cap): see tests/differential_draft_wol_welcome.rs. Returns the log's length. */
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <string>
#include <vector>
#include <map>
#include "Lib/BaseType.h"
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(x, y)
#define NAMEKEY(x) TheNameKeyGenerator->nameToKey(x)
#define REAL_TO_INT(x) ((Int)(x))
#define max(a, b) (((a) > (b)) ? (a) : (b))

namespace ui3a_welcome {
#include "ui3a_host.h"
#include "gw_enable.inc"
#include "ws_color.inc"

enum { GSCOLOR_DEFAULT = 0, GSCOLOR_MOTD, GSCOLOR_MOTD_HEADING, GSCOLOR_MAX };
Color GameSpyColor[GSCOLOR_MAX];

/* ---- the gadgets and the GameSpy layer ---- */
std::map<GameWindow *, UnicodeString> g_label;
void GadgetStaticTextSetText(GameWindow *w, UnicodeString text) { if (!w) return; g_label[w] = text; }
void GadgetCheckBoxSetText(GameWindow *w, UnicodeString text) { if (!w) return; g_label[w] = text; }
void GadgetListBoxReset(GameWindow *lw) { if (!lw) return; g_list.rows.clear(); g_list.selectPos = -1; }
std::string g_motd;
class GameSpyInfo { public: AsciiString getMOTD() { return AsciiString(g_motd.c_str()); } };
GameSpyInfo g_info;
GameSpyInfo *TheGameSpyInfo = NULL;
std::wstring MultiByteToWideCharSingleLine(const char *orig) { std::wstring w; for (; *orig; ++orig) w += (wchar_t)(unsigned char)*orig; return w; }
struct PlayerTemplate
{
	AsciiString side;
	Bool playable;
	AsciiString getSide() const { return side; }
	Bool isPlayableSide() const { return playable; }
};
std::vector<PlayerTemplate> g_templates;
class PlayerTemplateStore
{
public:
	Int getPlayerTemplateCount() { return (Int)g_templates.size(); }
	const PlayerTemplate *getNthPlayerTemplate(Int i) { return &g_templates[i]; }
};
PlayerTemplateStore g_pts;
PlayerTemplateStore *ThePlayerTemplateStore = &g_pts;

#include "ws_statics.inc"
#include "ws_server.inc"
#include "ws_enable.inc"
#include "ws_stats.inc"

/* ---- the case ---- */
std::string unhex(const std::string &h)
{
	std::string r;
	for (size_t i = 0; i + 2 <= h.size(); i += 2) { unsigned v; sscanf(h.c_str() + i, "%2x", &v); r += (char)v; }
	return r;
}
std::string hexOf(const std::string &s)
{
	std::string r;
	char b[4];
	for (size_t i = 0; i < s.size(); ++i) { snprintf(b, sizeof b, "%02x", (unsigned char)s[i]); r += b; }
	return r.empty() ? "-" : r;
}
std::string wideHex(const UnicodeString &u)
{
	std::string r;
	char b[8];
	for (size_t i = 0; i < u.s.size(); ++i) { snprintf(b, sizeof b, "%04x", (unsigned)u.s[i]); r += b; }
	return r.empty() ? "-" : r;
}
const char *g_sides[3] = { "USA", "China", "GLA" };

void dumpState(GameWindow *numWin, GameWindow *serverWin, GameWindow *qmBtn, GameWindow *lobbyBtn, GameWindow *checks[3])
{
	u3aLog("last %d total %08x server [%s]", lastNumPlayersOnline, *(unsigned *)&s_totalWinPercent, wideHex(gServerName).c_str());
	u3aLog("numwin [%s] server [%s]", numWin ? wideHex(g_label[numWin]).c_str() : "absent", serverWin ? wideHex(g_label[serverWin]).c_str() : "absent");
	u3aLog("enabled qm %d lobby %d", qmBtn ? BitTest(qmBtn->m_status, WIN_STATUS_ENABLED) : -1, lobbyBtn ? BitTest(lobbyBtn->m_status, WIN_STATUS_ENABLED) : -1);
	std::string rows;
	for (size_t i = 0; i < g_list.rows.size(); ++i)
	{
		char b[32];
		snprintf(b, sizeof b, "{%d}", (int)g_list.rows[i].color);
		rows += " [" + wideHex(g_list.rows[i].text) + "]" + b;
	}
	u3aLog("list %d%s", (int)g_list.rows.size(), rows.c_str());
	std::string st;
	for (std::map<AsciiString, float>::iterator it = s_winStats.begin(); it != s_winStats.end(); ++it)
	{
		char b[64];
		snprintf(b, sizeof b, " %s=%08x", it->first.str(), *(unsigned *)&it->second);
		st += b;
	}
	u3aLog("stats%s", st.c_str());
	std::string ck;
	for (int i = 0; i < 3; ++i) ck += std::string(" [") + (checks[i] ? wideHex(g_label[checks[i]]).c_str() : "absent") + "]";
	u3aLog("checks%s", ck.c_str());
}

} /* namespace ui3a_welcome */

/* The case is text: lines `flags <bits>`, `colors <heading> <motd>`, `text <label> <hex>`, `motd <hex>`, `tpl <playable> <side hex>`, then the ops
** `num <n>`, `stats <hex>`, `upd`, `server <hex>` (UTF-16 units), `enable <0|1>`. */
extern "C" int ref_u3a_welcome_run(const char *in, char *out, int cap)
{
	using namespace ui3a_welcome;
	resetWindows();
	g_list = ListState();
	g_list.selectPos = -1;
	g_list.listLength = 100;
	g_label.clear();
	g_templates.clear();
	g_motd.clear();
	TheGameSpyInfo = NULL;
	lastNumPlayersOnline = 0;
	s_winStats.clear();
	s_totalWinPercent = 0;
	gServerName = UnicodeString();
	GameWindow *numWin = NULL, *serverWin = NULL, *qm = NULL, *lobby = NULL, *list = NULL, *checks[3] = { NULL, NULL, NULL };
	std::vector<std::string> ops;
	int flags = 0;
	const char *p = in;
	std::string line;
	while (*p)
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
		if (cmd == "flags") flags = atoi(rest);
		else if (cmd == "colors") { int a, b; sscanf(rest, "%d %d", &a, &b); GameSpyColor[GSCOLOR_MOTD_HEADING] = a; GameSpyColor[GSCOLOR_MOTD] = b; }
		else if (cmd == "text") { char lab[128], hx[8192]; hx[0] = 0; sscanf(rest, "%127s %8191s", lab, hx); g_gameTexts[lab] = unhex(hx); }
		else if (cmd == "motd") { char hx[16384]; hx[0] = 0; sscanf(rest, "%16383s", hx); g_motd = std::string(hx) == "-" ? "" : unhex(hx); }
		else if (cmd == "tpl") { int pl; char hx[256]; hx[0] = 0; sscanf(rest, "%d %255s", &pl, hx); PlayerTemplate t; t.side = AsciiString(unhex(hx).c_str()); t.playable = pl != 0; g_templates.push_back(t); }
		else ops.push_back(line);
	}
	/* the windows: the number-of-players static, the server name static, the list, two buttons, three check boxes */
	std::string pre = "WOLWelcomeMenu.wnd:";
	if (flags & 1) numWin = makeWindow(pre + "StaticTextNumPlayersOnline", NULL, WIN_STATUS_ENABLED, 0, 0);
	if (flags & 2) list = makeWindow(pre + "ListboxInfo", NULL, WIN_STATUS_ENABLED, 0, 0);
	if (flags & 4) serverWin = makeWindow(pre + "StaticTextServerName", NULL, WIN_STATUS_ENABLED, 0, 0);
	if (flags & 8) qm = makeWindow(pre + "ButtonQuickMatch", NULL, WIN_STATUS_ENABLED, 0, 0);
	if (flags & 16) lobby = makeWindow(pre + "ButtonLobby", NULL, WIN_STATUS_ENABLED, 0, 0);
	for (int i = 0; i < 3; ++i) if (flags & (32 << i)) checks[i] = makeWindow(pre + "Percent" + g_sides[i], NULL, WIN_STATUS_ENABLED, 0, 0);
	if (flags & 256) TheGameSpyInfo = &g_info;
	listboxInfo = list;
	staticTextServerName = serverWin;
	buttonQuickMatch = qm;
	buttonLobby = lobby;
	for (size_t k = 0; k < ops.size(); ++k)
	{
		u3aLog("> %s", ops[k].c_str());
		char op[32] = "";
		sscanf(ops[k].c_str(), "%31s", op);
		std::string cmd(op);
		const char *rest = ops[k].c_str() + strlen(op);
		while (*rest == ' ') ++rest;
		if (cmd == "num") HandleNumPlayersOnline(atoi(rest));
		else if (cmd == "stats") { std::string t = unhex(rest); HandleOverallStats(t.c_str(), (unsigned)t.size()); }
		else if (cmd == "upd") updateOverallStats();
		else if (cmd == "server")
		{
			UnicodeString u;
			std::string h(rest);
			if (h != "-") for (size_t i = 0; i + 4 <= h.size(); i += 4) { unsigned v; sscanf(h.c_str() + i, "%4x", &v); u.s += (WideChar)v; }
			updateServerDisplay(u);
		}
		else if (cmd == "enable") enableControls(atoi(rest) != 0);
		dumpState(numWin, serverWin, qm, lobby, checks);
	}
	int len = (int)g_log.size();
	if (len > cap) len = cap;
	memcpy(out, g_log.data(), len);
	return (int)g_log.size();
}
