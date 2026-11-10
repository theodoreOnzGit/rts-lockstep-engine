/* Differential-test program for the window transitions and the animate window manager (GitHub #77, upstream commit
** 0a05454d), `wt_ref`.
**
** build.rs copies these upstream files VERBATIM into OUT_DIR/wt_gen (each cut from its `#include "PreRTS.h"` to the
** end, the marker asserted once) and compiles them, with the window manager's and the gadgets' files of wm_ref
** (GameWindowManager.cpp, GameWindow.cpp, WinInstanceData.cpp, GameWindowGlobal.cpp, Gadget*.cpp, as build.rs copies
** them for wm_ref), into one program with this file:
**   GameEngine/Source/GameClient/GUI/GameWindowTransitions.cpp, GameWindowTransitionsStyles.cpp,
**   AnimateWindowManager.cpp, ProcessAnimateWindow.cpp,
** and the header GameEngine/Include/GameClient/GameWindowTransitions.h (verbatim, into wt_gen/GameClient: wm_ref's
** shim_wm has a stand-in for it, which this program must not see).
**
** This file is differential/wrap_wm.cpp compiled again with WT_REF defined (see the top of that file for its
** stand-ins), and adds, all of them STAND-INS written for this test:
** - differential/shim_wt: PreRTS.h (wm_ref's, with the containers and INI.h), Common/INI.h (the declarations
**   GameWindowTransitions.cpp uses), GameClient/Display.h (the draws recorded), GameClient/Controlbar.h (the arrow
**   image), Gamelogic/GameLogic.h (empty).
** - The INI reader's members GameWindowTransitions.cpp calls (INI::load of the file's text, getNextToken,
**   initFromINI, parseBool, parseInt, parseAsciiString, parseLookupList), transcribed from INI.cpp for what a
**   WindowTransitions.ini holds (the real INI.cpp needs every block handler of the game). `load` reads the text the
**   test gave (`! ini`); its only block is `WindowTransition`.
** - cosf, sinf and sincosf: the `libm` crate's (which petir forwards to, and the translation uses), transcribed for
**   |x| < 2^28 * pi/2 (abort beyond); g++ resolves ProcessAnimateWindow.cpp's `cos(float)` to cosf, as MSVC's float
**   overload is. See tests/support/mod.rs for the same binding in the library builds.
** - TheControlBar (its arrow image, set by `! arrow`), TheDisplay's draws and DisplayString::draw (recorded).
** The access specifiers of GameWindowTransitions.h and AnimateWindowManager.h are opened (`#define private public`)
** for this file only, so that the dump can read the handler's, groups' and windows' state; the classes' layout does
** not change.
**
** Operations (`!` and a word; tests/differential_wt.rs documents each): name, img, stxt, ctr, ini, set, rev, rem,
** upd, draw, frame (upd then draw, as a game frame), fin, reset, arrow, areg, arev, aupd, arest, areset. After each operation the dump of wrap_wm.cpp is
** followed by the windows' images and static texts, the handler, its groups, their windows and transitions, and the
** animate manager. */
#define WT_REF 1
#include "PreRTS.h"
#include <math.h>
#include <string>
#include <sstream>
#include "Common/GameMemory.h"
#include "GameClient/Image.h"
#include "GameClient/Color.h"
#include "GameClient/Controlbar.h"
#define private public
#define protected public
#include "GameClient/GameWindowTransitions.h"
#include "GameClient/AnimateWindowManager.h"
#include "GameClient/ProcessAnimateWindow.h"
#undef private
#undef protected

class TestDisplayString;
class TestWindowManager;
static void wt_text_draw(TestDisplayString *s, Int x, Int y, Color c, Color d);
static void wt_op(TestWindowManager *wm, std::istringstream &in);
static void wt_dump(std::ostringstream &o);
static void wt_begin(void);

#include "wrap_wm.cpp"

