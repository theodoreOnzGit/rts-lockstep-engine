/* Differential-test wrapper for the draft src/drafts/sweep_wwlib/systimer.rs (#215, group wwlib3), upstream commit 0a05454d:
**   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/systimer.cpp: SysTimeClass::Is_Getting_Late, cut VERBATIM by build.rs.
** Stand-in: SysTimeClass::Get(), which is timeGetTime() (the milliseconds since boot, an OS call): it returns the value the test
** sets (as a 32-bit unsigned, Windows' unsigned long). The rest of SysTimeClass (timeGetTime itself) is not testable. */
#include <stdint.h>
static uint32_t g_now = 0;
class SysTimeClass {
public:
	unsigned long Get(void) { return (unsigned long)g_now; }
	bool Is_Getting_Late(void);
};
#include "systimer_late.inc"
extern "C" int ref_st_is_getting_late(uint32_t now)
{
	g_now = now;
	SysTimeClass t;
	return t.Is_Getting_Late() ? 1 : 0;
}
