/* Differential-test wrapper for the replay list (GitHub #98, upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ReplayMenu.cpp, the pieces build.rs cuts
**   verbatim into OUT_DIR/replay_list_gen/replay_list_cut.inc: GetReplayFilenameFromListbox and
**   PopulateReplayFileListbox.
**
** Everything below is a STAND-IN, written for this test:
** - UnicodeString::concat(UnicodeString) (UnicodeString.h: concat of the other's str()), which the shim lacks, as a
**   free helper the cut reaches through a derived class;
** - TheRecorder (getReplayDir "Replays\\", getReplayExtention ".rep", getLastReplayFileName the case's, and
**   readReplayHeader: the case's header for the replay folder's file of that name, if the folder has it, its unread
**   fields flagged when read),
**   RecorderClass::ReplayHeader's fields, ReplayGameInfo and ParseAsciiStringToGameInfo (the case's answer and map:
**   the game options are "<parsed>|<map>");
** - TheFileSystem::getFileListInDirectory (the case's paths under the folder ending in the search's extension,
**   without case, into the REAL FilenameList: std::set with the real STLTypedefs.h less_than_nocase<AsciiString>);
** - TheMapCache (findMap of the lower-cased name: the case's display names; updateCache does nothing), TheVersion,
**   TheGlobalData's CRCs, TheGameText's GUI:LastReplay (the case's text);
** - getUnicodeTimeBuffer: the host's en-US `h:mm tt`, flagged undefined for an unread or out-of-range time (the Rust
**   host's default);
** - the list box (Reset, AddEntryText, SetSelected, GetText): a table of rows of four cells; an empty text is " " (the
**   real GadgetListBoxAddEntryText's); row -1 appends; the selection as GLM_SET_SELECTION leaves it on a list of
**   LENGTH 100.
**
** Entry point (extern "C"):
**   ref_replay_list_run(in, out, cap)  runs one case (tests/differential_replay_list.rs's text) and writes the dump;
**                                      its length. */
#define GameWindow RlTestGameWindow
#define MapMetaData RlTestMapMetaData
#define TheMapCache RlTestTheMapCache
#define TheGlobalData RlTestTheGlobalData
#define TheRecorder RlTestTheRecorder
#define TheFileSystem RlTestTheFileSystem
#define TheVersion RlTestTheVersion
#define TheGameText RlTestTheGameText
#define RecorderClass RlTestRecorderClass
#define ReplayGameInfo RlTestReplayGameInfo
#define SYSTEMTIME RlTestSystemTime
#define FilenameList RlTestFilenameList
#define FilenameListIter RlTestFilenameListIter
#define UnicodeString RlTestUnicodeStringBase

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
#undef UnicodeString
/* the shim's class, with UnicodeString.h's concat(UnicodeString) */
class UnicodeString : public RlTestUnicodeStringBase
{
public:
	UnicodeString() {}
	UnicodeString(const RlTestUnicodeStringBase &b) : RlTestUnicodeStringBase(b) {}
	UnicodeString(const WideChar *s) : RlTestUnicodeStringBase(s) {}
	UnicodeString(const wchar_t *s) : RlTestUnicodeStringBase(s) {}
	using RlTestUnicodeStringBase::concat;
	void concat(const UnicodeString &o)
	{
		for (int i = 0; i < o.getLength(); ++i) RlTestUnicodeStringBase::concat(o.getCharAt(i));
	}
};
#include "Common/STLTypedefs.h"
#include "GameClient/Color.h"

typedef std::set<AsciiString, rts::less_than_nocase<AsciiString> > RlTestFilenameList;
typedef RlTestFilenameList::iterator RlTestFilenameListIter;