/* ---- the libm crate's cosf / sinf (k_cosf, k_sinf, rem_pio2f's medium path, cosf, sinf), transcribed */
static float k_cosf(double x)
{
	const double C0 = -0.499999997251031003120, C1 = 0.0416666233237390631894, C2 = -0.00138867637746099294692, C3 = 0.0000243904487962774090654;
	double z = x * x;
	double w = z * z;
	double r = C2 + z * C3;
	return (float)(((1.0 + z * C0) + w * C1) + (w * z) * r);
}
static float k_sinf(double x)
{
	const double S1 = -0.166666666416265235595, S2 = 0.0083333293858894631756, S3 = -0.000198393348360966317347, S4 = 0.0000027183114939898219064;
	double z = x * x;
	double w = z * z;
	double r = S3 + z * S4;
	double s = z * x;
	return (float)((x + s * (S1 + z * S2)) + s * w * r);
}
static int rem_pio2f(float x, double *y)
{
	const double TOINT = 1.5 / 2.220446049250313080847e-16, INV_PIO2 = 6.36619772367581382433e-01;
	const double PIO2_1 = 1.57079631090164184570e+00, PIO2_1T = 1.58932547735281966916e-08;
	unsigned int ix;
	memcpy(&ix, &x, 4);
	ix &= 0x7fffffff;
	if (ix >= 0x4dc90fdb) abort(); /* the large path (rem_pio2_large) is not transcribed */
	double x64 = x;
	double tmp = x64 * INV_PIO2 + TOINT;
	double fn = tmp - TOINT;
	*y = x64 - fn * PIO2_1 - fn * PIO2_1T;
	return (int)fn;
}
static const double PIO2[4] = { 1 * 1.57079632679489661923, 2 * 1.57079632679489661923, 3 * 1.57079632679489661923, 4 * 1.57079632679489661923 };
extern "C" float cosf(float x)
{
	double x64 = x;
	unsigned int ix;
	memcpy(&ix, &x, 4);
	int sign = ix >> 31;
	ix &= 0x7fffffff;
	if (ix <= 0x3f490fda) { if (ix < 0x39800000) return 1.0f; return k_cosf(x64); }
	if (ix <= 0x407b53d1)
	{
		if (ix > 0x4016cbe3) return -k_cosf(sign ? x64 + PIO2[1] : x64 - PIO2[1]);
		else if (sign) return k_sinf(x64 + PIO2[0]);
		else return k_sinf(PIO2[0] - x64);
	}
	if (ix <= 0x40e231d5)
	{
		if (ix > 0x40afeddf) return k_cosf(sign ? x64 + PIO2[3] : x64 - PIO2[3]);
		else if (sign) return k_sinf(-x64 - PIO2[2]);
		else return k_sinf(x64 - PIO2[2]);
	}
	if (ix >= 0x7f800000) return x - x;
	double y;
	int n = rem_pio2f(x, &y);
	switch (n & 3)
	{
	case 0: return k_cosf(y);
	case 1: return k_sinf(-y);
	case 2: return -k_cosf(y);
	default: return k_sinf(y);
	}
}
extern "C" float sinf(float x)
{
	double x64 = x;
	unsigned int ix;
	memcpy(&ix, &x, 4);
	int sign = ix >> 31;
	ix &= 0x7fffffff;
	if (ix <= 0x3f490fda) { if (ix < 0x39800000) return x; return k_sinf(x64); }
	if (ix <= 0x407b53d1)
	{
		if (ix <= 0x4016cbe3) { if (sign) return -k_cosf(x64 + PIO2[0]); else return k_cosf(x64 - PIO2[0]); }
		return k_sinf(sign ? -(x64 + PIO2[1]) : -(x64 - PIO2[1]));
	}
	if (ix <= 0x40e231d5)
	{
		if (ix <= 0x40afeddf) { if (sign) return k_cosf(x64 + PIO2[2]); else return -k_cosf(x64 - PIO2[2]); }
		return k_sinf(sign ? x64 + PIO2[3] : x64 - PIO2[3]);
	}
	if (ix >= 0x7f800000) return x - x;
	double y;
	int n = rem_pio2f(x, &y);
	switch (n & 3)
	{
	case 0: return k_sinf(y);
	case 1: return k_cosf(y);
	case 2: return k_sinf(-y);
	default: return -k_cosf(y);
	}
}
extern "C" void sincosf(float x, float *s, float *c) { *s = sinf(x); *c = cosf(x); }

