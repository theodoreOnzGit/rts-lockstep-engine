/* Differential-test wrapper for the NAT traversal draft (#215, group network3, upstream commit 0a05454d), src/drafts/network/nat.rs: the whole of
** GameNetwork/NAT.cpp from the connection-pair table to the end of the file (update, connectionUpdate, establishConnectionPaths, attachSlotList,
** getSlotPort, generatePortNumbers, doThisConnectionRound, sendAProbe, sendMangledSourcePort, processManglerResponse, the done tests, probed,
** gotMangledPort, gotInternalAddress, the notify functions, processGlobalMessage, setConnectionState), on the REAL class declaration of NAT.h and the REAL
** FirewallHelper.h. Cut verbatim by build.rs (drafts_network3) into OUT_DIR/draft_network3_gen:
**   nat_net.inc   NetworkDefs.h: `enum ConnectionNumbers` to the packet-size comment (MAX_PLAYER, MAX_SLOTS), `#define MAX_MESSAGE_LEN` to the CommandPacket
**                 line (MAX_MESSAGES), and NETWORK_BASE_PORT_NUMBER;
**   nat_h.inc     NAT.h from `enum NATStateType` to the closing #endif (the states, the node struct, the class);
**   nat_cpp.inc   NAT.cpp from `NAT::m_connectionPairs` to the end of the file; the edits (each asserted once) are MSVC's pre-standard for-scope: `Int i;`
**                 is declared before the first loop in doThisConnectionRound and establishConnectionPaths.
** The classes are renamed by build.rs (NAT, TheNAT, FirewallHelperClass, TheFirewallHelper, createFirewallHelper) so that no symbol meets the firewall
** wrapper's or the game-info wrapper's. Built on upstream's UnicodeString.cpp and AsciiString.cpp (unchanged; -fshort-wchar; shim_draft_n3/n3_prelude.h).
** Stand-ins (outside the cuts), logging into theLog as tests/differential_draft_nat.rs's host does:
**   GameSlot (a table of 8: exists, human, ip, port, NAT behaviour, name), TheGameSpyGame (getLocalSlotNum; getGameSpySlot; the stats of a slot have arrived when
**   its bit of a mask is set), Transport (queueSend logs the datagram, update and doSend do nothing observable, m_inBuffer is filled by the program),
**   FirewallHelperClass's members NAT calls (constructor, destructor, getNextTemporarySourcePort, openSpareSocket, sendToManglerFromPort, getManglerResponse (a script
**   of answers), closeSpareSocket, flagNeedToRefresh, getManglerName), gethostbyname, TheEstablishConnectionsMenu, TheGameSpyPeerMessageQueue->addRequest (a
**   PeerRequest with its UTM flag, id, nick and options), TheWritableGlobalData/TheGlobalData (m_firewallPortOverride, m_firewallPortAllocationDelta), timeGetTime.
** NAT's constructor leaves most members unset; the game's allocator zero-fills them (calloc here).
** Entry points (extern "C", tests/differential_draft_nat.rs): ref_nat_pairs (the connection-pair table) and ref_nat_run (programs, see there). */
#include "Common/STLTypedefs.h"
#include "Common/CriticalSection.h"
#include <string.h>
#include <stdlib.h>
#include <ctype.h>
#include <time.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <vector>
#include <map>
#include <string>
#include <new>

namespace {
DynamicMemoryAllocator theAllocator;
}
DynamicMemoryAllocator *TheDynamicMemoryAllocator = &theAllocator;
CriticalSection *TheUnicodeStringCriticalSection = 0;

#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define DEBUG_CRASH(m) ((void)0)
#define DEBUG_LOG(m) ((void)0)
typedef Int Color;
typedef short Short;
typedef char Char;

static std::vector<unsigned> theLog;
static std::vector<unsigned> theSends;   /* the datagrams of one operation: the draft's transport is read back after the operation, so they are appended after its events */
static unsigned fnv(unsigned h, unsigned char c) { return (h ^ c) * 16777619u; }
static unsigned hashBytes(const unsigned char *p, unsigned n) { unsigned h = 2166136261u; for (unsigned i = 0; i < n; ++i) h = fnv(h, p[i]); return h; }
static unsigned hashUnits(const WideChar *s) { unsigned h = 2166136261u; for (; *s; ++s) { h = fnv(h, (unsigned char)(*s & 255)); h = fnv(h, (unsigned char)(*s >> 8)); } return h; }
static unsigned hashStr(const char *s) { return hashBytes((const unsigned char *)s, (unsigned)strlen(s)); }
static unsigned g_now;
static unsigned timeGetTime() { return g_now; }

