/* Differential-test wrapper for the save/load screen's list of saves (GitHub #78, upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/Common/GameState.h            (unchanged, renamed by the macros below)
**   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp, the pieces build.rs cuts verbatim into
**   OUT_DIR/save_list_gen/save_list_cut.inc: SaveGameInfo's constructor and destructor, SaveDate::isNewerThan,
**   clearAvailableGames, addGameToAvailableList and populateSaveGameListbox, iterateSaveFiles.
**
** Everything below is a STAND-IN, written for this test:
** - the save directory (Win32's GetCurrentDirectory, SetCurrentDirectory, FindFirstFile, FindNextFile, FindClose and
**   WIN32_FIND_DATA): the case's entries in its order, `.` and `..` (directories) first;
** - getSaveGameInfoFromFile: the case's information of the file, or `throw` for a file the case marks unreadable (the
**   real one throws on an unknown block or a block that does not read);
** - getUnicodeDateBuffer / getUnicodeTimeBuffer (Win32's GetDateFormatW / GetTimeFormatW): the en-US system locale's
**   short date `M/d/yyyy` and time `h:mm tt`, as the Rust's defaults;
** - TheGameText: `T:` + the label; a label starting `NOTEXT` does not exist;
** - the list box (GadgetListBoxReset, GadgetListBoxAddEntryText, GadgetListBoxSetItemData, GadgetListBoxSetSelected):
**   a table of rows of three cells, a row added at the end for row -1, column -1 being 0, an empty text added as " "
**   (GadgetListBoxAddEntryText's own rule), and the selection as GLM_SET_SELECTION leaves it (none after the reset;
**   out of the list's length, none; a row with no cells, kept). The gadget itself is tests/differential_wm.rs's.
** The shim's UnicodeString::format follows MSVC's `%S` (a narrow string) here (SHIM_UNICODE_MSVC_FORMAT).
**
** Entry point (extern "C"):
**   ref_save_list_run(in, out, cap)  runs one case (tests/differential_save_list.rs's text) and writes the dump; its
**                                    length. */
#define SHIM_UNICODE_MSVC_FORMAT 1
#define GameState SlTestGameState
#define TheGameState SlTestTheGameState
#define SaveGameInfo SlTestSaveGameInfo
#define SaveDate SlTestSaveDate
#define AvailableGameInfo SlTestAvailableGameInfo
#define GameWindow SlTestGameWindow
#define getUnicodeDateBuffer SlTestGetUnicodeDateBuffer
#define getUnicodeTimeBuffer SlTestGetUnicodeTimeBuffer
#define TheGameText SlTestTheGameText
#define GadgetListBoxReset SlTestGadgetListBoxReset
#define GadgetListBoxAddEntryText SlTestGadgetListBoxAddEntryText
#define GadgetListBoxSetItemData SlTestGadgetListBoxSetItemData
#define GadgetListBoxSetSelected SlTestGadgetListBoxSetSelected
#define FindFirstFile SlTestFindFirstFile
#define FindNextFile SlTestFindNextFile
#define FindClose SlTestFindClose
#define GetCurrentDirectory SlTestGetCurrentDirectory
#define SetCurrentDirectory SlTestSetCurrentDirectory
#define addGameToAvailableList SlTestAddGameToAvailableList

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <string>
#include <vector>
#include <list>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/Snapshot.h"
/* Windows' SYSTEMTIME, which GameState.h's declarations name */
typedef struct { UnsignedShort wYear, wMonth, wDayOfWeek, wDay, wHour, wMinute, wSecond, wMilliseconds; } SYSTEMTIME;
class INI;
#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Include/Common/GameState.h"
#undef protected
#undef private
#include "GameClient/Color.h"

#ifndef _MAX_PATH
#define _MAX_PATH 260
#endif
#ifndef stricmp
#define stricmp strcasecmp
#endif

namespace {

/* one directory entry of the case */
struct Entry
{
	std::string name;
	bool dir, throws;
	unsigned date[8]; /* year, month, dayOfWeek, day, hour, minute, second, milliseconds */
	std::string desc, label;
	int type;
};
std::vector<Entry> g_dir;
size_t g_at;

/* the list box stand-in */
struct Row
{
	bool set[3];
	std::string text[3];
	int color[3];
	void *data;
};
std::vector<Row> g_rows;
int g_selected;

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

} // namespace

