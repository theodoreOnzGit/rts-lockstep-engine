/* Differential-test wrapper for the shell menu scheme drafts (#215, upstream commit 0a05454d), src/drafts/sweep_gui_logic/shell_menu_scheme.rs
** (ShellMenuSchemeLine::default, ShellMenuSchemeManager::new_shell_menu_scheme, set_shell_menu_scheme; `parse_line_part` is not covered, see the
** test), cut verbatim by build.rs (drafts_guilogic3) into:
**   sms_color.inc  GameClient/Color.h, GAME_COLOR_UNDEFINED and `Color`;   sms_line.inc  GameClient/ShellMenuScheme.h, `class ShellMenuSchemeLine`;
**   sms_ctor.inc   GameClient/GUI/Shell/ShellMenuScheme.cpp, ShellMenuSchemeLine constructor and destructor;
**   sms_fns.inc    ShellMenuSchemeManager::newShellMenuScheme (to `init`) and setShellMenuScheme (to `draw`), as two cuts.
** Built on the REAL AsciiString (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cuts): ShellMenuScheme (m_name; the real class also holds images and lines and draws them); ShellMenuSchemeManager as
** ShellMenuScheme.h:119-146 declares it (m_schemeList, m_currentScheme, the two functions), `private` read as `public`; `NEW` is `new`;
** DEBUG_ASSERTCRASH is a no-op.
**
** Entry points (extern "C", tests/differential_draft_shellscheme.rs): ref_sms_line(out): the default line [sx, sy, ex, ey, width, color];
** ref_sms_run(in, n, out, cap): ops 0 name: newShellMenuScheme; 1 name: setShellMenuScheme; 2: dump -> [count, (len, name..).., current place or -1];
** a name is (len, chars..). */
#include <string.h>
#include <list>
#include <vector>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theSmsAllocator;

#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define NEW new
#define NULL 0

#include "sms_color.inc"
#include "sms_line.inc"
#include "sms_ctor.inc"

class ShellMenuScheme { public: AsciiString m_name; };
class ShellMenuSchemeManager
{
public:
	ShellMenuSchemeManager() : m_currentScheme(NULL) {}
	void setShellMenuScheme( AsciiString name );
	ShellMenuScheme *newShellMenuScheme(AsciiString name);
	typedef std::list< ShellMenuScheme* > ShellMenuSchemeList;
	typedef ShellMenuSchemeList::iterator ShellMenuSchemeListIt;
	ShellMenuSchemeList m_schemeList;
	ShellMenuScheme *m_currentScheme;
};
#include "sms_new.inc"
#include "sms_set.inc"

extern "C" int ref_sms_line(int *out)
{
	TheDynamicMemoryAllocator = &theSmsAllocator;
	ShellMenuSchemeLine l;
	out[0] = l.m_startPos.x; out[1] = l.m_startPos.y; out[2] = l.m_endPos.x; out[3] = l.m_endPos.y; out[4] = l.m_width; out[5] = l.m_color;
	return 6;
}

static AsciiString readName(const int *in, int &at)
{
	int n = in[at++];
	char buf[64];
	for (int i = 0; i < n; ++i) buf[i] = (char)in[at++];
	buf[n] = 0;
	return AsciiString(buf);
}

extern "C" int ref_sms_run(const int *in, int nin, int *out, int cap)
{
	TheDynamicMemoryAllocator = &theSmsAllocator;
	ShellMenuSchemeManager m;
	std::vector<int> o;
	int at = 0;
	while (at < nin)
	{
		int op = in[at++];
		if (op == 0) m.newShellMenuScheme(readName(in, at));
		else if (op == 1) m.setShellMenuScheme(readName(in, at));
		else
		{
			o.push_back((int)m.m_schemeList.size());
			int place = -1, i = 0;
			for (ShellMenuSchemeManager::ShellMenuSchemeListIt it = m.m_schemeList.begin(); it != m.m_schemeList.end(); ++it, ++i)
			{
				const char *s = (*it)->m_name.str();
				o.push_back((int)strlen(s));
				for (const char *c = s; *c; ++c) o.push_back((unsigned char)*c);
				if (*it == m.m_currentScheme) place = i;
			}
			o.push_back(place);
		}
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}
