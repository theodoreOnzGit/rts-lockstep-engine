/* Differential-test driver for PropagandaTowerBehavior (p1w13-sonnet, batch R37; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaTowerBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: five objects (ids 1-5; kinds, status bits, effectively dead, the disabled
** bits, whether the weapon set has a damage weapon, maybe the module build's MBody (health 100 of 200; attemptHealing a line), maybe a container,
** the weapon bonus bits, which the module's calls set and clear), the tower (object 1, maybe a player of three: the neutral player is the first),
** the local player, the players' and the tower's completed upgrades, the upgrade center (the upgrade build's, with Upgrade_A (a player upgrade) and
** Upgrade_B (an object upgrade) loaded by the test), the scan (the objects the test lists, which pass the filters; the call a line), the FX lists
** (FX_A, FX_B; doFXObj as the log's "fx" entries), the REAL Object::attemptHealingFromSoleBenefactor (mod_gen/ObjectHealing.inc),
** UpdateModule, INI and Xfer; the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaTowerBehavior.cpp"

namespace {

ModuleData *l9r37Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = PropagandaTowerBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r37Saved;
UpdateModule *g_r37Updating = 0;
void r37Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r37Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *g_r37Objs[5];
Object *r37Find(ObjectID id) { return (id >= 1 && id <= 5) ? g_r37Objs[id - 1] : 0; }
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r37Kinds[6] = { KINDOF_SCORE, KINDOF_SCORE_CREATE, KINDOF_SCORE_DESTROY, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_VEHICLE, KINDOF_PORTABLE_STRUCTURE };
const ObjectStatusTypes g_r37Status[4] = { OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_DETECTED };
UpgradeMaskType r37Mask(unsigned bits)
{
	UpgradeMaskType m;
	const char *names[2] = { "Upgrade_A", "Upgrade_B" };
	for (int k = 0; k < 2; ++k) if (bits >> k & 1) {
		const UpgradeTemplate *u = TheUpgradeCenter->findUpgrade(AsciiString(names[k]));
		if (u) m.set(u->getUpgradeMask());
	}
	return m;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a PropagandaTowerBehavior and make it on object 1 of five at frame cfg[0]. `cfg` (24 words; 1-10 also
** settable by op 2): 1 the tower's player + 1 (0 none), 2 the local player (3 none), 3 the players' completed upgrades (bits 2p and 2p+1:
** Upgrade_A, Upgrade_B), 4 the tower's own (bits 0-1), 5-9 objects 1-5: bits 0-5 their kinds (of g_r37Kinds), 6-9 status bits (of g_r37Status),
** 10 effectively dead, 11 DISABLED_HELD, 12 DISABLED_EMP, 13 a damage weapon, 14 a body, 15-17 the object containing it (0 none); 10 the scan
** (nibbles of object numbers 1-5, 0 ends). The data dump goes first, then the constructor's (and onObjectCreated's) effects and wake calls. Then
** `n` ops of 4 words [op, a, b, c]: 0 update()  1 the frame = a  2 cfg[a] = b  3 onDie  4 onDelete  5 onCapture(the tower's player, player a; 3
** NULL)  6 xfer save  7 xfer load of the last save  8 xfer CRC. Each op appends: the CATCH_ALL status, the result, the effect log, the wake calls
** (count, last frame), the xfer status, bytes and CRC, the state (the last scan frame, whether the upgrade was found, the tracked ids from the
** head), and per object the weapon bonus bits and the health. */
int ref_l9r37_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r37Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r37Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const PropagandaTowerBehaviorModuleData *pd = (const PropagandaTowerBehaviorModuleData *)d;
		putU32(realBits(pd->m_scanRadius));
		putU32(pd->m_scanDelayInFrames);
		putU32(realBits(pd->m_autoHealPercentPerSecond));
		putU32(realBits(pd->m_upgradedAutoHealPercentPerSecond));
		putI32(fxId(pd->m_pulseFX));
		putI32(fxId(pd->m_upgradedPulseFX));
		putStr(pd->m_upgradeRequired.str(), (int)pd->m_upgradeRequired.getLength());
		putU32(pd->m_affectsSelf ? 1 : 0);
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) { players[p].m_idx = p; teams[p].m_dummy = p + 1; teams[p].m_controller = &players[p]; }
	g_tbNeutral = &players[0];
	PlayerList playerList;
	ThePlayerList = &playerList;
	MBody bodies[5];
	for (int k = 0; k < 5; ++k) {
		g_r37Objs[k] = newObject();
		g_r37Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r37Objs[k];
		bodies[k].m_max = 200.0f;
		bodies[k].m_health = 100.0f;
	}
	Object *me = g_r37Objs[0];
	auto scene = [&]() {
		unsigned tp = cfg[1];
		me->m_team = (tp >= 1 && tp <= 3) ? &teams[tp - 1] : 0;
		me->m_player = (tp >= 1 && tp <= 3) ? &players[tp - 1] : 0;
		g_spLocal = cfg[2] < 3 ? &players[cfg[2]] : 0;
		for (int p = 0; p < 3; ++p) players[p].m_upgradesCompleted = r37Mask(cfg[3] >> (2 * p) & 3);
		me->m_objectUpgradesCompleted = r37Mask(cfg[4] & 3);
		for (int k = 0; k < 5; ++k) {
			Object *o = g_r37Objs[k];
			unsigned f = cfg[5 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 6; ++b) if (f >> b & 1) o->m_kindOf.set(g_r37Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_status.clear();
			for (int b = 0; b < 4; ++b) if (f >> (6 + b) & 1) o->m_status.set(g_r37Status[b]);
			o->m_effDead = (f >> 10 & 1) != 0;
			o->m_disabledBits = ((f >> 11 & 1) ? 1u << DISABLED_HELD : 0) | ((f >> 12 & 1) ? 1u << DISABLED_EMP : 0);
			o->m_disabled = o->m_disabledBits != 0;
			o->m_hasDamageWeapon = (f >> 13 & 1) != 0;
			o->m_body = (f >> 14 & 1) ? &bodies[k] : 0;
			unsigned c = f >> 15 & 7;
			o->m_containedBy = (c >= 1 && c <= 5) ? g_r37Objs[c - 1] : 0;
		}
		g_ahRange.clear();
		for (int k = 0; k < 5; ++k) { unsigned id = cfg[10] >> (4 * k) & 15; if (id == 0) break; if (id <= 5) g_ahRange.push_back(g_r37Objs[id - 1]); }
	};
	scene();
	g_ahRangeHook = r35ScanHook;
	g_modFindHook = r37Find;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r37Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	PropagandaTowerBehavior *m = new PropagandaTowerBehavior(me, d);
	m->onObjectCreated();
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 10) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
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
		try {
			switch (o[0]) {
			case 0: g_r37Updating = m; res = (unsigned)m->update(); break;
			case 3: { DamageInfo di; m->onDie(&di); break; }
			case 4: m->onDelete(); break;
			case 5: m->onCapture(me->m_player, o[1] < 3 ? &players[o[1]] : 0); break;
			case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r37Saved.assign(bytes, bytes + blen); break;
			case 7: {
				std::vector<unsigned char> b = g_r37Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 8: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r37Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_lastScanFrame);
		putU32(m->m_upgradeRequired ? 1 : 0);
		unsigned count = 0;
		for (ObjectTracker *t = m->m_insideList; t; t = t->next) ++count;
		putU32(count);
		for (ObjectTracker *t = m->m_insideList; t; t = t->next) putU32((unsigned)t->objectID);
		for (int k = 0; k < 5; ++k) {
			putU32((unsigned)g_r37Objs[k]->m_weaponBonusCondition);
			putU32(realBits(bodies[k].m_health));
		}
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_modFindHook = 0;
	g_tbNeutral = 0;
	g_spLocal = 0;
	ThePlayerList = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r37Objs[k];
		x->m_body = 0; x->m_team = 0; x->m_player = 0; x->m_containedBy = 0;
		delete x;
		g_r37Objs[k] = 0;
	}
	TheGameLogic = 0;
	ThePartitionManager = 0;
	return 0;
}

}  /* extern "C" */
