/* Differential-test wrapper for the WOL lobby's and game setup's slash commands (#215, group wol6, upstream commit 0a05454d):
** src/drafts/ui/wol_lobby_menu.rs `handle_lobby_slash_commands` (with `refresh_game_list` and `refresh_player_list`, which `/refresh` calls) and
** src/drafts/ui/wol_game_setup_menu.rs `handle_game_setup_slash_commands`. Cut verbatim by build.rs (fn drafts_wol6) from
**   WOLLobbyMenu.cpp       lobby_statics.inc  the four refresh statics; lobby_slash.inc  `handleLobbySlashCommands` (from its `extern "C"` declarations,
**                          to `s_tryingToHostOrJoin`); lobby_refresh.inc  `refreshGameList` and `refreshPlayerList`;
**   WOLGameSetupMenu.cpp   gs_slash.inc  `handleGameSetupSlashCommands` (the same way, to `getNextSelectablePlayer`);
**   PeerDefs.h             gscolor.inc  the `GameSpyColors` enum.
** The shipped build has neither `_DEBUG` nor `_INTERNAL`, and so has this one: the cuts' `#if defined(_INTERNAL) || defined(_DEBUG)` branches
** (`/fakecrc`, `/slots`, `/discon`) are compiled out, as in the release game.
** Stand-ins: differential/wol6_slash_host.h lists them. The file's `listboxLobbyPlayers` static is a pointer to a window with the id 7, or NULL; `PopulateLobbyPlayerListbox()`
** is empty (the real one returns at once when `listboxLobbyPlayers` is NULL, which the generator guarantees for `/refresh`).
** Every global this file defines is in a namespace except the entry point and the two C-linkage names the cuts declare (renamed Wol6...).
** Excluded inputs (upstream is undefined there): a text with an embedded NUL unit (a UnicodeString cannot hold one; a unit whose low byte is 0 is allowed, and
** dropped by `translate`), a first token longer than 2,047 characters (`toLower`'s 2,048-byte buffer), and a clock past 2^31 (`time_t` is 64 bits here;
** the draft's statics are 32 bits and wrap, as an old 32-bit `time_t` would).
**
** Entry point ref_wol6_slash_run(in, out, cap): a case is one line
**   `<menu 0 lobby | 1 game setup> <qr2> <thread> <colour> <players window 0|1> <changed> <game time> <player time> <now> <text, hex units>`;
** the output is the stand-ins' calls in order, `ret <0|1>`, and `statics <game refresh time> <player refresh time>`. Returns the output's length. */
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <time.h>
#include <wchar.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#define DEBUG_LOG(x)
#define getQR2HostingStatus Wol6getQR2HostingStatus
#define isThreadHosting Wol6isThreadHosting

static int wol6_qr2 = 0;
static int wol6_thread = 0;
extern "C" int Wol6getQR2HostingStatus(void) { return wol6_qr2; }

namespace wol6_lobby {
#include "gscolor.inc"
#include "wol6_slash_host.h"
static GameWindow *listboxLobbyPlayers = NULL;
void PopulateLobbyPlayerListbox() {}
#include "lobby_statics.inc"
#include "lobby_refresh.inc"
#include "lobby_slash.inc"
int Wol6isThreadHosting = 0;
static std::string w6_run(int qr2, int thread, int color, int players, int changed, unsigned gtime, unsigned ptime, unsigned now, const std::vector<WideChar> &text)
{
	w6log.clear();
	wol6_qr2 = qr2;
	Wol6isThreadHosting = thread;
	GameSpyColor[GSCOLOR_DEFAULT] = color;
	listboxLobbyPlayers = players ? &w6_players_window : NULL;
	w6_list_changed = changed;
	gameListRefreshTime = gtime;
	playerListRefreshTime = ptime;
	w6_now = now;
	UnicodeString u;
	u.v = text;
	u.v.push_back(0);
	Bool r = handleLobbySlashCommands(u);
	w6printf("ret %d", (int)r);
	w6printf("statics %lld %lld", (long long)gameListRefreshTime, (long long)playerListRefreshTime);
	return w6log;
}
}

namespace wol6_gs {
#include "gscolor.inc"
#include "wol6_slash_host.h"
#include "gs_slash.inc"
int Wol6isThreadHosting = 0;
static std::string w6_run(int qr2, int thread, int color, const std::vector<WideChar> &text)
{
	w6log.clear();
	wol6_qr2 = qr2;
	Wol6isThreadHosting = thread;
	GameSpyColor[GSCOLOR_DEFAULT] = color;
	UnicodeString u;
	u.v = text;
	u.v.push_back(0);
	Bool r = handleGameSetupSlashCommands(u);
	w6printf("ret %d", (int)r);
	return w6log;
}
}

extern "C" int ref_wol6_slash_run(const char *in, char *out, int cap)
{
	int menu, qr2, thread, color, players, changed;
	unsigned gtime, ptime, now;
	char hx[1 << 14];
	hx[0] = 0;
	int n = sscanf(in, "%d %d %d %d %d %d %u %u %u %16383s", &menu, &qr2, &thread, &color, &players, &changed, &gtime, &ptime, &now, hx);
	if (n < 9) return -1;
	std::vector<WideChar> text;
	if (strcmp(hx, "-") != 0)
	{
		const char *p = hx;
		while (*p)
		{
			text.push_back((WideChar)strtoul(p, NULL, 16));
			const char *c = strchr(p, ',');
			if (!c) break;
			p = c + 1;
		}
	}
	std::string r = menu == 0 ? wol6_lobby::w6_run(qr2, thread, color, players, changed, gtime, ptime, now, text) : wol6_gs::w6_run(qr2, thread, color, text);
	int len = (int)r.size() < cap - 1 ? (int)r.size() : cap - 1;
	memcpy(out, r.data(), len);
	out[len] = 0;
	return len;
}