namespace {

bool g_ub;

struct Cell
{
	bool set;
	std::string s;
	int color;
};
struct Row
{
	Cell c[4];
};
std::vector<Row> g_rows;
int g_selected;

std::string narrow(const UnicodeString &u)
{
	std::string s;
	for (int i = 0; i < u.getLength(); ++i) {
		WideChar c = u.getCharAt(i);
		if (c < 0x80) s += (char)c;
		else { char b[16]; snprintf(b, sizeof b, "\\u%04x", (unsigned)c); s += b; }
	}
	return s;
}

UnicodeString wide(const std::string &s)
{
	std::basic_string<WideChar> w;
	for (size_t i = 0; i < s.size(); ++i) w += (WideChar)(unsigned char)s[i];
	UnicodeString u;
	u.set(w.c_str());
	return u;
}

std::string lower(std::string s)
{
	for (size_t i = 0; i < s.size(); ++i) s[i] = (char)tolower((unsigned char)s[i]);
	return s;
}

/* A header field the case may leave unread: reading it unread is flagged. */
struct UbUInt
{
	UnsignedInt v;
	bool set;
	operator UnsignedInt() const { if (!set) g_ub = true; return v; }
};

struct CaseHeader
{
	bool ok;
	std::string options;
	UbUInt versionNumber, exeCRC, iniCRC;
	int local;
	bool timeSet;
	int hour, minute, second, ms;
	std::string versionString;
};
std::map<std::string, CaseHeader> g_headers; /* by the file name, lower-cased (the file system's) */
std::vector<std::string> g_files;
std::map<std::string, std::string> g_maps; /* lower-cased key: display name */
std::string g_lastName, g_lastText, g_thisVersion;
UnsignedInt g_vnum, g_exe, g_ini;

} // namespace

class RlTestGameWindow {};

struct RlTestSystemTime
{
	unsigned short wYear, wMonth, wDayOfWeek, wDay, wHour, wMinute, wSecond, wMilliseconds;
	bool set; /* (the test's: whether the read reached it) */
};

class RlTestRecorderClass
{
public:
	struct ReplayHeader
	{
		AsciiString filename;
		Bool forPlayback;
		UnicodeString replayName;
		SYSTEMTIME timeVal;
		UnicodeString versionString;
		UnicodeString versionTimeString;
		UbUInt versionNumber;
		UbUInt exeCRC;
		UbUInt iniCRC;
		AsciiString gameOptions;
		Int localPlayerIndex;
	};
	AsciiString getReplayDir() { return AsciiString("Replays\\"); }
	AsciiString getReplayExtention() { return AsciiString(".rep"); }
	AsciiString getLastReplayFileName() { return AsciiString(g_lastName.c_str()); }
	Bool readReplayHeader(ReplayHeader &header)
	{
		/* (the replay folder's file of that name: opened only if it is there) */
		std::string path = lower(std::string("Replays\\") + header.filename.str());
		bool there = false;
		for (size_t i = 0; i < g_files.size(); ++i) if (lower(g_files[i]) == path) there = true;
		std::map<std::string, CaseHeader>::const_iterator it = g_headers.find(lower(header.filename.str()));
		if (!there || it == g_headers.end() || !it->second.ok) return FALSE;
		const CaseHeader &c = it->second;
		header.replayName = wide("(name)");
		header.timeVal.set = c.timeSet;
		header.timeVal.wHour = (unsigned short)c.hour;
		header.timeVal.wMinute = (unsigned short)c.minute;
		header.timeVal.wSecond = (unsigned short)c.second;
		header.timeVal.wMilliseconds = (unsigned short)c.ms;
		header.versionString = wide(c.versionString);
		header.versionNumber = c.versionNumber;
		header.exeCRC = c.exeCRC;
		header.iniCRC = c.iniCRC;
		header.gameOptions.set(c.options.c_str());
		header.localPlayerIndex = c.local;
		return TRUE;
	}
};
static RlTestRecorderClass g_recorder;
RlTestRecorderClass *TheRecorder = &g_recorder;

class RlTestReplayGameInfo
{
public:
	AsciiString m_map;
	AsciiString getMap() const { return m_map; }
};
Bool ParseAsciiStringToGameInfo(RlTestReplayGameInfo *info, AsciiString options)
{
	std::string o(options.str());
	size_t bar = o.find('|');
	if (bar == std::string::npos) return FALSE;
	info->m_map.set(o.substr(bar + 1).c_str());
	return o[0] == '1';
}

class RlTestFileSystem
{
public:
	void getFileListInDirectory(const AsciiString &dir, const AsciiString &search, RlTestFilenameList &list, Bool recursive)
	{
		(void)recursive;
		std::string d = lower(dir.str()), ext = lower(std::string(search.str()).substr(1));
		for (size_t i = 0; i < g_files.size(); ++i) {
			std::string l = lower(g_files[i]);
			if (l.compare(0, d.size(), d) == 0 && l.size() >= ext.size() && l.compare(l.size() - ext.size(), ext.size(), ext) == 0)
				list.insert(AsciiString(g_files[i].c_str()));
		}
	}
};
static RlTestFileSystem g_fs;
RlTestFileSystem *TheFileSystem = &g_fs;