class SlTestGameWindow {};

/* Win32's directory search */
typedef void *HANDLE;
#define INVALID_HANDLE_VALUE ((HANDLE)(long)-1)
#define FILE_ATTRIBUTE_DIRECTORY 0x10
typedef struct { unsigned dwFileAttributes; char cFileName[_MAX_PATH]; } WIN32_FIND_DATA;
static void fill(WIN32_FIND_DATA *d)
{
	d->dwFileAttributes = g_dir[g_at].dir ? FILE_ATTRIBUTE_DIRECTORY : 0;
	snprintf(d->cFileName, sizeof d->cFileName, "%s", g_dir[g_at].name.c_str());
}
HANDLE FindFirstFile(const char *, WIN32_FIND_DATA *d)
{
	g_at = 0;
	if (g_dir.empty()) return INVALID_HANDLE_VALUE;
	fill(d);
	return (HANDLE)1;
}
int FindNextFile(HANDLE, WIN32_FIND_DATA *d)
{
	if (++g_at >= g_dir.size()) return 0;
	fill(d);
	return 1;
}
int FindClose(HANDLE) { return 1; }
unsigned GetCurrentDirectory(unsigned, char *b) { b[0] = 0; return 0; }
int SetCurrentDirectory(const char *) { return 1; }

/* the date formats of an en-US system */
UnicodeString getUnicodeDateBuffer(SYSTEMTIME t)
{
	char b[64];
	snprintf(b, sizeof b, "%u/%u/%u", t.wMonth, t.wDay, t.wYear);
	return wide(b);
}
UnicodeString getUnicodeTimeBuffer(SYSTEMTIME t)
{
	char b[64];
	unsigned h = t.wHour % 12;
	snprintf(b, sizeof b, "%u:%02u %s", h ? h : 12, t.wMinute, t.wHour < 12 ? "AM" : "PM");
	return wide(b);
}

/* TheGameText */
class SlTestGameTextClass
{
public:
	UnicodeString fetch(AsciiString label, Bool *exists = NULL) { return fetch(label.str(), exists); }
	UnicodeString fetch(const char *label, Bool *exists = NULL)
	{
		if (exists) *exists = strncmp(label, "NOTEXT", 6) != 0;
		return wide(std::string("T:") + label);
	}
};
static SlTestGameTextClass g_text;
SlTestGameTextClass *TheGameText = &g_text;

/* the list box */
void GadgetListBoxReset(GameWindow *) { g_rows.clear(); g_selected = -1; }
Int GadgetListBoxAddEntryText(GameWindow *listbox, UnicodeString text, Color color, Int row, Int column = -1, Bool overwrite = TRUE)
{
	if (!listbox) return -1;
	if (text.isEmpty()) text = UnicodeString(L" ");
	if (row == -1) {
		Row r;
		memset(r.set, 0, sizeof r.set);
		memset(r.color, 0, sizeof r.color);
		r.data = NULL;
		g_rows.push_back(r);
		row = (Int)g_rows.size() - 1;
	}
	if (column == -1) column = 0;
	if (row < 0 || row >= (Int)g_rows.size() || column < 0 || column > 2) return -1;
	g_rows[row].set[column] = true;
	g_rows[row].text[column] = narrow(text);
	g_rows[row].color[column] = color;
	(void)overwrite;
	return row;
}
void GadgetListBoxSetItemData(GameWindow *, void *data, Int row, Int column = 0)
{
	if (row >= 0 && row < (Int)g_rows.size() && column == 0) g_rows[row].data = data;
}
/* GLM_SET_SELECTION on a single-select list box of LENGTH 100: out of the list, none; a row without cells (not added),
** the selection kept; else the row */
void GadgetListBoxSetSelected(GameWindow *, Int selectIndex)
{
	if (selectIndex < 0 || 100 <= selectIndex) g_selected = -1;
	else if (selectIndex < (Int)g_rows.size()) g_selected = selectIndex;
}

