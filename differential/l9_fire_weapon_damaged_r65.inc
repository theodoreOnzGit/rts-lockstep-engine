/* Differential-test driver for FireWeaponWhenDamagedBehavior (p1w13-sonnet, batch R65; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDamagedBehavior.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the object (id 1, at (11, 22, 5), maybe
** with a body: a REAL InactiveBody whose damage state the test gives), the weapon store of batches R62 and R64 (WEAP_A, WEAP_B), the
** stand-in Weapon with Weapon.cpp's own constructor, crc, xfer, loadPostProcess and getStatus (build.rs, mod_gen/Weapon_r62.inc) and
** Weapon.h's setters, its reloadAmmo and forceFireWeapon (batch R64's) as lines; the game logic (its frame, its wake calls applied as
** GameLogic::friend_awakenUpdateModule applies them, batch R52's way); the REAL UpdateModule, UpgradeMux, UpgradeCenter, INI and Xfer. */
/* (the file defines the same three unused file-scope constants as FireWeaponWhenDeadBehavior.cpp, earlier in this unit: renamed by macro for
** the include) */
#define MAX_IDX R65_MAX_IDX
#define BEGIN_MIDPOINT_RATIO R65_BEGIN_MIDPOINT_RATIO
#define END_MIDPOINT_RATIO R65_END_MIDPOINT_RATIO
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDamagedBehavior.cpp"
#undef END_MIDPOINT_RATIO
#undef BEGIN_MIDPOINT_RATIO
#undef MAX_IDX

void Weapon::reloadAmmo(const Object *source)
{
	ulog("reloadammo %d %s", source ? (int)source->m_oid : 0, m_template ? m_template->getName().str() : "");
}
extern const void *(*g_weaponFindHook)(const char *);
extern "C" const void *(*ref_weapon_lookup_hook)(const char *);

