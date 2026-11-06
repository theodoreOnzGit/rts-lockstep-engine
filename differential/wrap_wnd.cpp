/* Differential-test wrapper for the WND layout reader (#12, upstream commit 0a05454d).
**
** GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp is cut VERBATIM by build.rs into OUT_DIR/wnd_gen:
** - wnd_script_a.inc: from `// PRIVATE TYPES` (the buffer and stack sizes, the parse structs) to
**   `// GameWindowManager::winCreateLayout`: the statics, every parser, getDataTemplate, parseData, setWindowText,
**   createGadget, createWindow, parseChildWindows, gameWindowFieldList, parseWindow, parseInit/Update/Shutdown and
**   parseLayoutBlock;
** - wnd_script_b.inc: from `WindowLayoutInfo::WindowLayoutInfo() :` to the end: the info's constructor and
**   winCreateFromScript.
** (winCreateLayout and freeStaticStrings, between them, need WindowLayout and are not reached.)
** The gadget data structs come verbatim from Gadget.h (wnd_gadget_types.inc: `typedef struct _SliderData` to
** `// INLINING`).
**
** Everything else here is a STAND-IN, written for this test:
** - the base types, AsciiString (a std::string: set(NULL) is empty, compare is strcmp), UnicodeString (units of
**   WideChar; translate sign-extends each char, as upstream's (WideChar) cast of a char), WinInstanceData (the fields
**   of WinInstanceData.h; setTooltipText keeps the text);
** - the File: RAMFile's read, scanInt, scanString (a NUL byte appends nothing: concat of { c, 0 }) and nextLine,
**   transcribed from RAMFile.cpp lines 263-457. Where upstream would read stale buffer bytes or loop forever, it
**   throws (`read` of one byte at the end of the file; 64 `scanString`s in a row at the end);
** - TheWindowManager: winCreate (the region, normalised, the size, the status; the instance data copied),
**   the gogoGadget* constructors (each records the gadget's data and, for sliders, list boxes and combo boxes, the
**   inner parts' draw data as the script's globals hold them at that moment; no inner windows are created, so
**   winGetChild and the Gadget*Get* part accessors return NULL), winDestroy (throws: the Rust stops there too),
**   winSendInputMsg (nothing);
** - the Gadget*SetText functions and winSetText record the text; the callbacks record the script's
**   theSystemString ... theDrawString (a resolved pointer is set only for a name other than "" and "[None]");
** - TheNameKeyGenerator: keys 1, 2, ... in order of first use; TheMappedImageCollection: names starting `img` (any
**   case) are images named `Img` + the rest; TheGameText: `T:` + the label; TheFontLibrary: a font per request;
**   TheHeaderTemplateManager: `HT1` gives the font HTFont 12 bold; TheDisplay: the size the test gives;
** - sscanf, atoi, strcmp, stricmp, strcpy and strlen through checked versions that throw where upstream is undefined
**   (a NULL argument; a number out of Int's range; a strcpy longer than its array).
**
** Each case runs in a forked child (fresh statics, as a new process; crashes and hangs cannot take the test down),
** which writes a dump of the windows and the info, in the format the Rust test writes, to a pipe. */
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <cassert>
#include <climits>
#include <string>
#include <vector>
#include <list>
#include <map>
#include <unistd.h>
#include <signal.h>
#include <sys/wait.h>
#include <strings.h>

/* ---- base types (Lib/BaseType.h) */
typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
typedef short Short;
typedef unsigned short UnsignedShort;
typedef float Real;
typedef char Byte;
typedef char Char;
typedef unsigned char UnsignedByte;
typedef wchar_t WideChar;
typedef Int Color;
typedef UnsignedInt NameKeyType;
#define TRUE 1
#define FALSE 0
#define NEW new
#define _MAX_PATH 260
#define DEBUG_LOG(x) ((void)0)
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define BitTest(x, i) (((x) & (i)) != 0)
struct ICoord2D { Int x, y; };
struct IRegion2D { ICoord2D lo, hi; };

struct WndStop {};

