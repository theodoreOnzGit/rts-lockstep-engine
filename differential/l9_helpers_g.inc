/* Differential-test driver for two layer-9 object helpers (p1w11-sonnet, helper batch G; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/SubdualDamageHelper.cpp, ObjectDefectionHelper.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_crates_f.inc, whose MiscAudio and flash
** it uses, and l9_helpers.inc, whose ObjectHelper.cpp it needs) on the stand-ins of differential/shim_module: an Object with a body (for the
** subdual helper: a heal rate, a heal amount, whether it has subdual damage, and an attemptDamage that logs the damage it is given), a defector
** flag, firing and death, a Drawable that logs its flash, a game logic whose frame the test sets and whose wake recorder counts `setWakeFrame`'s
** calls, an audio manager that logs the events (l9_dies_d.inc), and the REAL UpdateModule, ObjectHelper and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/SubdualDamageHelper.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectDefectionHelper.cpp"

namespace {

class SubdBody : public TestBody {
public:
	SubdBody(Thing *t, const ModuleData *d) : TestBody(t, d), m_rate(0), m_amount(0), m_has(false) {}
	virtual void attemptDamage(DamageInfo *info)
	{
		ulog("attemptdamage %d %u %d %d", (int)info->in.m_damageType, realBits(info->in.m_amount), (int)info->in.m_sourceID, (int)info->in.m_deathType);
	}
	virtual UnsignedInt getSubdualDamageHealRate() const { return m_rate; }
	virtual Real getSubdualDamageHealAmount() const { return m_amount; }
	virtual Bool hasAnySubdualDamage() const { return m_has; }
	UnsignedInt m_rate;
	Real m_amount;
	Bool m_has;
};

std::vector<unsigned char> g_hgSaved;
Drawable g_hgDrawable;

}  // namespace

extern "C" {

/* Make the helper `kind` (0 SubdualDamageHelper, 1 ObjectDefectionHelper) on an Object (id 1) and run `n` ops of 6 words [op, a, b, c]. `cfg`: [the
** drawable is present, the object is an undetected defector, it is effectively dead, it is firing a weapon, the body's heal rate, its heal amount
** (float bits), it has subdual damage]. Ops of kind 0: 0 update() (a times, 0 is once)  1 notifySubdualDamage(float bits a)  2 the game logic's frame = a  3 the body's
** rate = a, amount = bits b, has = c  4 xfer save  5 xfer load of the last save  6 xfer CRC. Ops of kind 1: 0 update() (a times)  1 startDefectionTimer(a frames,
** fx b)  2 the frame = a  3 the object's state: bit 0 undetected defector, 1 dead, 2 firing, 3 drawable present  4, 5, 6 as above. The dump goes: the
** constructor's wake calls (count, frame of the last); then per op the CATCH_ALL status, the sleep time update() returned, the effect log (count,
** then each as a string), the number of wake calls and the frame of the last, the xfer status, its bytes and CRC, the module's state words (count,
** then each) and whether the object is an undetected defector. */
int ref_l9hg_run(int kind, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len)
{
	g_dump.clear();
	g_hgSaved.clear();
	*len = 0;
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_drawable = cfg[0] ? &g_hgDrawable : 0;
	obj->m_undetected = cfg[1] != 0;
	obj->m_effDead = cfg[2] != 0;
	if (cfg[3]) { ObjectStatusMaskType m; m.set(OBJECT_STATUS_IS_FIRING_WEAPON); obj->m_status = m; }
	SubdBody *body = new SubdBody(obj, new ModuleData);
	body->m_rate = cfg[4];
	memcpy(&body->m_amount, &cfg[5], 4);
	body->m_has = cfg[6] != 0;
	obj->m_body = body;
	GameLogic gl;
	TheGameLogic = &gl;
	TheAudio = &g_ddAudio;
	g_cfMisc.m_defectorTimerTickSound.setEventName(AsciiString("defectorTick"));
	g_cfMisc.m_defectorTimerDingSound.setEventName(AsciiString("defectorDing"));
	UpdateModuleData data;
	UpdateModule *m = 0;
	if (kind == 0) m = new SubdualDamageHelper(obj, &data);
	else m = new ObjectDefectionHelper(obj, &data);
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
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
		int sleep = 0;
		try {
			switch (o[0]) {
			case 0: for (unsigned r = 0; r < (o[1] ? o[1] : 1); ++r) sleep = (int)m->update(); break;   /* update() o[1] times (0 is once) */
			case 1: {
				if (kind == 0) { Real amount; memcpy(&amount, &o[1], 4); ((SubdualDamageHelper *)m)->notifySubdualDamage(amount); }
				else ((ObjectDefectionHelper *)m)->startDefectionTimer(o[1], o[2] != 0);
				break;
			}
			case 2: gl.m_frame = o[1]; break;
			case 3:
				if (kind == 0) { body->m_rate = o[1]; memcpy(&body->m_amount, &o[2], 4); body->m_has = o[3] != 0; }
				else {
					obj->m_undetected = (o[1] & 1) != 0;
					obj->m_effDead = (o[1] & 2) != 0;
					ObjectStatusMaskType sm;
					if (o[1] & 4) sm.set(OBJECT_STATUS_IS_FIRING_WEAPON);
					obj->m_status = sm;
					obj->m_drawable = (o[1] & 8) ? &g_hgDrawable : 0;
				}
				break;
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_hgSaved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_hgSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putI32(sleep);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind == 0) {
			putU32(1);
			putU32(((SubdualDamageHelper *)m)->m_healingStepCountdown);
		} else {
			ObjectDefectionHelper *h = (ObjectDefectionHelper *)m;
			putU32(4);
			putU32(h->m_defectionDetectionStart);
			putU32(h->m_defectionDetectionEnd);
			putU32(realBits(h->m_defectionDetectionFlashPhase));
			putU32(h->m_doDefectorFX ? 1 : 0);
		}
		putU32(obj->m_undetected ? 1 : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete body;
	delete obj;
	TheGameLogic = 0;
	TheAudio = 0;
	return 0;
}

}  /* extern "C" */
