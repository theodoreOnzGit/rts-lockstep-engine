/* Differential-test wrapper for the Version draft (#215, upstream commit 0a05454d), src/drafts/logic_common/version.rs:
**   GeneralsMD/Code/GameEngine/Include/Common/version.h (read unchanged) and
**   GameEngine/Source/Common/version.cpp from `Version *TheVersion = NULL;` to the end of the file (the constructor,
**   setVersion, getVersionNumber, getAsciiVersion, getUnicodeVersion, getFullUnicodeVersion, getAsciiBuildTime,
**   getUnicodeBuildTime, getAsciiBuildLocation, getUnicodeBuildLocation, getAsciiBuildUser, getUnicodeBuildUser), cut
**   verbatim by build.rs into version.inc.
**
** Built twice (build.rs): once as the SHIPPED game is (no _DEBUG, no _INTERNAL: the `#else` branches) and once with `_DEBUG`
** defined after the headers are read (the classes renamed VersionDbg by the -D below, entry points prefixed ref_version_dbg_),
** the draft's `debug_build`. `_INTERNAL` is not modelled by the draft and not tested.
**
** Stand-ins (all outside the cut): UnicodeString and AsciiString are upstream's own (UnicodeString.cpp and AsciiString.cpp
** unchanged, -fshort-wchar, shim_draft_text/text_prelude.h, whose `_vsnwprintf` is the MSVC wide printf for %d %c %s(wide) %%;
** classes renamed by build.rs); GameText.h is not read: TheGameText is a stand-in whose `fetch(label)` returns the text the test
** stored for that label (UnicodeString), as GameText's string table would.
**
** Entry points (extern "C", tests/differential_draft_version.rs), the dbg build under ref_version_dbg_*:
**   ref_version_text(label, units)    stores fetch's answer for a label (NUL-terminated units)
**   ref_version_case(use_default, major, minor, build, local, user, loc, time, date, show, out)
**       a Version made by its constructor, then (unless use_default) setVersion(...) with the C strings, then
**       setShowFullVersion(show) when show >= 0; writes to out (unsigned short words): getVersionNumber (2 words), showFullVersion
**       (1), then each of getAsciiVersion, getUnicodeVersion, getFullUnicodeVersion, getAsciiBuildTime, getUnicodeBuildTime,
**       getAsciiBuildLocation, getUnicodeBuildLocation, getAsciiBuildUser, getUnicodeBuildUser as a length and that many words
**       (ASCII results widened to words). Returns the number of words written. */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <string>
#include <map>

#ifdef LOGICX_VERSION_DEBUG
#define Version VersionDbg
#define TheVersion TheVersionDbg
#define REF(n) ref_version_dbg_##n
#else
#define REF(n) ref_version_##n
#endif
#include "Common/version.h"

#ifdef LOGICX_VERSION_DEBUG
#define _DEBUG 1
#endif

class GameTextStandIn
{
public:
	std::map<std::string, std::basic_string<unsigned short> > table;
	UnicodeString fetch(const char *label)
	{
		UnicodeString s;
		std::basic_string<unsigned short> &t = table[label];
		for (size_t i = 0; i < t.size(); ++i) s.concat((WideChar)t[i]);
		return s;
	}
};
static GameTextStandIn theTextStore;
static GameTextStandIn *TheGameText = &theTextStore;

#define NULL 0
#include "version.inc"

namespace {
int put_a(const AsciiString &s, unsigned short *out)
{
	int n = s.getLength();
	out[0] = (unsigned short)n;
	for (int i = 0; i < n; ++i) out[1 + i] = (unsigned char)s.getCharAt(i);
	return n + 1;
}
int put_u(const UnicodeString &s, unsigned short *out)
{
	int n = s.getLength();
	out[0] = (unsigned short)n;
	for (int i = 0; i < n; ++i) out[1 + i] = (unsigned short)s.getCharAt(i);
	return n + 1;
}
}

extern "C" {

void REF(text)(const char *label, const unsigned short *units)
{
	std::basic_string<unsigned short> t;
	for (; *units; ++units) t.push_back(*units);
	theTextStore.table[label] = t;
}

int REF(case)(int use_default, int major, int minor, int build, int local, const char *user, const char *loc, const char *time, const char *date, int show, unsigned short *out)
{
	Version v;
	if (!use_default) v.setVersion(major, minor, build, local, AsciiString(user), AsciiString(loc), AsciiString(time), AsciiString(date));
	if (show >= 0) v.setShowFullVersion(show != 0);
	unsigned int num = v.getVersionNumber();
	int n = 0;
	out[n++] = (unsigned short)(num & 0xFFFF);
	out[n++] = (unsigned short)(num >> 16);
	out[n++] = v.showFullVersion() ? 1 : 0;
	n += put_a(v.getAsciiVersion(), out + n);
	n += put_u(v.getUnicodeVersion(), out + n);
	n += put_u(v.getFullUnicodeVersion(), out + n);
	n += put_a(v.getAsciiBuildTime(), out + n);
	n += put_u(v.getUnicodeBuildTime(), out + n);
	n += put_a(v.getAsciiBuildLocation(), out + n);
	n += put_u(v.getUnicodeBuildLocation(), out + n);
	n += put_a(v.getAsciiBuildUser(), out + n);
	n += put_u(v.getUnicodeBuildUser(), out + n);
	return n;
}

}
