/* Differential-test wrapper for the WOL login menu's pure logic (#215, upstream commit 0a05454d):
** src/drafts/ui/wol_login_menu.rs (obfuscate, GameSpyLoginPreferences, isNickOkay, isAgeOkay). Cut verbatim by build.rs:
**   login_prefs.inc   GameClient/GUI/GUICallbacks/Menus/WOLLoginMenu.cpp, from `class GameSpyLoginPreferences` to
**                     `static const char *PREF_FILENAME` (the class, obfuscate, load, write, and the getters), with ONE mechanical
**                     edit (asserted once): `*c = *c++ ^ *c2++;` in obfuscate is unsequenced in C++ (undefined), and g++ (all
**                     standards) evaluates the right side first, so the left side `*c` is the NEXT byte and the loop runs past the
**                     string. The edit takes the left side's address first, `{ char *lhs_ = c; *lhs_ = *c++ ^ *c2++; }`, the
**                     reading the draft documents (and the one the shipped compiler's order gives, not re-checked here);
**   login_checks.inc  WOLLoginMenu.cpp, isNickOkay and isAgeOkay (to the system callback's comment);
**   login_up.inc      Common/UserPreferences.h (PreferenceMap and class UserPreferences) and UserPreferences.cpp (intAsStr to the
**                     end of setAsciiString), as differential/wrap_user_prefs.cpp cuts them (#93);
**   qp_helpers.inc, qp_to.inc, qp_from.inc   Common/QuotedPrintable.cpp: MAGIC_CHAR, intToHexDigit and hexDigitToInt;
**                     AsciiStringToQuotedPrintable; QuotedPrintableToAsciiString (the two ASCII functions the preferences use).
** Built on the REAL AsciiString.h and AsciiString.cpp (shim_ascii; classes renamed by build.rs), -fshort-wchar (WideChar 16 bit, as
** MSVC's), -fpermissive (the L"..." literal to a WideChar pointer) and -fno-operator-names (the upstream names a variable `xor`).
** Everything in namespace uilo.
**
** Stand-ins (outside the cuts): TheGlobalData->getPath_UserData() (the empty string, so the preference file name is the name given);
**   fopen: mode "r" is fmemopen of the case's file text (NULL when the case says there is no file), mode "w" is open_memstream, so no
**   file is touched (fgets, fprintf, fclose are the C library's); AsciiStringList and AsciiStringListIterator (STLTypedefs.h:83-84);
**   wcschr over 16-bit WideChar (the CRT's, for MSVC's 16-bit wchar_t); GetDateFormat (Windows): "yyyy", "MM" and "dd" give the
**   date the case sets, as 4, 2 and 2 digits; the shim UnicodeString (getLength, getCharAt) is the string type of isNickOkay.
** Not covered: the preference file's parsing beyond what UserPreferences::load does with generated lines (that is #93's);
**   `write()` in C++ inserts empty "lastEmail", "lastName", "useProfiles" entries into the map through operator[] (the draft's
**   reads leave it); the written text is compared, not that side effect.
**
** Entry points (extern "C", tests/differential_draft_login.rs):
**   ref_login_obfuscate(in, out, cap) -> length;  ref_login_qp(which 0 to, 1 from, in, out, cap) -> length;
**   ref_login_nick(units, n) -> 0/1;
**   ref_login_age(month, day, year, sysYear, sysMonth, sysDay, out_month, out_day, cap) -> 0/1 (strings NUL-terminated);
**   ref_login_prefs(in, len, out, cap) -> length (-1 too small): `in` is [has file u8][file name str][file text str][nops u32][ops],
**   str = u32 length and bytes; ops: 0 getPasswordForEmail(email), 1 getDateForEmail(email, month, date, year), 2 getNicksForEmail(email),
**   3 addLogin(email, nick, password, date), 4 forgetLogin(email), 5 getEmails(), 6 write(). `out` is load's answer (u8), then per op:
**   0 str; 1 str full, str month, str date, str year; 2 u32 count and strs; 5 u32 count and strs; 6 u8 answer, str text; 3 and 4 nothing. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <map>
#include <list>
#include <vector>
#include <string>
#include <algorithm>

/* (the allocator pointer is renamed in this build, see build.rs) */
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theUilogicLoginAllocator;

namespace uilo {

typedef std::list< AsciiString > AsciiStringList;
typedef std::list< AsciiString >::iterator AsciiStringListIterator;

struct GlobalData { AsciiString getPath_UserData() const { return AsciiString(""); } };
static GlobalData g_gd;
static GlobalData *TheGlobalData = &g_gd;

static bool g_hasFile = false;
static std::string g_text;
static char *g_wbuf = 0;
static size_t g_wlen = 0;
static FILE *fopen(const char *path, const char *mode)
{
	(void)path;
	if (mode[0] == 'r')
	{
		if (!g_hasFile) return NULL;
		return fmemopen((void *)(g_text.empty() ? "\n" : g_text.c_str()), g_text.empty() ? 1 : g_text.size(), "r");
	}
	return open_memstream(&g_wbuf, &g_wlen);
}

static const WideChar *wcschr(const WideChar *s, WideChar c)
{
	for (;; ++s)
	{
		if (*s == c) return s;
		if (!*s) return NULL;
	}
}

static int g_sysYear = 2000, g_sysMonth = 1, g_sysDay = 1;
static const int LOCALE_SYSTEM_DEFAULT = 0;
static int GetDateFormat(int locale, unsigned flags, const void *date, const char *fmt, char *out, int cch)
{
	(void)locale; (void)flags; (void)date;
	if (!strcmp(fmt, "yyyy")) return snprintf(out, cch, "%04d", g_sysYear);
	if (!strcmp(fmt, "MM")) return snprintf(out, cch, "%02d", g_sysMonth);
	return snprintf(out, cch, "%02d", g_sysDay);
}

#include "login_up.inc"
#include "qp_helpers.inc"
#include "qp_to.inc"
#include "qp_from.inc"
#include "login_prefs.inc"
#include "login_checks.inc"

struct Reader
{
	const unsigned char *p; size_t n, i;
	unsigned u8() { return p[i++]; }
	unsigned u32() { unsigned v; memcpy(&v, p + i, 4); i += 4; return v; }
	std::string str() { unsigned l = u32(); std::string s((const char *)p + i, l); i += l; return s; }
};
struct Writer
{
	std::vector<unsigned char> b;
	void u8(unsigned v) { b.push_back((unsigned char)v); }
	void u32(unsigned v) { b.insert(b.end(), (unsigned char *)&v, (unsigned char *)&v + 4); }
	void str(const char *s) { size_t n = strlen(s); u32((unsigned)n); b.insert(b.end(), s, s + n); }
};

} // namespace uilo

