/* Differential-test wrapper for the download manager draft (#215, group net4, upstream commit 0a05454d), src/drafts/network/download_manager.rs, cut verbatim by
** build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   dl_header.inc  Include/GameNetwork/DownloadManager.h from `class CDownload;` to the include guard's #endif (QueuedDownload, DownloadManager);
**   dl_cpp.inc     Source/GameNetwork/DownloadManager.cpp from `DownloadManager *TheDownloadManager;` to the end of the file.
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs).
** Stand-ins (outside the cuts): `IDownload` and `CDownload` (WWDownload/download.h is not in the tree: DownloadFile, PumpMessages and GetLastLocalFile log
**   their arguments and answer from variables the case sets), the DOWNLOADEVENT_* / DOWNLOADSTATUS_* values (shim_draft_net4/downloadDefs.h: chosen here, see
**   it), HRESULT/S_OK, `TheGameText->fetch` (the text is "T:" and the label, widened), winsock (shim_draft_net4/winsock_standin.h), NEW is new, DEBUG_LOG and
**   DEBUG_CRASH are nothing. DEBUG_CRASH is compiled out in the release build, so `downloadNextQueuedFile` with an empty queue returns S_OK.
** Entry point (extern "C", tests/differential_draft_dl.rs): ref_dl_run(in, n, out, cap): a program of calls, see run(). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <vector>
#include <list>
#include <string>

/* the allocator and the string critical section are defined once, in wrap_draft_ping.cpp (the three wrappers share one library) */
extern DynamicMemoryAllocator *TheDynamicMemoryAllocator;
extern CriticalSection *TheUnicodeStringCriticalSection;

#define NULL 0
#define NEW new
#define DEBUG_LOG(x) ((void)0)
#define DEBUG_CRASH(x) ((void)0)
#include "winsock_standin.h"
#include "downloadDefs.h"

typedef int HRESULT;
#define S_OK 0
typedef std::vector<unsigned> Words;
static Words theLog;
static HRESULT g_pump = 0, g_download = 0;
static char g_lastLocal[256] = "";
static void logStr(const char *s) { size_t n = strlen(s); theLog.push_back((unsigned)n); for (size_t i = 0; i < n; ++i) theLog.push_back((unsigned char)s[i]); }

class IDownload
{
public:
	virtual ~IDownload() {}
	virtual HRESULT OnError(Int error) = 0;
	virtual HRESULT OnEnd() = 0;
	virtual HRESULT OnQueryResume() = 0;
	virtual HRESULT OnProgressUpdate(Int bytesread, Int totalsize, Int timetaken, Int timeleft) = 0;
	virtual HRESULT OnStatusUpdate(Int status) = 0;
};
class CDownload
{
public:
	CDownload(IDownload *) {}
	HRESULT PumpMessages() { theLog.push_back(1); return g_pump; }
	HRESULT DownloadFile(const char *server, const char *user, const char *pass, const char *file, const char *local, const char *reg, bool resume)
	{
		theLog.push_back(2); logStr(server); logStr(user); logStr(pass); logStr(file); logStr(local); logStr(reg); theLog.push_back(resume ? 1u : 0u);
		return g_download;
	}
	void GetLastLocalFile(char *buf, int len) { strncpy(buf, g_lastLocal, len); }
};

struct GameTextStandIn
{
	UnicodeString fetch(AsciiString label)
	{
		WideChar w[80];
		int n = 0;
		w[n++] = 'T'; w[n++] = ':';
		for (const char *c = label.str(); *c && n < 78; ++c) w[n++] = (WideChar)(unsigned char)*c;
		w[n] = 0;
		UnicodeString u;
		u.set(w);
		return u;
	}
};
static GameTextStandIn theTextObj;
static GameTextStandIn *TheGameText = &theTextObj;

#include "dl_header.inc"
#include "dl_cpp.inc"

static void putUnits(Words &w, const UnicodeString &s)
{
	const WideChar *c = s.str();
	unsigned n = 0;
	while (c[n]) ++n;
	w.push_back(n);
	for (unsigned i = 0; i < n; ++i) w.push_back((unsigned)c[i]);
}
static std::string getStr(const unsigned *&p)
{
	unsigned n = *p++;
	std::string s;
	for (unsigned i = 0; i < n; ++i) s.push_back((char)*p++);
	return s;
}
static AsciiString getAscii(const unsigned *&p)
{
	std::string s = getStr(p);
	AsciiString a;
	a.set(s.c_str());
	return a;
}

/* A program: [pump result, download result, last local file (length + bytes), number of operations], then per operation the opcode and its arguments. */
extern "C" int ref_dl_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	theLog.clear();
	g_pump = (HRESULT)*p++;
	g_download = (HRESULT)*p++;
	{ std::string s = getStr(p); strncpy(g_lastLocal, s.c_str(), 255); g_lastLocal[255] = 0; }
	DownloadManager m;
	Words w;
	putUnits(w, m.getStatusString());
	unsigned ops = *p++;
	for (unsigned o = 0; o < ops; ++o)
	{
		unsigned op = *p++;
		w.push_back(0xA5A5A5A5u);
		w.push_back(op);
		switch (op)
		{
		case 0: w.push_back((unsigned)m.update()); break;
		case 1: { AsciiString a = getAscii(p), b = getAscii(p), c = getAscii(p), d = getAscii(p), e = getAscii(p), f = getAscii(p); bool r = *p++ != 0; w.push_back((unsigned)m.downloadFile(a, b, c, d, e, f, r)); break; }
		case 2: { AsciiString a = getAscii(p), b = getAscii(p), c = getAscii(p), d = getAscii(p), e = getAscii(p), f = getAscii(p); bool r = *p++ != 0; m.queueFileForDownload(a, b, c, d, e, f, r); break; }
		case 3: w.push_back((unsigned)m.downloadNextQueuedFile()); break;
		case 4: w.push_back(m.isFileQueuedForDownload() ? 1u : 0u); break;
		case 5: { AsciiString s = m.getLastLocalFile(); logStr(s.str()); break; }
		case 6: w.push_back(m.isDone() ? 1u : 0u); w.push_back(m.isOk() ? 1u : 0u); w.push_back(m.wasError() ? 1u : 0u); break;
		case 7: putUnits(w, m.getStatusString()); putUnits(w, m.getErrorString()); break;
		case 8: w.push_back((unsigned)m.OnError((Int)*p++)); break;
		case 9: w.push_back((unsigned)m.OnEnd()); break;
		case 10: w.push_back((unsigned)m.OnQueryResume()); break;
		case 11: { Int a = (Int)*p++, b = (Int)*p++, c = (Int)*p++, d = (Int)*p++; w.push_back((unsigned)m.OnProgressUpdate(a, b, c, d)); break; }
		case 12: w.push_back((unsigned)m.OnStatusUpdate((Int)*p++)); break;
		}
	}
	w.push_back(0xB6B6B6B6u);
	for (size_t i = 0; i < theLog.size(); ++i) w.push_back(theLog[i]);
	if ((int)w.size() > cap) return -1;
	for (size_t i = 0; i < w.size(); ++i) out[i] = w[i];
	return (int)w.size();
}