/* ---- checked C library calls */
static int shim_scan(const char *s, long long *out)
{
	if (s == NULL) throw WndStop();
	const char *p = s;
	while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\v' || *p == '\f' || *p == '\r') ++p;
	bool neg = false;
	if (*p == '+' || *p == '-') { neg = (*p == '-'); ++p; }
	if (*p < '0' || *p > '9') return 0;
	long long m = 0;
	while (*p >= '0' && *p <= '9') { if (m < 10000000000LL) m = m * 10 + (*p - '0'); ++p; }
	m = neg ? -m : m;
	if (m < INT_MIN || m > INT_MAX) throw WndStop();
	*out = m;
	return 1;
}
static int shim_sscanf(const char *s, const char *, Int *v) { long long x; int r = shim_scan(s, &x); if (r) *v = (Int)x; return r; }
static int shim_sscanf(const char *s, const char *, UnsignedInt *v) { long long x; int r = shim_scan(s, &x); if (r) *v = (UnsignedInt)(Int)x; return r; }
static int shim_atoi(const char *s) { long long x = 0; if (!shim_scan(s, &x)) return 0; return (int)x; }
static int shim_strcmp(const char *a, const char *b) { if (!a || !b) throw WndStop(); return strcmp(a, b); }
static int shim_stricmp(const char *a, const char *b) { if (!a || !b) throw WndStop(); return strcasecmp(a, b); }
static size_t shim_strlen(const char *a) { if (!a) throw WndStop(); return strlen(a); }
template <size_t N> static char *shim_strcpy(char (&d)[N], const char *s)
{
	if (!s || strlen(s) >= N) throw WndStop();
	return strcpy(d, s);
}

/* ---- strings */
class AsciiString
{
public:
	static const AsciiString TheEmptyString;
	AsciiString() {}
	AsciiString(const char *c) { set(c); }
	void set(const char *c) { m = c ? c : ""; }
	AsciiString &operator=(const char *c) { set(c); return *this; }
	const char *str() const { return m.c_str(); }
	void clear() { m.clear(); }
	Int compare(const char *c) const { return strcmp(m.c_str(), c); }
	Bool isEmpty() const { return m.empty(); }
	Int getLength() const { return (Int)m.size(); }
	char getCharAt(Int i) const { return m[i]; }
	void concat(char c) { char t[2] = { c, 0 }; m += t; }
	std::string m;
};
const AsciiString AsciiString::TheEmptyString;

class UnicodeString
{
public:
	void set(const WideChar *s) { m.clear(); while (*s) m.push_back(*s++); }
	void translate(const AsciiString &a) { m.clear(); for (Int i = 0; i < a.getLength(); ++i) m.push_back((WideChar)a.getCharAt(i)); }
	Int getLength() const { return (Int)m.size(); }
	std::vector<WideChar> m;
};

/* ---- the things the script points to */
class Image { public: std::string name; };
class GameFont { public: std::string name; Int size; Bool bold; };
class DisplayString;
class GameWindow;
class WindowLayout;

/* ---- GameWindow.h, Gadget.h, WinInstanceData.h: the constants (transcribed) */
enum { WIN_COLOR_UNDEFINED = 0x00FFFFFF };
enum { GWM_SCRIPT_CREATE = 23 };
enum { MAX_WINDOW_NAME_LEN = 64, MAX_DRAW_DATA = 9, MAX_TEXT_LABEL = 128 };
enum { GWS_PUSH_BUTTON = 0x1, GWS_RADIO_BUTTON = 0x2, GWS_CHECK_BOX = 0x4, GWS_VERT_SLIDER = 0x8, GWS_HORZ_SLIDER = 0x10,
	GWS_SCROLL_LISTBOX = 0x20, GWS_ENTRY_FIELD = 0x40, GWS_STATIC_TEXT = 0x80, GWS_PROGRESS_BAR = 0x100, GWS_USER_WINDOW = 0x200,
	GWS_MOUSE_TRACK = 0x400, GWS_ANIMATED = 0x800, GWS_TAB_STOP = 0x1000, GWS_TAB_CONTROL = 0x2000, GWS_TAB_PANE = 0x4000,
	GWS_COMBO_BOX = 0x8000 };
typedef UnsignedInt WindowMsgData;
enum WindowMsgHandledType { MSG_IGNORED, MSG_HANDLED };
struct WinInstanceData;
typedef void (*GameWinDrawFunc)(GameWindow *, WinInstanceData *);
typedef void (*GameWinTooltipFunc)(GameWindow *, WinInstanceData *, UnsignedInt);
typedef WindowMsgHandledType (*GameWinInputFunc)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData);
typedef WindowMsgHandledType (*GameWinSystemFunc)(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData);
typedef void (*WindowLayoutInitFunc)(WindowLayout *layout, void *userData);
typedef void (*WindowLayoutUpdateFunc)(WindowLayout *layout, void *userData);
typedef void (*WindowLayoutShutdownFunc)(WindowLayout *layout, void *userData);

#include "wnd_gadget_types.inc"

