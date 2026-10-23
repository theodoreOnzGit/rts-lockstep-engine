/* Differential-test driver for SupplyWarehouseCripplingBehavior (p1w13-sonnet, batch R17; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SupplyWarehouseCripplingBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the flammable driver's stand-ins (differential/l9_flammable_n.inc): an Object with maybe a body (MBody,
** l9_body_m.inc: it logs the healing it is given) and maybe a dock (the contain driver's CtDock, whose setDockCrippled logs here), the REAL
** Object::attemptHealing (Object.cpp, l9_auto_heal_m.inc), the game logic whose frame the test sets and whose wake recorder counts the calls
** setWakeFrame makes, and the REAL UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SupplyWarehouseCripplingBehavior.cpp"

namespace {

ModuleData *l9r17Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SupplyWarehouseCripplingBehavior::friend_newModuleData(&ini);
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

/* the contain driver's dock, whose setDockCrippled logs */
struct R17Dock : public CtDock {
	R17Dock(Thing *t, const ModuleData *d) : CtDock(t, d) {}
	virtual void setDockCrippled(Bool setting) { ulog("crippled %d %d", (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid, setting ? 1 : 0); }
};

std::vector<unsigned char> g_r17Saved;
Drawable g_r17Drawable;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SupplyWarehouseCripplingBehavior and make the module on an Object with a drawable, a body (health
** cfg[1] float bits, maximum cfg[2]; none if cfg[3] is 0) and a dock (none if cfg[4] is 0), at the frame cfg[0]. The data dump goes first (the three
** fields). Then the constructor's wake calls. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 onDamage (damage type a)
** 2 onBodyDamageStateChange (old state a, new state b)  4 the frame = a  5 the body's health = a (float bits)  6 xfer save  7 xfer load of the last
** save  8 xfer CRC. Each op appends: the CATCH_ALL status, the result (the sleep time update() returned), the effect log (count, then each:
** "heal <object> <amount> <source>", "crippled <object> <0|1>"), the number of wake calls and the frame of the last, the xfer status, bytes and
** CRC, the module's two state words and the body's health bits. */
static int l9r17RunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r17Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r17Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const SupplyWarehouseCripplingBehaviorModuleData *pd = (const SupplyWarehouseCripplingBehaviorModuleData *)d;
		putU32(pd->m_selfHealSupression);
		putU32(pd->m_selfHealDelay);
		putU32(realBits(pd->m_selfHealAmount));
	}
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_drawable = &g_r17Drawable;
	MBody body;
	body.m_owner = obj;
	memcpy(&body.m_health, &cfg[1], 4);
	memcpy(&body.m_max, &cfg[2], 4);
	obj->m_body = cfg[3] ? &body : 0;
	ModuleData dockData;
	R17Dock dock(obj, &dockData);
	BehaviorModule *behaviors[2] = { cfg[4] ? (BehaviorModule *)&dock : 0, 0 };
	obj->m_behaviors = behaviors;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	g_hlog.clear();
	SupplyWarehouseCripplingBehavior *m = 0;
	status = 0;
	try { m = new SupplyWarehouseCripplingBehavior(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0;
		return 5;
	}
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			g_hlog.clear();
			gl.m_awakenCalls = 0;
			gl.m_awakenFrame = 0;
			try {
				switch (o[0]) {
				case 0: res = (unsigned)m->update(); break;
				case 1: {
					DamageInfo info;
					info.in.m_damageType = (DamageType)o[1];
					m->onDamage(&info);
					break;
				}
				case 2: {
					DamageInfo info;
					m->onBodyDamageStateChange(&info, (BodyDamageType)o[1], (BodyDamageType)o[2]);
					break;
				}
				case 4: gl.m_frame = o[1]; break;
				case 5: memcpy(&body.m_health, &o[1], 4); break;
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r17Saved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_r17Saved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				case 8: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				default: break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
			putI32(st);
			putU32(res);
		}
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
		putU32(m->m_healingSupressedUntilFrame);
		putU32(m->m_nextHealingFrame);
		putU32(obj->m_body ? realBits(body.m_health) : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	obj->m_drawable = 0;
	obj->m_body = 0;
	obj->m_behaviors = 0;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r17_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r17RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
