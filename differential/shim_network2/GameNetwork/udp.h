/* Stand-in for GameNetwork/udp.h (and udp.cpp), differential tests only (#215, group network2): the BSD/Winsock datagram socket
** replaced by in-memory queues, with the member functions Transport.cpp calls. It does what the drafts' queued socket
** (src/drafts/network/udp.rs `UdpBackend::Queued`) does, which is what the real UDP does on a working socket:
**   Write: ip 0 or port 0 returns ADDRNOTAVAIL without touching the status (udp.cpp:244); else the datagram is logged and len returned
**   Read:  the first inbound datagram, truncated to len, with the sender in `from` in network byte order; 0 when none
** and a failure switch (set_failure) standing for the OS failing: Write and Read return -1 and leave the status at the code given. */
#pragma once
#ifndef UDP_HEADER
#define UDP_HEADER
#include <netdb.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <deque>
#include <vector>
#include "Lib/BaseType.h"

struct StandInDatagram { std::vector<unsigned char> bytes; unsigned addr; unsigned short port; };

class UDP
{
public:
	enum sockStat
	{
		OK = 0, UNKNOWN = -1, ISCONN = -2, INPROGRESS = -3, ALREADY = -4, AGAIN = -5, ADDRINUSE = -6, ADDRNOTAVAIL = -7,
		BADF = -8, CONNREFUSED = -9, INTR = -10, NOTSOCK = -11, PIPE = -12, WOULDBLOCK = -13, INVAL = -14, TIMEDOUT = -15
	};
	Int m_lastError;
	bool m_fail; Int m_failStatus;
	std::deque<StandInDatagram> inbound, outbound;
	UDP() : m_lastError(0), m_fail(false), m_failStatus(0) {}
	Int Write(const unsigned char *msg, UnsignedInt len, UnsignedInt IP, UnsignedShort port)
	{
		if ((IP == 0) || (port == 0)) return ADDRNOTAVAIL;
		ClearStatus();
		if (m_fail) { m_lastError = m_failStatus; return -1; }
		StandInDatagram d; d.bytes.assign(msg, msg + len); d.addr = IP; d.port = port;
		outbound.push_back(d);
		return (Int)len;
	}
	Int Read(unsigned char *msg, UnsignedInt len, sockaddr_in *from)
	{
		if (m_fail) { m_lastError = m_failStatus; return -1; }
		if (inbound.empty()) return 0;
		StandInDatagram d = inbound.front(); inbound.pop_front();
		unsigned n = d.bytes.size() < len ? (unsigned)d.bytes.size() : len;
		for (unsigned i = 0; i < n; ++i) msg[i] = d.bytes[i];
		if (from) { from->sin_addr.s_addr = htonl(d.addr); from->sin_port = htons(d.port); }
		return (Int)n;
	}
	sockStat GetStatus(void) { return (sockStat)m_lastError; }
	void ClearStatus(void) { m_lastError = OK; }
	Int AllowBroadcasts(Bool) { return 1; }
};
#endif