struct WinDrawData { const Image *image; Color color; Color borderColor; };
struct TextDrawData { Color color; Color borderColor; };
struct WinInstanceData
{
	WinInstanceData() { init(); m_headerTemplateName.clear(); }
	void init()
	{
		for (Int i = 0; i < MAX_DRAW_DATA; i++) {
			WinDrawData d = { NULL, WIN_COLOR_UNDEFINED, WIN_COLOR_UNDEFINED };
			m_enabledDrawData[i] = d; m_disabledDrawData[i] = d; m_hiliteDrawData[i] = d;
		}
		TextDrawData t = { WIN_COLOR_UNDEFINED, WIN_COLOR_UNDEFINED };
		m_enabledText = t; m_disabledText = t; m_hiliteText = t;
		m_id = 0; m_state = 0; m_style = 0; m_status = 0; m_owner = NULL;
		m_textLabelString.clear(); m_tooltipString.clear(); m_tooltipDelay = -1; m_decoratedNameString.clear();
		m_imageOffset.x = 0; m_imageOffset.y = 0; m_font = NULL; m_hasTooltip = false;
	}
	void setTooltipText(UnicodeString tip) { m_tooltip = tip; m_hasTooltip = true; }
	UnsignedInt getStatus() { return m_status; }
	Int m_id; Int m_state; UnsignedInt m_style; UnsignedInt m_status; GameWindow *m_owner;
	WinDrawData m_enabledDrawData[MAX_DRAW_DATA], m_disabledDrawData[MAX_DRAW_DATA], m_hiliteDrawData[MAX_DRAW_DATA];
	TextDrawData m_enabledText, m_disabledText, m_hiliteText;
	ICoord2D m_imageOffset; GameFont *m_font;
	AsciiString m_textLabelString, m_decoratedNameString, m_tooltipString, m_headerTemplateName;
	Int m_tooltipDelay;
	UnicodeString m_tooltip; bool m_hasTooltip;
};

struct GameWindowEditData { AsciiString systemCallbackString, inputCallbackString, tooltipCallbackString, drawCallbackString; };

struct PartDraw { int part; WinDrawData e[MAX_DRAW_DATA], d[MAX_DRAW_DATA], h[MAX_DRAW_DATA]; };

class GameWindow
{
public:
	Int m_index; GameWindow *m_parent; IRegion2D m_region; ICoord2D m_size; UnsignedInt m_status; Int m_windowId;
	WinInstanceData m_instData; std::string m_cb[4]; bool m_hasText; UnicodeString m_text; std::string m_gadget;
	std::vector<PartDraw> m_parts;
	Int winGetSize(Int *w, Int *h) { *w = m_size.x; *h = m_size.y; return 0; }
	Int winGetScreenPosition(Int *x, Int *y)
	{
		*x = m_region.lo.x; *y = m_region.lo.y;
		for (GameWindow *p = m_parent; p; p = p->m_parent) { *x += p->m_region.lo.x; *y += p->m_region.lo.y; }
		return 0;
	}
	GameWindow *winGetChild() { return NULL; }
	GameWindow *winGetNext() { return NULL; }
	WinInstanceData *winGetInstanceData() { return &m_instData; }
	UnsignedInt winGetStyle() { return m_instData.m_style; }
	Int winSetWindowId(Int id) { m_windowId = id; return 0; }
	void winSetSystemFunc(GameWinSystemFunc);
	void winSetInputFunc(GameWinInputFunc);
	void winSetTooltipFunc(GameWinTooltipFunc);
	void winSetDrawFunc(GameWinDrawFunc);
	GameWindowEditData *winGetEditData() { return NULL; }
	Int winSetInstanceData(WinInstanceData *d) { m_instData = *d; return 0; }
	Int winSetText(UnicodeString t) { m_text = t; m_hasText = true; return 0; }
};

class WindowLayoutInfo
{
public:
	WindowLayoutInfo();
	UnsignedInt version;
	WindowLayoutInitFunc init;
	WindowLayoutUpdateFunc update;
	WindowLayoutShutdownFunc shutdown;
	AsciiString initNameString, updateNameString, shutdownNameString;
	std::list<GameWindow *> windows;
};

