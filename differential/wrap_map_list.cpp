/* Differential-test wrapper for the skirmish map list (GitHub #89, upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp, the pieces build.rs cuts verbatim into
**   OUT_DIR/map_list_gen/map_list_cut.inc: populateMapListboxNoReset and populateMapListbox.
**
** Everything below is a STAND-IN, written for this test:
** - rts::less_than_nocase<UnicodeString> (the real STLTypedefs.h lines 216-222: compareNoCase < 0) and UnicodeString's
**   operator< (UnicodeString.h: wcscmp), which the shim's UnicodeString lacks;
** - TheMapCache (a std::map<AsciiString, MapMetaData> with getMapDir() "Maps", as MapUtil.cpp's), MapMetaData's three
**   fields the cut reads, TheGlobalData's user data path;
** - TheMappedImageCollection (the case's star images, by name; Image::getImageWidth the case's gold width) and
**   SkirmishBattleHonors::getEnduranceMedal (the case's medals by map and slot state);
** - the list box (GadgetListBoxGetNumColumns, GetColumnWidth, Reset, AddEntryImage, AddEntryText, SetItemData,
**   SetSelected, GetTopVisibleEntry, GetBottomVisibleEntry, SetTopVisibleEntry): a table of rows of two cells; a row
**   added at the end for row -1; the selection as GLM_SET_SELECTION leaves it on a list of LENGTH 100 (out of it, none;
**   a row with no cells, kept); the top row after the selection (GLM_SET_SELECTION scrolls a row into view) and after
**   SetTopVisibleEntry(x), and the bottom row, the case's (the Rust gadget's answer in the same state: see tests/differential_map_list.rs), and on
**   an empty list the read of listData[-1] recorded as undefined.
**
** Entry point (extern "C"):
**   ref_map_list_run(in, out, cap)  runs one case (tests/differential_map_list.rs's text) and writes the dump; its
**                                   length. */
#define GameWindow MlTestGameWindow
#define Image MlTestImage
#define MapCache MlTestMapCache
#define MapMetaData MlTestMapMetaData
#define SkirmishBattleHonors MlTestSkirmishBattleHonors
#define TheMapCache MlTestTheMapCache
#define TheGlobalData MlTestTheGlobalData
#define TheMappedImageCollection MlTestTheMappedImageCollection

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <string>
#include <vector>
#include <map>
#include <set>
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/STLTypedefs.h"
#include "GameClient/Color.h"

/* the real STLTypedefs.h's specialisation for UnicodeString, and UnicodeString's operator< */
inline bool operator<(const UnicodeString &a, const UnicodeString &b) { return a.compare(b.str()) < 0; }
namespace rts
{
	template<> struct less_than_nocase<UnicodeString>
	{
		bool operator()(const UnicodeString& __t1, const UnicodeString& __t2) const
		{
			return (__t1.compareNoCase(__t2) < 0);
		}
	};
}

template <typename T> static T max(T a, T b) { return a > b ? a : b; }
template <typename T> static T min(T a, T b) { return a < b ? a : b; }

/* GameInfo.h's SlotState values the cut reads */
enum { SLOT_EASY_AI = 2, SLOT_MED_AI = 3, SLOT_BRUTAL_AI = 4 };

namespace {

struct Cell
{
	int kind; /* 0 none, 1 text, 2 image */
	std::string s;
	int color, w, h;
};
struct Row
{
	Cell c[2];
	const void *data0;
	long data1;
	bool data1Set;
};
std::vector<Row> g_rows;
int g_selected, g_top, g_columns, g_colw0, g_goldWidth, g_call, g_bottom[2];
bool g_ub;
int g_selTop[2]; /* the gadget's top row after the call's GLM_SET_SELECTION (the Rust gadget's) */
std::vector<int> g_tops[2]; /* the gadget's top row after SetTopVisibleEntry(x), per call (the Rust gadget's) */
std::set<std::string> g_images;
std::map<std::string, int> g_medals; /* "<map>|<state>" */
std::string g_userData;

std::string narrow(const UnicodeString &u)
{
	std::string s;
	for (int i = 0; i < u.getLength(); ++i) s += (char)u.getCharAt(i);
	return s;
}

UnicodeString wide(const std::string &s)
{
	UnicodeString u;
	std::basic_string<WideChar> w;
	for (size_t i = 0; i < s.size(); ++i) w += (WideChar)(unsigned char)s[i];
	u.set(w.c_str());
	return u;
}

Row newRow()
{
	Row r;
	for (int i = 0; i < 2; ++i) { r.c[i].kind = 0; r.c[i].color = 0; r.c[i].w = 0; r.c[i].h = 0; }
	r.data0 = NULL;
	r.data1 = 0;
	r.data1Set = false;
	return r;
}

} // namespace

class MlTestGameWindow {};