/* GameState's members the cut calls that it does not define */
GameState::GameState(void) { m_availableGames = NULL; m_isInLoadGame = FALSE; }
GameState::~GameState(void) { clearAvailableGames(); }
void GameState::init(void) {}
void GameState::reset(void) {}
void GameState::xfer(Xfer *) {}
AsciiString GameState::getSaveDirectory() const { return AsciiString("Save\\"); }
void GameState::getSaveGameInfoFromFile(AsciiString filename, SaveGameInfo *info)
{
	for (size_t i = 0; i < g_dir.size(); ++i) {
		const Entry &e = g_dir[i];
		if (e.name != filename.str()) continue;
		if (e.throws) throw (int)SC_INVALID_DATA;
		info->date.year = e.date[0];
		info->date.month = e.date[1];
		info->date.dayOfWeek = e.date[2];
		info->date.day = e.date[3];
		info->date.hour = e.date[4];
		info->date.minute = e.date[5];
		info->date.second = e.date[6];
		info->date.milliseconds = e.date[7];
		info->description = wide(e.desc);
		info->mapLabel.set(e.label.c_str());
		info->saveFileType = (SaveFileType)e.type;
		return;
	}
}
GameState *TheGameState = NULL;

/* ---- the upstream code ---- */
#include "save_list_cut.inc"

/* The case's text: the layout type on the first line, then one entry a line: name, dir, throws, the eight date fields,
** the type, then the description and the label after `|` each. */
extern "C" int ref_save_list_run(const char *in, char *out, int cap)
{
	g_dir.clear();
	g_rows.clear();
	g_selected = -99;
	Entry dot;
	dot.dir = true;
	dot.throws = false;
	memset(dot.date, 0, sizeof dot.date);
	dot.type = 0;
	dot.name = ".";
	g_dir.push_back(dot);
	dot.name = "..";
	g_dir.push_back(dot);
	std::string text(in);
	size_t at = 0;
	int layoutType = 0;
	bool first = true;
	while (at < text.size()) {
		size_t nl = text.find('\n', at);
		if (nl == std::string::npos) nl = text.size();
		std::string line = text.substr(at, nl - at);
		at = nl + 1;
		if (first) { layoutType = atoi(line.c_str()); first = false; continue; }
		if (line.empty()) continue;
		size_t p1 = line.find('|'), p2 = line.find('|', p1 + 1), p3 = line.find('|', p2 + 1);
		Entry e;
		e.name = line.substr(0, p1);
		unsigned dir, th;
		sscanf(line.substr(p1 + 1, p2 - p1 - 1).c_str(), "%u %u %u %u %u %u %u %u %u %u %d", &dir, &th, &e.date[0], &e.date[1], &e.date[2], &e.date[3], &e.date[4], &e.date[5], &e.date[6], &e.date[7], &e.type);
		e.dir = dir != 0;
		e.throws = th != 0;
		e.desc = line.substr(p2 + 1, p3 - p2 - 1);
		e.label = line.substr(p3 + 1);
		g_dir.push_back(e);
	}
	GameState gs;
	TheGameState = &gs;
	SlTestGameWindow listbox;
	gs.populateSaveGameListbox(&listbox, (SaveLoadLayoutType)layoutType);
	std::string o;
	char b[256];
	for (size_t r = 0; r < g_rows.size(); ++r) {
		snprintf(b, sizeof b, "row %d:", (int)r);
		o += b;
		for (int c = 0; c < 3; ++c) {
			if (!g_rows[r].set[c]) { o += " -"; continue; }
			snprintf(b, sizeof b, " [%s]#%08x", g_rows[r].text[c].c_str(), (unsigned)g_rows[r].color[c]);
			o += b;
		}
		o += " data ";
		o += g_rows[r].data ? ((AvailableGameInfo *)g_rows[r].data)->filename.str() : "NULL";
		o += "\n";
	}
	snprintf(b, sizeof b, "selected %d\n", g_selected);
	o += b;
	TheGameState = NULL;
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), n);
	return n;
}