class File
{
public:
	enum { READ = 1 };
	std::string data; Int pos; Int eofScans;
	Int read(void *buffer, Int bytes)
	{
		Int left = (Int)data.size() - pos;
		if (bytes > left) bytes = left;
		if (buffer != NULL && bytes <= 0) throw WndStop(); /* readUntilSemicolon past the end */
		if (bytes > 0 && buffer != NULL) memcpy(buffer, &data[pos], bytes);
		pos += bytes;
		return bytes;
	}
	Bool scanInt(Int &newInt)
	{
		newInt = 0;
		AsciiString tempstr;
		Int size = (Int)data.size();
		while (pos < size && (data[pos] < '0' || data[pos] > '9') && data[pos] != '-') ++pos;
		if (pos >= size) { pos = size; return FALSE; }
		do { tempstr.concat(data[pos]); ++pos; } while (pos < size && data[pos] >= '0' && data[pos] <= '9');
		newInt = shim_atoi(tempstr.str());
		return TRUE;
	}
	Bool scanString(AsciiString &newString)
	{
		newString.clear();
		Int size = (Int)data.size();
		while (pos < size && isspace(data[pos])) ++pos;
		if (pos >= size) { pos = size; if (++eofScans > 64) throw WndStop(); return FALSE; }
		do { newString.concat(data[pos]); ++pos; } while (pos < size && !isspace(data[pos]));
		return TRUE;
	}
	void nextLine(Char *buf = NULL, Int bufSize = 0)
	{
		Int size = (Int)data.size();
		while (pos < size && data[pos] != '\n') ++pos;
		if (pos < size) ++pos;
	}
	File *convertToRAMFile() { return this; }
	void close() {}
};

