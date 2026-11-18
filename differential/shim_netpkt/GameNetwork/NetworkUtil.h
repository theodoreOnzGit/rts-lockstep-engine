/* Stand-in for GameNetwork/NetworkUtil.h (upstream's is the lower-case networkutil.h, which includes NetworkInterface.h and
** through it most of the network layer), differential tests only (differential/wrap_draft_netpkt.cpp, #215): the same
** declarations, minus the debug build's dumpBufferToLog. */
#ifndef SHIM_NETPKT_NETWORKUTIL_H
#define SHIM_NETPKT_NETWORKUTIL_H
#include "GameNetwork/NetworkDefs.h"
#include "GameNetwork/NetCommandMsg.h"
#include "Common/AsciiString.h"

UnsignedInt ResolveIP(AsciiString host);
UnsignedShort GenerateNextCommandID();
Bool DoesCommandRequireACommandID(NetCommandType type);
Bool CommandRequiresAck(NetCommandMsg *msg);
Bool CommandRequiresDirectSend(NetCommandMsg *msg);
Bool IsCommandSynchronized(NetCommandType type);
AsciiString GetAsciiNetCommandType(NetCommandType type);

#define LOGBUFFER(buf, len) {}
#endif