#include "nat_net.inc"
#include "GameNetwork/FirewallHelper.h"
class GameSlot;
class Transport
{
public:
	struct Msg { Int length; UnsignedInt addr; UnsignedShort port; UnsignedByte data[MAX_MESSAGE_LEN + 1]; };
	Transport() { memset(m_inBuffer, 0, sizeof(m_inBuffer)); }
	/* the real Transport keeps queued datagrams in m_outBuffer and sends them in doSend (called from update): deleting it drops the unsent ones. Pending entries of theSends all belong to the one live transport (they are flushed after every operation), so they go with it. */
	~Transport() { theSends.clear(); }
	Msg m_inBuffer[MAX_MESSAGES];
	Bool queueSend(UnsignedInt addr, UnsignedShort port, const unsigned char *buf, Int len)
	{
		theSends.push_back(90); theSends.push_back(addr); theSends.push_back(port); theSends.push_back((unsigned)len); theSends.push_back(hashBytes(buf, (unsigned)len));
		return TRUE;
	}
	Bool update() { return TRUE; }
	Bool doSend() { return TRUE; }
	Bool init(UnsignedInt ip, UnsignedShort port) { return TRUE; }
};

#define protected public
#include "nat_h.inc"
#undef protected

/* ---- the world the NAT asks of ---- */
class GameSlot
{
public:
	Bool isHuman() const { return m_human; }
	UnsignedInt getIP() const { return m_ip; }
	void setIP(UnsignedInt ip) { m_ip = ip; }
	UnsignedShort getPort() const { return m_port; }
	void setPort(UnsignedShort port) { m_port = port; }
	FirewallHelperClass::tFirewallBehaviorType getNATBehavior() const { return (FirewallHelperClass::tFirewallBehaviorType)m_nat; }
	UnicodeString getName() const { return m_name; }
	Bool m_human; UnsignedInt m_ip; UnsignedShort m_port; unsigned m_nat; UnicodeString m_name;
};
static GameSlot g_slotObjs[MAX_SLOTS];
static GameSlot *g_slotList[MAX_SLOTS];
static Int g_localSlot;
static unsigned g_statsMask;
struct GameSpyGameSlot { Bool isHuman() const { return m_human; } Int getProfileID() const { return m_id; } Bool m_human; Int m_id; };
static GameSpyGameSlot g_gsSlots[MAX_SLOTS];
struct GameSpyGameStandIn
{
	Int getLocalSlotNum() { return g_localSlot; }
	const GameSpyGameSlot *getGameSpySlot(Int i) { if (g_slotList[i] == NULL) return NULL; g_gsSlots[i].m_human = g_slotList[i]->isHuman(); g_gsSlots[i].m_id = i; return &g_gsSlots[i]; }
};
static GameSpyGameStandIn theGameObj;
static GameSpyGameStandIn *TheGameSpyGame = &theGameObj;
struct PSPlayerStats { Int id; };
struct PSQueueStandIn { PSPlayerStats findPlayerStatsByID(Int id) { PSPlayerStats s; s.id = ((g_statsMask >> id) & 1u) ? 1 : 0; return s; } };
static PSQueueStandIn thePSObj;
static PSQueueStandIn *TheGameSpyPSMessageQueue = &thePSObj;

struct PeerRequest
{
	enum { PEERREQUEST_UTMPLAYER = 5 };
	PeerRequest() { peerRequestType = 0; UTM.isStagingRoom = FALSE; }
	int peerRequestType;
	struct { Bool isStagingRoom; } UTM;
	std::string id, nick, options;
};
struct PeerQueueStandIn
{
	void addRequest(const PeerRequest &r)
	{
		theLog.push_back(71); theLog.push_back((unsigned)r.peerRequestType); theLog.push_back(r.UTM.isStagingRoom ? 1u : 0u);
		theLog.push_back(hashStr(r.id.c_str())); theLog.push_back(hashStr(r.nick.c_str())); theLog.push_back(hashStr(r.options.c_str()));
	}
};
static PeerQueueStandIn thePeerObj;
static PeerQueueStandIn *TheGameSpyPeerMessageQueue = &thePeerObj;

