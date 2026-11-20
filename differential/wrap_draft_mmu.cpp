/* Differential-test wrapper for the main-menu "go online" gate draft (#215, group net4, upstream commit 0a05454d), src/drafts/network/main_menu_utils.rs,
** cut verbatim by build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   mmu_qd.inc  Include/GameNetwork/DownloadManager.h, class QueuedDownload;
**   mmu_a.inc   Source/GameNetwork/GameSpy/MainMenuUtils.cpp from the file's statics to before asyncGethostbynameThreadFunc: StartDownloadingPatches, the two
**               message-box callbacks, hasWriteAccess, startOnline, queuePatch, motdCallback, configCallback, configHeadCallback, gamePatchCheckCallback,
**               CancelPatchCheckCallback(AndReopenDropdown), overallStatsCallback, numPlayersOnlineCallback, CheckOverallStats, CheckNumPlayersOnline;
**   mmu_b.inc   the same file from `// GameSpy's HTTP SDK has had at least 1 crash bug` to the end: isHttpOk, HTTPThinkWrapper, StopAsyncDNSCheck,
**               StartPatchCheck, reallyStartPatchCheck.
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs).
** Stand-ins (outside the cuts; each logs what it is asked, as tests/differential_draft_mmu.rs's host does, and answers from the case's configuration):
**   ghttp (ghttpGet, ghttpHead, ghttpThink, ghttpSetProxy, ghttpGetHeaders; a request is kept pending and the program delivers it later by calling the
**   callback, so the order of the answers is the program's), the window manager and a layout, the four message boxes (they remember their callbacks, the
**   program presses them), TheGameText (the text is the label widened), the script engine, the shell, SetUpGameSpy, TheDownloadManager (queue and start),
**   the registry (GetUnsignedIntFromRegistry, SetUnsignedIntInRegistry, GetStringFromRegistry, FormatURLFromRegistry), remove/_open/_close (hasWriteAccess's
**   test file), fopen/fread/fwrite/fseek/ftell/fclose on `Config.txt` (one in-memory file), HandleCanceledDownload, HandleOverallStats,
**   HandleNumPlayersOnline, TheGlobalData->getPath_UserData, and asyncGethostbyname. NOT cut: asyncGethostbynameThreadFunc and asyncGethostbyname (a
**   winsock thread; the stand-in is a miniature of it: the first call "starts the thread" (handle set), the answer comes after the case's number of polls,
**   and a finished lookup clears s_asyncDNSLookupInProgress and the handle as the real one does); TerminateThread (logged).
**   `(Int)param` of a pointer is the -fpermissive cast; DEBUG_* are nothing; NEW is new.
** Entry point (extern "C", tests/differential_draft_mmu.cpp's test): ref_mmu_run(in, n, out, cap): a program, see run(). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <vector>
#include <list>
#include <string>

/* the allocator and the string critical section are defined once, in wrap_draft_ping.cpp (the wrappers share one library) */
extern DynamicMemoryAllocator *TheDynamicMemoryAllocator;
extern CriticalSection *TheUnicodeStringCriticalSection;

#define NULL 0
#undef NEW
#define NEW new
#define DEBUG_LOG(x) ((void)0)
#define DEBUG_CRASH(x) ((void)0)
#define DEBUG_ASSERTCRASH(c, x) ((void)0)

typedef std::vector<unsigned> Words;
static Words theLog;
static void logStr(const char *s) { size_t n = strlen(s); theLog.push_back((unsigned)n); for (size_t i = 0; i < n; ++i) theLog.push_back((unsigned char)s[i]); }
static void logBytes(const char *s, size_t n) { theLog.push_back((unsigned)n); for (size_t i = 0; i < n; ++i) theLog.push_back((unsigned char)s[i]); }
static void logUnits(const WideChar *s) { unsigned n = 0; while (s[n]) ++n; theLog.push_back(n); for (unsigned i = 0; i < n; ++i) theLog.push_back((unsigned)s[i]); }

#include "mmu_qd.inc"

