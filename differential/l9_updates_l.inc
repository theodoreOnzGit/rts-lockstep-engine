/* Differential-test driver for AutoDepositUpdate (p1w11-sonnet, update batch L; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoDepositUpdate.cpp   (#included, with one mechanical edit, see below)
** with its header unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_crates_f.inc, whose money, score keeper and players it
** shares, and l9_powers_h.inc, whose player list and local player it uses) on the stand-ins of differential/shim_module: an Object with a controlling player,
** status bits, a construction percentage, kinds, a geometry and a position, three players (a colour, completed upgrades, a money that logs the deposit, a
** score keeper that logs), a game text whose labels the floating text remembers, an in-game UI that logs the floating text, a game logic whose frame the
** test sets, and the REAL UpdateModule, UpgradeCenter, INI, Xfer and (RandomValue.cpp, compiled by wrap_random_value.cpp) client random values.
**
** The one mechanical edit (build.rs, mod_gen/AutoDepositUpdate_static.cpp, asserted to apply once): the function-local `static const UpgradeTemplate *`
** inside the loop of getUpgradedSupplyBoost -- initialised once per process, an upstream defect that Rust keeps per World -- becomes a global and a flag
** that the driver resets for each case, as for MoneyCrateCollide. */
#include <limits.h>
#include <setjmp.h>
#include <signal.h>
namespace { sigjmp_buf g_l9ulJmp; volatile sig_atomic_t g_l9ulArmed = 0; void l9ulCrashHandler(int) { if (g_l9ulArmed) siglongjmp(g_l9ulJmp, 1); } }
namespace { const UpgradeTemplate *g_adStatic = 0; bool g_adStaticInit = false; }
#include "AutoDepositUpdate_static.cpp"

namespace {

ModuleData *l9ulRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = AutoDepositUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_ulSaved;
GameTextInterface g_ulText;

}  // namespace

GameTextInterface *TheGameText = 0;
Bool Object::isLocallyControlled() const { return m_player == ThePlayerList->getLocalPlayer(); }
void InGameUI::addFloatingText(const UnicodeString &text, const Coord3D *pos, Color color)
{
	ulog("float %s %d %u,%u,%u %08x", text.m_label, text.m_arg, realBits(pos->x), realBits(pos->y), realBits(pos->z), (unsigned)color);
}

