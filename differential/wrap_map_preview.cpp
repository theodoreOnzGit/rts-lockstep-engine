/* Differential-test wrapper for the skirmish map preview (GitHub #157, #94; upstream commit 0a05454d). The pieces
** build.rs cuts verbatim into OUT_DIR/map_preview_gen/map_preview_cut.inc:
**   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp: copyFromBigToDir, getMapPreviewImage (to
**     parseMapPreviewChunk), findDrawPositions;
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp:
**     positionStartSpotControls, TheSupplyAndTechImageLocations, positionAdditionalImages and
**     positionStartSpots(AsciiString, ...) (to positionStartSpots(GameInfo *, ...));
**   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp:
**     W3DDrawMapPreview and drawSkinnyBorder (to W3DCommandBarHelpPopupDraw).
**
** Everything below is a STAND-IN, written for this test (on the std::string AsciiString of differential/shim):
** GameWindow (a screen or parent-relative position, a size, a status, enabled image 0, user data, hidden);
** TheDisplay (each drawFillRect / drawLine / drawImage recorded; a NULL image draws nothing, as W3DDisplay::drawImage);
** W3DGameWinDefaultDraw (recorded as one line); TheMappedImageCollection (a fixed set of images, each present or not by
** the case, and those getMapPreviewImage adds, dropped after the case); TheMapCache (the case's maps: extent, players,
** multiplayer flag, waypoints, supply and tech positions; findMap lower-cases); TheGameState's
** realMapPathToPortableMapPath (the Rust side's answer, given); TheGlobalData's user data path (empty, as the
** client's); TheFileSystem (the case's archive files, matched without case; the files written, recorded).
**
** Entry point (extern "C"):
**   ref_map_preview_run(in, out, cap)  runs one case (tests/differential_map_preview.rs's text) and writes the dump;
**                                      its length. */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <string>
#include <vector>
#include <map>
#include <list>
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "GameClient/Color.h"

#ifndef DEBUG_LOG
#define DEBUG_LOG(x)
#endif
#ifndef DEBUG_CRASH
#define DEBUG_CRASH(x)
#endif
#ifndef DEBUG_ASSERTCRASH
#define DEBUG_ASSERTCRASH(c, x)
#endif
#define NEW new
template <typename T> static T max(T a, T b) { return a > b ? a : b; }
template <typename T> static T min(T a, T b) { return a < b ? a : b; }
#define newInstance(T) new T
#define MAP_PREVIEW_DIR_PATH "%sMapPreviews/"
enum { SC_INVALID_DATA = 0xDEAD0003 };
enum { MAX_SLOTS = 8, SUPPLY_TECH_SIZE = 15 };
enum { WIN_STATUS_IMAGE = 0x00000080 };
enum { IMAGE_STATUS_NONE = 0 };

namespace {
std::string g_out;
void outf(const char *fmt, ...)
{
	char b[1024];
	va_list a;
	va_start(a, fmt);
	vsnprintf(b, sizeof b, fmt, a);
	va_end(a);
	g_out += b;
}
}

/* ---- images ---- */
class Image
{
public:
	AsciiString m_name, m_filename;
	UnsignedInt m_status;
	Region2D m_uv;
	Int m_tw, m_th;
	Image() : m_status(0), m_tw(0), m_th(0) { m_uv.lo.x = m_uv.lo.y = m_uv.hi.x = m_uv.hi.y = 0; }
	void setName(AsciiString n) { m_name = n; }
	void setFilename(AsciiString n) { m_filename = n; }
	void setStatus(UnsignedInt s) { m_status = s; }
	void setUV(Region2D *uv) { m_uv = *uv; }
	void setTextureHeight(Int h) { m_th = h; }
	void setTextureWidth(Int w) { m_tw = w; }
};
class MpImageCollection
{
public:
	std::map<std::string, Image *> stable;  /* every name the test uses, kept for the run (upstream's statics hold them) */
	std::map<std::string, bool> present;
	std::vector<Image *> added;
	const Image *findImageByName(const AsciiString &n)
	{
		for (size_t i = 0; i < added.size(); ++i) if (strcmp(added[i]->m_name.str(), n.str()) == 0) return added[i];
		std::map<std::string, bool>::iterator it = present.find(n.str());
		if (it == present.end() || !it->second) return NULL;
		Image *&img = stable[n.str()];
		if (!img) { img = new Image; img->setName(n); }
		return img;
	}
	void addImage(Image *i) { added.push_back(i); }
};
static MpImageCollection g_images;
static MpImageCollection *TheMappedImageCollection = &g_images;

