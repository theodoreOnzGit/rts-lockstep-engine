/* Differential-test driver for OverchargeBehavior (p1w13-sonnet, batch R24; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/OverchargeBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the module's object (id 1) with a body (the module build's MBody, whose
** attemptDamage logs), up to two power plant updates whose extendRods logs, a controlling player of three (or none), whose addPowerBonus and
** removePowerBonus log (l9_upgrades_c.inc), the local player; the in-game UI's message and the radar's createEvent (lines, here); the game logic,
** whose wake calls the test counts; and the REAL UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/OverchargeBehavior.cpp"

void (*g_uiMessageHook)(const char *) = 0;
void Radar::createEvent( const Coord3D *world, RadarEventType type, float secondsToLive )
{
	ulog("radarevent %08x %08x %08x %d %08x", realBits(world->x), realBits(world->y), realBits(world->z), (int)type, realBits(secondsToLive));
}

namespace {

ModuleData *l9r24Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = OverchargeBehavior::friend_newModuleData(&ini);
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

/* a behaviour with a PowerPlantUpdateInterface whose extendRods logs its slot */
class R24Plant : public TestBehavior, public PowerPlantUpdateInterface {
public:
	R24Plant(Thing *t, const ModuleData *d) : TestBehavior(t, d), m_slot(0) {}
	virtual PowerPlantUpdateInterface *getPowerPlantUpdateInterface() { return this; }
	virtual void extendRods(Bool extend) { ulog("extendrods %d %d", m_slot, (int)extend); }
	int m_slot;
};

std::vector<unsigned char> g_r24Saved;
void r24Message(const char *label) { ulog("message %s", label); }
Player *g_r24Local = 0;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of an OverchargeBehavior and make the module on object 1 at frame 0. `cfg` (5 words): [0 the controlling
** player + 1 (0 none), 1 the local player + 1 (0 none), 2 the number of power plant updates (0-2), 3 the health and 4 the maximum (float bits)]. The
** object's modules are, in order: the overcharge (slot 0), the body (slot 1), the power plants (slots 2 and 3). The data dump goes first, then the
** constructor's wake calls. Then `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 toggle  2 enable(a & 1)  3 isOverchargeActive  4 onDelete
** 5 onCapture(player a - 1, player b - 1; 0 none)  6 onDamage  7 xfer save  8 xfer load of the last save  9 xfer CRC  10 loadPostProcess  11 the
** health = a (float bits)  12 the local player = a - 1  13 the controlling player = a - 1  14 the frame = a  15 the object disabled = a & 1. Each op
** appends: the CATCH_ALL status, the result (update's sleep time, or isOverchargeActive), the effect log (count, then each), the wake calls (count,
** the last frame), the xfer status, bytes and CRC, m_overchargeActive and the body's health. */
int ref_l9r24_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r24Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r24Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const OverchargeBehaviorModuleData *od = (const OverchargeBehaviorModuleData *)d;
		putU32(realBits(od->m_healthPercentToDrainPerSecond));
		putU32(realBits(od->m_notAllowedWhenHealthBelowPercent));
	}
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	Player players[3];
	for (int i = 0; i < 3; ++i) players[i].m_idx = i;
	g_spPlayerPtrs[0] = &players[0]; g_spPlayerPtrs[1] = &players[1]; g_spPlayerPtrs[2] = &players[2];
	g_spPlayerCount = 3;
	ThePlayerList = &g_spList;
	g_spLocal = cfg[1] ? &players[(cfg[1] - 1) % 3] : 0;
	TheInGameUI = &g_spUi;
	g_uiMessageHook = r24Message;
	Radar radar;
	TheRadar = &radar;
	Object *me = newObject();
	me->m_oid = 1;
	me->m_player = cfg[0] ? &players[(cfg[0] - 1) % 3] : 0;
	me->m_pos.x = 10.0f;
	me->m_pos.y = -2.5f;
	me->m_pos.z = 1.0f;
	MBody body;
	body.m_owner = me;
	memcpy(&body.m_health, &cfg[3], 4);
	memcpy(&body.m_max, &cfg[4], 4);
	me->m_body = &body;
	ModuleData plain;
	R24Plant plants[2] = { R24Plant(me, &plain), R24Plant(me, &plain) };
	BehaviorModule *behaviors[4] = { 0, 0, 0, 0 };
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	OverchargeBehavior *m = new OverchargeBehavior(me, d);
	int nb = 0;
	behaviors[nb++] = m;
	for (unsigned k = 0; k < cfg[2] % 3; ++k) { plants[k].m_slot = 2 + (int)k; behaviors[nb++] = &plants[k]; }
	me->m_behaviors = behaviors;
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046) */
	m->friend_setNextCallFrame(gl.m_awakenCalls ? gl.m_awakenFrame : 1);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 11) memcpy(&body.m_health, &o[1], 4);
		if (o[0] == 12) g_spLocal = o[1] ? &players[(o[1] - 1) % 3] : 0;
		if (o[0] == 13) me->m_player = o[1] ? &players[(o[1] - 1) % 3] : 0;
		if (o[0] == 14) gl.m_frame = o[1];
		if (o[0] == 15) me->m_disabled = (o[1] & 1) != 0;
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
			case 0: res = (unsigned)m->update(); break;
			case 1: m->toggle(); break;
			case 2: m->enable((o[1] & 1) != 0); break;
			case 3: res = m->isOverchargeActive() ? 1 : 0; break;
			case 4: m->onDelete(); break;
			case 5: m->onCapture(o[1] ? &players[(o[1] - 1) % 3] : 0, o[2] ? &players[(o[2] - 1) % 3] : 0); break;
			case 6: { DamageInfo info; m->onDamage(&info); break; }
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r24Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r24Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 10: m->loadPostProcess(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		/* the op's wake call as GameLogic::friend_awakenUpdateModule applies it (GameLogic.cpp:2983-3024; the batch R20 correction) */
		if (gl.m_awakenCalls) {
			UnsignedInt now = gl.m_frame, when = gl.m_awakenFrame, next = m->friend_getNextCallFrame();
			if (when != next && !(now > 0 && next == now && when == now + 1)) m->friend_setNextCallFrame(when);
		}
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_overchargeActive ? 1 : 0);
		putU32(realBits(body.m_health));
	}
	*len = finish(buf, cap);
	me->m_behaviors = 0;
	me->m_body = 0;
	me->m_player = 0;
	delete m;
	delete d;
	delete me;
	g_uiMessageHook = 0;
	TheRadar = 0;
	TheInGameUI = 0;
	ThePlayerList = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
