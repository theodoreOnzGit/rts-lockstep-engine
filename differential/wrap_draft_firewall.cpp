/* Differential-test wrapper for the firewall helper draft (#215, group network3, upstream commit 0a05454d),
** src/drafts/network/firewall_helper.rs: all of GameNetwork/FirewallHelper.cpp, from `FirewallHelperClass *TheFirewallHelper = NULL;` to
** the end of the file, on the REAL FirewallHelper.h (self-contained: no includes), cut by build.rs (drafts_network3) into
** OUT_DIR/draft_network3_gen/fw_all.inc with these mechanical edits, each asserted to occur once:
**   MSVC's pre-standard for-scope (a `for (Int i ...)` variable used by a later loop of the same function), in the constructor,
**   getManglerResponse, openSpareSocket and detectionBeginUpdate: `i` is declared before the first loop;
** and the file's own include lines (PreRTS.h, crc.h, UserPreferences.h, FirewallHelper.h, NAT.h, udp.h, NetworkDefs.h, GSConfig.h) are not
** part of the cut: the headers it needs are the stand-ins below.
** Built on the REAL AsciiString.h/AsciiString.cpp (shim_ascii; classes renamed by build.rs) and upstream's crc.h/crc.cpp (unchanged:
** _DEBUG selects the C++ CRC class for those lines only, as the conn wrapper does).
** Stand-ins (outside the cut), logging into theLog as tests/differential_draft_firewall.rs's host does:
**   UDP: Bind (succeeds), Write and Read on in-memory queues (the draft's sockets are native, so the socket-using paths are NOT compared);
**   OptionPreferences (the options map: find, operator[], write: the written keys are logged), TheWritableGlobalData / TheGlobalData
**   (m_firewallBehavior, m_firewallPortOverride, m_firewallSendDelay, m_firewallPortAllocationDelta), TheGameSpyConfig->getManglerLocation,
**   gethostbyname (a table from the configuration; NULL for an unknown name), timeGetTime (a settable clock), itoa, WSAGetLastError (0),
**   GENERALS_MAGIC_NUMBER (NetworkDefs.h:178, 0xF00D), the constant the C++ takes `static const` there.
** Entry points (extern "C", tests/differential_draft_firewall.rs): ref_fw_scheme, ref_fw_sizes, ref_fw_run; ops at ref_fw_run. */
#include <string.h>
#include <stdlib.h>
#include <stdarg.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <vector>
#include <map>
#include <set>
#include <string>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theFwAllocator;

#define NULL 0
#define NEW new
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define DEBUG_CRASH(m) ((void)0)
#define DEBUG_LOG(m) ((void)0)
typedef short Short;
typedef char Char;

/* crc.h's release CRC is an MSVC `_asm` block that g++ cannot compile; _DEBUG (for these lines only) selects its C++ class, the same loop */
#define _DEBUG 1
#include "Common/crc.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/crc.cpp"
#undef _DEBUG

#define private public
#define protected public
#include "GameNetwork/FirewallHelper.h"
#undef private
#undef protected

static const UnsignedShort GENERALS_MAGIC_NUMBER = 0xF00D;

static std::vector<unsigned> theLog;
static unsigned g_now;
static unsigned timeGetTime() { return g_now; }
static int WSAGetLastError() { return 0; }
static char *itoa(int v, char *buf, int base) { snprintf(buf, 16, base == 10 ? "%d" : "%x", v); return buf; }

class UDP
{
public:
	UDP() {}
	Int Bind(UnsignedInt ip, UnsignedShort port) { theLog.push_back(80); theLog.push_back(ip); theLog.push_back(port); return 0; }
	Int Write(const unsigned char *msg, UnsignedInt len, UnsignedInt IP, UnsignedShort port) { theLog.push_back(81); theLog.push_back(len); theLog.push_back(IP); theLog.push_back(port); return (Int)len; }
	Int Read(unsigned char *msg, UnsignedInt len, sockaddr_in *from) { return 0; }
};

struct GlobalDataStandIn { UnsignedInt m_firewallBehavior; UnsignedShort m_firewallPortOverride; Bool m_firewallSendDelay; Int m_firewallPortAllocationDelta; };
static GlobalDataStandIn g_global;
static GlobalDataStandIn *TheGlobalData = &g_global;
static GlobalDataStandIn *TheWritableGlobalData = &g_global;

