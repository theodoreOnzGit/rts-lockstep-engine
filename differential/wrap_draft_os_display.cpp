/* Differential-test wrapper for the OS display draft (#215, group lc2, upstream commit 0a05454d), src/drafts/logic_common/win32_os_display.rs:
**   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32OSDisplay.cpp from `static void RTSFlagsToOSFlags` to the end
**   (RTSFlagsToOSFlags and OSDisplayWarningBox), cut verbatim by build.rs into os_display.inc;
**   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32GameEngine.cpp from `void Win32GameEngine::update( void )` to `}  // end update`
**   (os_update.inc);
**   GeneralsMD/Code/GameEngine/Include/Common/OSDisplay.h: os_enums.inc, the two enums `OSDisplayButtonType` and `OSDisplayOtherFlags`.
**
** Stand-ins (outside the cuts): BitTest is BaseType.h's macro; the Windows `MB_*` and `IDOK` constants are windows.h's values (MB_OK 0, MB_OKCANCEL 1,
** MB_ICONEXCLAMATION 0x30, MB_ICONINFORMATION 0x40, MB_ICONERROR = MB_ICONSTOP = 0x10, MB_APPLMODAL 0, MB_SYSTEMMODAL 0x1000, MB_TASKMODAL 0x2000, IDOK 1);
** MessageBoxW/A, SetWindowPos, IsIconic, Sleep and PeekMessage-level `serviceWindowsOS` are fakes that record what they were asked and answer from the
** test's script; `TheGameText->fetch` returns the label widened and prefixed with '#'; UnicodeString/AsciiString are minimal (translate narrows);
** TheLAN, TheGameEngine, TheGameLogic and TheAudio are recording stubs. `TheSystemIsUnicode` is a variable the test sets (the shipped value is a
** constant per build).
**
** Entry points (tests/differential_draft_os_display.rs): ref_lc2_osd_flags, ref_lc2_osd_box, ref_lc2_ge_update. */
#include <string>
#include <vector>
#include <string.h>

typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
#define TRUE true
#define FALSE false
#define NULL 0
#define BitTest( x, i ) ( ( (x) & (i) ) != 0 )

namespace {

typedef void *HWND;
void *HWND_NOTOPMOST = 0;
const unsigned MB_OK = 0, MB_OKCANCEL = 1, MB_ICONEXCLAMATION = 0x30, MB_ICONINFORMATION = 0x40, MB_ICONERROR = 0x10, MB_ICONSTOP = 0x10;
const unsigned MB_APPLMODAL = 0, MB_SYSTEMMODAL = 0x1000, MB_TASKMODAL = 0x2000;
const int IDOK = 1;
const int SWP_NOSIZE = 1, SWP_NOMOVE = 2;

#include "os_enums.inc"

class UnicodeString
{
public:
	std::u16string s;
	const char16_t *str() const { return s.c_str(); }
};

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	const char *str() const { return s.c_str(); }
	void translate(const UnicodeString &u) { s.clear(); for (size_t i = 0; i < u.s.size(); ++i) s += (char)(u.s[i] & 0xff); }
};

// the fakes
bool g_unicode = false;
#define TheSystemIsUnicode g_unicode
std::vector<int> g_log;
int g_boxResult = 1;
unsigned g_boxFlags = 0;
std::u16string g_boxMesg, g_boxPrompt;
HWND ApplicationHWnd = 0;

struct GameTextStandIn { UnicodeString fetch(AsciiString l) { UnicodeString u; u.s = u"#"; for (size_t i = 0; i < l.s.size(); ++i) u.s += (char16_t)(unsigned char)l.s[i]; return u; } };
GameTextStandIn gtStandIn;
GameTextStandIn *TheGameText = &gtStandIn;

int MessageBoxW(HWND, const char16_t *mesg, const char16_t *prompt, unsigned flags) { g_boxMesg = mesg; g_boxPrompt = prompt; g_boxFlags = flags; return g_boxResult; }
int MessageBoxA(HWND, const char *mesg, const char *prompt, unsigned flags)
{
	g_boxMesg.clear(); g_boxPrompt.clear();
	for (const char *c = mesg; *c; ++c) g_boxMesg += (char16_t)(unsigned char)*c;
	for (const char *c = prompt; *c; ++c) g_boxPrompt += (char16_t)(unsigned char)*c;
	g_boxFlags = flags;
	return g_boxResult;
}
void SetWindowPos(HWND, HWND, int, int, int, int, int) {}

#include "os_display.inc"