/* ---- the INI reader's members, transcribed from INI.cpp for a WindowTransitions.ini (see the top) */
static std::string g_ini_text;
static const char *const INI_SEPS = " \n\r\t=";
INI::INI() : m_text(NULL), m_pos(0), m_tok(NULL) { m_line[0] = 0; }
INI::~INI() {}
Bool INI::readLine()
{
	if (!m_text || m_text[m_pos] == 0) return FALSE;
	size_t n = 0;
	while (m_text[m_pos] && m_text[m_pos] != '\n')
	{
		if (n < sizeof(m_line) - 1) m_line[n++] = m_text[m_pos];
		++m_pos;
	}
	if (m_text[m_pos] == '\n') ++m_pos;
	m_line[n] = 0;
	/* a comment ends the line */
	char *semi = strchr(m_line, ';');
	if (semi) *semi = 0;
	return TRUE;
}
const char *INI::getNextToken(const char *seps)
{
	const char *sp = seps ? seps : INI_SEPS;
	char *t = strtok_r(NULL, sp, &m_tok);
	if (!t) { fprintf(stderr, "wt_ref: INI: a token is missing\n"); abort(); }
	return t;
}
void INI::load(AsciiString, INILoadType, Xfer *)
{
	m_text = g_ini_text.c_str();
	m_pos = 0;
	while (readLine())
	{
		char *save = NULL;
		char *t = strtok_r(m_line, INI_SEPS, &save);
		if (!t) continue;
		m_tok = save;
		if (strcmp(t, "WindowTransition") == 0) parseWindowTransitions(this);
		else { fprintf(stderr, "wt_ref: INI: unknown block %s\n", t); abort(); }
	}
}
void INI::initFromINI(void *what, const FieldParse *parseTable)
{
	while (readLine())
	{
		char *save = NULL;
		char *field = strtok_r(m_line, INI_SEPS, &save);
		if (!field) continue;
		m_tok = save;
		if (strcasecmp(field, "End") == 0) return;
		const FieldParse *f = parseTable;
		while (f->token && strcmp(f->token, field) != 0) ++f;
		if (!f->token) { fprintf(stderr, "wt_ref: INI: unknown field %s\n", field); abort(); }
		f->parse(this, what, (char *)what + f->offset, f->userData);
	}
	fprintf(stderr, "wt_ref: INI: no End\n");
	abort();
}
void INI::parseBool(INI *ini, void *, void *store, const void *)
{
	const char *t = ini->getNextToken();
	if (strcasecmp(t, "yes") == 0 || strcasecmp(t, "true") == 0) *(Bool *)store = TRUE;
	else if (strcasecmp(t, "no") == 0 || strcasecmp(t, "false") == 0) *(Bool *)store = FALSE;
	else abort();
}
void INI::parseInt(INI *ini, void *, void *store, const void *)
{
	const char *t = ini->getNextToken();
	if (sscanf(t, "%d", (Int *)store) != 1) abort();
}
void INI::parseAsciiString(INI *ini, void *, void *store, const void *)
{
	*(AsciiString *)store = ini->getNextToken();
}
void INI::parseLookupList(INI *ini, void *, void *store, const void *userData)
{
	const char *t = ini->getNextToken();
	for (const LookupListRec *l = (const LookupListRec *)userData; l->name; ++l)
		if (strcasecmp(l->name, t) == 0) { *(Int *)store = l->value; return; }
	abort();
}

/* ---- TheControlBar, TheDisplay's draws */
static ControlBar s_controlBar;
ControlBar *TheControlBar = &s_controlBar;

