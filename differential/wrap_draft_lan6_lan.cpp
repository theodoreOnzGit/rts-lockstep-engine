/* The reference PROGRAM for src/drafts/ui/lan_map_select_menu.rs (#215, group lan6, upstream commit 0a05454d): upstream's LanMapSelectMenu.cpp on upstream's OWN window manager and
** gadgets (list box, radio button, push button; built by build.rs `drafts_lan6`, path `LAN6_LAN_EXE`). The shared stand-ins are in lan6_menu.h (read its header).
**
** Upstream text cut VERBATIM (each anchor asserted once, no edit):
**   lms.inc        GUICallbacks/Menus/LanMapSelectMenu.cpp from `static NameKeyType buttonBack` to the end;
**   lan_pref.inc   GUICallbacks/Menus/LanLobbyMenu.cpp, `LANPreferences::usesSystemMapDir` (to `getPreferredMap`).
** Stand-ins of this file: TheLAN (LANAPI::GetMyGame(): lan6_menu.h's game), LANPreferences (the in-memory ini; `write()` logs), mapSelectLayout (the global of LanGameOptionsMenu.cpp),
** PostToLanGameOptions(MAP_BACK | SEND_GAME_OPTS) (logs `post <n>`; LANAPICallbacks.cpp posts to the game options screen).
** Operations: `@ ms init | shutdown | update | msl <0|1>` besides lan6_menu.h's. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "lan6_menu.h"

enum PostToLanGameOptionsType { MAP_BACK, SEND_GAME_OPTS };
static void PostToLanGameOptions(int t) { g_log << "post " << t << "\n"; }
typedef L6Game LANGameInfo;
class LANAPI
{
public:
	LANGameInfo *GetMyGame(void) { return &l6_game; }
};
static LANAPI l6_lan;
static LANAPI *TheLAN = &l6_lan;
static WindowLayout *mapSelectLayout = NULL;

class LANPreferences : public PreferenceMap
{
public:
	LANPreferences() { l6_prefs_load(*this); }
	Bool write(void) { l6_prefs_write(*this); return true; }
	Bool usesSystemMapDir(void);
};
#include "lan_pref.inc"
#include "lms.inc"

static const U4Menu u4_menus[] = {
	{ "lms", LanMapSelectMenuSystem, LanMapSelectMenuInput },
};
static void u4_menu_setup(void) {}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "ms") return false;
	std::string f;
	in >> f;
	if (f == "init") LanMapSelectMenuInit(l6_layout, NULL);
	else if (f == "shutdown") LanMapSelectMenuShutdown(l6_layout, NULL);
	else if (f == "update") LanMapSelectMenuUpdate(l6_layout, NULL);
	else if (f == "msl") mapSelectLayout = u4_li(in) ? l6_layout : NULL;
	else l6_common_op(f, in);
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "ms ids " << buttonBack << " " << buttonOK << " " << listboxMap << " " << winMapPreviewID << " " << radioButtonSystemMapsID << " " << radioButtonUserMapsID << " wins " << widx(parent) << " " << widx(mapList) << " " << widx(winMapPreview) << " sp";
	for (int i = 0; i < MAX_SLOTS; ++i) o << " " << widx(buttonMapStartPosition[i]);
	o << " spid";
	for (int i = 0; i < MAX_SLOTS; ++i) o << " " << buttonMapStartPositionID[i];
	o << " msl " << u4_lay_index(mapSelectLayout) << "\n";
	l6_dump_common(o);
}
#include "ui4_main.h"