class MlTestImage
{
public:
	std::string name;
	Int getImageWidth() const { return g_goldWidth; }
};
class MlTestImageCollection
{
public:
	MlTestImage images[4];
	const MlTestImage *findImageByName(const char *name)
	{
		static const char *names[4] = { "Star-Bronze", "Star-Silver", "Star-Gold", "RedYell_Star" };
		for (int i = 0; i < 4; ++i) {
			if (strcmp(name, names[i]) == 0 && g_images.count(name)) {
				images[i].name = name;
				return &images[i];
			}
		}
		return NULL;
	}
};
static MlTestImageCollection g_collection;
MlTestImageCollection *TheMappedImageCollection = &g_collection;

class MlTestSkirmishBattleHonors
{
public:
	Int getEnduranceMedal(AsciiString map, Int state) const
	{
		char k[512];
		snprintf(k, sizeof k, "%s|%d", map.str(), state);
		std::map<std::string, int>::const_iterator it = g_medals.find(k);
		return it == g_medals.end() ? 0 : it->second;
	}
};

struct MlTestMapMetaData
{
	UnicodeString m_displayName;
	Int m_numPlayers;
	Bool m_isMultiplayer;
};
class MlTestMapCache : public std::map<AsciiString, MlTestMapMetaData>
{
public:
	AsciiString getMapDir() const { return AsciiString("Maps"); }
};
MlTestMapCache *TheMapCache = NULL;

class MlTestGlobalData
{
public:
	AsciiString getPath_UserData() const { return AsciiString(g_userData.c_str()); }
};
static MlTestGlobalData g_globalData;
MlTestGlobalData *TheGlobalData = &g_globalData;

/* the list box */
Int GadgetListBoxGetNumColumns(GameWindow *) { return g_columns; }
Int GadgetListBoxGetColumnWidth(GameWindow *, Int column) { return column == 0 ? g_colw0 : 0; }
void GadgetListBoxReset(GameWindow *) { g_rows.clear(); g_selected = -1; g_top = 0; }
Int GadgetListBoxAddEntryImage(GameWindow *listbox, const MlTestImage *image, Int row, Int column, Int hight, Int width, Bool overwrite, Color color = -1)
{
	if (!listbox) return -1;
	if (row == -1) {
		g_rows.push_back(newRow());
		row = (Int)g_rows.size() - 1;
	}
	if (row < 0 || row >= (Int)g_rows.size() || column < 0 || column > 1) return -1;
	Cell &c = g_rows[row].c[column];
	c.kind = 2;
	c.s = image ? image->name : std::string("<none>");
	c.w = width;
	c.h = hight;
	c.color = color;
	(void)overwrite;
	return row;
}
Int GadgetListBoxAddEntryText(GameWindow *listbox, UnicodeString text, Color color, Int row, Int column = -1, Bool overwrite = TRUE)
{
	if (!listbox) return -1;
	if (row == -1) {
		g_rows.push_back(newRow());
		row = (Int)g_rows.size() - 1;
	}
	if (column == -1) column = 0;
	if (row < 0 || row >= (Int)g_rows.size() || column < 0 || column > 1) return -1;
	Cell &c = g_rows[row].c[column];
	c.kind = 1;
	c.s = narrow(text);
	c.color = color;
	(void)overwrite;
	return row;
}
void GadgetListBoxSetItemData(GameWindow *, void *data, Int row, Int column = 0)
{
	if (row < 0 || row >= (Int)g_rows.size()) return;
	if (column == 0) g_rows[row].data0 = data;
	else if (column == 1) { g_rows[row].data1 = (long)data; g_rows[row].data1Set = true; }
}
void GadgetListBoxSetSelected(GameWindow *, const Int *selectList, Int selectCount)
{
	(void)selectCount;
	Int i = selectList[0];
	if (i < 0 || 100 <= i) g_selected = -1;
	else if (i < (Int)g_rows.size()) g_selected = i;
	g_top = g_selTop[g_call];
}
Int GadgetListBoxGetTopVisibleEntry(GameWindow *) { return g_top; }
Int GadgetListBoxGetBottomVisibleEntry(GameWindow *)
{
	if (g_rows.empty()) { g_ub = true; return 0; }
	return g_bottom[g_call];
}
void GadgetListBoxSetTopVisibleEntry(GameWindow *, Int newPos)
{
	const std::vector<int> &t = g_tops[g_call];
	if (newPos >= 0 && newPos < (Int)t.size()) g_top = t[newPos];
	else g_ub = true; /* (not in the table: the comparison would be meaningless) */
}

/* ---- the upstream code ---- */
#include "map_list_cut.inc"