namespace {

ModuleData *l9r65Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FireWeaponWhenDamagedBehavior::friend_newModuleData(&ini);
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

/* a body whose damage state the test gives */
class R65Body : public InactiveBody
{
public:
	R65Body(Thing *t, const ModuleData *d) : InactiveBody(t, d) {}
	virtual BodyDamageType getDamageState() const { return m_r65State; }
	BodyDamageType m_r65State = BODY_PRISTINE;
};

WeaponTemplate g_r65Tmpl[2];
const char *const g_r65Names[2] = { "WEAP_A", "WEAP_B" };
const void *r65Lookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r65Names[i]) == 0) return &g_r65Tmpl[i];
	return 0;   /* "None" and any other name: NULL */
}
int r65Code(const WeaponTemplate *t) { return t == &g_r65Tmpl[0] ? 1 : (t == &g_r65Tmpl[1] ? 2 : 0); }
WeaponStore g_r65Store;
std::vector<unsigned char> g_r65Saved;
Object *g_r65Me;
Object *r65Find(ObjectID id) { return (int)id == 1 ? g_r65Me : 0; }
Real r65F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
UpdateModule *g_r65Module = 0;
UpdateModule *g_r65Updating = 0;
int g_r65Wakes = 0;
UnsignedInt g_r65WakeFrame = 0;
void r65Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	if (u == g_r65Module) { ++g_r65Wakes; g_r65WakeFrame = whenToWakeUp; }
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r65Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a FireWeaponWhenDamagedBehavior and make the module on object 1 at frame cfg[0]. `cfg` (12 words):
** 1 bit 0 the object has a body, 2 its damage state; 3-5 WEAP_A's minimum and maximum target pitch (float bits) and shots per barrel, 6 its
** suspend-FX delay, 7-10 WEAP_B's. The data dump goes first (the mux, StartsActive, the damage types (two words), the amount, the eight weapon
** templates: 0 NULL, 1 WEAP_A, 2 WEAP_B), then the constructor's effect log, its wake calls (count, last frame) and the next call frame, and
** which weapons were made (a bit each). Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 the frame += 1  3 xfer save
** 4 xfer load of the last save  5 xfer CRC  6 attemptUpgrade(m)  7 resetUpgrade(m) (m: a..d, 128 bits)  8 onDamage(type a, dealt b, amount c:
** float bits)  9 the body's damage state = a  10 weapon a's setStatus(b)  11 its setPossibleNextShotFrame(b)  12 its setPreAttackFinishedFrame(b)
** (10-12 nothing without the weapon). Each op appends: the CATCH_ALL status, the result, isAlreadyUpgraded, the effect log, the wake calls
** (count, last frame), the xfer status, bytes and CRC. */
static int l9r65RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r65Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	for (int k = 0; k < 2; ++k) {
		g_r65Tmpl[k].m_name = AsciiString(g_r65Names[k]);
		g_r65Tmpl[k].m_minTargetPitch = r65F(cfg_in[3 + 4 * k]);
		g_r65Tmpl[k].m_maxTargetPitch = r65F(cfg_in[4 + 4 * k]);
		g_r65Tmpl[k].m_shotsPerBarrel = (Int)cfg_in[5 + 4 * k];
		g_r65Tmpl[k].m_suspendFXDelay = cfg_in[6 + 4 * k];
	}
	initDamageTypeFlags();   /* (GameEngine::init does, before any INI is read: DAMAGE_TYPE_FLAGS_ALL is the data's default) */
	const void *(*oldLookup)(const char *) = ref_weapon_lookup_hook;
	ref_weapon_lookup_hook = r65Lookup;
	try { d = l9r65Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = oldLookup;
	if (status) return status;
	unsigned cfg[12];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const FireWeaponWhenDamagedBehaviorModuleData *fd = (const FireWeaponWhenDamagedBehaviorModuleData *)d;
		putMux(fd->m_upgradeMuxData);
		putU32(fd->m_initiallyActive ? 1 : 0);
		unsigned w[2] = { 0, 0 };
		for (int i = 0; i < DAMAGE_NUM_TYPES; ++i) if (getDamageTypeFlag(fd->m_damageTypes, (DamageType)i)) w[i / 32] |= 1u << (i % 32);
		putU32(w[0]); putU32(w[1]);
		putU32(realBits(fd->m_damageAmount));
		const WeaponTemplate *const t[8] = { fd->m_reactionWeaponPristine, fd->m_reactionWeaponDamaged, fd->m_reactionWeaponReallyDamaged,
			fd->m_reactionWeaponRubble, fd->m_continuousWeaponPristine, fd->m_continuousWeaponDamaged, fd->m_continuousWeaponReallyDamaged,
			fd->m_continuousWeaponRubble };
		for (int k = 0; k < 8; ++k) putU32((unsigned)r65Code(t[k]));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r65Awaken;
	TheWeaponStore = &g_r65Store;
	g_weaponFindHook = r65Lookup;
	BehaviorModule *none[1] = { 0 };
	g_r65Me = newObject();
	Object *me = g_r65Me;
	me->m_oid = 1;
	me->m_behaviors = none;
	me->m_pos.x = 11.0f; me->m_pos.y = 22.0f; me->m_pos.z = 5.0f;
	ModuleData bodyData;
	R65Body *body = (cfg[1] & 1) ? new R65Body(me, &bodyData) : 0;
	me->m_body = body;
	auto scene = [&]() { if (body) body->m_r65State = (BodyDamageType)cfg[2]; };
	scene();
	g_modFindHook = r65Find;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	g_events.clear();
	g_ulog.clear();
	g_r65Wakes = 0;
	g_r65WakeFrame = 0;
	FireWeaponWhenDamagedBehavior *m = 0;
	try { m = newInstance(FireWeaponWhenDamagedBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the Rust side excludes a case whose constructor throws: not reached) */
	g_r65Module = m;
	BehaviorModule *behaviors[3] = { m, body, 0 };
	me->m_behaviors = behaviors;
	putLog();
	/* (the constructor's wake calls came before g_r65Module was the module: they are counted by the next call frame they left) */
	putU32(m->friend_getNextCallFrame());
	{
		Weapon *const w[8] = { m->m_reactionWeaponPristine, m->m_reactionWeaponDamaged, m->m_reactionWeaponReallyDamaged, m->m_reactionWeaponRubble,
			m->m_continuousWeaponPristine, m->m_continuousWeaponDamaged, m->m_continuousWeaponReallyDamaged, m->m_continuousWeaponRubble };
		unsigned made = 0;
		for (int k = 0; k < 8; ++k) if (w[k]) made |= 1u << k;
		putU32(made);
	}
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 2) gl.m_frame += 1;
		if (o[0] == 9) cfg[2] = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		g_r65Wakes = 0;
		g_r65WakeFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		Weapon *const w[8] = { m->m_reactionWeaponPristine, m->m_reactionWeaponDamaged, m->m_reactionWeaponReallyDamaged, m->m_reactionWeaponRubble,
			m->m_continuousWeaponPristine, m->m_continuousWeaponDamaged, m->m_continuousWeaponReallyDamaged, m->m_continuousWeaponRubble };
		Weapon *wa = o[1] < 8 ? w[o[1]] : 0;
		try {
			switch (o[0]) {
			case 0: g_r65Updating = m; res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r65Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r65Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 7: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 8: {
				DamageInfo info;
				info.in.m_damageType = (DamageType)o[1];
				info.out.m_actualDamageDealt = r65F(o[2]);
				info.in.m_amount = r65F(o[3]);
				m->onDamage(&info);
				break;
			}
			case 10: if (wa) wa->setStatus((WeaponStatus)o[2]); break;
			case 11: if (wa) wa->setPossibleNextShotFrame(o[2]); break;
			case 12: if (wa) wa->setPreAttackFinishedFrame(o[2]); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r65Updating = 0;
		putI32(st);
		putU32(res);
		putU32(m->isAlreadyUpgraded() ? 1 : 0);
		putLog();
		putU32((unsigned)g_r65Wakes);
		putU32(g_r65Wakes ? g_r65WakeFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_r65Module = 0;
	g_modFindHook = 0;
	g_weaponFindHook = 0;
	TheWeaponStore = 0;
	me->m_behaviors = 0;
	me->m_body = 0;
	delete m;
	delete body;
	delete d;
	delete me;
	g_r65Me = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r65_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r65RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
