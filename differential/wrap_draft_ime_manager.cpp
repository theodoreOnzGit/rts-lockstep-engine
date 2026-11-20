/* Differential-test wrapper for the IME manager draft (#215, group lc3, upstream commit 0a05454d), src/drafts/sweep_gui_logic/ime_manager.rs. Cut verbatim by build.rs
** from GameEngine/Source/GameClient/GUI/IMEManager.cpp (each cut ends at the banner comment of the function after it):
**   ime_comp_get.inc   getCompositionString;
**   ime_comp.inc       updateCompositionString, getResultsString, convertToUnicode;   ime_open.inc  openCandidateList, closeCandidateList;
**   ime_cand.inc       updateCandidateList, updateProperties, getIndexBase, resizeCandidateWindow;
**   ime_get.inc        getCandidate;   ime_page.inc  getCandidatePageStart;   ime_status.inc  openStatusWindow, closeStatusWindow.
**
** Stand-ins (outside the cuts): the class IMEManager (upstream declares it inside IMEManager.cpp as a class with virtual methods of the interface; here only the
** members the cuts read and the cut functions); Win32 types and constants (imm.h values; DWORD is 32-bit as on Windows, UnsignedInt is `unsigned long` so that
** upstream's `(UnsignedInt) clist + clist->dwOffset[i]` keeps a 64-bit pointer: it is a 32-bit pointer cast upstream); the IMM calls (ImmGetCompositionStringW/A,
** ImmGetCandidateListCountW/A, ImmGetCandidateListW/A, GetKeyboardLayout, ImmGetProperty), answering from the case's fake block; MultiByteToWideChar (each
** byte b becomes the unit b + 0x100; -1 length counts the terminator; a NULL buffer asks the size; 0 on failure, or when the buffer is too small); _mbsnccnt
** (bytes >= 0x81 and <= 0xFE are lead bytes of two-byte characters); GameStrlen / GameArrayEnd (Language.h, copied); UnicodeString (a vector of units with set,
** clear, assignment from a WideChar*; `TheEmptyString`); GameWindow (hidden flag, region, size, children's order; winHide hides children too and drops the
** modal top, as GameWindowManager's windowHiding does); TheWindowManager (the modal stack); TheDisplay (a width); GameFont (a height).
** The draft's own deviations, excluded from the test: a page size of 0 (upstream divides by zero), strings over MAX_COMPSTRINGLEN, a NULL text area with a
** candidate window (upstream dereferences NULL), embedded NUL bytes in an ANSI string.
**
** Entry point ref_lc3_ime_run(prog, nprog, out): see tests/differential_draft_ime_manager.rs for the program layout. Returns the output word count. */
#include <vector>
#include <string>
#include <string.h>
#include <stdint.h>
#include <stddef.h>

typedef int Int;
typedef bool Bool;
typedef char Char;
typedef unsigned long UnsignedInt;
typedef unsigned short WideChar;
typedef long LONG;
typedef uint32_t DWORD;
typedef void *HIMC;
typedef void *HKL;
#define TRUE true
#define FALSE false
#define NULL 0
#define NEW new
#define DEBUG_ASSERTCRASH(a, b)
#define GameArrayEnd(array) (array)[(sizeof(array)/sizeof((array)[0]))-1] = 0

const int GCS_COMPSTR = 8, GCS_CURSORPOS = 0x80, GCS_RESULTSTR = 0x800, CP_ACP = 0, IGP_PROPERTY = 4;
const int IME_PROP_CANDLIST_START_FROM_1 = 0x10000, IME_PROP_UNICODE = 0x80000;
const DWORD IME_CAND_UNKNOWN = 0, IME_CAND_CODE = 1;

struct CANDIDATELIST { DWORD dwSize, dwStyle, dwCount, dwSelection, dwPageStart, dwPageSize, dwOffset[1]; };

