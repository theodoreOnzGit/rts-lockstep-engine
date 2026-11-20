/* Differential-test wrapper for the selection translator draft (#215, group lc3, upstream commit 0a05454d),
** src/drafts/sweep_common_client/selection_xlat.rs: SelectionXlat.cpp from `Bool SelectionTranslator::selectFriends` to the doc comment
** of `translateGameMessage` (selectFriends and killThemKillThemAll), cut verbatim by build.rs into sel_xlat.inc.
**
** Stand-ins (outside the cut): the class SelectionTranslator with only the two members the cut reads (m_displayedMaxWarning, m_selectCountMap
** as std::map<const ThingTemplate*, int>, as SelectionXlat.h declares them) and the two functions; Drawable (a template pointer, an object id or
** none, and two fake answers of CanSelectDrawable for dragSelecting FALSE and TRUE); CanSelectDrawable; TheInGameUI (getMaxSelectCount,
** getSelectCount, selectDrawable which counts, message which logs the number formatted into the text); TheGameText->fetch (the string "%d");
** UnicodeString::format (one int argument); GameMessage::appendObjectIDArgument (a list of ids).
**
** Entry point: ref_lc3_sel_run(prog, nprog, out). prog: [maxSelect, selectCount, nDraw, nDraw x (template 0..3, objectId (0 none), canSelect(drag FALSE),
** canSelect(drag TRUE)), nOps, nOps x (kind, draw (1-based, 0 NULL), hasMsg, drag)]. kind 0 selectFriends, kind 1 killThemKillThemAll.
** out per op: result, number of ids appended, the ids, displayedMaxWarning, number of messages, then the logged numbers, the number of selectDrawable
** calls, the select count, and m_selectCountMap's count for templates 0..3. Returns the word count. */
#include <map>
#include <vector>
#include <string.h>

typedef int Int;
typedef bool Bool;
#define TRUE true
#define FALSE false
#define NULL 0

namespace {

class ThingTemplate { public: int id; };
ThingTemplate g_templates[4];

class Drawable
{
public:
	int tmpl;
	int objId;
	int can[2];
	const ThingTemplate *getTemplate() const { return &g_templates[tmpl]; }
	Drawable *getObject() { return objId ? this : NULL; }
	int getID() { return objId; }
};
Drawable g_draws[64];
int g_nDraws = 0;

bool CanSelectDrawable(const Drawable *draw, bool dragSelecting) { return draw->can[dragSelecting ? 1 : 0]; }

typedef Drawable Object; // draw->getObject()->getID(): the "object" is the drawable's own id field

struct UnicodeString
{
	int value;
	void format(const char *, int v) { value = v; }
};
struct GameTextString { const char *str() const { return "%d"; } };
struct GameTextStandIn { GameTextString fetch(const char *) { return GameTextString(); } } gameTextStandIn;
GameTextStandIn *TheGameText = &gameTextStandIn;

struct GameMessage
{
	std::vector<int> ids;
	void appendObjectIDArgument(int id) { ids.push_back(id); }
};

struct InGameUIStandIn
{
	int maxSel, selCount, selectCalls;
	std::vector<int> messages;
	int getMaxSelectCount() { return maxSel; }
	int getSelectCount() { return selCount; }
	void selectDrawable(Drawable *) { ++selCount; ++selectCalls; }
	void message(UnicodeString m) { messages.push_back(m.value); }
} g_ui;
InGameUIStandIn *TheInGameUI = &g_ui;

typedef std::map<const ThingTemplate *, int> SelectCountMap;

class SelectionTranslator
{
public:
	Bool m_displayedMaxWarning;
	SelectCountMap m_selectCountMap;
	Bool selectFriends(Drawable *draw, GameMessage *createTeamMsg, Bool dragSelecting);
	Bool killThemKillThemAll(Drawable *draw, GameMessage *killThemAllMsg);
};

#include "sel_xlat.inc"

}

extern "C" int ref_lc3_sel_run(const int *p, int np, int *out)
{
	int n = 0;
	int k = 0;
	for (int i = 0; i < 4; ++i) g_templates[i].id = i;
	g_ui.maxSel = p[k++];
	g_ui.selCount = p[k++];
	g_ui.selectCalls = 0;
	g_ui.messages.clear();
	g_nDraws = p[k++];
	for (int i = 0; i < g_nDraws; ++i) {
		g_draws[i + 1].tmpl = p[k++];
		g_draws[i + 1].objId = p[k++];
		g_draws[i + 1].can[0] = p[k++];
		g_draws[i + 1].can[1] = p[k++];
	}
	SelectionTranslator t;
	t.m_displayedMaxWarning = FALSE;
	int nOps = p[k++];
	for (int o = 0; o < nOps; ++o) {
		int kind = p[k++], d = p[k++], hasMsg = p[k++], drag = p[k++];
		GameMessage msg;
		Drawable *draw = d ? &g_draws[d] : NULL;
		bool r;
		if (kind == 0) r = t.selectFriends(draw, hasMsg ? &msg : NULL, drag != 0);
		else r = t.killThemKillThemAll(draw, hasMsg ? &msg : NULL);
		out[n++] = r;
		out[n++] = (int)msg.ids.size();
		for (size_t i = 0; i < msg.ids.size(); ++i) out[n++] = msg.ids[i];
		out[n++] = t.m_displayedMaxWarning;
		out[n++] = (int)g_ui.messages.size();
		for (size_t i = 0; i < g_ui.messages.size(); ++i) out[n++] = g_ui.messages[i];
		out[n++] = g_ui.selectCalls;
		out[n++] = g_ui.selCount;
		for (int i = 0; i < 4; ++i) {
			SelectCountMap::iterator it = t.m_selectCountMap.find(&g_templates[i]);
			out[n++] = it == t.m_selectCountMap.end() ? 0 : it->second;
		}
	}
	return n;
}