class FileSystem { public: File *openFile(const char *, Int); };
class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &s)
	{
		std::map<std::string, NameKeyType>::iterator it = keys.find(s.m);
		if (it != keys.end()) return it->second;
		NameKeyType k = (NameKeyType)keys.size() + 1;
		keys[s.m] = k; names[k] = s.m;
		return k;
	}
	std::map<std::string, NameKeyType> keys; std::map<NameKeyType, std::string> names;
};
static WindowMsgHandledType fakeSystem(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
static void fakeDraw(GameWindow *, WinInstanceData *) {}
static void fakeTooltip(GameWindow *, WinInstanceData *, UnsignedInt) {}
static void fakeLayout(WindowLayout *, void *) {}
class FunctionLexicon
{
public:
	bool known(NameKeyType key);
	GameWinSystemFunc gameWinSystemFunc(NameKeyType k) { return known(k) ? fakeSystem : NULL; }
	GameWinInputFunc gameWinInputFunc(NameKeyType k) { return known(k) ? fakeSystem : NULL; }
	GameWinTooltipFunc gameWinTooltipFunc(NameKeyType k) { return known(k) ? fakeTooltip : NULL; }
	GameWinDrawFunc gameWinDrawFunc(NameKeyType k) { return known(k) ? fakeDraw : NULL; }
	WindowLayoutInitFunc winLayoutInitFunc(NameKeyType k) { return known(k) ? fakeLayout : NULL; }
	WindowLayoutUpdateFunc winLayoutUpdateFunc(NameKeyType k) { return known(k) ? fakeLayout : NULL; }
	WindowLayoutShutdownFunc winLayoutShutdownFunc(NameKeyType k) { return known(k) ? fakeLayout : NULL; }
};
class Display { public: Int w, h; Int getWidth() { return w; } Int getHeight() { return h; } };
class FontLibrary { public: GameFont *getFont(AsciiString name, Int size, Bool bold) { GameFont *f = new GameFont; f->name = name.m; f->size = size; f->bold = bold; return f; } };
class HeaderTemplateManager
{
public:
	GameFont *getFontFromTemplate(AsciiString name)
	{
		if (name.m != "HT1") return NULL;
		static GameFont f; f.name = "HTFont"; f.size = 12; f.bold = true; return &f;
	}
};
class ImageCollection
{
public:
	const Image *findImageByName(const AsciiString &name)
	{
		if (name.m.size() < 3 || strncasecmp(name.m.c_str(), "img", 3) != 0) return NULL;
		std::string canon = "Img" + name.m.substr(3);
		Image *&im = images[canon];
		if (!im) { im = new Image; im->name = canon; }
		return im;
	}
	std::map<std::string, Image *> images;
};
class GameTextInterface
{
public:
	UnicodeString fetch(const char *label)
	{
		UnicodeString u; std::string s = std::string("T:") + label;
		for (size_t i = 0; i < s.size(); ++i) u.m.push_back((WideChar)(unsigned char)s[i]);
		return u;
	}
};

class GameWindowManager
{
public:
	GameWindow *winCreate(GameWindow *parent, UnsignedInt status, Int x, Int y, Int width, Int height, GameWinSystemFunc system,
		WinInstanceData *instData = NULL);
	GameWindow *gogoGadgetPushButton(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, GameFont *, Bool);
	GameWindow *gogoGadgetCheckbox(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, GameFont *, Bool);
	GameWindow *gogoGadgetRadioButton(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, RadioButtonData *, GameFont *, Bool);
	GameWindow *gogoGadgetTabControl(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, TabControlData *, GameFont *, Bool);
	GameWindow *gogoGadgetListBox(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, ListboxData *, GameFont *, Bool);
	GameWindow *gogoGadgetSlider(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, SliderData *, GameFont *, Bool);
	GameWindow *gogoGadgetComboBox(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, ComboBoxData *, GameFont *, Bool);
	GameWindow *gogoGadgetProgressBar(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, GameFont *, Bool);
	GameWindow *gogoGadgetStaticText(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, TextData *, GameFont *, Bool);
	GameWindow *gogoGadgetTextEntry(GameWindow *, UnsignedInt, Int, Int, Int, Int, WinInstanceData *, EntryData *, GameFont *, Bool);
	WindowMsgHandledType winSendInputMsg(GameWindow *, UnsignedInt, WindowMsgData, WindowMsgData) { return MSG_IGNORED; }
	Int winDestroy(GameWindow *) { throw WndStop(); }
	Color winMakeColor(UnsignedByte red, UnsignedByte green, UnsignedByte blue, UnsignedByte alpha);
	GameWindow *winCreateFromScript(AsciiString filename, WindowLayoutInfo *info = NULL);
	void winCreateLayout(AsciiString); /* declared, not defined: the segments leave it out */
	std::vector<GameWindow *> created;
};

/* GameClient/Color.h (transcribed) */
inline Color GameMakeColor(UnsignedByte red, UnsignedByte green, UnsignedByte blue, UnsignedByte alpha)
{
	return (alpha << 24) | (red << 16) | (green << 8) | (blue);
}

static NameKeyGenerator g_names;
static FunctionLexicon g_lexicon;
static Display g_display;
static FontLibrary g_fonts;
static HeaderTemplateManager g_headers;
static ImageCollection g_images;
static GameTextInterface g_text;
static GameWindowManager g_manager;
static FileSystem g_fs;
static std::vector<File> g_files;
static size_t g_nextFile = 0;

NameKeyGenerator *TheNameKeyGenerator = &g_names;
FunctionLexicon *TheFunctionLexicon = &g_lexicon;
Display *TheDisplay = &g_display;
FontLibrary *TheFontLibrary = &g_fonts;
HeaderTemplateManager *TheHeaderTemplateManager = &g_headers;
ImageCollection *TheMappedImageCollection = &g_images;
GameTextInterface *TheGameText = &g_text;
GameWindowManager *TheWindowManager = &g_manager;
FileSystem *TheFileSystem = &g_fs;

bool FunctionLexicon::known(NameKeyType key)
{
	std::string n = g_names.names[key];
	return !n.empty() && n != "[None]";
}
File *FileSystem::openFile(const char *, Int) { return &g_files[g_nextFile++]; }

static void GadgetButtonSetText(GameWindow *w, UnicodeString t) { w->winSetText(t); }
static void GadgetRadioSetText(GameWindow *w, UnicodeString t) { w->winSetText(t); }
static void GadgetCheckBoxSetText(GameWindow *w, UnicodeString t) { w->winSetText(t); }
static void GadgetStaticTextSetText(GameWindow *w, UnicodeString t) { w->winSetText(t); }
static void GadgetTextEntrySetText(GameWindow *w, UnicodeString t) { w->winSetText(t); }
static void GadgetTabControlFixupSubPaneList(GameWindow *) {}
static GameWindow *GadgetListBoxGetUpButton(GameWindow *) { return NULL; }
static GameWindow *GadgetListBoxGetDownButton(GameWindow *) { return NULL; }
static GameWindow *GadgetListBoxGetSlider(GameWindow *) { return NULL; }
static GameWindow *GadgetComboBoxGetDropDownButton(GameWindow *) { return NULL; }
static GameWindow *GadgetComboBoxGetEditBox(GameWindow *) { return NULL; }
static GameWindow *GadgetComboBoxGetListBox(GameWindow *) { return NULL; }

/* the segments see the checked C calls */
#define sscanf shim_sscanf
#define atoi shim_atoi
#define strcmp shim_strcmp
#define stricmp shim_stricmp
#define strcpy shim_strcpy
#define strlen shim_strlen
#include "wnd_script_a.inc"
#include "wnd_script_b.inc"
#undef sscanf
#undef atoi
#undef strcmp
#undef stricmp
#undef strcpy
#undef strlen

/* ---- the stand-ins that see the segments' statics */
void GameWindow::winSetSystemFunc(GameWinSystemFunc) { m_cb[0] = theSystemString.m; }
void GameWindow::winSetInputFunc(GameWinInputFunc) { m_cb[1] = theInputString.m; }
void GameWindow::winSetTooltipFunc(GameWinTooltipFunc) { m_cb[2] = theTooltipString.m; }
void GameWindow::winSetDrawFunc(GameWinDrawFunc) { m_cb[3] = theDrawString.m; }
Color GameWindowManager::winMakeColor(UnsignedByte r, UnsignedByte g, UnsignedByte b, UnsignedByte a) { return GameMakeColor(r, g, b, a); }

GameWindow *GameWindowManager::winCreate(GameWindow *parent, UnsignedInt status, Int x, Int y, Int width, Int height,
	GameWinSystemFunc, WinInstanceData *instData)
{
	GameWindow *w = new GameWindow;
	w->m_index = (Int)created.size();
	created.push_back(w);
	w->m_parent = parent;
	w->m_status = status;
	w->m_size.x = width; w->m_size.y = height;
	w->m_region.lo.x = x; w->m_region.lo.y = y; w->m_region.hi.x = x + width; w->m_region.hi.y = y + height;
	if (w->m_region.lo.x > w->m_region.hi.x) { Int t = w->m_region.lo.x; w->m_region.lo.x = w->m_region.hi.x; w->m_region.hi.x = t; }
	if (w->m_region.lo.y > w->m_region.hi.y) { Int t = w->m_region.lo.y; w->m_region.lo.y = w->m_region.hi.y; w->m_region.hi.y = t; }
	w->m_windowId = 0; w->m_hasText = false;
	if (instData) w->winSetInstanceData(instData);
	return w;
}

static void addPart(GameWindow *w, int part, WinDrawData *e, WinDrawData *d, WinDrawData *h)
{
	PartDraw p; p.part = part;
	for (int i = 0; i < MAX_DRAW_DATA; ++i) { p.e[i] = e[i]; p.d[i] = d[i]; p.h[i] = h[i]; }
	w->m_parts.push_back(p);
}
/* the part numbers are the Rust GadgetPart order */
static void listParts(GameWindow *w)
{
	addPart(w, 0, enabledUpButtonDrawData, disabledUpButtonDrawData, hiliteUpButtonDrawData);
	addPart(w, 1, enabledDownButtonDrawData, disabledDownButtonDrawData, hiliteDownButtonDrawData);
	addPart(w, 2, enabledSliderDrawData, disabledSliderDrawData, hiliteSliderDrawData);
	addPart(w, 3, enabledSliderThumbDrawData, disabledSliderThumbDrawData, hiliteSliderThumbDrawData);
}
static std::string fmt(const char *f, long long a, long long b = 0, long long c = 0, long long d = 0, long long e = 0, long long g = 0, long long h = 0)
{
	char buf[256]; snprintf(buf, sizeof buf, f, a, b, c, d, e, g, h); return buf;
}

#define GOGO(name, extra) GameWindow *GameWindowManager::name(GameWindow *parent, UnsignedInt status, Int x, Int y, Int w, Int h, \
	WinInstanceData *instData extra, GameFont *, Bool)