struct HostEntry { std::string name; unsigned addr; };
static std::vector<HostEntry> g_hosts;       /* the mangler names, in order, with the address each resolves to (0: lookup fails) */
struct GameSpyConfigStandIn
{
	void getManglerLocation(Int index, AsciiString &host, UnsignedShort &port)
	{
		host = ((size_t)index < g_hosts.size()) ? g_hosts[index].name.c_str() : "";
		port = 4321;
	}
};
static GameSpyConfigStandIn theConfigObj;
static GameSpyConfigStandIn *TheGameSpyConfig = &theConfigObj;

static unsigned char g_addrBytes[4];
static unsigned char *g_addrList[2];
static struct hostent g_hostent;
static struct hostent *gethostbyname_n3(const char *name)
{
	for (size_t i = 0; i < g_hosts.size(); ++i) {
		if (g_hosts[i].name == name && g_hosts[i].addr != 0) {
			g_addrBytes[0] = (unsigned char)(g_hosts[i].addr >> 24); g_addrBytes[1] = (unsigned char)(g_hosts[i].addr >> 16);
			g_addrBytes[2] = (unsigned char)(g_hosts[i].addr >> 8); g_addrBytes[3] = (unsigned char)g_hosts[i].addr;
			g_addrList[0] = g_addrBytes; g_addrList[1] = 0;
			g_hostent.h_addr_list = (char **)g_addrList;
			return &g_hostent;
		}
	}
	return NULL;
}
#define gethostbyname gethostbyname_n3

typedef std::map<AsciiString, AsciiString> PreferenceMap;
struct OptionPreferences : public PreferenceMap
{
	OptionPreferences();
	AsciiString &operator[](const AsciiString &k) { touched.insert(std::string(k.str())); return PreferenceMap::operator[](k); }
	void write();
	std::set<std::string> touched;   /* the keys the caller assigned (the file's other keys are written back unchanged and not logged) */
};
static PreferenceMap g_prefs;               /* the options file's content, set by the case (the key FirewallNeedToRefresh only) */
OptionPreferences::OptionPreferences() { *(PreferenceMap *)this = g_prefs; }
void OptionPreferences::write()
{
	/* what the options file would hold: the keys the helper writes, as numbers */
	PreferenceMap::iterator a = find("FirewallBehavior"), b = find("FirewallPortAllocationDelta"), c = find("FirewallNeedToRefresh");
	if (touched.count("FirewallBehavior") && touched.count("FirewallPortAllocationDelta") && a != end() && b != end()) { theLog.push_back(61); theLog.push_back((unsigned)atoi(a->second.str())); theLog.push_back((unsigned)atoi(b->second.str())); }
	if (touched.count("FirewallNeedToRefresh") && c != end()) { theLog.push_back(62); theLog.push_back(c->second.compareNoCase("TRUE") == 0 ? 1u : 0u); }
}

#include "fw_all.inc"

/* the game's allocator zero-fills what NEW returns (the constructor leaves m_spareSockets[i].udp unset, and reset() deletes a non-NULL one) */
#include <new>
static FirewallHelperClass *makeHelper() { void *m = calloc(1, sizeof(FirewallHelperClass)); return new (m) FirewallHelperClass(); }
static void freeHelper(FirewallHelperClass *h) { h->~FirewallHelperClass(); free(h); }