static int copyOut(const char *s, char *out, int cap)
{
	int n = (int)strlen(s);
	if (n + 1 > cap) return -1;
	memcpy(out, s, n + 1);
	return n;
}

extern "C" int ref_login_obfuscate(const char *in, char *out, int cap)
{
	TheDynamicMemoryAllocator = &theUilogicLoginAllocator;
	AsciiString r = uilo::obfuscate(AsciiString(in));
	return copyOut(r.str(), out, cap);
}

extern "C" int ref_login_qp(int which, const char *in, char *out, int cap)
{
	TheDynamicMemoryAllocator = &theUilogicLoginAllocator;
	AsciiString r = which == 0 ? uilo::AsciiStringToQuotedPrintable(AsciiString(in)) : uilo::QuotedPrintableToAsciiString(AsciiString(in));
	return copyOut(r.str(), out, cap);
}

extern "C" int ref_login_nick(const unsigned short *units, int n)
{
	TheDynamicMemoryAllocator = &theUilogicLoginAllocator;
	UnicodeString s(units, n);
	return uilo::isNickOkay(s) ? 1 : 0;
}

extern "C" int ref_login_age(const char *month, const char *day, const char *year, int sy, int sm, int sd, char *omonth, char *oday, int cap)
{
	TheDynamicMemoryAllocator = &theUilogicLoginAllocator;
	uilo::g_sysYear = sy; uilo::g_sysMonth = sm; uilo::g_sysDay = sd;
	AsciiString m(month), d(day), y(year);
	int r = uilo::isAgeOkay(m, d, y) ? 1 : 0;
	copyOut(m.str(), omonth, cap);
	copyOut(d.str(), oday, cap);
	return r;
}

extern "C" int ref_login_prefs(const unsigned char *in, int len, unsigned char *out, int cap)
{
	using namespace uilo;
	TheDynamicMemoryAllocator = &theUilogicLoginAllocator;
	Reader r = { in, (size_t)len, 0 };
	Writer w;
	g_hasFile = r.u8() != 0;
	std::string fname = r.str();
	g_text = r.str();
	unsigned nops = r.u32();
	GameSpyLoginPreferences p;
	w.u8(p.load(AsciiString(fname.c_str())) ? 1 : 0);
	for (unsigned k = 0; k < nops; ++k)
	{
		unsigned kind = r.u8();
		switch (kind)
		{
			case 0: { std::string e = r.str(); AsciiString v = p.getPasswordForEmail(AsciiString(e.c_str())); w.str(v.str()); break; }
			case 1:
			{
				std::string e = r.str(), m = r.str(), d = r.str(), y = r.str();
				AsciiString am(m.c_str()), ad(d.c_str()), ay(y.c_str());
				AsciiString full = p.getDateForEmail(AsciiString(e.c_str()), am, ad, ay);
				w.str(full.str()); w.str(am.str()); w.str(ad.str()); w.str(ay.str());
				break;
			}
			case 2:
			{
				std::string e = r.str();
				AsciiStringList l = p.getNicksForEmail(AsciiString(e.c_str()));
				w.u32((unsigned)l.size());
				for (AsciiStringList::iterator it = l.begin(); it != l.end(); ++it) w.str(it->str());
				break;
			}
			case 3:
			{
				std::string e = r.str(), n = r.str(), pw = r.str(), d = r.str();
				p.addLogin(AsciiString(e.c_str()), AsciiString(n.c_str()), AsciiString(pw.c_str()), AsciiString(d.c_str()));
				break;
			}
			case 4: { std::string e = r.str(); p.forgetLogin(AsciiString(e.c_str())); break; }
			case 5:
			{
				AsciiStringList l = p.getEmails();
				w.u32((unsigned)l.size());
				for (AsciiStringList::iterator it = l.begin(); it != l.end(); ++it) w.str(it->str());
				break;
			}
			default:
			{
				g_wbuf = 0; g_wlen = 0;
				bool ok = p.write() != 0;
				w.u8(ok ? 1 : 0);
				if (g_wbuf) { std::string t(g_wbuf, g_wlen); w.str(t.c_str()); free(g_wbuf); g_wbuf = 0; }
				else w.str("");
				break;
			}
		}
	}
	if ((int)w.b.size() > cap) return -1;
	memcpy(out, w.b.data(), w.b.size());
	return (int)w.b.size();
}