class EstablishConnectionsMenu
{
public:
	void initMenu() { theLog.push_back(72); }
	void endMenu() { theLog.push_back(73); }
	void setPlayerName(Int i, UnicodeString name) { theLog.push_back(74); theLog.push_back((unsigned)i); theLog.push_back(hashUnits(name.str())); }
	void setPlayerStatus(Int i, NATConnectionState s) { theLog.push_back(75); theLog.push_back((unsigned)i); theLog.push_back((unsigned)s); }
};
static EstablishConnectionsMenu *TheEstablishConnectionsMenu = NULL;

struct GlobalDataStandIn { UnsignedShort m_firewallPortOverride; Int m_firewallPortAllocationDelta; };
static GlobalDataStandIn g_global;
static GlobalDataStandIn *TheGlobalData = &g_global;
static GlobalDataStandIn *TheWritableGlobalData = &g_global;

static unsigned g_nextTempPort;
static std::vector<unsigned> g_responses;
static unsigned g_responseAt;
static unsigned g_manglerAddr;
FirewallHelperClass::FirewallHelperClass(void) { theLog.push_back(76); }
FirewallHelperClass::~FirewallHelperClass() { theLog.push_back(77); }
UnsignedShort FirewallHelperClass::getNextTemporarySourcePort(Int skip) { theLog.push_back(78); theLog.push_back((unsigned)skip); return (UnsignedShort)(g_nextTempPort++); }
Bool FirewallHelperClass::openSpareSocket(UnsignedShort port) { theLog.push_back(79); theLog.push_back(port); return TRUE; }
Bool FirewallHelperClass::sendToManglerFromPort(UnsignedInt address, UnsignedShort port, UnsignedShort packetID, Bool blitzme) { theLog.push_back(80); theLog.push_back(address); theLog.push_back(port); theLog.push_back(packetID); return TRUE; }
UnsignedShort FirewallHelperClass::getManglerResponse(UnsignedShort packetID, Int time) { theLog.push_back(81); theLog.push_back(packetID); return (UnsignedShort)((g_responseAt < g_responses.size()) ? g_responses[g_responseAt++] : 0u); }
void FirewallHelperClass::closeSpareSocket(UnsignedShort port) { theLog.push_back(82); theLog.push_back(port); }
void FirewallHelperClass::flagNeedToRefresh(Bool flag) { theLog.push_back(83); theLog.push_back(flag ? 1u : 0u); }
void FirewallHelperClass::getManglerName(Int manglerIndex, Char *nameBuf) { strcpy(nameBuf, manglerIndex == 1 ? "mangler.example" : ""); }
FirewallHelperClass *TheFirewallHelper = NULL;
FirewallHelperClass *createFirewallHelper() { return new FirewallHelperClass(); }

static unsigned char g_hostAddr[4];
static char *g_hostList[2];
static struct hostent g_hostent;
static struct hostent *gethostbyname_n3(const char *name)
{
	if (g_manglerAddr == 0) return NULL;
	unsigned a = htonl(g_manglerAddr);
	memcpy(g_hostAddr, &a, 4);
	g_hostList[0] = (char *)g_hostAddr; g_hostList[1] = 0;
	g_hostent.h_addr_list = g_hostList;
	return &g_hostent;
}
#define gethostbyname gethostbyname_n3

#include "nat_cpp.inc"

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