namespace {

struct Rd
{
	const unsigned *w; int n; int i;
	unsigned next() { return i < n ? w[i++] : 0; }
	std::string bytes()
	{
		unsigned len = next();
		std::string s;
		for (unsigned k = 0; k < len; k += 4) {
			unsigned v = next();
			for (unsigned j = 0; j < 4 && k + j < len; ++j) s += (char)(v >> (8 * j));
		}
		return s;
	}
};

void dump(FirewallHelperClass *h)
{
	theLog.push_back(0xD0D0D0D0u);
	theLog.push_back((unsigned)h->m_behavior); theLog.push_back((unsigned)h->m_lastBehavior);
	theLog.push_back((unsigned)h->m_sourcePortAllocationDelta); theLog.push_back((unsigned)h->m_lastSourcePortAllocationDelta);
	theLog.push_back((unsigned)FirewallHelperClass::m_sourcePortPool);
	theLog.push_back((unsigned)h->m_numManglers);
	for (int i = 0; i < MAX_NUM_MANGLERS; ++i) theLog.push_back(h->m_manglers[i]);
	theLog.push_back(h->m_packetID); theLog.push_back((unsigned)h->m_currentState);
	theLog.push_back(h->m_timeoutStart); theLog.push_back(h->m_timeoutLength); theLog.push_back((unsigned)h->m_currentTry); theLog.push_back((unsigned)h->m_numResponses);
	for (int i = 0; i < MAX_SPARE_SOCKETS; ++i) { theLog.push_back(h->m_spareSockets[i].port); theLog.push_back(h->m_sparePorts[i]); theLog.push_back(h->m_mangledPorts[i]); theLog.push_back((unsigned)h->m_messages[i].length); }
	theLog.push_back(g_global.m_firewallBehavior); theLog.push_back(g_global.m_firewallPortAllocationDelta);
}

} // namespace