namespace {

size_t GameStrlen(const WideChar *s) { size_t n = 0; while (s[n]) ++n; return n; }

// ---- the case's fake IMM answers (one block per operation)
struct FakeImm {
	int compW_res; std::vector<WideChar> compW; int compW_cursor;
	int compA_res; std::vector<unsigned char> compA; int compA_cursor;
	int resW_res; std::vector<WideChar> resW;
	int resA_res; std::vector<unsigned char> resA;
	int mbFail, propv, cntW, cntA, style, count, sel, pstart, psize, bytesMode;
	std::vector<std::vector<unsigned char> > strs;
} g_f;

int ImmGetCompositionStringW(HIMC, int what, WideChar *buf, int bytes)
{
	if (what == GCS_COMPSTR) { if (g_f.compW_res >= 0) for (size_t i = 0; i < g_f.compW.size() && (int)(2 * i + 2) <= bytes; ++i) buf[i] = g_f.compW[i]; return g_f.compW_res; }
	if (what == GCS_CURSORPOS) return g_f.compW_cursor;
	if (g_f.resW_res >= 0) for (size_t i = 0; i < g_f.resW.size() && (int)(2 * i + 2) <= bytes; ++i) buf[i] = g_f.resW[i];
	return g_f.resW_res;
}
int ImmGetCompositionStringA(HIMC, int what, char *buf, int max)
{
	if (what == GCS_COMPSTR) { for (size_t i = 0; i < g_f.compA.size() && (int)i < max; ++i) buf[i] = (char)g_f.compA[i]; return g_f.compA_res; }
	if (what == GCS_CURSORPOS) return g_f.compA_cursor;
	for (size_t i = 0; i < g_f.resA.size() && (int)i < max; ++i) buf[i] = (char)g_f.resA[i];
	return g_f.resA_res;
}
int ImmGetCompositionString(HIMC c, int what, char *buf, int max) { return ImmGetCompositionStringA(c, what, buf, max); }
int MultiByteToWideChar(int, int, const char *s, int len, WideChar *w, int size)
{
	if (g_f.mbFail) return 0;
	int n = len == -1 ? (int)strlen(s) + 1 : len;
	if (w == NULL) return n;
	if (n > size) return 0;
	for (int i = 0; i < n; ++i) w[i] = s[i] ? (WideChar)((unsigned char)s[i] + 0x100) : 0;
	return n;
}
int _mbsnccnt(const unsigned char *b, int n)
{
	int i = 0, c = 0;
	while (i < n && b[i]) { if (b[i] >= 0x81 && b[i] <= 0xFE && b[i + 1]) i += 2; else i += 1; ++c; }
	return c;
}
HKL GetKeyboardLayout(int) { return NULL; }
int ImmGetProperty(HKL, int) { return g_f.propv; }
DWORD ImmGetCandidateListCountW(HIMC, unsigned long *) { return (DWORD)g_f.cntW; }
DWORD ImmGetCandidateListCountA(HIMC, unsigned long *) { return (DWORD)g_f.cntA; }
DWORD candList(int idx, bool wide, CANDIDATELIST *cl, int size)
{
	int n = (int)g_f.strs.size();
	std::vector<unsigned char> b(24 + 4 * n, 0);
	std::vector<unsigned char> data;
	std::vector<DWORD> offs;
	for (int k = 0; k < n; ++k) {
		offs.push_back((DWORD)(24 + 4 * n + data.size()));
		for (size_t j = 0; j < g_f.strs[k].size(); ++j) {
			unsigned v = g_f.strs[k][j] + (k == 0 && j == 0 ? idx : 0);
			if (wide) { unsigned w = v + 0x300; data.push_back(w & 255); data.push_back(w >> 8); } else data.push_back((unsigned char)v);
		}
		data.push_back(0); if (wide) data.push_back(0);
	}
	DWORD hdr[6] = { (DWORD)(24 + 4 * n + data.size()), (DWORD)g_f.style, (DWORD)g_f.count, (DWORD)g_f.sel, (DWORD)g_f.pstart, (DWORD)g_f.psize };
	memcpy(&b[0], hdr, 24);
	for (int k = 0; k < n; ++k) memcpy(&b[24 + 4 * k], &offs[k], 4);
	b.insert(b.end(), data.begin(), data.end());
	int cp = (int)b.size() < size ? (int)b.size() : size;
	memcpy(cl, &b[0], cp);
	if (g_f.bytesMode == 1) return 0;
	if (g_f.bytesMode == 2) return (DWORD)(size + 1);
	return (DWORD)b.size();
}
DWORD ImmGetCandidateListW(HIMC, int i, CANDIDATELIST *cl, int size) { return candList(i, true, cl, size); }
DWORD ImmGetCandidateListA(HIMC, int i, CANDIDATELIST *cl, int size) { return candList(i, false, cl, size); }

class UnicodeString
{
public:
	std::vector<WideChar> v;
	static UnicodeString TheEmptyString;
	void set(const WideChar *s) { v.assign(s, s + GameStrlen(s)); }
	void clear() { v.clear(); }
	UnicodeString &operator=(const WideChar *s) { set(s); return *this; }
};
UnicodeString UnicodeString::TheEmptyString;

struct GameFont { int height; };
GameFont g_font;
bool g_hasFont = false;

std::vector<int> g_order;   // top-level window ids, front first
std::vector<int> g_modal;   // modal stack, top first
struct DisplayStandIn { int w; unsigned getWidth() { return (unsigned)w; } } g_display;
DisplayStandIn *TheDisplay = &g_display;
Int IMECandidateWindowLineSpacing = 2;

class GameWindow
{
public:
	int id;
	bool hidden;
	int x0, y0, x1, y1;
	std::vector<GameWindow *> children;
	GameWindow *parent;
	bool hasFont;
	GameWindow() : id(0), hidden(false), x0(0), y0(0), x1(0), y1(0), parent(NULL), hasFont(false) {}
	void winHide(Bool hide)
	{
		hidden = hide;
		if (hide) {
			if (!g_modal.empty() && g_modal[0] == id) g_modal.erase(g_modal.begin());
			for (size_t i = 0; i < children.size(); ++i) children[i]->hideNotify();
		}
	}
	void hideNotify() { if (!g_modal.empty() && g_modal[0] == id) g_modal.erase(g_modal.begin()); for (size_t i = 0; i < children.size(); ++i) children[i]->hideNotify(); }
	void winBringToTop() { if (!parent) { for (size_t i = 0; i < g_order.size(); ++i) if (g_order[i] == id) { g_order.erase(g_order.begin() + i); break; } g_order.insert(g_order.begin(), id); } }
	void winGetSize(Int *w, Int *h) { *w = x1 - x0; *h = y1 - y0; }
	void winSetSize(Int w, Int h) { x1 = x0 + w; y1 = y0 + h; }
	void winGetPosition(Int *x, Int *y) { *x = x0; *y = y0; }
	void winSetPosition(Int x, Int y) { int w = x1 - x0, h = y1 - y0; x0 = x; y0 = y; x1 = x + w; y1 = y + h; }
	void winGetScreenPosition(Int *x, Int *y) { *x = 3; *y = 4; }
	void winGetCursorPosition(Int *x, Int *y) { *x = 5; *y = 6; }
	GameFont *winGetFont() { return hasFont ? &g_font : NULL; }
};

struct WindowManagerStandIn
{
	void winSetModal(GameWindow *w) { g_modal.insert(g_modal.begin(), w->id); }
	void winUnsetModal(GameWindow *w) { if (!g_modal.empty() && g_modal[0] == w->id) g_modal.erase(g_modal.begin()); }
} g_wm;
WindowManagerStandIn *TheWindowManager = &g_wm;

enum { MAX_COMPSTRINGLEN = 2 * 1024 };

class IMEManager
{
public:
	HIMC m_context;
	WideChar m_compositionString[MAX_COMPSTRINGLEN + 1];
	WideChar m_resultsString[MAX_COMPSTRINGLEN + 1];
	Int m_compositionCursorPos, m_compositionStringLength, m_indexBase;
	Int m_pageSize, m_pageStart, m_selectedIndex, m_candidateCount;
	UnicodeString *m_candidateString;
	Bool m_unicodeIME;
	GameWindow *m_candidateWindow, *m_candidateTextArea, *m_candidateUpArrow, *m_candidateDownArrow, *m_statusWindow, *m_window;
	IMEManager() : m_context(NULL), m_compositionCursorPos(0), m_compositionStringLength(0), m_indexBase(1), m_pageSize(0), m_pageStart(0), m_selectedIndex(0), m_candidateCount(0), m_candidateString(NULL), m_unicodeIME(FALSE), m_candidateWindow(NULL), m_candidateTextArea(NULL), m_candidateUpArrow(NULL), m_candidateDownArrow(NULL), m_statusWindow(NULL), m_window(NULL)
	{ memset(m_compositionString, 0, sizeof m_compositionString); memset(m_resultsString, 0, sizeof m_resultsString); }
	void getCompositionString(UnicodeString &string);
	void updateCompositionString(void);
	void getResultsString(void);
	void convertToUnicode(Char *mbcs, UnicodeString &unicode);
	void openCandidateList(Int candidateFlags);
	void closeCandidateList(Int candidateFlags);
	void updateCandidateList(Int candidateFlags);
	void updateProperties(void);
	Int getIndexBase(void);
	void resizeCandidateWindow(Int pageSize);
	UnicodeString *getCandidate(Int index);
	Int getCandidatePageStart();
	void openStatusWindow(void);
	void closeStatusWindow(void);
};

#include "ime_comp_get.inc"
#include "ime_comp.inc"
#include "ime_open.inc"
#include "ime_cand.inc"
#include "ime_get.inc"
#include "ime_page.inc"
#include "ime_status.inc"

}