void dump(NAT *a)
{
	theLog.push_back(0xD0D0D0D0u);
	theLog.push_back((unsigned)a->m_NATState); theLog.push_back((unsigned)a->m_localNodeNumber); theLog.push_back((unsigned)a->m_targetNodeNumber);
	theLog.push_back(a->m_localIP); theLog.push_back(a->m_numNodes); theLog.push_back(a->m_connectionRound);
	theLog.push_back((unsigned)a->m_numRetries); theLog.push_back((unsigned)a->m_maxNumRetriesAllowed); theLog.push_back(a->m_packetID); theLog.push_back(a->m_spareSocketPort);
	theLog.push_back((unsigned)(a->m_manglerRetryTime & 0xffffffffull)); theLog.push_back((unsigned)((unsigned long long)a->m_manglerRetryTime >> 32));
	theLog.push_back((unsigned)a->m_manglerRetries); theLog.push_back(a->m_previousSourcePort); theLog.push_back(a->m_beenProbed ? 1u : 0u); theLog.push_back(a->m_manglerAddress);
	theLog.push_back((unsigned)(a->m_timeTillNextSend & 0xffffffffull)); theLog.push_back((unsigned)((unsigned long long)a->m_timeTillNextSend >> 32));
	for (int i = 0; i < MAX_SLOTS; ++i) theLog.push_back((unsigned)a->m_connectionStates[i]);
	for (int i = 0; i < MAX_SLOTS; ++i) { theLog.push_back((unsigned)a->m_connectionNodes[i].m_behavior); theLog.push_back(a->m_connectionNodes[i].m_slotIndex); }
	for (int i = 0; i < MAX_SLOTS; ++i) theLog.push_back(a->m_sourcePorts[i]);
	for (int i = 0; i < MAX_SLOTS; ++i) theLog.push_back(a->m_myConnections[i] ? 1u : 0u);
	theLog.push_back((unsigned)(a->m_nextKeepaliveTime & 0xffffffffull)); theLog.push_back((unsigned)((unsigned long long)a->m_nextKeepaliveTime >> 32));
	theLog.push_back((unsigned)a->m_connectionPairIndex); theLog.push_back(a->m_startingPortNumber);
	theLog.push_back((unsigned)(a->m_nextPortSendTime & 0xffffffffull)); theLog.push_back((unsigned)((unsigned long long)a->m_nextPortSendTime >> 32));
	theLog.push_back((unsigned)(a->m_timeoutTime & 0xffffffffull)); theLog.push_back((unsigned)((unsigned long long)a->m_timeoutTime >> 32));
	theLog.push_back((unsigned)(a->m_roundTimeout & 0xffffffffull)); theLog.push_back((unsigned)((unsigned long long)a->m_roundTimeout >> 32));
	theLog.push_back(a->m_transport ? 1u : 0u); theLog.push_back(TheFirewallHelper ? 1u : 0u);
	for (int i = 0; i < MAX_SLOTS; ++i) { theLog.push_back(g_slotList[i] ? 1u : 0u); if (g_slotList[i]) { theLog.push_back(g_slotList[i]->m_ip); theLog.push_back(g_slotList[i]->m_port); } }
}

} // namespace

