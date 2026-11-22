/* The reference PROGRAM for the quick match menu's combo box populate functions of src/drafts/ui/wol_quick_match_menu.rs (#215, group wol7, upstream commit 0a05454d):
** `populate_qm_color_combo_box` (`populateQMColorComboBox`) and `populate_qm_side_combo_box` (`populateQMSideComboBox`), on upstream's OWN window manager and gadgets (as
** wrap_draft_wol7_lobby.cpp; built by build.rs `drafts_wol7`). The path is `WOL7_QM_EXE`.
**
** Upstream text, cut VERBATIM by build.rs: qm_combos.inc  WOLQuickMatchMenu.cpp, from `populateQMColorComboBox` to `HandleQMLadderSelection` (no edit).
**
** Stand-ins (what the cuts call outside themselves; each carries the members the cuts read, the signatures of MultiplayerSettings.h / PlayerTemplate.h / ChallengeGenerals.h):
**   - TheMultiplayerSettings: `getNumColors()` and `getColor(which)` (`PLAYERTEMPLATE_RANDOM` = -1, GameInfo.h:51, is the random definition; an index of the case's list; NULL for a hole
**     or out of range, as MultiplayerSettings.cpp's map lookup); MultiplayerColorDefinition's `getColor()` and `getTooltipName()`.
**   - ThePlayerTemplateStore: `getPlayerTemplateCount()`, `getNthPlayerTemplate(i)` (NULL out of range); PlayerTemplate's `getStartingBuilding()`, `getSide()`, `getName()`.
**   - TheChallengeGenerals->getGeneralByTemplateName(name): the case's per-template-name persona (NULL when the template has none); `isStartingEnabled()`.
**   - LadderInfo: the two members the cut reads (`validFactions`, `randomFactions`).  QuickMatchPreferences: `getColor()` answers the case's value.
**   - TheGameText: ui4_host.h's (`T:` + the label).
** Operations (besides ui4_main.h's), `@ w7 <op>`: `qmwin <color win> <side win>` (the two statics), `qmrand <color>`, `qmcols <n>`, `qmcol <i> <color> <tooltip name>` (a definition; the list's
** other slots stay NULL), `qmpref <color>`, `qmtpl <k> <name> <side> <starting building empty> <general: -1 none, 0 locked, 1 enabled>`, `popc`, `pops <favSide> <ladder: - | <randomFactions> <n> <side>...>`.
** The dump adds every list box's item data. */
#include <set>
#include <map>
#include <string>
#include <algorithm>
#include <list>
#include <stdint.h>
#include <strings.h>
static bool w7_in_menu = false;
static bool w7_image_exists(const char *) { return false; }
static int w7_image_width(const char *) { return 0; }
#include "w7_wm_base.inc"
#include "ui4_host.h"

enum { PLAYERTEMPLATE_RANDOM = -1 };
static GameWindow *comboBoxColor = NULL;
static GameWindow *comboBoxSide = NULL;

class MultiplayerColorDefinition
{
public:
	Color m_color;
	AsciiString m_tip;
	MultiplayerColorDefinition() : m_color(0) {}
	Color getColor(void) const { return m_color; }
	AsciiString getTooltipName(void) const { return m_tip; }
};
static MultiplayerColorDefinition w7_random_def;
static std::vector<MultiplayerColorDefinition *> w7_defs;
class MultiplayerSettings
{
public:
	Int getNumColors(void) { return (Int)w7_defs.size(); }
	MultiplayerColorDefinition *getColor(Int which)
	{
		if (which == PLAYERTEMPLATE_RANDOM) return &w7_random_def;
		return (which >= 0 && which < (Int)w7_defs.size()) ? w7_defs[which] : NULL;
	}
};
static MultiplayerSettings w7_ms;
static MultiplayerSettings *TheMultiplayerSettings = &w7_ms;

class PlayerTemplate
{
public:
	AsciiString m_name, m_side, m_building;
	AsciiString getStartingBuilding(void) const { return m_building; }
	AsciiString getSide(void) const { return m_side; }
	AsciiString getName(void) const { return m_name; }
};
static std::vector<PlayerTemplate> w7_templates;
class PlayerTemplateStore
{
public:
	Int getPlayerTemplateCount(void) const { return (Int)w7_templates.size(); }
	const PlayerTemplate *getNthPlayerTemplate(Int i) const { return (i >= 0 && i < (Int)w7_templates.size()) ? &w7_templates[i] : NULL; }
};
static PlayerTemplateStore w7_pts;
static PlayerTemplateStore *ThePlayerTemplateStore = &w7_pts;

