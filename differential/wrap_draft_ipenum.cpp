/* Differential-test wrapper for the local-address enumeration draft (#215, group net4, upstream commit 0a05454d), src/drafts/network/ip_enumeration.rs, cut
** verbatim by build.rs (drafts_net4) into OUT_DIR/draft_net4_gen:
**   ip_header.inc  Include/GameNetwork/IPEnumeration.h from `class EnumeratedIP` to the include guard's #endif (EnumeratedIP, IPEnumeration);
**   ip_cpp.inc     Source/GameNetwork/IPEnumeration.cpp from `IPEnumeration::IPEnumeration( void )` to the end of the file.
** Built on upstream's own UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; classes renamed by build.rs).
** Stand-ins (outside the cuts): the memory-pool base class and its glue macros (differential/shim/Common/GameMemory.h, through STLTypedefs.h: plain new and
**   delete, allocations zero-filled as upstream's allocator does), DEBUG_LOG (nothing), and winsock (shim_draft_net4/winsock_standin.h: WSAStartup, gethostname and
**   gethostbyname answer from variables this wrapper sets from the case).
** Entry point (extern "C", tests/differential_draft_ipenum.rs): ref_ip_run(in, n, out, cap): a program of calls with a changing host configuration. */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <vector>
#include <string>

/* the allocator and the string critical section are defined once, in wrap_draft_ping.cpp (the three wrappers share one library) */
extern DynamicMemoryAllocator *TheDynamicMemoryAllocator;
extern CriticalSection *TheUnicodeStringCriticalSection;

#define NULL 0
#define DEBUG_LOG(x) ((void)0)
#include "winsock_standin.h"

#include "ip_header.inc"
#include "ip_cpp.inc"

typedef std::vector<unsigned> Words;
static void putStr(Words &w, const char *s)
{
	size_t n = strlen(s);
	w.push_back((unsigned)n);
	for (size_t i = 0; i < n; ++i) w.push_back((unsigned char)s[i]);
}
static void dumpList(Words &w, EnumeratedIP *ip)
{
	unsigned count = 0;
	for (EnumeratedIP *p = ip; p; p = p->getNext()) ++count;
	w.push_back(ip ? 1u : 0u);
	w.push_back(count);
	for (EnumeratedIP *p = ip; p; p = p->getNext())
	{
		putStr(w, p->getIPstring().str());
		w.push_back(p->getIP());
	}
}

/* A program: per call, a configuration (0 = the lookups work, 1 = gethostname fails, 2 = gethostbyname fails, 3 = winsock fails to start), the machine name
** (length + bytes), the address count and each address' four bytes; then the opcode (0 = getAddresses, 1 = getMachineName). */
extern "C" int ref_ip_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	const unsigned *p = in;
	IPEnumeration e;
	Words w;
	unsigned calls = *p++;
	static char addrs[16][4];
	for (unsigned c = 0; c < calls; ++c)
	{
		unsigned cfg = *p++;
		unsigned nl = *p++;
		for (unsigned i = 0; i < nl && i < 63; ++i) g_hostname[i] = (char)*p++;
		g_hostname[nl < 63 ? nl : 63] = 0;
		unsigned na = *p++;
		for (unsigned a = 0; a < na; ++a) { for (int b = 0; b < 4; ++b) addrs[a][b] = (char)*p++; g_addrList[a] = addrs[a]; }
		g_addrList[na] = 0;
		g_hostEnt.h_length = 4;
		g_hostEnt.h_addr_list = g_addrList;
		g_hostnameFail = cfg == 1;
		g_lookupFail = cfg == 2;
		g_wsaErr = cfg == 3 ? 10091 : 0;
		unsigned op = *p++;
		w.push_back(0xA5A5A5A5u);
		w.push_back(op);
		if (op == 0) dumpList(w, e.getAddresses());
		else { AsciiString s = e.getMachineName(); putStr(w, s.str()); }
	}
	if ((int)w.size() > cap) return -1;
	for (size_t i = 0; i < w.size(); ++i) out[i] = w[i];
	return (int)w.size();
}