GOGO(gogoGadgetPushButton, ) { return winCreate(parent, status, x, y, w, h, NULL, instData); }
GOGO(gogoGadgetCheckbox, ) { return winCreate(parent, status, x, y, w, h, NULL, instData); }
GOGO(gogoGadgetProgressBar, ) { return winCreate(parent, status, x, y, w, h, NULL, instData); }
GameWindow *GameWindowManager::gogoGadgetRadioButton(GameWindow *parent, UnsignedInt status, Int x, Int y, Int w, Int h,
	WinInstanceData *instData, RadioButtonData *r, GameFont *, Bool)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	win->m_gadget = fmt("radio %lld %lld", r->screen, r->group);
	return win;
}
#undef GOGO
#define GOGO2(name, T) GameWindow *GameWindowManager::name(GameWindow *parent, UnsignedInt status, Int x, Int y, Int w, Int h, \
	WinInstanceData *instData, T *data, GameFont *, Bool)
GOGO2(gogoGadgetTabControl, TabControlData)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	TabControlData *t = data;
	win->m_gadget = fmt("tab %lld %lld %lld %lld %lld %lld", t->tabOrientation, t->tabEdge, t->tabWidth, t->tabHeight, t->tabCount, t->paneBorder);
	for (int i = 0; i < 8; ++i) win->m_gadget += t->subPaneDisabled[i] ? " 1" : " 0";
	return win;
}
GOGO2(gogoGadgetListBox, ListboxData)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	ListboxData *l = data;
	win->m_gadget = fmt("list %lld %lld %lld%lld%lld%lld%lld", l->listLength, l->columns, l->autoScroll, l->autoPurge, l->scrollBar,
		l->multiSelect, l->forceSelect) + fmt("%lld", l->scrollIfAtEnd);
	if (l->columnWidthPercentage) for (int i = 0; i < l->columns; ++i) win->m_gadget += fmt(" %lld", l->columnWidthPercentage[i]);
	listParts(win);
	return win;
}
GOGO2(gogoGadgetSlider, SliderData)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	win->m_gadget = fmt("slider %lld %lld", data->minVal, data->maxVal);
	addPart(win, 3, enabledSliderThumbDrawData, disabledSliderThumbDrawData, hiliteSliderThumbDrawData);
	return win;
}
GOGO2(gogoGadgetComboBox, ComboBoxData)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	ComboBoxData *c = data;
	win->m_gadget = fmt("combo %lld %lld %lld %lld %lld", c->isEditable, c->maxDisplay, c->maxChars, c->asciiOnly, c->lettersAndNumbersOnly);
	addPart(win, 4, enabledDropDownButtonDrawData, disabledDropDownButtonDrawData, hiliteDropDownButtonDrawData);
	addPart(win, 5, enabledEditBoxDrawData, disabledEditBoxDrawData, hiliteEditBoxDrawData);
	addPart(win, 6, enabledListBoxDrawData, disabledListBoxDrawData, hiliteListBoxDrawData);
	listParts(win);
	return win;
}
GOGO2(gogoGadgetStaticText, TextData)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	win->m_gadget = fmt("text %lld %lld %lld %lld", data->centered, data->centeredVertically, data->leftMargin, data->topMargin);
	return win;
}
GOGO2(gogoGadgetTextEntry, EntryData)
{
	GameWindow *win = winCreate(parent, status, x, y, w, h, NULL, instData);
	EntryData *e = data;
	win->m_gadget = fmt("entry %lld %lld %lld %lld %lld", e->secretText, e->numericalOnly, e->alphaNumericalOnly, e->aSCIIOnly, e->maxTextLen);
	return win;
}
#undef GOGO2