static void dump_clip(std::ostringstream &o, Bool on, const IRegion2D &r)
{
	if (on) o << " clip " << r.lo.x << " " << r.lo.y << " " << r.hi.x << " " << r.hi.y;
	else o << " clip -";
}
void Display::setClipRegion(IRegion2D *r) { m_clipRegion = *r; m_clip = TRUE; }
void Display::enableClipping(Bool on) { m_clip = on; }
void Display::drawImage(const Image *image, Int x0, Int y0, Int x1, Int y1, Color color, Int mode)
{
	if (image == NULL) return;
	g_log << "img " << image->getName().str() << " " << x0 << " " << y0 << " " << x1 << " " << y1 << " " << (unsigned int)color << " " << mode;
	dump_clip(g_log, m_clip, m_clipRegion);
	g_log << "\n";
}
void Display::drawOpenRect(Int x, Int y, Int w, Int h, Real width, Color color)
{
	g_log << "open " << x << " " << y << " " << w << " " << h << " " << width << " " << (unsigned int)color << "\n";
}
void Display::drawFillRect(Int x, Int y, Int w, Int h, Color color)
{
	g_log << "fill " << x << " " << y << " " << w << " " << h << " " << (unsigned int)color << "\n";
}
static void wt_text_draw(TestDisplayString *s, Int x, Int y, Color c, Color d)
{
	g_log << "text";
	dump_text(g_log, s->getText());
	g_log << " " << x << " " << y << " " << (unsigned int)c << " " << (unsigned int)d << " wrap " << s->m_wrap << " cen " << (int)s->m_centered << " font " << (s->getFont() ? s->getFont()->height : -1);
	dump_clip(g_log, s->m_hasClip, s->m_clip);
	g_log << "\n";
}

/* ---- the animate manager, made per case (its spiral reads the display's width) */
static AnimateWindowManager *g_awm = NULL;
static void wt_begin(void)
{
	TheDisplay->m_clip = FALSE;
	s_controlBar.m_genArrow = NULL;
	g_awm = new AnimateWindowManager;
}

static const char *gname(TransitionGroup *g) { return g ? g->getName().str() : "-"; }
static int gidx(TransitionGroup *g)
{
	int i = 0;
	for (GameWindowTransitionsHandler::TransitionGroupList::iterator it = TheTransitionHandler->m_transitionGroupList.begin(); it != TheTransitionHandler->m_transitionGroupList.end(); ++it, ++i)
		if (*it == g) return i;
	return -1;
}

static void dump_anim(std::ostringstream &o, const char *list, AnimateWindowList &l)
{
	for (AnimateWindowList::iterator it = l.begin(); it != l.end(); ++it)
	{
		AnimateWindow *a = *it;
		unsigned int vx, vy;
		memcpy(&vx, &a->m_vel.x, 4);
		memcpy(&vy, &a->m_vel.y, 4);
		o << "ae " << list << " win " << widx(a->m_win) << " type " << (int)a->m_animType << " cur " << a->m_curPos.x << " " << a->m_curPos.y << " start " << a->m_startPos.x << " " << a->m_startPos.y << " end " << a->m_endPos.x << " " << a->m_endPos.y << " rest " << a->m_restPos.x << " " << a->m_restPos.y << " vel " << vx << " " << vy << " t " << a->m_startTime << " " << a->m_endTime << " delay " << a->m_delay << " fin " << (int)a->m_isFinished << "\n";
	}
}

static void wt_dump(std::ostringstream &o)
{
	for (size_t i = 0; i < g_windows.size(); ++i)
	{
		if (g_freed[i]) continue;
		GameWindow *w = g_windows[i];
		const Image *e = w->winGetEnabledImage(0), *d = w->winGetDisabledImage(0);
		o << "i" << i << " " << (e ? e->getName().str() : "-") << " " << (d ? d->getName().str() : "-");
		if (BitTest(w->winGetStyle(), GWS_STATIC_TEXT) && w->winGetUserData())
		{
			TextData *t = (TextData *)w->winGetUserData();
			o << " st";
			dump_text(o, t->text->getText());
			o << " " << (int)t->centered;
		}
		o << "\n";
	}
	GameWindowTransitionsHandler *th = TheTransitionHandler;
	o << "th cur " << gidx(th->m_currentGroup) << " pend " << gidx(th->m_pendingGroup) << " draw " << gidx(th->m_drawGroup) << " sec " << gidx(th->m_secondaryDrawGroup) << " fin " << (int)th->isFinished() << "\n";
	for (GameWindowTransitionsHandler::TransitionGroupList::iterator it = th->m_transitionGroupList.begin(); it != th->m_transitionGroupList.end(); ++it)
	{
		TransitionGroup *g = *it;
		o << "tg " << g->getName().str() << " once " << (int)g->m_fireOnce << " dir " << g->m_directionMultiplier << " frame " << g->m_currentFrame << "\n";
		for (TransitionGroup::TransitionWindowList::iterator wi = g->m_transitionWindowList.begin(); wi != g->m_transitionWindowList.end(); ++wi)
		{
			TransitionWindow *t = *wi;
			o << " tw " << t->m_winName.str() << " delay " << t->m_frameDelay << " cur " << t->m_currentFrameDelay << " style " << t->m_style << " win " << widx(t->m_win);
			if (t->m_transition)
			{
				Transition *x = t->m_transition;
				o << " len " << x->m_frameLength << " fin " << (int)x->m_isFinished << " fwd " << (int)x->m_isForward << " xw " << widx(x->m_win);
			}
			o << "\n";
		}
	}
	o << "aw nu " << (int)g_awm->m_needsUpdate << " rev " << (int)g_awm->m_reverse << " empty " << (int)g_awm->isEmpty() << "\n";
	dump_anim(o, "must", g_awm->m_winMustFinishList);
	dump_anim(o, "plain", g_awm->m_winList);
}