/* ---- the map cache ---- */
typedef std::map<AsciiString, Coord3D> WaypointMap;
typedef std::list<Coord3D> Coord3DList;
typedef std::list<ICoord2D> ICoord2DList;
class TechAndSupplyImages
{
public:
	ICoord2DList m_techPosList;
	ICoord2DList m_supplyPosList;
};
struct MapMetaData
{
	std::string key;
	Region3D m_extent;
	Int m_numPlayers;
	Bool m_isMultiplayer;
	WaypointMap m_waypoints;
	Coord3DList m_supplyPositions;
	Coord3DList m_techPositions;
};
class MapCache : public std::map<AsciiString, MapMetaData>
{
public:
	const MapMetaData *findMap(AsciiString n)
	{
		std::string s(n.str());
		for (size_t i = 0; i < s.size(); ++i) s[i] = (char)tolower((unsigned char)s[i]);
		iterator it = find(AsciiString(s.c_str()));
		return it == end() ? NULL : &it->second;
	}
};
static MapCache *TheMapCache = NULL;

/* ---- windows ---- */
class WinInstanceData {};
class GameWindow
{
public:
	std::string name;
	GameWindow *parent;
	Int x, y, w, h;
	UnsignedInt status;
	const Image *image0;
	void *user;
	Bool hidden;
	GameWindow() : parent(0), x(0), y(0), w(0), h(0), status(0), image0(0), user(0), hidden(FALSE) {}
	void winGetScreenPosition(Int *ox, Int *oy) { Int px = 0, py = 0; if (parent) parent->winGetScreenPosition(&px, &py); *ox = px + x; *oy = py + y; }
	void winGetSize(Int *ow, Int *oh) { *ow = w; *oh = h; }
	void winSetPosition(Int nx, Int ny) { x = nx; y = ny; }
	void winSetUserData(void *u) { user = u; }
	void *winGetUserData() { return user; }
	void winSetStatus(UnsignedInt s) { status |= s; }
	void winClearStatus(UnsignedInt s) { status &= ~s; }
	UnsignedInt winGetStatus() { return status; }
	void winSetEnabledImage(Int, const Image *i) { image0 = i; }
	const Image *winGetEnabledImage(Int) { return image0; }
	void winHide(Bool b) { hidden = b; }
	Bool winIsHidden() { return hidden; }
};
static void W3DGameWinDefaultDraw(GameWindow *, WinInstanceData *) { outf("draw default\n"); }

/* ---- the display ---- */
class MpDisplay
{
public:
	void drawFillRect(Int x, Int y, Int w, Int h, UnsignedInt color) { outf("draw fill %d %d %d %d #%08x\n", x, y, w, h, color); }
	void drawLine(Int x0, Int y0, Int x1, Int y1, Real width, UnsignedInt color) { (void)width; outf("draw line %d %d %d %d #%08x\n", x0, y0, x1, y1, color); }
	void drawImage(const Image *image, Int x0, Int y0, Int x1, Int y1, Color color = 0xFFFFFFFF)
	{
		(void)color;
		if (image == NULL) return;
		outf("draw image %s %d %d %d %d\n", image->m_name.str(), x0, y0, x1, y1);
	}
};
static MpDisplay g_display;
static MpDisplay *TheDisplay = &g_display;