// Win32GameEngine::update's environment
std::vector<int> g_iconic;
size_t g_iconicIdx = 0;
int g_iconicCalls = 0;
bool g_active = false, g_quitting = false, g_internet = false, g_lanGame = false;
bool IsIconic(HWND) { ++g_iconicCalls; return g_iconicIdx < g_iconic.size() ? g_iconic[g_iconicIdx++] != 0 : false; }
void Sleep(int ms) { g_log.push_back(2); g_log.push_back(ms); }
struct LANStandIn { void setIsActive(bool a) { g_log.push_back(4); g_log.push_back(a); } void update() { g_log.push_back(5); } } lanStandIn;
LANStandIn *TheLAN = 0;
struct EngineStandIn { bool getQuitting() { return g_quitting; } } engineStandIn;
EngineStandIn *TheGameEngine = &engineStandIn;
struct LogicStandIn { bool isInInternetGame() { return g_internet; } bool isInLanGame() { return g_lanGame; } } logicStandIn;
LogicStandIn *TheGameLogic = &logicStandIn;
enum AudioAffect : int { AudioAffect_Stand = 0 };
struct AudioStandIn { float getVolume(AudioAffect) { return 0.5f; } void setVolume(float, AudioAffect aa) { g_log.push_back(6); g_log.push_back((int)aa); } } audioStandIn;
AudioStandIn *TheAudio = &audioStandIn;
class GameEngine { public: void update() { g_log.push_back(1); } };
class Win32GameEngine : public GameEngine
{
public:
	void update(void);
	void serviceWindowsOS(void) { g_log.push_back(3); }
	bool isActive(void) { return g_active; }
};

#include "os_update.inc"

}

extern "C" {

/* RTSFlagsToOSFlags(buttonFlags, otherFlags) */
unsigned ref_lc2_osd_flags(unsigned buttonFlags, unsigned otherFlags)
{
	unsigned r = 0xFFFFFFFFu;
	RTSFlagsToOSFlags(buttonFlags, otherFlags, r);
	return r;
}

/* OSDisplayWarningBox(p, m, button, other) with TheGameText present (hasText 1) or NULL, the narrow or wide box, and the box's answer.
** out: result, flags passed to the box, message length and sum, prompt length and sum (the box's text is the fetched '#label', narrowed on the A path), box called. */
int ref_lc2_osd_box(int hasText, int unicode, int boxResult, const char *p, const char *m, unsigned buttonFlags, unsigned otherFlags, unsigned *out)
{
	g_unicode = unicode != 0;
	g_boxResult = boxResult;
	g_boxFlags = 0xFFFFFFFFu;
	g_boxMesg.clear(); g_boxPrompt.clear();
	TheGameText = hasText ? &gtStandIn : 0;
	unsigned r = (unsigned)OSDisplayWarningBox(AsciiString(p), AsciiString(m), buttonFlags, otherFlags);
	out[0] = r;
	out[1] = g_boxFlags;
	unsigned sm = 0, sp = 0;
	for (size_t i = 0; i < g_boxMesg.size(); ++i) sm += (unsigned)g_boxMesg[i] * (unsigned)(i + 1);
	for (size_t i = 0; i < g_boxPrompt.size(); ++i) sp += (unsigned)g_boxPrompt[i] * (unsigned)(i + 1);
	out[2] = (unsigned)g_boxMesg.size(); out[3] = sm; out[4] = (unsigned)g_boxPrompt.size(); out[5] = sp;
	out[6] = g_boxFlags != 0xFFFFFFFFu;
	TheGameText = &gtStandIn;
	return 7;
}

/* Win32GameEngine::update with the window handle present or not, TheLAN present or not, isActive, the quit / internet / LAN-game flags and the
** script of IsIconic answers. out: the event log (1 base update, 2 ms Sleep, 3 serviceWindowsOS, 4 active setIsActive, 5 LAN update, 6 aa setVolume),
** then the IsIconic call count. */
int ref_lc2_ge_update(int hwnd, int lan, int active, int quitting, int internet, int lanGame, const int *answers, int nanswers, int *out)
{
	g_log.clear();
	ApplicationHWnd = hwnd ? (HWND)&g_log : 0;
	TheLAN = lan ? &lanStandIn : 0;
	g_active = active != 0; g_quitting = quitting != 0; g_internet = internet != 0; g_lanGame = lanGame != 0;
	g_iconic.assign(answers, answers + nanswers);
	g_iconicIdx = 0;
	g_iconicCalls = 0;
	Win32GameEngine e;
	e.update();
	int w = 0;
	for (size_t i = 0; i < g_log.size(); ++i) out[w++] = g_log[i];
	out[w++] = g_iconicCalls;
	return w;
}

}