struct RlTestMapMetaData
{
	UnicodeString m_displayName;
};
class RlTestMapCache
{
public:
	RlTestMapMetaData md;
	void updateCache() {}
	const RlTestMapMetaData *findMap(AsciiString name)
	{
		std::map<std::string, std::string>::const_iterator it = g_maps.find(lower(name.str()));
		if (it == g_maps.end()) return NULL;
		md.m_displayName = wide(it->second);
		return &md;
	}
};
RlTestMapCache *TheMapCache = NULL;

class RlTestVersion
{
public:
	UnicodeString getUnicodeVersion() const { return wide(g_thisVersion); }
	UnsignedInt getVersionNumber() const { return g_vnum; }
};
static RlTestVersion g_version;
RlTestVersion *TheVersion = &g_version;

class RlTestGlobalData
{
public:
	UnsignedInt m_exeCRC, m_iniCRC;
};
static RlTestGlobalData g_globalData;
RlTestGlobalData *TheGlobalData = &g_globalData;

class RlTestGameText
{
public:
	UnicodeString fetch(const char *label)
	{
		if (strcmp(label, "GUI:LastReplay") == 0) return wide(g_lastText);
		return wide(std::string("?") + label);
	}
};
static RlTestGameText g_text;
RlTestGameText *TheGameText = &g_text;

/* the host's en-US GetTimeFormatW(TIME_NOSECONDS): `h:mm tt`; an unread or out-of-range time is undefined */
UnicodeString getUnicodeTimeBuffer(RlTestSystemTime t)
{
	if (!t.set || t.wHour > 23 || t.wMinute > 59 || t.wSecond > 59 || t.wMilliseconds > 999) {
		g_ub = true;
		return UnicodeString();
	}
	int h12 = t.wHour % 12 == 0 ? 12 : t.wHour % 12;
	char b[64];
	snprintf(b, sizeof b, "%d:%02d %s", h12, (int)t.wMinute, t.wHour < 12 ? "AM" : "PM");
	return wide(b);
}

/* the list box */
void GadgetListBoxReset(GameWindow *listbox) { if (listbox) { g_rows.clear(); g_selected = -1; } }
Int GadgetListBoxAddEntryText(GameWindow *listbox, UnicodeString text, Color color, Int row, Int column = -1, Bool overwrite = TRUE)
{
	if (!listbox) return -1;
	if (text.isEmpty()) text = wide(" ");
	if (row == -1) {
		Row r;
		for (int i = 0; i < 4; ++i) { r.c[i].set = false; r.c[i].color = 0; }
		g_rows.push_back(r);
		row = (Int)g_rows.size() - 1;
	}
	if (column == -1) column = 0;
	if (row < 0 || row >= (Int)g_rows.size() || column < 0 || column > 3) return -1;
	Cell &c = g_rows[row].c[column];
	c.set = true;
	c.s = narrow(text);
	c.color = color;
	(void)overwrite;
	return row;
}
void GadgetListBoxSetSelected(GameWindow *listbox, Int i)
{
	if (!listbox) return;
	if (i < 0 || 100 <= i) g_selected = -1;
	else if (i < (Int)g_rows.size()) g_selected = i;
}
UnicodeString GadgetListBoxGetText(GameWindow *listbox, Int row, Int column = 0)
{
	if (!listbox || row < 0 || row >= (Int)g_rows.size() || !g_rows[row].c[column].set) return UnicodeString();
	std::basic_string<WideChar> w;
	const std::string &s = g_rows[row].c[column].s;
	/* (the dump's text: ASCII and `\uXXXX` escapes, read back) */
	for (size_t i = 0; i < s.size(); ++i) {
		if (s[i] == '\\' && i + 5 < s.size() && s[i + 1] == 'u') {
			w += (WideChar)strtoul(s.substr(i + 2, 4).c_str(), NULL, 16);
			i += 5;
		} else w += (WideChar)(unsigned char)s[i];
	}
	UnicodeString u;
	u.set(w.c_str());
	return u;
}

/* ---- the upstream code ---- */
#include "replay_list_cut.inc"