/* The case's text: `cfg sys mp cols colw0 images goldw second sys2 mp2 bottom1 bottom2 user`, then one line a map
** (`map <key>|<display>|<players>|<mp>|<easy> <medium> <brutal>`), then `sel <mapToSelect>`. */
extern "C" int ref_map_list_run(const char *in, char *out, int cap)
{
	MlTestMapCache cache;
	TheMapCache = &cache;
	g_rows.clear();
	g_medals.clear();
	g_images.clear();
	g_selected = -1;
	g_top = 0;
	g_ub = false;
	g_call = 0;
	g_tops[0].clear();
	g_tops[1].clear();
	int sys = 0, mp = 0, second = 0, sys2 = 0, mp2 = 0, imageMask = 0;
	std::string sel;
	std::string text(in);
	size_t at = 0;
	while (at < text.size()) {
		size_t nl = text.find('\n', at);
		if (nl == std::string::npos) nl = text.size();
		std::string line = text.substr(at, nl - at);
		at = nl + 1;
		if (line.compare(0, 4, "cfg ") == 0) {
			char user[256] = "";
			sscanf(line.c_str() + 4, "%d %d %d %d %d %d %d %d %d %d %d %255s", &sys, &mp, &g_columns, &g_colw0, &imageMask, &g_goldWidth, &second, &sys2, &mp2, &g_bottom[0], &g_bottom[1], user);
			g_userData = user;
			static const char *names[4] = { "Star-Bronze", "Star-Silver", "Star-Gold", "RedYell_Star" };
			for (int i = 0; i < 4; ++i) if (imageMask & (1 << i)) g_images.insert(names[i]);
		} else if (line.compare(0, 4, "map ") == 0) {
			std::string rest = line.substr(4);
			size_t p1 = rest.find('|'), p2 = rest.find('|', p1 + 1), p3 = rest.find('|', p2 + 1), p4 = rest.find('|', p3 + 1);
			std::string key = rest.substr(0, p1);
			MlTestMapMetaData md;
			md.m_displayName = wide(rest.substr(p1 + 1, p2 - p1 - 1));
			md.m_numPlayers = atoi(rest.substr(p2 + 1, p3 - p2 - 1).c_str());
			md.m_isMultiplayer = atoi(rest.substr(p3 + 1, p4 - p3 - 1).c_str()) != 0;
			int e = 0, m = 0, b = 0;
			sscanf(rest.substr(p4 + 1).c_str(), "%d %d %d", &e, &m, &b);
			char k[512];
			snprintf(k, sizeof k, "%s|%d", key.c_str(), SLOT_EASY_AI); g_medals[k] = e;
			snprintf(k, sizeof k, "%s|%d", key.c_str(), SLOT_MED_AI); g_medals[k] = m;
			snprintf(k, sizeof k, "%s|%d", key.c_str(), SLOT_BRUTAL_AI); g_medals[k] = b;
			cache[AsciiString(key.c_str())] = md;
		} else if (line.compare(0, 4, "sel ") == 0) {
			sel = line.substr(4);
		} else if (line.compare(0, 8, "seltops ") == 0) {
			sscanf(line.c_str() + 8, "%d %d", &g_selTop[0], &g_selTop[1]);
		} else if (line.compare(0, 4, "tops") == 0) {
			int k = line[4] - '1';
			const char *p = line.c_str() + 5;
			char *e;
			for (;;) {
				long v = strtol(p, &e, 10);
				if (e == p) break;
				g_tops[k].push_back((int)v);
				p = e;
			}
		}
	}
	MlTestGameWindow listbox;
	int r1 = populateMapListbox(&listbox, sys != 0, mp != 0, AsciiString(sel.c_str()));
	int r2 = -99;
	if (second) {
		g_call = 1;
		r2 = populateMapListboxNoReset(&listbox, sys2 != 0, mp2 != 0, AsciiString(sel.c_str()));
	}
	std::string o;
	char b[1024];
	for (size_t r = 0; r < g_rows.size(); ++r) {
		snprintf(b, sizeof b, "row %d:", (int)r);
		o += b;
		for (int c = 0; c < g_columns && c < 2; ++c) {
			const Cell &cl = g_rows[r].c[c];
			if (cl.kind == 0) o += " -";
			else if (cl.kind == 1) { snprintf(b, sizeof b, " [%s]#%08x", cl.s.c_str(), (unsigned)cl.color); o += b; }
			else { snprintf(b, sizeof b, " img:%s %dx%d", cl.s.c_str(), cl.w, cl.h); o += b; }
		}
		o += " data ";
		o += g_rows[r].data0 ? (const char *)g_rows[r].data0 : "NULL";
		if (g_rows[r].data1Set) { snprintf(b, sizeof b, " level %ld", g_rows[r].data1); o += b; }
		o += "\n";
	}
	snprintf(b, sizeof b, "selected %d top %d ret %d %d ub %d\n", g_selected, g_top, r1, r2, g_ub ? 1 : 0);
	o += b;
	TheMapCache = NULL;
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), n);
	return n;
}