extern "C" {

int ref_fw_scheme(int num, unsigned short *orig, unsigned short *mang, int *rel, int *good)
{
	FirewallHelperClass *hp = makeHelper();
	FirewallHelperClass &h = *hp;
	Bool r = 0, g = 0;
	int v = h.getNATPortAllocationScheme(num, orig, mang, r, g);
	*rel = r; *good = g;
	freeHelper(hp);
	return v;
}

/* sizeof(ManglerData), sizeof(ManglerMessage), MANGLER_PORT, MAX_NUM_MANGLERS, MAX_SPARE_SOCKETS, NUM_TEST_PORTS, the behaviour values */
int ref_fw_sizes(unsigned *out)
{
	int n = 0;
	out[n++] = sizeof(ManglerData); out[n++] = sizeof(ManglerMessage); out[n++] = MANGLER_PORT; out[n++] = MAX_NUM_MANGLERS; out[n++] = MAX_SPARE_SOCKETS; out[n++] = NUM_TEST_PORTS;
	out[n++] = FirewallHelperClass::FIREWALL_TYPE_UNKNOWN; out[n++] = FirewallHelperClass::FIREWALL_TYPE_SIMPLE; out[n++] = FirewallHelperClass::FIREWALL_TYPE_DUMB_MANGLING;
	out[n++] = FirewallHelperClass::FIREWALL_TYPE_SMART_MANGLING; out[n++] = FirewallHelperClass::FIREWALL_TYPE_NETGEAR_BUG; out[n++] = FirewallHelperClass::FIREWALL_TYPE_SIMPLE_PORT_ALLOCATION;
	out[n++] = FirewallHelperClass::FIREWALL_TYPE_RELATIVE_PORT_ALLOCATION; out[n++] = FirewallHelperClass::FIREWALL_TYPE_DESTINATION_PORT_DELTA;
	out[n++] = DETECTIONSTATE_IDLE; out[n++] = DETECTIONSTATE_BEGIN; out[n++] = DETECTIONSTATE_TEST1; out[n++] = DETECTIONSTATE_TEST2; out[n++] = DETECTIONSTATE_TEST3;
	out[n++] = DETECTIONSTATE_TEST3_WAITFORRESPONSES; out[n++] = DETECTIONSTATE_TEST4_1; out[n++] = DETECTIONSTATE_TEST4_2; out[n++] = DETECTIONSTATE_TEST5; out[n++] = DETECTIONSTATE_DONE;
	return n;
}

/* in: [clock, refresh (0 none, 1 "TRUE", 2 "true", 3 "FALSE", 4 "x"), behaviour, override, sendDelay, nHosts, (name bytes, addr)..., ops...];
**   1 construct   2 setSourcePortPoolStart(p)   3 getSourcePortPool   4 detectFirewall   5 behaviorDetectionUpdate   6 getFirewallBehavior
**   7 getRawFirewallBehavior   8 isNAT()   9 isNetgear()   10 writeFirewallBehavior   11 flagNeedToRefresh(f)   12 getManglerName(i)   13 reset
**   14 getSourcePortAllocationDelta   15 advance the clock(dt)   16 set the global behaviour(b)   17 set the port override(v)   18 set the send delay(v)
**   19 getFirewallHardness(b)   20 getFirewallRetries(b)   21 isNAT(b)   22 isNetgear(b)
** After each op the helper's fields are dumped. */
int ref_fw_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	theLog.clear();
	Rd r = { in, nin, 0 };
	g_now = r.next();
	unsigned refresh = r.next();
	g_global.m_firewallBehavior = r.next();
	g_global.m_firewallPortOverride = (UnsignedShort)r.next();
	g_global.m_firewallSendDelay = r.next() != 0;
	g_global.m_firewallPortAllocationDelta = 0;
	g_prefs.clear();
	if (refresh == 1) g_prefs[AsciiString("FirewallNeedToRefresh")] = AsciiString("TRUE");
	if (refresh == 2) g_prefs[AsciiString("FirewallNeedToRefresh")] = AsciiString("true");
	if (refresh == 3) g_prefs[AsciiString("FirewallNeedToRefresh")] = AsciiString("FALSE");
	if (refresh == 4) g_prefs[AsciiString("FirewallNeedToRefresh")] = AsciiString("x");
	g_hosts.clear();
	unsigned nh = r.next();
	for (unsigned i = 0; i < nh; ++i) { HostEntry e; e.name = r.bytes(); e.addr = r.next(); g_hosts.push_back(e); }
	FirewallHelperClass *h = makeHelper();
	TheFirewallHelper = h;
	dump(h);
	while (r.i < r.n) {
		unsigned op = r.next();
		theLog.push_back(0xA5A5A5A5u); theLog.push_back(op);
		switch (op) {
		case 1: freeHelper(h); h = makeHelper(); TheFirewallHelper = h; break;
		case 2: h->setSourcePortPoolStart((Int)r.next()); break;
		case 3: theLog.push_back((unsigned)h->getSourcePortPool()); break;
		case 4: theLog.push_back(h->detectFirewall() ? 1u : 0u); break;
		case 5: theLog.push_back(h->behaviorDetectionUpdate() ? 1u : 0u); break;
		case 6: theLog.push_back((unsigned)h->getFirewallBehavior()); break;
		case 7: theLog.push_back(h->getRawFirewallBehavior()); break;
		case 8: theLog.push_back(h->isNAT() ? 1u : 0u); break;
		case 9: theLog.push_back(h->isNetgear() ? 1u : 0u); break;
		case 10: h->writeFirewallBehavior(); break;
		case 11: h->flagNeedToRefresh(r.next() != 0); break;
		case 12: { char buf[256]; buf[0] = 0; FirewallHelperClass::getManglerName((Int)r.next(), buf); theLog.push_back((unsigned)strlen(buf)); for (char *c = buf; *c; ++c) theLog.push_back((unsigned char)*c); break; }
		case 13: h->reset(); break;
		case 14: theLog.push_back((unsigned)(int)h->getSourcePortAllocationDelta()); break;
		case 15: g_now += r.next(); break;
		case 16: g_global.m_firewallBehavior = r.next(); break;
		case 17: g_global.m_firewallPortOverride = (UnsignedShort)r.next(); break;
		case 18: g_global.m_firewallSendDelay = r.next() != 0; break;
		case 19: theLog.push_back((unsigned)h->getFirewallHardness((FirewallHelperClass::FirewallBehaviorType)r.next())); break;
		case 20: theLog.push_back((unsigned)h->getFirewallRetries((FirewallHelperClass::FirewallBehaviorType)r.next())); break;
		case 21: theLog.push_back(h->isNAT((FirewallHelperClass::FirewallBehaviorType)r.next()) ? 1u : 0u); break;
		case 22: theLog.push_back(h->isNetgear((FirewallHelperClass::FirewallBehaviorType)r.next()) ? 1u : 0u); break;
		default: break;
		}
		dump(h);
	}
	freeHelper(h);
	TheFirewallHelper = NULL;
	int n = (int)theLog.size();
	if (n > cap) n = cap;
	memcpy(out, &theLog[0], n * sizeof(unsigned));
	return (int)theLog.size();
}

}