static void wt_op(TestWindowManager *wm, std::istringstream &in)
{
	std::string word;
	in >> word;
	int a = 0, b = 0, c = 0, d = 0;
	std::string s;
	if (word == "name")
	{
		in >> a >> s;
		GameWindow *w = win(a);
		w->winGetInstanceData()->m_decoratedNameString = s.c_str();
		w->winSetWindowId(TheNameKeyGenerator->nameToKey(s.c_str()));
	}
	else if (word == "img")
	{
		in >> a >> b >> c >> s;
		const Image *im = s == "-" ? NULL : TheMappedImageCollection->findImageByName(AsciiString(s.c_str()));
		if (b == 0) win(a)->winSetEnabledImage(c, im);
		else win(a)->winSetDisabledImage(c, im);
	}
	else if (word == "stxt")
	{
		in >> a >> s;
		UnicodeString u;
		for (size_t i = 0; i < s.size(); ++i) if (s[i] != '_') u.concat((WideChar)s[i]);
		GadgetStaticTextSetText(win(a), u);
	}
	else if (word == "ctr")
	{
		in >> a >> b;
		((TextData *)win(a)->winGetUserData())->centered = b;
	}
	else if (word == "ini")
	{
		std::getline(in, s);
		for (size_t i = 0; i < s.size(); ++i) if (s[i] == '|') s[i] = '\n';
		g_ini_text = s;
		TheTransitionHandler->load();
	}
	else if (word == "set") { in >> s >> a; TheTransitionHandler->setGroup(AsciiString(s == "_" ? "" : s.c_str()), a); }
	else if (word == "rev") { in >> s; TheTransitionHandler->reverse(AsciiString(s == "_" ? "" : s.c_str())); }
	else if (word == "rem") { in >> s >> a; TheTransitionHandler->remove(AsciiString(s == "_" ? "" : s.c_str()), a); }
	else if (word == "upd") wm->update();
	else if (word == "draw") TheTransitionHandler->draw();
	else if (word == "frame") { wm->update(); TheTransitionHandler->draw(); }
	else if (word == "fin") g_log << "fin " << (int)TheTransitionHandler->isFinished() << "\n";
	else if (word == "reset") TheTransitionHandler->reset();
	else if (word == "arrow") { in >> s; s_controlBar.m_genArrow = s == "-" ? NULL : TheMappedImageCollection->findImageByName(AsciiString(s.c_str())); }
	else if (word == "areg") { in >> a >> b >> c >> d; int e; in >> e; g_awm->registerGameWindow(win(a), (AnimTypes)b, c, (UnsignedInt)d, (UnsignedInt)e); }
	else if (word == "arev") g_awm->reverseAnimateWindow();
	else if (word == "aupd") { in >> a; for (int i = 0; i < a; ++i) g_awm->update(); }
	else if (word == "arest") g_awm->resetToRestPosition();
	else if (word == "areset") g_awm->reset();
	else if (word == "afin") g_log << "afin " << (int)g_awm->isFinished() << " " << (int)g_awm->isReversed() << "\n";
	else { fprintf(stderr, "wt_ref: unknown operation %s\n", word.c_str()); abort(); }
}