/* ---- GameSpy's http SDK ---- */
typedef int GHTTPBool;
typedef int GHTTPRequest;
typedef int GHTTPResult;
#define GHTTPTrue 1
#define GHTTPFalse 0
enum { GHTTPSuccess = 0, GHTTPFailed = 1 };
typedef GHTTPBool (*GHTTPCallback)(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
static GHTTPBool motdCallback(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
static GHTTPBool configCallback(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
static GHTTPBool configHeadCallback(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
static GHTTPBool gamePatchCheckCallback(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
static GHTTPBool overallStatsCallback(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
static GHTTPBool numPlayersOnlineCallback(GHTTPRequest request, GHTTPResult result, char *buffer, int bufferLen, void *param);
/* the kind of a callback, in the draft's HttpRequestKind order: PatchCheck 0, ConfigHead 1, Config 2, Motd 3, OverallStats 4, NumPlayersOnline 5 */
static int kindOf(GHTTPCallback cb)
{
	if (cb == gamePatchCheckCallback) return 0;
	if (cb == configHeadCallback) return 1;
	if (cb == configCallback) return 2;
	if (cb == motdCallback) return 3;
	if (cb == overallStatsCallback) return 4;
	if (cb == numPlayersOnlineCallback) return 5;
	return 99;
}
struct Pending { GHTTPCallback cb; int kind; void *param; };
static std::vector<Pending> thePending;
static std::string g_headers;
static int g_thinkThrows = 0;
static GHTTPRequest ghttpGet(const char *url, GHTTPBool, GHTTPCallback cb, void *param)
{
	theLog.push_back(1); theLog.push_back((unsigned)kindOf(cb)); logStr(url); theLog.push_back((unsigned)(intptr_t)param);
	Pending p = { cb, kindOf(cb), param };
	thePending.push_back(p);
	return (GHTTPRequest)thePending.size();
}
static GHTTPRequest ghttpHead(const char *url, GHTTPBool, GHTTPCallback cb, void *param)
{
	theLog.push_back(2); theLog.push_back((unsigned)kindOf(cb)); logStr(url); theLog.push_back((unsigned)(intptr_t)param);
	Pending p = { cb, kindOf(cb), param };
	thePending.push_back(p);
	return (GHTTPRequest)thePending.size();
}
static void ghttpThink() { theLog.push_back(3); if (g_thinkThrows) throw 1; }
static void ghttpSetProxy(const char *proxy) { theLog.push_back(4); logStr(proxy); }
static const char *ghttpGetHeaders(GHTTPRequest) { return g_headers.c_str(); }

/* ---- windows, message boxes, the shell ---- */
struct GameWindow { int unused; };
typedef void (*GameWinMsgBoxFunc)(void);
struct WindowLayoutStandIn
{
	void runInit() { theLog.push_back(10); }
	void hide(Bool h) { theLog.push_back(11); theLog.push_back(h ? 1u : 0u); }
	void bringForward() { theLog.push_back(12); }
};
typedef WindowLayoutStandIn WindowLayout;
static WindowLayoutStandIn theLayout;
static GameWindow theCancelWindow;
struct WindowManagerStandIn
{
	WindowLayout *winCreateLayout(AsciiString name) { theLog.push_back(13); logStr(name.str()); return &theLayout; }
	void winDestroy(GameWindow *w) { theLog.push_back(14); theLog.push_back(w ? 1u : 0u); }
};
static WindowManagerStandIn theWinMgrObj;
static WindowManagerStandIn *TheWindowManager = &theWinMgrObj;
static std::vector<GameWinMsgBoxFunc> theBoxButtons;
static void patchBeforeOnlineCallback(void);
static void noPatchBeforeOnlineCallback(void);
/* the prototypes of GameNetwork/GameSpy/MainMenuUtils.h that the cut uses before it defines them */
void CancelPatchCheckCallbackAndReopenDropdown(void);
void CancelPatchCheckCallback(void);
/* callbacks as ids in the draft's MenuButton order: PatchBeforeOnline 0, NoPatchBeforeOnline 1, CancelPatchCheckAndReopenDropdown 2 */
static unsigned buttonId(GameWinMsgBoxFunc f)
{
	if (f == patchBeforeOnlineCallback) return 0;
	if (f == noPatchBeforeOnlineCallback) return 1;
	if (f == CancelPatchCheckCallbackAndReopenDropdown) return 2;
	return 99;
}
static void box(unsigned kind, UnicodeString title, UnicodeString text, GameWinMsgBoxFunc a, GameWinMsgBoxFunc b)
{
	theLog.push_back(20); theLog.push_back(kind); logUnits(title.str()); logUnits(text.str());
	theBoxButtons.clear();
	theBoxButtons.push_back(a);
	theLog.push_back(buttonId(a));
	if (b) { theBoxButtons.push_back(b); theLog.push_back(buttonId(b)); }
}
static GameWindow *MessageBoxOk(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc ok) { box(0, t, x, ok, 0); return &theCancelWindow; }
static GameWindow *MessageBoxOkCancel(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc ok, GameWinMsgBoxFunc cancel) { box(1, t, x, ok, cancel); return &theCancelWindow; }
static GameWindow *MessageBoxYesNo(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc yes, GameWinMsgBoxFunc no) { box(2, t, x, yes, no); return &theCancelWindow; }
static GameWindow *MessageBoxCancel(UnicodeString t, UnicodeString x, GameWinMsgBoxFunc cancel) { box(3, t, x, cancel, 0); return &theCancelWindow; }
struct GameTextStandIn
{
	UnicodeString fetch(const char *label)
	{
		WideChar w[80];
		int n = 0;
		for (const char *c = label; *c && n < 78; ++c) w[n++] = (WideChar)(unsigned char)*c;
		w[n] = 0;
		UnicodeString u;
		u.set(w);
		return u;
	}
};
static GameTextStandIn theTextObj;
static GameTextStandIn *TheGameText = &theTextObj;
enum { SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED = 0 };
static const char *TheShellHookNames[] = { "MainMenuOnlineSelected" };
struct ScriptEngineStandIn { void signalUIInteract(const AsciiString &hook) { theLog.push_back(30); logStr(hook.str()); } };
static ScriptEngineStandIn theScriptObj;
static ScriptEngineStandIn *TheScriptEngine = &theScriptObj;
struct ShellStandIn { void push(AsciiString name) { theLog.push_back(31); logStr(name.str()); } };
static ShellStandIn theShellObj;
static ShellStandIn *TheShell = &theShellObj;
static void SetUpGameSpy(const char *motd, const char *config)
{
	theLog.push_back(32);
	if (!motd) motd = "";
	if (!config) config = "";
	logStr(motd); logStr(config);
}
static void HandleCanceledDownload(Bool resetDropDown = TRUE) { theLog.push_back(33); theLog.push_back(resetDropDown ? 1u : 0u); }
static void HandleOverallStats(const char *buf, unsigned len) { theLog.push_back(34); logBytes(buf, len); }
static void HandleNumPlayersOnline(Int n) { theLog.push_back(35); theLog.push_back((unsigned)n); }

/* ---- the download manager ---- */
struct DownloadManagerStandIn
{
	void queueFileForDownload(AsciiString server, AsciiString user, AsciiString pass, AsciiString file, AsciiString local, AsciiString reg, Bool tryResume)
	{
		theLog.push_back(40); logStr(server.str()); logStr(user.str()); logStr(pass.str()); logStr(file.str()); logStr(local.str()); logStr(reg.str()); theLog.push_back(tryResume ? 1u : 0u);
	}
	long downloadNextQueuedFile() { theLog.push_back(41); return 0; }
};
static DownloadManagerStandIn theDlObj;
static DownloadManagerStandIn *TheDownloadManager = &theDlObj;

/* ---- the registry, files, the user-data path ---- */
static int g_canCreate = 1, g_versionGet = 1, g_versionSet = 1, g_haveProxy = 0;
static unsigned g_version = 0;
static std::string g_proxy, g_gameURL, g_mapURL, g_configURL, g_motdURL;
static bool GetUnsignedIntFromRegistry(const char *, const char *, unsigned int &val) { theLog.push_back(50); if (!g_versionGet) return false; val = g_version; return true; }
static bool SetUnsignedIntInRegistry(const char *, const char *, unsigned int val) { theLog.push_back(51); theLog.push_back(val); return g_versionSet != 0; }
static bool GetStringFromRegistry(const char *, const char *, std::string &val) { theLog.push_back(52); if (!g_haveProxy) return false; val = g_proxy; return true; }
static void FormatURLFromRegistry(std::string &game, std::string &map, std::string &config, std::string &motd) { theLog.push_back(53); game = g_gameURL; map = g_mapURL; config = g_configURL; motd = g_motdURL; }
static int remove(const char *) { return 0; }
#define _O_CREAT 1
#define _O_RDWR 2
#define _S_IREAD 4
#define _S_IWRITE 8
static int _open(const char *, int, int) { theLog.push_back(54); return g_canCreate ? 3 : -1; }
static void _close(int) {}
struct GlobalDataStandIn { AsciiString getPath_UserData() { AsciiString a; a.set("UD/"); return a; } };
static GlobalDataStandIn theGlobalObj;
static GlobalDataStandIn *TheGlobalData = &theGlobalObj;
/* Config.txt: one in-memory file */
struct Net4File { int mode; size_t pos; };
static bool g_fileExists = false;
static std::string g_file;
static Net4File theFileHandle;
static Net4File *net4_fopen(const char *, const char *mode)
{
	if (mode[0] == 'w') { g_fileExists = true; g_file.clear(); theFileHandle.mode = 1; theFileHandle.pos = 0; return &theFileHandle; }
	if (!g_fileExists) return 0;
	theFileHandle.mode = 0; theFileHandle.pos = 0;
	return &theFileHandle;
}
static size_t net4_fwrite(const void *p, size_t sz, size_t n, Net4File *) { g_file.append((const char *)p, sz * n); return n; }
static size_t net4_fread(void *p, size_t sz, size_t n, Net4File *f) { size_t want = sz * n; if (want > g_file.size() - f->pos) want = g_file.size() - f->pos; memcpy(p, g_file.data() + f->pos, want); f->pos += want; return want / (sz ? sz : 1); }
static int net4_fseek(Net4File *f, long off, int whence) { if (whence == SEEK_END) f->pos = g_file.size() + off; else f->pos = off; return 0; }
static long net4_ftell(Net4File *f) { return (long)f->pos; }
static int net4_fclose(Net4File *) { return 0; }
#define FILE Net4File
#define fopen net4_fopen
#define fwrite net4_fwrite
#define fread net4_fread
#define fseek net4_fseek
#define ftell net4_ftell
#define fclose net4_fclose

/* ---- the winsock thread of asyncGethostbyname ---- */
typedef void *HANDLE;
static int TerminateThread(HANDLE, int) { theLog.push_back(60); return 1; }

#include "mmu_a.inc"

static int g_dnsPolls = 1, g_dnsSucceeds = 1, g_dnsStat = 0, g_dnsPollCount = 0;
int asyncGethostbyname(char *szName)
{
	theLog.push_back(61); logStr(szName);
	if (g_dnsStat == 0)
	{
		s_asyncDNSThreadDone = FALSE;
		s_asyncDNSThreadHandle = (HANDLE)&theLayout;
		g_dnsStat = 1;
		g_dnsPollCount = 0;
	}
	if (g_dnsStat == 1)
	{
		if (g_dnsPollCount++ >= g_dnsPolls)
		{
			g_dnsStat = 0;
			s_asyncDNSLookupInProgress = FALSE;
			s_asyncDNSThreadHandle = NULL;
			return g_dnsSucceeds ? LOOKUP_SUCCEEDED : LOOKUP_FAILED;
		}
	}
	return LOOKUP_INPROGRESS;
}

#include "mmu_b.inc"

static std::string getStr(const unsigned *&p)
{
	unsigned n = *p++;
	std::string s;
	for (unsigned i = 0; i < n; ++i) s.push_back((char)*p++);
	return s;
}
static void dumpState()
{
	theLog.push_back(0xD0D0D0D0u);
	theLog.push_back(checkingForPatchBeforeGameSpy ? 1u : 0u);
	theLog.push_back((unsigned)checksLeftBeforeOnline);
	theLog.push_back((unsigned)timeThroughOnline);
	theLog.push_back(mustDownloadPatch ? 1u : 0u);
	theLog.push_back(cantConnectBeforeOnline ? 1u : 0u);
	theLog.push_back((unsigned)queuedDownloads.size());
	for (std::list<QueuedDownload>::iterator it = queuedDownloads.begin(); it != queuedDownloads.end(); ++it)
	{
		logStr(it->server.str()); logStr(it->userName.str()); logStr(it->password.str()); logStr(it->file.str()); logStr(it->localFile.str()); logStr(it->regKey.str());
		theLog.push_back(it->tryResume ? 1u : 0u);
	}
	theLog.push_back(MOTDBuffer ? 1u : 0u);
	if (MOTDBuffer) logStr(MOTDBuffer);
	theLog.push_back(configBuffer ? 1u : 0u);
	if (configBuffer) logStr(configBuffer);
	theLog.push_back(onlineCancelWindow ? 1u : 0u);
	theLog.push_back(s_asyncDNSLookupInProgress ? 1u : 0u);
	theLog.push_back(isHttpOk ? 1u : 0u);
}

/* A program: [configuration, number of operations] then per operation its opcode and arguments.
** Configuration: canCreate, versionGet, versionSet, version, haveProxy, proxy, thinkThrows, hasDownloadManager, dnsPolls, dnsSucceeds, the four URLs, the file (exists, bytes).
**  0 StartPatchCheck   1 deliver(pending index, result, buffer, headers)   2 HTTPThinkWrapper   3 StopAsyncDNSCheck   4 press(button index)   5 CheckOverallStats
**  6 CheckNumPlayersOnline   7 CancelPatchCheckCallback   8 StartDownloadingPatches   9 reconfigure (the configuration again)   10 queuePatch(mandatory, url)
** After every operation: the log written since the last dump, then the state. */
extern "C" int ref_mmu_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	theLog.clear();
	thePending.clear();
	theBoxButtons.clear();
	queuedDownloads.clear();
	checkingForPatchBeforeGameSpy = FALSE; checksLeftBeforeOnline = 0; timeThroughOnline = 0; mustDownloadPatch = FALSE; cantConnectBeforeOnline = FALSE;
	MOTDBuffer = NULL; configBuffer = NULL; onlineCancelWindow = NULL;
	s_asyncDNSThreadDone = TRUE; s_asyncDNSThreadSucceeded = FALSE; s_asyncDNSLookupInProgress = FALSE; s_asyncDNSThreadHandle = NULL; isHttpOk = TRUE;
	g_dnsStat = 0; g_fileExists = false; g_file.clear();
	Words w;
	unsigned nops = 0;
	bool first = true;
	unsigned ops_done = 0;
	for (;;)
	{
		unsigned op;
		if (first)
		{
			op = 9; /* the first operation is always the configuration, read from the stream as op 9 does */
			first = false;
			nops = *p++;
		}
		else
		{
			if (ops_done >= nops) break;
			op = *p++;
			++ops_done;
		}
		theLog.push_back(0xA5A5A5A5u);
		theLog.push_back(op);
		switch (op)
		{
		case 0: StartPatchCheck(); break;
		case 1:
		{
			unsigned idx = *p++;
			unsigned result = *p++;
			std::string buf = getStr(p);
			g_headers = getStr(p);
			if (thePending.empty()) break;
			idx %= (unsigned)thePending.size();
			Pending pd = thePending[idx];
			thePending.erase(thePending.begin() + idx);
			std::vector<char> b(buf.begin(), buf.end());
			b.push_back(0);
			theLog.push_back(70); theLog.push_back((unsigned)pd.kind);
			pd.cb(idx + 1, (GHTTPResult)result, &b[0], (int)buf.size(), pd.param);
			break;
		}
		case 2: HTTPThinkWrapper(); break;
		case 3: StopAsyncDNSCheck(); break;
		case 4:
		{
			unsigned k = *p++;
			if (theBoxButtons.empty()) break;
			GameWinMsgBoxFunc f = theBoxButtons[k % theBoxButtons.size()];
			theLog.push_back(71); theLog.push_back(buttonId(f));
			f();
			break;
		}
		case 5: CheckOverallStats(); break;
		case 6: CheckNumPlayersOnline(); break;
		case 7: CancelPatchCheckCallback(); break;
		case 8: StartDownloadingPatches(); break;
		case 9:
		{
			g_canCreate = (int)*p++; g_versionGet = (int)*p++; g_versionSet = (int)*p++; g_version = *p++; g_haveProxy = (int)*p++; g_proxy = getStr(p);
			g_thinkThrows = (int)*p++;
			TheDownloadManager = *p++ ? &theDlObj : 0;
			g_dnsPolls = (int)*p++; g_dnsSucceeds = (int)*p++;
			g_gameURL = getStr(p); g_mapURL = getStr(p); g_configURL = getStr(p); g_motdURL = getStr(p);
			unsigned ex = *p++;
			std::string f = getStr(p);
			if (ex) { g_fileExists = true; g_file = f; } else { g_fileExists = false; g_file.clear(); }
			break;
		}
		case 10: { unsigned mandatory = *p++; std::string u = getStr(p); AsciiString a; a.set(u.c_str()); queuePatch(mandatory != 0, a); break; }
		}
		dumpState();
		theLog.push_back(0xE1E1E1E1u);
		theLog.push_back(g_fileExists ? 1u : 0u);
		logBytes(g_file.data(), g_file.size());
	}
	/* the stand-in TheDownloadManager must not outlive the program as a dangling configuration */
	TheDownloadManager = &theDlObj;
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}