/* ---- the file system, the global data, the game state ---- */
namespace {
std::map<std::string, std::string> g_archive; /* lower-case path -> bytes */
std::string lower(const char *s) { std::string o(s); for (size_t i = 0; i < o.size(); ++i) o[i] = (char)tolower((unsigned char)o[i]); return o; }
std::string g_portable;
}
class File
{
public:
	enum { READ = 1, WRITE = 2, CREATE = 4, BINARY = 8 };
	enum { START, CURRENT, END };
	std::string data, path;
	Bool writing;
	Int pos;
	Int seek(Int off, Int how) { pos = how == END ? (Int)data.size() + off : off; return pos; }
	Int read(void *buf, Int n) { Int k = min(n, (Int)data.size() - pos); memcpy(buf, data.data() + pos, k); pos += k; return k; }
	Int write(const void *buf, Int n) { data.append((const char *)buf, n); return n; }
	void close() { if (writing) outf("fs write %s %d %s\n", path.c_str(), (int)data.size(), data.c_str()); delete this; }
};
class MpFileSystem
{
public:
	Bool doesFileExist(const char *p) { return g_archive.count(lower(p)) != 0; }
	void createDirectory(AsciiString) {}
	File *openFile(const char *p, Int mode)
	{
		File *f = new File;
		f->path = p;
		f->pos = 0;
		f->writing = (mode & File::WRITE) != 0;
		if (!f->writing) {
			std::map<std::string, std::string>::iterator it = g_archive.find(lower(p));
			if (it == g_archive.end()) { delete f; return NULL; }
			f->data = it->second;
		}
		return f;
	}
};
static MpFileSystem g_fs;
static MpFileSystem *TheFileSystem = &g_fs;
class MpGlobalData
{
public:
	AsciiString getPath_UserData() const { return AsciiString(""); }
};
static MpGlobalData g_gd;
static MpGlobalData *TheGlobalData = &g_gd;
class MpGameState
{
public:
	AsciiString realMapPathToPortableMapPath(AsciiString) const { return AsciiString(g_portable.c_str()); }
};
static MpGameState g_gs;
static MpGameState *TheGameState = &g_gs;

/* ---- the upstream code ---- */
#include "map_preview_cut.inc"

namespace {
std::vector<std::string> words(const std::string &l)
{
	std::vector<std::string> w;
	size_t i = 0;
	while (i < l.size()) {
		while (i < l.size() && l[i] == ' ') ++i;
		size_t j = i;
		while (j < l.size() && l[j] != ' ') ++j;
		if (j > i) w.push_back(l.substr(i, j - i));
		i = j;
	}
	return w;
}
Coord3D c3(const std::string &x, const std::string &y)
{
	Coord3D c;
	c.x = (Real)strtod(x.c_str(), NULL);
	c.y = (Real)strtod(y.c_str(), NULL);
	c.z = 0;
	return c;
}
}