class GeneralPersona
{
public:
	Bool m_enabled;
	Bool isStartingEnabled(void) const { return m_enabled; }
};
static std::map<std::string, GeneralPersona> w7_generals;
class ChallengeGenerals
{
public:
	const GeneralPersona *getGeneralByTemplateName(const AsciiString &name)
	{
		std::map<std::string, GeneralPersona>::iterator it = w7_generals.find(name.str());
		return it == w7_generals.end() ? NULL : &it->second;
	}
};
static ChallengeGenerals w7_cg;
static ChallengeGenerals *TheChallengeGenerals = &w7_cg;

class LadderInfo
{
public:
	std::list<AsciiString> validFactions;
	Bool randomFactions;
};
static Int w7_pref_color = 0;
class QuickMatchPreferences
{
public:
	Int getColor(void) { return w7_pref_color; }
};

#include "qm_combos.inc"

static WindowMsgHandledType W7Sys(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static WindowMsgHandledType W7Inp(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static const U4Menu u4_menus[] = {
	{ "w7", W7Sys, W7Inp },
};
static void u4_menu_setup(void) {}
static std::string w7_name(const std::string &h)
{
	std::string o;
	if (h == "-") return o;
	for (size_t i = 0; i + 1 < h.size(); i += 2) o += (char)strtoul(h.substr(i, 2).c_str(), NULL, 16);
	return o;
}
static bool u4_menu_op(TestWindowManager *wm, const std::string &cmd, std::istringstream &in)
{
	if (cmd != "w7") return false;
	std::string f;
	in >> f;
	if (f == "qmwin") { int a = u4_li(in), b = u4_li(in); comboBoxColor = u4_win(a); comboBoxSide = u4_win(b); }
	else if (f == "qmrand") { int c = u4_li(in); w7_random_def.m_color = c; }
	else if (f == "qmcols") { int n = u4_li(in); w7_defs.assign(n, (MultiplayerColorDefinition *)NULL); }
	else if (f == "qmcol") { int i = u4_li(in); int c = u4_li(in); std::string t; in >> t; MultiplayerColorDefinition *d = new MultiplayerColorDefinition; d->m_color = c; d->m_tip = AsciiString(w7_name(t).c_str()); if (i < (int)w7_defs.size()) w7_defs[i] = d; }
	else if (f == "qmpref") w7_pref_color = u4_li(in);
	else if (f == "qmtpl")
	{
		int k = u4_li(in);
		std::string name, side;
		int empty = 0, gen = -1;
		in >> name >> side >> empty >> gen;
		PlayerTemplate t;
		t.m_name = AsciiString(w7_name(name).c_str());
		t.m_side = AsciiString(w7_name(side).c_str());
		t.m_building = AsciiString(empty ? "" : "Building");
		if ((int)w7_templates.size() <= k) w7_templates.resize(k + 1);
		w7_templates[k] = t;
		if (gen >= 0) { GeneralPersona p; p.m_enabled = gen != 0; w7_generals[t.m_name.str()] = p; }
		else w7_generals.erase(t.m_name.str());
	}
	else if (f == "popc") { QuickMatchPreferences pref; populateQMColorComboBox(pref); }
	else if (f == "pops")
	{
		int fav = u4_li(in);
		std::string first;
		in >> first;
		if (first == "-") populateQMSideComboBox(fav);
		else
		{
			LadderInfo li;
			li.randomFactions = atoi(first.c_str()) != 0;
			int n = u4_li(in);
			for (int i = 0; i < n; ++i) { std::string s; in >> s; li.validFactions.push_back(AsciiString(w7_name(s).c_str())); }
			populateQMSideComboBox(fav, &li);
		}
	}
	return true;
}
static void u4_menu_dump(std::ostringstream &o)
{
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i] || !BitTest(g_windows[i]->winGetInstanceData()->getStyle(), GWS_SCROLL_LISTBOX)) continue;
		ListboxData *l = (ListboxData *)g_windows[i]->winGetUserData();
		if (!l) continue;
		o << "w7 items " << i;
		for (Int r = 0; r < l->endPos; ++r) o << " " << (unsigned)(uintptr_t)GadgetListBoxGetItemData(g_windows[i], r, 0);
		o << "\n";
	}
}
#include "ui4_main.h"
