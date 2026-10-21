/* Differential-test driver for SpyVisionUpdate (p1w11-sonnet, update batch I; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpyVisionUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_updates_e.inc, whose game logic wake recorder it
** uses, and l9_powers_h.inc, whose players, player list and relationships it shares) on the stand-ins of differential/shim_module: three players (a
** relationship to the default team of each, and a setUnitsVisionSpied that logs), a player list of them, an Object with a controlling player, the
** game logic whose frame the test sets and whose wake recorder counts the calls setWakeFrame makes, and the REAL UpdateModule, UpgradeMux and
** UpgradeCenter, INI and Xfer. The same real class is the one the special power batch H's SpyVisionSpecialPower drives. */
#include <limits.h>      /* (UINT_MAX: the real PreRTS.h brings it) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpyVisionUpdate.cpp"

namespace {

ModuleData *l9uiRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SpyVisionUpdate::friend_newModuleData(&ini);
		} catch (...) {
			char buff[1024];
			sprintf(buff, "Error parsing INI file '%s' (Line: '%s')\n", ini.m_filename.str(), currentLine.str());
			throw INIException(buff);
		}
	} catch (...) {
		ini.unPrepFile();
		throw;
	}
	ini.unPrepFile();
	return data;
}

std::vector<unsigned char> g_uiSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SpyVisionUpdate and make the module on an Object whose controlling player is player cfg[0] of three
** (0xFFFFFFFF none). `cfg`: [the controlling player, the players' relationships to each player's default team (2 bits each, [player][default
** team of player]: 0 enemies, 1 neutral, 2 allies), the game logic's frame]. The data dump goes first (the mux, then needsUpgrade, selfPowered,
** the duration, the interval, the kinds as the indices of the set bits). Then the constructor's wake calls (count, frame of the last). Then `n`
** ops of 5 words [op, a, b, c, d]: 0 update()  1 activateSpyVision(a)  2 setDisabledUntilFrame(a)  3 onCapture(player a - 1, player b - 1; 0
** is NULL)  4 onDisabledEdge(a & 1)  5 onDelete  6 attemptUpgrade(m)  7 wouldUpgrade(m)  8 resetUpgrade(m)  9 isAlreadyUpgraded  10
** forceRefreshUpgrade (m = a..d as 128 bits)  11 the frame = a  12 the controlling player = a - 1 (0 none)  13 the relationships = a  14 xfer save
** 15 xfer load of the last save  16 xfer CRC. Each op appends: the CATCH_ALL status, the result (the sleep time update() returned, or the Bool of
** the upgrade calls), isAlreadyUpgraded(), the effect log (count, then each as a string: "spied <player> <setting> <by> <kinds>", "fx<id>",
** "remove <name>"), the number of wake calls and the frame of the last, the xfer status, bytes and CRC, and the state words (deactivate frame, active,
** reset flag, disabled-until frame). */
int ref_l9ui_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_uiSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9uiRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const SpyVisionUpdateModuleData *sd = (const SpyVisionUpdateModuleData *)d;
		putMux(sd->m_upgradeMuxData);
		putU32(sd->m_needsUpgrade ? 1 : 0);
		putU32(sd->m_selfPowered ? 1 : 0);
		putU32(sd->m_selfPoweredDuration);
		putU32(sd->m_selfPoweredInterval);
		std::string bits;
		for (int k = 0; k < KINDOF_COUNT; ++k) if (sd->m_spyOnKindof.test((KindOfType)k)) { char b[16]; sprintf(b, "%d,", k); bits += b; }
		putStr(bits.c_str(), (int)bits.size());
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_defaultTeam = (Team *)(size_t)(101 + i);
		g_spPlayerPtrs[i] = &players[i];
	}
	g_spPlayerCount = 3;
	for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_spMatrix[a][b] = (cfg[1] >> (2 * (3 * a + b))) & 3;
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	GameLogic gl;
	gl.m_frame = cfg[2];
	TheGameLogic = &gl;
	ThePlayerList = &g_spList;
	SpyVisionUpdate *m = new SpyVisionUpdate(obj, d);
	putU32((unsigned)gl.m_awakenCalls);        /* the constructor's wake calls, and the frame of the last */
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		g_ulog.clear();
		gl.m_awakenCalls = 0;
		gl.m_awakenFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 1: m->activateSpyVision(o[1]); break;
			case 2: m->setDisabledUntilFrame(o[1]); break;
			case 3: m->onCapture(o[1] ? &players[o[1] - 1] : 0, o[2] ? &players[o[2] - 1] : 0); break;
			case 4: m->onDisabledEdge((o[1] & 1) != 0); break;
			case 5: m->onDelete(); break;
			case 6: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 7: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 8: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 9: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 10: m->forceRefreshUpgrade(); break;
			case 11: gl.m_frame = o[1]; break;
			case 12: obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 13: for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_spMatrix[a][b] = (o[1] >> (2 * (3 * a + b))) & 3; break;
			case 14: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_uiSaved.assign(bytes, bytes + blen); break;
			case 15: {
				std::vector<unsigned char> b = g_uiSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putU32(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_deactivateFrame);
		putU32(m->m_currentlyActive ? 1 : 0);
		putU32(m->m_resetTimersNextUpdate ? 1 : 0);
		putU32(m->m_disabledUntilFrame);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	ThePlayerList = 0;
	return 0;
}

}  /* extern "C" */