/* The case's text (tests/differential_map_preview.rs): `window x y w h hidden`, `buttons w h`, `img <name> <0|1>`,
** `map <key> <lox> <loy> <hix> <hiy> <players> <mp>`, `wp <key> <name> <x> <y>`, `supply <key> <x> <y>`,
** `tech <key> <x> <y>`, `file <path> <bytes>`, `portable <path>`, `select <mapName>` (`-` for none). */
extern "C" int ref_map_preview_run(const char *in, char *out, int cap)
{
	g_out.clear();
	g_archive.clear();
	for (size_t i = 0; i < g_images.added.size(); ++i) delete g_images.added[i];
	g_images.added.clear();
	g_images.present.clear();
	TheSupplyAndTechImageLocations.m_supplyPosList.clear();
	TheSupplyAndTechImageLocations.m_techPosList.clear();
	MapCache cache;
	TheMapCache = &cache;
	GameWindow root, mapWindow, buttons[MAX_SLOTS];
	GameWindow *bp[MAX_SLOTS];
	mapWindow.parent = &root;
	for (int i = 0; i < MAX_SLOTS; ++i) { buttons[i].parent = &mapWindow; bp[i] = &buttons[i]; }
	std::string select;
	std::string text(in);
	size_t at = 0;
	while (at < text.size()) {
		size_t nl = text.find('\n', at);
		if (nl == std::string::npos) nl = text.size();
		std::vector<std::string> w = words(text.substr(at, nl - at));
		at = nl + 1;
		if (w.empty()) continue;
		if (w[0] == "window") {
			mapWindow.x = atoi(w[1].c_str()); mapWindow.y = atoi(w[2].c_str());
			mapWindow.w = atoi(w[3].c_str()); mapWindow.h = atoi(w[4].c_str());
			mapWindow.hidden = atoi(w[5].c_str()) != 0;
		} else if (w[0] == "buttons") {
			for (int i = 0; i < MAX_SLOTS; ++i) {
				buttons[i].w = atoi(w[1].c_str()); buttons[i].h = atoi(w[2].c_str());
				buttons[i].x = 20 * i; buttons[i].y = 0;
			}
		} else if (w[0] == "img") {
			g_images.present[w[1]] = atoi(w[2].c_str()) != 0;
		} else if (w[0] == "map") {
			MapMetaData &m = cache[AsciiString(w[1].c_str())];
			m.key = w[1];
			m.m_extent.lo = c3(w[2], w[3]);
			m.m_extent.hi = c3(w[4], w[5]);
			m.m_numPlayers = atoi(w[6].c_str());
			m.m_isMultiplayer = atoi(w[7].c_str()) != 0;
		} else if (w[0] == "wp") {
			cache[AsciiString(w[1].c_str())].m_waypoints[AsciiString(w[2].c_str())] = c3(w[3], w[4]);
		} else if (w[0] == "supply") {
			cache[AsciiString(w[1].c_str())].m_supplyPositions.push_back(c3(w[2], w[3]));
		} else if (w[0] == "tech") {
			cache[AsciiString(w[1].c_str())].m_techPositions.push_back(c3(w[2], w[3]));
		} else if (w[0] == "file") {
			g_archive[lower(w[1].c_str())] = w[2];
		} else if (w[0] == "portable") {
			g_portable = w[1];
		} else if (w[0] == "select") {
			select = w[1] == "-" ? std::string() : w[1];
		}
	}
	positionStartSpots(AsciiString(select.c_str()), bp, &mapWindow);
	const MapMetaData *u = (const MapMetaData *)mapWindow.winGetUserData();
	outf("user %s\n", u ? u->key.c_str() : "NULL");
	outf("status %d image %s\n", (mapWindow.status & WIN_STATUS_IMAGE) ? 1 : 0, mapWindow.image0 ? mapWindow.image0->m_name.str() : "NULL");
	for (int i = 0; i < MAX_SLOTS; ++i) outf("button %d %d %d %d\n", i, buttons[i].x, buttons[i].y, buttons[i].hidden ? 1 : 0);
	for (ICoord2DList::iterator it = TheSupplyAndTechImageLocations.m_supplyPosList.begin(); it != TheSupplyAndTechImageLocations.m_supplyPosList.end(); ++it) outf("supply %d %d\n", it->x, it->y);
	for (ICoord2DList::iterator it = TheSupplyAndTechImageLocations.m_techPosList.begin(); it != TheSupplyAndTechImageLocations.m_techPosList.end(); ++it) outf("tech %d %d\n", it->x, it->y);
	for (size_t i = 0; i < g_images.added.size(); ++i) {
		const Image *m = g_images.added[i];
		outf("preview %s %s %u %g %g %g %g %d %d\n", m->m_name.str(), m->m_filename.str(), m->m_status, m->m_uv.lo.x, m->m_uv.lo.y, m->m_uv.hi.x, m->m_uv.hi.y, m->m_tw, m->m_th);
	}
	W3DDrawMapPreview(&mapWindow, NULL);
	TheMapCache = NULL;
	int n = (int)g_out.size();
	if (n <= cap) memcpy(out, g_out.data(), n);
	return n;
}
