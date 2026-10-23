/* Differential-test driver for PoisonedBehavior (p1w13-sonnet, batch R16; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PoisonedBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the flammable driver's stand-ins (differential/l9_flammable_n.inc): an Object with a drawable (its tint
** calls are l9_helpers.inc's hlog records) and maybe a body (MBody, l9_body_m.inc: it logs the damage it is given; here it also keeps the damage FX
** override of the last), the game logic whose frame the test sets and whose wake recorder counts the calls setWakeFrame makes, and the REAL
** UpdateModule, INI and Xfer. */
/* PoisonedBehavior.cpp uses the macro min of BaseType.h (as SlowDeathBehavior.cpp does: l9_slow_death_q.inc) */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PoisonedBehavior.cpp"
#undef min

namespace {

ModuleData *l9r16pRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = PoisonedBehavior::friend_newModuleData(&ini);
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

/* the flammable driver's body, keeping the damage FX override of the last damage */
class R16Body : public MBody {
public:
	void attemptDamage(DamageInfo *info) { m_lastFx = (int)info->in.m_damageFXOverride; MBody::attemptDamage(info); }
	int m_lastFx = -1;
};

std::vector<unsigned char> g_r16pSaved;
Drawable g_r16pDrawable;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a PoisonedBehavior and make the module on an Object with a drawable, a body (health cfg[1] float bits,
** maximum cfg[2]; none if cfg[3] is 0), at the frame cfg[0]. The data dump goes first (the two fields). Then the constructor's wake calls. Then `n`
** ops of 5 words [op, a, b, c, d]: 0 update()  1 onDamage (damage type a, actual damage dealt b as float bits, death type c)  2 onHealing  4 the
** frame = a  5 the object is effectively dead = a  6 xfer save  7 xfer load of the last save  8 xfer CRC. Each op appends: the CATCH_ALL status,
** the result (the sleep time update() returned), the effect log (count, then each: "damage <object> <amount> <type> <death> <source>"), the tint
** calls (count, then each status and whether set), the number of wake calls and the frame of the last, the xfer status, bytes and CRC, the module's
** four state words, the body's health bits and the damage FX override of the last damage (-1: none). */
static int l9r16pRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r16pSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r16pRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const PoisonedBehaviorModuleData *pd = (const PoisonedBehaviorModuleData *)d;
		putU32(pd->m_poisonDamageIntervalData);
		putU32(pd->m_poisonDurationData);
	}
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_drawable = &g_r16pDrawable;
	R16Body body;
	body.m_owner = obj;
	memcpy(&body.m_health, &cfg[1], 4);
	memcpy(&body.m_max, &cfg[2], 4);
	obj->m_body = cfg[3] ? &body : 0;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	g_hlog.clear();
	PoisonedBehavior *m = 0;
	status = 0;
	try { m = new PoisonedBehavior(obj, d); }
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
					memcpy(&info.out.m_actualDamageDealt, &o[2], 4);
					info.in.m_deathType = (DeathType)o[3];
					m->onDamage(&info);
					break;
				}
				case 2: {
					DamageInfo info;
					m->onHealing(&info);
					break;
				}
				case 4: gl.m_frame = o[1]; break;
				case 5: obj->m_effDead = o[1] != 0; break;
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r16pSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_r16pSaved;
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
		std::vector<std::pair<unsigned, unsigned> > tints;
		for (size_t j = 0; j < g_hlog.size(); ++j) if (g_hlog[j].first == 7 || g_hlog[j].first == 8) tints.push_back(std::make_pair(g_hlog[j].second, g_hlog[j].first == 7 ? 1u : 0u));
		putU32((unsigned)tints.size());
		for (size_t j = 0; j < tints.size(); ++j) { putU32(tints[j].first); putU32(tints[j].second); }
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
		putU32(m->m_poisonDamageFrame);
		putU32(m->m_poisonOverallStopFrame);
		putU32(realBits(m->m_poisonDamageAmount));
		putU32((unsigned)m->m_deathType);
		putU32(obj->m_body ? realBits(body.m_health) : 0);
		putI32(body.m_lastFx);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	obj->m_drawable = 0;
	obj->m_body = 0;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r16p_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len,
	char *msg, int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r16pRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