/* ---- the dump (the Rust test writes the same) */
static void dumpUnits(std::string &o, bool has, const std::vector<WideChar> &u)
{
	if (!has) { o += "-"; return; }
	o += "[";
	for (size_t i = 0; i < u.size(); ++i) o += fmt(i ? " %llx" : "%llx", (long long)(UnsignedShort)u[i]);
	o += "]";
}
static void dumpDraw(std::string &o, const WinDrawData *d)
{
	for (int i = 0; i < MAX_DRAW_DATA; ++i) {
		o += " ";
		o += d[i].image ? d[i].image->name : std::string("-");
		o += fmt("/%llx/%llx", (long long)(UnsignedInt)d[i].color, (long long)(UnsignedInt)d[i].borderColor);
	}
}
static std::string dumpAll(const std::vector<std::pair<GameWindow *, WindowLayoutInfo> > &results)
{
	std::string o;
	for (size_t f = 0; f < results.size(); ++f) {
		const WindowLayoutInfo &info = results[f].second;
		o += fmt("F %lld first=%lld", (long long)f, results[f].first ? results[f].first->m_index : -1);
		o += " init=" + info.initNameString.m + " update=" + info.updateNameString.m + " shutdown=" + info.shutdownNameString.m;
		o += fmt(" r=%lld%lld%lld v=%lld", info.init != NULL, info.update != NULL, info.shutdown != NULL, info.version);
		o += " windows=";
		for (std::list<GameWindow *>::const_iterator it = info.windows.begin(); it != info.windows.end(); ++it)
			o += fmt("%lld,", *it ? (*it)->m_index : -1);
		o += "\n";
	}
	for (size_t i = 0; i < g_manager.created.size(); ++i) {
		GameWindow *w = g_manager.created[i];
		WinInstanceData &d = w->m_instData;
		o += fmt("W %lld p=%lld st=%llx sty=%llx id=%lld", w->m_index, w->m_parent ? w->m_parent->m_index : -1, w->m_status, d.m_style, w->m_windowId);
		o += fmt(" rg=%lld,%lld,%lld,%lld sz=%lld,%lld", w->m_region.lo.x, w->m_region.lo.y, w->m_region.hi.x, w->m_region.hi.y, w->m_size.x, w->m_size.y);
		o += " n=" + d.m_decoratedNameString.m + " lbl=" + d.m_textLabelString.m + " tip=" + d.m_tooltipString.m + " ht=" + d.m_headerTemplateName.m;
		o += fmt(" td=%lld off=%lld,%lld st2=%llx", d.m_tooltipDelay, d.m_imageOffset.x, d.m_imageOffset.y, d.m_status);
		o += " font=";
		o += d.m_font ? d.m_font->name + fmt(",%lld,%lld", d.m_font->size, d.m_font->bold) : std::string("-");
		o += " txt="; dumpUnits(o, w->m_hasText, w->m_text.m);
		o += " tt="; dumpUnits(o, d.m_hasTooltip, d.m_tooltip.m);
		o += " cb=" + w->m_cb[0] + "|" + w->m_cb[1] + "|" + w->m_cb[2] + "|" + w->m_cb[3];
		o += " g=" + (w->m_gadget.empty() ? std::string("-") : w->m_gadget) + "\n";
		o += " E"; dumpDraw(o, d.m_enabledDrawData); o += "\n";
		o += " D"; dumpDraw(o, d.m_disabledDrawData); o += "\n";
		o += " H"; dumpDraw(o, d.m_hiliteDrawData); o += "\n";
		o += fmt(" T %llx/%llx %llx/%llx %llx/%llx\n", (UnsignedInt)d.m_enabledText.color, (UnsignedInt)d.m_enabledText.borderColor,
			(UnsignedInt)d.m_disabledText.color, (UnsignedInt)d.m_disabledText.borderColor, (UnsignedInt)d.m_hiliteText.color,
			(UnsignedInt)d.m_hiliteText.borderColor);
		for (size_t p = 0; p < w->m_parts.size(); ++p) {
			o += fmt(" P%lld", w->m_parts[p].part);
			dumpDraw(o, w->m_parts[p].e); dumpDraw(o, w->m_parts[p].d); dumpDraw(o, w->m_parts[p].h);
			o += "\n";
		}
	}
	return o;
}