/* The case's text: `cfg <hasCache> <vnum> <exe> <ini>`, `last <name>`, `text <GUI:LastReplay's text>`, `version <s>`,
** then `file <path>`, `hdr <name>|<ok>|<options>|<vnum or ->|<exe or ->|<ini or ->|<local>|<h m s ms or ->|<version>`,
** `map <key>|<display>`. */
extern "C" int ref_replay_list_run(const char *in, char *out, int cap)
{
	RlTestMapCache cache;
	g_rows.clear();
	g_selected = -1;
	g_ub = false;
	g_headers.clear();
	g_files.clear();
	g_maps.clear();
	g_lastName.clear();
	g_lastText.clear();
	g_thisVersion.clear();
	int hasCache = 1;
	std::string text(in);
	size_t at = 0;
	while (at < text.size()) {
		size_t nl = text.find('\n', at);
		if (nl == std::string::npos) nl = text.size();
		std::string line = text.substr(at, nl - at);
		at = nl + 1;
		if (line.compare(0, 4, "cfg ") == 0) {
			sscanf(line.c_str() + 4, "%d %u %u %u", &hasCache, &g_vnum, &g_exe, &g_ini);
		} else if (line.compare(0, 5, "last ") == 0) {
			g_lastName = line.substr(5);
		} else if (line.compare(0, 5, "text ") == 0) {
			g_lastText = line.substr(5);
		} else if (line.compare(0, 8, "version ") == 0) {
			g_thisVersion = line.substr(8);
		} else if (line.compare(0, 5, "file ") == 0) {
			g_files.push_back(line.substr(5));
		} else if (line.compare(0, 4, "map ") == 0) {
			std::string rest = line.substr(4);
			size_t p = rest.find('|');
			g_maps[lower(rest.substr(0, p))] = rest.substr(p + 1);
		} else if (line.compare(0, 4, "hdr ") == 0) {
			std::vector<std::string> f;
			std::string rest = line.substr(4);
			size_t s0 = 0;
			for (;;) {
				size_t p = rest.find('|', s0);
				f.push_back(rest.substr(s0, p == std::string::npos ? std::string::npos : p - s0));
				if (p == std::string::npos) break;
				s0 = p + 1;
			}
			CaseHeader h;
			h.ok = f[1] == "1";
			h.options = f[2] + "|" + f[3];
			UbUInt *u[3] = { &h.versionNumber, &h.exeCRC, &h.iniCRC };
			for (int i = 0; i < 3; ++i) {
				u[i]->set = f[4 + i] != "-";
				u[i]->v = u[i]->set ? (UnsignedInt)strtoul(f[4 + i].c_str(), NULL, 10) : 0;
			}
			h.local = atoi(f[7].c_str());
			h.timeSet = f[8] != "-";
			h.hour = h.minute = h.second = h.ms = 0;
			if (h.timeSet) sscanf(f[8].c_str(), "%d %d %d %d", &h.hour, &h.minute, &h.second, &h.ms);
			h.versionString = f[9];
			g_headers[lower(f[0])] = h;
		}
	}
	g_globalData.m_exeCRC = g_exe;
	g_globalData.m_iniCRC = g_ini;
	TheMapCache = hasCache ? &cache : NULL;
	RlTestGameWindow listbox;
	g_rows.clear();
	PopulateReplayFileListbox(&listbox);
	std::string o;
	char b[2048];
	for (size_t r = 0; r < g_rows.size(); ++r) {
		snprintf(b, sizeof b, "row %d:", (int)r);
		o += b;
		for (int c = 0; c < 4; ++c) {
			const Cell &cl = g_rows[r].c[c];
			if (!cl.set) o += " -";
			else { snprintf(b, sizeof b, " [%s]#%08x", cl.s.c_str(), (unsigned)cl.color); o += b; }
		}
		o += "\n";
	}
	snprintf(b, sizeof b, "selected %d ub %d\n", g_selected, g_ub ? 1 : 0);
	o += b;
	for (int r = -1; r < (int)g_rows.size(); ++r) {
		UnicodeString f = GetReplayFilenameFromListbox(&listbox, r);
		snprintf(b, sizeof b, "get %d: %s\n", r, narrow(f).c_str());
		o += b;
	}
	TheMapCache = NULL;
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), n);
	return n;
}