namespace {
int rd(const int *p, int &k) { return p[k++]; }
void rdUnits(const int *p, int &k, std::vector<WideChar> &v) { int n = rd(p, k); v.clear(); for (int i = 0; i < n; ++i) v.push_back((WideChar)rd(p, k)); }
void rdBytes(const int *p, int &k, std::vector<unsigned char> &v) { int n = rd(p, k); v.clear(); for (int i = 0; i < n; ++i) v.push_back((unsigned char)rd(p, k)); }
void putStr(const std::vector<WideChar> &v, int *out, int &n) { out[n++] = (int)v.size(); for (size_t i = 0; i < v.size(); ++i) out[n++] = v[i]; }
void putWin(const GameWindow &w, int *out, int &n) { out[n++] = w.hidden; out[n++] = w.x0; out[n++] = w.y0; out[n++] = w.x1; out[n++] = w.y1; }
}

/* prog: [hasContext, display width, winHeights: cand(x0,y0,x1,y1), text area (4), up (4), down (4), status (4), hidden flags x5, textAreaHasFont, fontHeight, hasWindow, nOps,
** then per op: kind, arg, fake block]. The fake block: compW_res, compW units (count, units), compW_cursor, compA_res, compA bytes (count, bytes), compA_cursor, resW_res,
** resW units, resA_res, resA bytes, mbFail, propv, cntW, cntA, style, count, sel, pstart, psize, bytesMode, nstrs, nstrs x (len, bytes).
** Ops: 0 updateCompositionString; 1 getResultsString; 2 convertToUnicode(the compA bytes); 3 updateProperties; 4 updateCandidateList(arg); 5 openCandidateList(arg);
** 6 closeCandidateList(arg); 7 resizeCandidateWindow(arg); 8 openStatusWindow; 9 closeStatusWindow; 10 getCandidate(arg); 11 getCompositionString. */
extern "C" int ref_lc3_ime_run(const int *p, int np, int *out)
{
	int k = 0, n = 0;
	IMEManager m;
	GameWindow cand, text, up, down, stat, user;
	GameWindow *ws[5] = { &cand, &text, &up, &down, &stat };
	m.m_context = rd(p, k) ? (HIMC)&m : NULL;
	g_display.w = rd(p, k);
	for (int i = 0; i < 5; ++i) { ws[i]->id = i; ws[i]->x0 = rd(p, k); ws[i]->y0 = rd(p, k); ws[i]->x1 = rd(p, k); ws[i]->y1 = rd(p, k); }
	for (int i = 0; i < 5; ++i) ws[i]->hidden = rd(p, k) != 0;
	text.hasFont = rd(p, k) != 0;
	g_font.height = rd(p, k);
	user.id = 5;
	if (rd(p, k)) m.m_window = &user;
	text.parent = up.parent = down.parent = &cand;
	cand.children.push_back(&text); cand.children.push_back(&up); cand.children.push_back(&down);
	m.m_candidateWindow = &cand; m.m_candidateTextArea = &text; m.m_candidateUpArrow = &up; m.m_candidateDownArrow = &down; m.m_statusWindow = &stat;
	g_order.clear(); g_order.push_back(0); g_order.push_back(4);
	g_modal.clear();
	int nOps = rd(p, k);
	for (int o = 0; o < nOps; ++o) {
		int kind = rd(p, k), arg = rd(p, k);
		g_f.compW_res = rd(p, k); rdUnits(p, k, g_f.compW); g_f.compW_cursor = rd(p, k);
		g_f.compA_res = rd(p, k); rdBytes(p, k, g_f.compA); g_f.compA_cursor = rd(p, k);
		g_f.resW_res = rd(p, k); rdUnits(p, k, g_f.resW);
		g_f.resA_res = rd(p, k); rdBytes(p, k, g_f.resA);
		g_f.mbFail = rd(p, k); g_f.propv = rd(p, k); g_f.cntW = rd(p, k); g_f.cntA = rd(p, k);
		g_f.style = rd(p, k); g_f.count = rd(p, k); g_f.sel = rd(p, k); g_f.pstart = rd(p, k); g_f.psize = rd(p, k); g_f.bytesMode = rd(p, k);
		int ns = rd(p, k); g_f.strs.clear();
		for (int i = 0; i < ns; ++i) { std::vector<unsigned char> s; rdBytes(p, k, s); g_f.strs.push_back(s); }
		switch (kind) {
		case 0: m.updateCompositionString(); break;
		case 1: m.getResultsString(); break;
		case 2: { std::string s(g_f.compA.begin(), g_f.compA.end()); UnicodeString u; m.convertToUnicode((Char *)s.c_str(), u); putStr(u.v, out, n); break; }
		case 3: m.updateProperties(); break;
		case 4: m.updateCandidateList(arg); break;
		case 5: m.openCandidateList(arg); break;
		case 6: m.closeCandidateList(arg); break;
		case 7: m.resizeCandidateWindow(arg); break;
		case 8: m.openStatusWindow(); break;
		case 9: m.closeStatusWindow(); break;
		case 10: putStr(m.getCandidate(arg)->v, out, n); break;
		case 11: { UnicodeString u; m.getCompositionString(u); putStr(u.v, out, n); break; }
		}
		out[n++] = m.m_compositionCursorPos; out[n++] = m.m_compositionStringLength;
		std::vector<WideChar> cs(m.m_compositionString, m.m_compositionString + m.m_compositionStringLength);
		putStr(cs, out, n);
		std::vector<WideChar> rs(m.m_resultsString, m.m_resultsString + GameStrlen(m.m_resultsString));
		putStr(rs, out, n);
		out[n++] = m.m_pageSize; out[n++] = m.m_pageStart; out[n++] = m.m_selectedIndex; out[n++] = m.m_candidateCount;
		out[n++] = m.m_candidateString != NULL;
		for (int i = 0; i < (m.m_candidateString ? m.m_candidateCount : 0); ++i) putStr(m.m_candidateString[i].v, out, n);
		out[n++] = m.getIndexBase(); out[n++] = m.m_unicodeIME; out[n++] = m.getCandidatePageStart();
		for (int i = 0; i < 5; ++i) putWin(*ws[i], out, n);
		out[n++] = (int)g_modal.size();
		for (size_t i = 0; i < g_modal.size(); ++i) out[n++] = g_modal[i];
		out[n++] = (int)g_order.size();
		for (size_t i = 0; i < g_order.size(); ++i) out[n++] = g_order[i];
	}
	return n;
}
