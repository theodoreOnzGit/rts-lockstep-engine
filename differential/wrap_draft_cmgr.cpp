/* Differential-test wrapper for ConnectionManager's accessors (#215, group network3, upstream commit 0a05454d), src/drafts/network/connection_manager.rs:
** getPacketRouterFallbackSlot, getPacketRouterSlot, areAllQueuesEmpty, canILeave, getMaximumLatency, getMinimumFps, isPacketRouter, getSlotAverageFPS,
** getNextPacketRouterSlot and getPacketArrivalCushion, cut verbatim by build.rs (drafts_network3) into OUT_DIR/draft_network3_gen:
**   cm_a.inc  ConnectionManager.cpp, getPacketRouterFallbackSlot to the doc comment of handleLocalPlayerLeaving;
**   cm_b.inc  getMaximumLatency to the doc comment of processFrameTick (getMaximumLatency, getMinimumFps, getMinimumCushion: getMinimumCushion is NOT called,
**             FrameMetrics.cpp is not in this build);
**   cm_c.inc  isPacketRouter, getAverageFPS (not called) and getSlotAverageFPS, to the debug-only printer;
**   cm_d.inc  getNextPacketRouterSlot (to requestFrameDataResend);   cm_e.inc  getPacketArrivalCushion (to sendChat).
** The whole of differential/wrap_draft_conn.cpp is #included first (the verified network2 environment: the real ConnectionManager.h, Connection.cpp, the Transport and
** UDP stand-ins, and the three accessors it cuts, isPlayerConnected, getLocalPlayerID and getNumPlayers, which are tested here too); the manager is a zero-filled
** block (its constructor is not in any cut), as the game's allocator would give it, with the connections real Connection objects.
** Entry point (extern "C", tests/differential_draft_cmgr.rs): ref_cm_run (programs; ops at the function). */
#include "wrap_draft_conn.cpp"
#include "cm_a.inc"
#include "cm_b.inc"
#include "cm_c.inc"
#include "cm_d.inc"
#include "cm_e.inc"

extern "C" {

/* in: [localSlot, routerSlot, fallback x8, fps x8, latency bits x8, smallestCushion, ops...];
**   1 connection(slot, mode 0 none 1 present 2 present and quitting)   2 queue a keep-alive command on a connection (slot, player, frame, id, relay)
**   3 set m_localSlot   4 set m_packetRouterSlot   5 set m_packetRouterFallback[i] (i, v)   6 set m_fpsAverages[i] (i, v)   7 set m_latencyAverages[i] (i, bits)
**   8 set m_smallestPacketArrivalCushion   10 getPacketRouterFallbackSlot(n)   11 getPacketRouterSlot   12 areAllQueuesEmpty   13 canILeave
**   14 getMaximumLatency (bits)   15 getMinimumFps   16 isPacketRouter   17 getSlotAverageFPS(slot)   18 getNextPacketRouterSlot(id)
**   19 getPacketArrivalCushion   20 isPlayerConnected(id)   21 getNumPlayers   22 getLocalPlayerID */
int ref_cm_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 0};
	Words o;
	g_now = 0;
	ConnectionManager *cm = (ConnectionManager *)calloc(1, sizeof(ConnectionManager));
	cm->m_localSlot = r.next(); cm->m_packetRouterSlot = r.next();
	for (int i = 0; i < MAX_SLOTS; ++i) cm->m_packetRouterFallback[i] = r.next();
	for (int i = 0; i < MAX_SLOTS; ++i) cm->m_fpsAverages[i] = (Int)r.next();
	for (int i = 0; i < MAX_SLOTS; ++i) { unsigned b = r.next(); memcpy(&cm->m_latencyAverages[i], &b, 4); }
	cm->m_smallestPacketArrivalCushion = r.next();
	while (r.i < r.n) {
		unsigned op = r.next();
		emit(o, 0xA5A5A5A5u); emit(o, op);
		switch (op) {
		case 1: {
			unsigned slot = r.next() % MAX_SLOTS, mode = r.next();
			if (cm->m_connections[slot]) { delete cm->m_connections[slot]; cm->m_connections[slot] = NULL; }
			if (mode != 0) { Connection *c = newInstance(Connection); c->init(); if (mode == 2) c->setQuitting(); cm->m_connections[slot] = c; }
			break; }
		case 2: {
			unsigned slot = r.next() % MAX_SLOTS; unsigned w[4] = { (unsigned)NETCOMMANDTYPE_KEEPALIVE, r.next(), r.next(), r.next() }; unsigned relay = r.next();
			if (cm->m_connections[slot]) { Rd r2 = {w, 4, 0}; NetCommandMsg *m = makeMsg(r2); cm->m_connections[slot]->sendNetCommandMsg(m, (UnsignedByte)relay); }
			break; }
		case 3: cm->m_localSlot = r.next(); break;
		case 4: cm->m_packetRouterSlot = r.next(); break;
		case 5: { unsigned i = r.next() % MAX_SLOTS; cm->m_packetRouterFallback[i] = r.next(); break; }
		case 6: { unsigned i = r.next() % MAX_SLOTS; cm->m_fpsAverages[i] = (Int)r.next(); break; }
		case 7: { unsigned i = r.next() % MAX_SLOTS; unsigned b = r.next(); memcpy(&cm->m_latencyAverages[i], &b, 4); break; }
		case 8: cm->m_smallestPacketArrivalCushion = r.next(); break;
		case 10: emit(o, cm->getPacketRouterFallbackSlot((Int)r.next())); break;
		case 11: emit(o, cm->getPacketRouterSlot()); break;
		case 12: emit(o, cm->areAllQueuesEmpty() ? 1u : 0u); break;
		case 13: emit(o, cm->canILeave() ? 1u : 0u); break;
		case 14: { Real v = cm->getMaximumLatency(); unsigned b; memcpy(&b, &v, 4); emit(o, b); break; }
		case 15: { Int a = 0, b = 0; cm->getMinimumFps(a, b); emit(o, (unsigned)a); emit(o, (unsigned)b); break; }
		case 16: emit(o, cm->isPacketRouter() ? 1u : 0u); break;
		case 17: emit(o, (unsigned)cm->getSlotAverageFPS((Int)r.next())); break;
		case 18: emit(o, cm->getNextPacketRouterSlot(r.next())); break;
		case 19: emit(o, cm->getPacketArrivalCushion()); break;
		case 20: emit(o, cm->isPlayerConnected((Int)r.next()) ? 1u : 0u); break;
		case 21: emit(o, (unsigned)cm->getNumPlayers()); break;
		case 22: emit(o, cm->getLocalPlayerID()); break;
		default: break;
		}
		emit(o, cm->m_localSlot); emit(o, cm->m_packetRouterSlot); emit(o, cm->m_smallestPacketArrivalCushion);
		for (int i = 0; i < MAX_SLOTS; ++i) { emit(o, cm->m_connections[i] ? 1u : 0u); emit(o, cm->m_connections[i] && cm->m_connections[i]->isQuitting() ? 1u : 0u); }
	}
	if ((int)o.size() > cap) return (int)o.size();
	memcpy(out, o.data(), o.size() * sizeof(unsigned));
	return (int)o.size();
}

}