extern "C" {

/* Read `filename` (block `D`) as the data of an AutoDepositUpdate and make the module on an Object (id 1) whose controlling player is player cfg[0] of three
** (0xFFFFFFFF none). `cfg`: [the controlling player, the local player (0xFFFFFFFF none), the game logic's frame, the client random seed, the object's kinds
** (bit 0 STRUCTURE), the geometry's major and minor radius (float bits, two words), the construction percentage (float bits)]; the players' colours are
** 0x00112233 * (index + 1) and player 0 is the neutral player. The data dump goes first (the four fields, the pairs of UpgradedBoost). Then the constructor's
** nothing: `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 awardInitialCaptureBonus(player a - 1; 0 is NULL)  2 the frame = a  3 the controlling
** player = a - 1 (0 none)  4 the construction percentage = float bits a  5 the object's status: STEALTHED = a & 1, DETECTED = a & 2  6 the local player
** = a - 1 (0 none)  7 the three players' completed upgrades = m (a..d, 128 bits)  8 the geometry = (a, b) float bits   10 xfer save
** 11 xfer load of the last save  12 xfer CRC  13 the position = (a, b, c) float bits. Each op appends: the CATCH_ALL status, the sleep time update()
** returned, the effect log (count, then each as a string: "deposit <player> <amount> <sound>", "earned <player> <amount>", "float <label> <amount>
** <position> <colour>"), the xfer status, bytes and CRC, and the state words (deposit-on frame, award flag, initialized). */
static int l9ulRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_ulSaved.clear();
	g_adStaticInit = false;
	g_adStatic = 0;
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9ulRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const AutoDepositUpdateModuleData *ad = (const AutoDepositUpdateModuleData *)d;
		putU32(ad->m_depositFrame);
		putI32(ad->m_depositAmount);
		putI32(ad->m_initialCaptureBonus);
		putU32(ad->m_isActualMoney ? 1 : 0);
		putU32((unsigned)ad->m_upgradeBoost.size());
		for (std::list<upgradePair>::const_iterator it = ad->m_upgradeBoost.begin(); it != ad->m_upgradeBoost.end(); ++it) {
			putStr(it->type.c_str(), (int)it->type.size());
			putI32(it->amount);
		}
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_money.m_idx = i;
		players[i].m_score.m_idx = i;
		players[i].m_color = 0x00112233 * (i + 1);
		players[i].m_defaultTeam = (Team *)(size_t)(101 + i);
		players[i].m_upgradesCompleted = UpgradeMaskType();
		g_spPlayerPtrs[i] = &players[i];
	}
	g_spPlayerCount = 3;
	g_spLocal = (cfg[1] != 0xFFFFFFFFu && cfg[1] < 3) ? &players[cfg[1]] : 0;
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	if (cfg[4] & 1) obj->m_kindOf.set(KINDOF_STRUCTURE);
	{ Real maj, min; memcpy(&maj, &cfg[5], 4); memcpy(&min, &cfg[6], 4); obj->m_geometry.setMajorRadius(maj); obj->m_geometry.setMinorRadius(min); }
	memcpy(&obj->m_constructionPercent, &cfg[7], 4);
	obj->m_pos.x = 11.0f; obj->m_pos.y = 22.0f; obj->m_pos.z = 5.0f;
	GameLogic gl;
	gl.m_frame = cfg[2];
	TheGameLogic = &gl;
	ThePlayerList = &g_spList;
	TheGameText = &g_ulText;
	TheInGameUI = &g_cfUi;
	InitRandom(cfg[3]);
	g_events.clear();
	g_ulog.clear();
	AutoDepositUpdate *m = new AutoDepositUpdate(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		g_ulog.clear();
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
			case 1: m->awardInitialCaptureBonus(o[1] ? &players[o[1] - 1] : 0); break;
			case 2: gl.m_frame = o[1]; break;
			case 3: obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 4: memcpy(&obj->m_constructionPercent, &o[1], 4); break;
			case 5: { ObjectStatusMaskType sm; if (o[1] & 1) sm.set(OBJECT_STATUS_STEALTHED); if (o[1] & 2) sm.set(OBJECT_STATUS_DETECTED); obj->m_status = sm; break; }
			case 6: g_spLocal = o[1] ? &players[o[1] - 1] : 0; break;
			case 7: for (int p = 0; p < 3; ++p) players[p].m_upgradesCompleted = mask; break;
			case 8: { Real maj, min; memcpy(&maj, &o[1], 4); memcpy(&min, &o[2], 4); obj->m_geometry.setMajorRadius(maj); obj->m_geometry.setMinorRadius(min); break; }
			case 10: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_ulSaved.assign(bytes, bytes + blen); break;
			case 11: {
				std::vector<unsigned char> b = g_ulSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 12: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 13: memcpy(&obj->m_pos.x, &o[1], 4); memcpy(&obj->m_pos.y, &o[2], 4); memcpy(&obj->m_pos.z, &o[3], 4); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_depositOnFrame);
		putU32(m->m_awardInitialCaptureBonus ? 1 : 0);
		putU32(m->m_initialized ? 1 : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	ThePlayerList = 0;
	TheGameText = 0;
	TheInGameUI = 0;
	g_spLocal = 0;
	return 0;
}

/* The crash trap: a planted bug in the Rust side can steer a case past an input that is undefined upstream (the Rust records it and the case is never sent), and
** the C++ then dereferences it. Rather than let that kill the process, the C++ run is made a mismatch: status 99. (Not a way to hide a crash in the C++: the
** unplanted run has no such case, and the test fails on 99.) */
int ref_l9ul_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	struct sigaction sa, oldSegv, oldBus;
	memset(&sa, 0, sizeof sa);
	sa.sa_handler = l9ulCrashHandler;
	sigemptyset(&sa.sa_mask);
	sigaction(SIGSEGV, &sa, &oldSegv);
	sigaction(SIGBUS, &sa, &oldBus);
	g_l9ulArmed = 1;
	int rc;
	if (sigsetjmp(g_l9ulJmp, 1) == 0) {
		rc = l9ulRunInner(filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
	} else {
		rc = 99;
		*len = 0;
		TheGameLogic = 0; ThePlayerList = 0; TheGameText = 0; TheInGameUI = 0; g_spLocal = 0;
	}
	g_l9ulArmed = 0;
	sigaction(SIGSEGV, &oldSegv, 0);
	sigaction(SIGBUS, &oldBus, 0);
	return rc;
}

}  /* extern "C" */