extern "C" {

/* Reads `count` files (each `lens[i]` bytes, concatenated in `data`) in order through winCreateFromScript, at a display
** of width x height, in a forked child. The dump goes to `out` (up to cap bytes; *out_len its full length).
** Returns 0 when every file was read, 1 when the reader stopped where upstream is undefined or never returns (a
** checked call threw), 2 when the child crashed, 3 when it ran past 10 seconds. */
int ref_wnd_run(const char *data, const int *lens, int count, int width, int height, char *out, int cap, int *out_len)
{
	int fds[2];
	if (pipe(fds) != 0) return -1;
	fflush(NULL);
	pid_t pid = fork();
	if (pid == 0) {
		close(fds[0]);
		alarm(10);
		g_display.w = width; g_display.h = height;
		int code = 0;
		std::vector<std::pair<GameWindow *, WindowLayoutInfo> > results;
		try {
			const char *p = data;
			for (int i = 0; i < count; ++i) {
				File f; f.data.assign(p, p + lens[i]); f.pos = 0; f.eofScans = 0;
				p += lens[i];
				g_files.push_back(f);
			}
			for (int i = 0; i < count; ++i) {
				WindowLayoutInfo info;
				GameWindow *first = g_manager.winCreateFromScript(AsciiString("test.wnd"), &info);
				results.push_back(std::make_pair(first, info));
			}
		} catch (WndStop &) { code = 1; }
		std::string o = code == 0 ? dumpAll(results) : std::string();
		size_t done = 0;
		while (done < o.size()) { ssize_t n = write(fds[1], o.data() + done, o.size() - done); if (n <= 0) break; done += n; }
		close(fds[1]);
		_exit(code);
	}
	close(fds[1]);
	std::string o;
	char buf[65536];
	ssize_t n;
	while ((n = ::read(fds[0], buf, sizeof buf)) > 0) o.append(buf, n);
	close(fds[0]);
	int status = 0;
	waitpid(pid, &status, 0);
	*out_len = (int)o.size();
	memcpy(out, o.data(), o.size() < (size_t)cap ? o.size() : (size_t)cap);
	if (WIFSIGNALED(status)) return WTERMSIG(status) == SIGALRM ? 3 : 2;
	return WEXITSTATUS(status);
}

} /* extern "C" */
