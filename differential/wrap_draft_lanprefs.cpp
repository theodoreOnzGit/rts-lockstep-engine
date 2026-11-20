/* Differential-test wrapper for the LAN preferences draft (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/lan_lobby_menu.rs (get_num_remote_ips, get_remote_ip_entry), cut verbatim by build.rs (drafts_guilogic4) into:
**   lan_fns.inc   GUICallbacks/Menus/LanLobbyMenu.cpp, LANPreferences::getNumRemoteIPs and getRemoteIPEntry (to `superweaponRestrictionKey`);
**   lan_qp_h.inc  System/QuotedPrintable.cpp, MAGIC_CHAR and the two hex digit helpers (to the unicode-to-ASCII converter);
**   lan_qp_u.inc  System/QuotedPrintable.cpp, QuotedPrintableToUnicodeString (to QuotedPrintableToAsciiString);
** and the base class REUSED from the verified differential/wrap_user_prefs.cpp's cut (OUT_DIR/user_prefs_gen/user_prefs_cut.inc: PreferenceMap and
** class UserPreferences and its functions), compiled here under names of this wrapper's own, on upstream's own UnicodeString.cpp and AsciiString.cpp
** (unchanged; -fshort-wchar; classes renamed by build.rs; shim_draft_g3/g3_prelude.h as the chat build).
** Stand-ins (outside the cuts): `class LANPreferences : public UserPreferences` with only the two members the cuts define (the real class is in
** UserPreferences.h, with the other accessors), TheGlobalData (getPath_UserData, which the base class's load and write read; never called here),
** the allocator pointers.
**
** Entry point (extern "C", tests/differential_draft_lanprefs.rs): ref_lan_prefs(n, keys, vals, op, i, out, cap): the map is the n pairs; op 0
** returns getNumRemoteIPs(); op 1 writes getRemoteIPEntry(i) (16-bit units) to out and returns its length (-1: too small). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <map>
#include <string>
#include <vector>

namespace {
DynamicMemoryAllocator theLanPrefsAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theLanPrefsAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

namespace {
struct LanTestGlobalData
{
	AsciiString m_userData;
	AsciiString getPath_UserData() const { return m_userData; }
};
LanTestGlobalData *TheGlobalData = 0;
}

#include "user_prefs_cut.inc"

class LANPreferences : public UserPreferences
{
public:
	Int getNumRemoteIPs(void);
	UnicodeString getRemoteIPEntry(Int i);
};

#include "lan_qp_h.inc"
#include "lan_qp_u.inc"
#include "lan_fns.inc"

extern "C" int ref_lan_prefs(int n, const char *const *keys, const char *const *vals, int op, int i, unsigned short *out, int cap)
{
	LANPreferences p;
	for (int k = 0; k < n; ++k)
		p[AsciiString(keys[k])] = AsciiString(vals[k]);
	if (op == 0)
		return p.getNumRemoteIPs();
	UnicodeString u = p.getRemoteIPEntry(i);
	int len = (int)u.getLength();
	if (len + 1 > cap)
		return -1;
	memcpy(out, u.str(), len * sizeof(unsigned short));
	out[len] = 0;
	return len;
}
