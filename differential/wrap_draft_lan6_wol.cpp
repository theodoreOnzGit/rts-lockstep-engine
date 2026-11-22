/* The reference PROGRAM for src/drafts/ui/wol_map_select_menu.rs (#215, group lan6, upstream commit 0a05454d): upstream's WOLMapSelectMenu.cpp on upstream's OWN window manager and
** gadgets (built by build.rs `drafts_lan6`, path `LAN6_WOL_EXE`). The shared stand-ins are in lan6_menu.h (read its header).
**
** Upstream text cut VERBATIM (each anchor asserted once, no edit):
**   wms.inc          GUICallbacks/Menus/WOLMapSelectMenu.cpp from `static NameKeyType buttonBack` to the end;
**   custom_pref.inc  Common/UserPreferences.cpp, `CustomMatchPreferences::usesSystemMapDir` and `setUsesSystemMapDir` (to `usesLongGameList`).
** Stand-ins of this file: TheGameSpyGame (lan6_menu.h's game), TheGameSpyInfo (`getCurrentStagingRoom()->getUseStats()` the case's `@ ms stats`; `setGameOptions()` logs),
** CustomMatchPreferences (the in-memory ini; `write()` logs), WOLMapSelectLayout (the global of WOLGameSetupMenu.cpp), WOLDisplaySlotList, WOLDisplayGameOptions,
** WOLPositionStartSpots, RaiseGSMessageBox (log `wol ...`; they draw the game setup screen / a message box).
** Operations: `@ ms init | shutdown | update | msl <0|1>` besides lan6_menu.h's. */
#include "ui4_wm_base.inc"
#include "ui4_host.h"
#include "lan6_menu.h"

typedef L6Game GameSpyGameInfo;
static GameSpyGameInfo *TheGameSpyGame = &l6_game;
class L6Room
{
public:
	Bool getUseStats(void) { return l6_stats; }
};
static L6Room l6_room;
class L6GsInfo
{
public:
	L6Room *getCurrentStagingRoom(void) { return &l6_room; }
	void setGameOptions(void) { g_log << "gsi setgameoptions\n"; }
};
static L6GsInfo l6_gsi;
static L6GsInfo *TheGameSpyInfo = &l6_gsi;
static void WOLDisplaySlotList(void) { g_log << "wol slotlist\n"; }
static void WOLDisplayGameOptions(void) { g_log << "wol gameoptions\n"; }
static void WOLPositionStartSpots(void) { g_log << "wol positionspots\n"; }
static void RaiseGSMessageBox(void) { g_log << "wol raisebox\n"; }
WindowLayout *WOLMapSelectLayout = NULL;

class CustomMatchPreferences : public PreferenceMap
{
public:
	CustomMatchPreferences() { l6_prefs_load(*this); }
	Bool write(void) { l6_prefs_write(*this); return true; }
	Bool usesSystemMapDir(void);
	void setUsesSystemMapDir(Bool val);
};
#include "custom_pref.inc"
#include "wms.inc"

static const U4Menu u4_menus[] = {
	{ "wms", WOLMapSelectMenuSystem, WOLMapSelectMenuInput },
};
static void u4_menu_setup(void) {}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "ms") return false;
	std::string f;
	in >> f;
	if (f == "init") WOLMapSelectMenuInit(l6_layout, NULL);
	else if (f == "shutdown") WOLMapSelectMenuShutdown(l6_layout, NULL);
	else if (f == "update") WOLMapSelectMenuUpdate(l6_layout, NULL);
	else if (f == "msl") WOLMapSelectLayout = u4_li(in) ? l6_layout : NULL;
	else l6_common_op(f, in);
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	o << "ms ids " << buttonBack << " " << buttonOK << " " << listboxMap << " " << winMapPreviewID << " " << radioButtonSystemMapsID << " " << radioButtonUserMapsID << " wins " << widx(parent) << " " << widx(mapList) << " " << widx(winMapPreview) << " " << widx(winMapWindow) << " sp";
	for (int i = 0; i < MAX_SLOTS; ++i) o << " " << widx(buttonMapStartPosition[i]);
	o << " spid";
	for (int i = 0; i < MAX_SLOTS; ++i) o << " " << buttonMapStartPositionID[i];
	o << " raise " << (int)raiseMessageBoxes << " msl " << u4_lay_index(WOLMapSelectLayout) << "\n";
	l6_dump_common(o);
}
#include "ui4_main.h"