extern "C" {

int ref_nat_pairs(int *out)
{
	int n = 0;
	for (int a = 0; a < MAX_SLOTS - 1; ++a) for (int b = 0; b < MAX_SLOTS - 1; ++b) for (int c = 0; c < MAX_SLOTS; ++c) out[n++] = NAT::m_connectionPairs[a][b][c];
	out[n++] = NAT::m_timeBetweenRetries; out[n++] = (int)NAT::m_manglerRetryTimeInterval; out[n++] = NAT::m_maxAllowedManglerRetries;
	out[n++] = (int)NAT::m_keepaliveInterval; out[n++] = (int)NAT::m_timeToWaitForPort; out[n++] = (int)NAT::m_timeForRoundTimeout; out[n++] = MS_TO_WAIT_FOR_STATS;
	return n;
}

/* in: [clock, localSlotNum, portOverride, portAllocationDelta, helperPresent, statsMask, manglerAddr (0: gethostbyname fails), nextTempPort, nResponses, responses...,
**      8 x (exists, human, ip, port, nat behaviour, name bytes), ops...];
**  1 attachSlotList(localSlot, localIP)   2 establishConnectionPaths   3 update   4 processGlobalMessage(slotNum, bytes)   5 advance the clock(dt)
**  6 put a datagram in m_inBuffer[i] (i, length, addr, port, bytes)   7 set the IP of slot i   8 set the port of slot i   9 set the NAT behaviour of slot i
**  10 getSlotPort(i)   11 generatePortNumbers(localSlot)   12 set the local slot number   13 set the port override   14 set the allocation delta   15 set the stats mask
**  16 push a mangler response   17 notifyTargetOfProbe(slot i)   18 sendMangledPortNumberToTarget(port, slot i)   19 notifyUsersOfConnectionDone(node)
**  20 notifyUsersOfConnectionFailed(node)   21 setConnectionState(node, state)   22 processManglerResponse(port)   23 sendMangledSourcePort   24 doThisConnectionRound
**  25 connectionUpdate   26 probed(node)   27 gotMangledPort(node, port)   28 gotInternalAddress(node, address)   29 allConnectionsDone, allConnectionsDoneThisRound
**  30 set m_connectionRound(v) 31 set m_myConnections[i] (i, v)   32 set m_previousSourcePort(v)   33 sendAProbe(ip, port, node)
** After each op the NAT's fields and the slots' addresses are dumped. */
int ref_nat_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	theLog.clear();
	Rd r = { in, nin, 0 };
	g_now = r.next(); g_localSlot = (Int)r.next(); g_global.m_firewallPortOverride = (UnsignedShort)r.next(); g_global.m_firewallPortAllocationDelta = (Short)r.next();
	Bool helper = r.next() != 0; g_statsMask = r.next(); g_manglerAddr = r.next(); g_nextTempPort = r.next();
	g_responses.clear(); g_responseAt = 0;
	unsigned nr = r.next(); for (unsigned i = 0; i < nr; ++i) g_responses.push_back(r.next());
	for (int i = 0; i < MAX_SLOTS; ++i) {
		bool ex = r.next() != 0;
		GameSlot &s = g_slotObjs[i];
		s.m_human = r.next() != 0; s.m_ip = r.next(); s.m_port = (UnsignedShort)r.next(); s.m_nat = r.next();
		std::string nm = r.bytes();
		std::vector<WideChar> w; for (size_t k = 0; k < nm.size(); ++k) w.push_back((WideChar)(unsigned char)nm[k]); w.push_back(0);
		s.m_name.set(&w[0]);
		g_slotList[i] = ex ? &s : NULL;
	}
	TheEstablishConnectionsMenu = NULL;
	TheFirewallHelper = helper ? new FirewallHelperClass() : NULL;
	theLog.clear();   /* the constructor's log line is the program's setup, not an operation */
	theSends.clear();
	void *mem = calloc(1, sizeof(NAT));
	NAT *a = new (mem) NAT();
	dump(a);
	while (r.i < r.n) {
		unsigned op = r.next();
		theLog.push_back(0xA5A5A5A5u); theLog.push_back(op);
		switch (op) {
		case 1: { Int ls = (Int)r.next(); UnsignedInt ip = r.next(); a->attachSlotList(g_slotList, ls, ip); break; }
		case 2: a->establishConnectionPaths(); break;
		case 3: theLog.push_back((unsigned)a->update()); break;
		case 4: {
			Int sn = (Int)r.next(); std::string o = r.bytes();
			/* PORT and PROBED reach m_connectionNodes[m_targetNodeNumber] (gotMangledPort, probed): -1 is an index before the array upstream; not run */
			const char *p = o.c_str(); while (isspace(*p)) ++p;
			if (a->m_targetNodeNumber < 0 && (!strncmp(p, "PORT", 4) || !strncmp(p, "PROBED", 6))) { theLog.push_back(0xBAD0BAD0u); break; }
			if (a->m_transport == NULL && (!strncmp(p, "PORT", 4) || !strncmp(p, "PROBED", 6))) { theLog.push_back(0xBAD0BAD1u); break; }
			/* CONNDONE indexes the pair table by the round: 7 and up is past the table */
			if (a->m_connectionRound >= 7 && !strncmp(p, "CONNDONE", 8)) { theLog.push_back(0xBAD0BAD2u); break; }
			a->processGlobalMessage(sn, o.c_str()); break; }
		case 5: g_now += r.next(); break;
		case 6: {
			unsigned i = r.next() % MAX_MESSAGES; Int len = (Int)r.next(); unsigned addr = r.next(); unsigned port = r.next(); std::string d = r.bytes();
			if (a->m_transport) {
				Transport::Msg &m = a->m_transport->m_inBuffer[i];
				memset(&m, 0, sizeof(m)); m.length = len; m.addr = addr; m.port = (UnsignedShort)port;
				memcpy(m.data, d.c_str(), d.size() < MAX_MESSAGE_LEN ? d.size() : MAX_MESSAGE_LEN);
			}
			break; }
		case 7: { unsigned i = r.next() % MAX_SLOTS; g_slotObjs[i].m_ip = r.next(); break; }
		case 8: { unsigned i = r.next() % MAX_SLOTS; g_slotObjs[i].m_port = (UnsignedShort)r.next(); break; }
		case 9: { unsigned i = r.next() % MAX_SLOTS; g_slotObjs[i].m_nat = r.next(); break; }
		case 10: theLog.push_back((unsigned)a->getSlotPort((Int)r.next())); break;
		case 11: a->generatePortNumbers(g_slotList, (Int)r.next()); break;
		case 12: g_localSlot = (Int)r.next(); break;
		case 13: g_global.m_firewallPortOverride = (UnsignedShort)r.next(); break;
		case 14: g_global.m_firewallPortAllocationDelta = (Short)r.next(); break;
		case 15: g_statsMask = r.next(); break;
		case 16: g_responses.push_back(r.next()); break;
		case 17: a->notifyTargetOfProbe(g_slotList[r.next() % MAX_SLOTS]); break;
		case 18: { unsigned p = r.next(); a->sendMangledPortNumberToTarget((UnsignedShort)p, g_slotList[r.next() % MAX_SLOTS]); break; }
		case 19: a->notifyUsersOfConnectionDone((Int)r.next()); break;
		case 20: a->notifyUsersOfConnectionFailed((Int)r.next()); break;
		case 21: { Int node = (Int)r.next(); a->setConnectionState(node, (NATConnectionState)r.next()); break; }
		case 22: { unsigned p = r.next(); if (a->m_targetNodeNumber < 0) { theLog.push_back(0xBAD0BAD0u); break; } a->processManglerResponse((UnsignedShort)p); break; }
		case 23: if (a->m_targetNodeNumber < 0) { theLog.push_back(0xBAD0BAD0u); break; } a->sendMangledSourcePort(); break;
		case 24: if (a->m_connectionRound >= 7) { theLog.push_back(0xBAD0BAD2u); break; } a->doThisConnectionRound(); break;   /* the pair table has rounds 0 to 6 */
		case 25: if (a->m_transport == NULL) { theLog.push_back(0xBAD0BAD1u); break; } theLog.push_back((unsigned)a->connectionUpdate()); break;
		case 26: { Int node = (Int)r.next(); if (a->m_targetNodeNumber < 0) { theLog.push_back(0xBAD0BAD0u); break; } if (a->m_transport == NULL) { theLog.push_back(0xBAD0BAD1u); break; } a->probed(node); break; }
		case 27: { Int node = (Int)r.next(); unsigned p = r.next(); if (a->m_targetNodeNumber < 0) { theLog.push_back(0xBAD0BAD0u); break; } if (a->m_transport == NULL) { theLog.push_back(0xBAD0BAD1u); break; } a->gotMangledPort(node, (UnsignedShort)p); break; }
		case 28: { Int node = (Int)r.next(); a->gotInternalAddress(node, r.next()); break; }
		case 29: theLog.push_back(a->allConnectionsDone() ? 1u : 0u); theLog.push_back(a->allConnectionsDoneThisRound() ? 1u : 0u); break;
		case 30: a->m_connectionRound = r.next(); break;
		case 31: { unsigned i = r.next() % MAX_SLOTS; a->m_myConnections[i] = r.next() != 0; break; }
		case 32: a->m_previousSourcePort = (UnsignedShort)r.next(); break;
		case 33: { unsigned ip = r.next(); unsigned port = r.next(); Int node = (Int)r.next(); if (a->m_transport == NULL) { theLog.push_back(0xBAD0BAD1u); break; } a->sendAProbe(ip, (UnsignedShort)port, node); break; }
		case 34: a->m_numRetries = (Int)r.next(); break;
		default: break;
		}
		for (size_t k = 0; k < theSends.size(); ++k) theLog.push_back(theSends[k]);
		theSends.clear();
		dump(a);
	}
	int total = (int)theLog.size();
	int n = total;
	if (n > cap) n = cap;
	memcpy(out, &theLog[0], n * sizeof(unsigned));
	if (TheFirewallHelper) { delete TheFirewallHelper; TheFirewallHelper = NULL; }   /* its log line is after the program's end */
	return total;
}

}
