/* Differential-test driver for BunkerBusterBehavior (p1w13-sonnet, batch R54; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BunkerBusterBehavior.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end, on the module build's stand-ins: four objects (ids 1-4; positions (10 k,
** 20 k, k)), the bomb (object 1: maybe a player of three, maybe MISSILE_KILLING_SELF, maybe an AI whose current victim is the test's), the
** targets (objects 2-4: maybe a contain module, bustable or not, whose killAllContained and harmAndForceExitAllContained are lines), the
** players' completed upgrades, the upgrade center (the REAL one, with the harness's upgrades Up0..Up7), the weapon store (WEAP_A and WEAP_B
** with their damage and death types, found by INI::parseWeaponTemplate through the lookup hook; createAndFireTempWeapon a line), the FX lists
** (FX_A, FX_B; doFXObj a line naming its object), the game logic (its frame; findObjectByID the four), and the REAL UpdateModule, INI and
** Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BunkerBusterBehavior.cpp"

namespace {

ModuleData *l9r54Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = BunkerBusterBehavior::friend_newModuleData(&ini);
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

/* the weapon store's templates, with the damage and death types of the test's WEAPON_TYPES */
WeaponTemplate g_r54Weapons[2];
const char *const g_r54WeaponNames[2] = { "WEAP_A", "WEAP_B" };
const void *r54WeaponLookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r54WeaponNames[i]) == 0) return &g_r54Weapons[i];
	return 0;   /* "None" and any other name: NULL */
}
const char *r54WeaponName(const WeaponTemplate *t) { return t == &g_r54Weapons[0] ? "WEAP_A" : "WEAP_B"; }
int r54WeaponCode(const WeaponTemplate *t) { return t == 0 ? 0 : (t == &g_r54Weapons[0] ? 1 : 2); }
WeaponStore g_r54Store;

/* a target's contain module: bustable or not; the two calls a bunker buster makes are lines */
class R54Contain : public UpgContain
{
public:
	virtual Bool isBustable() const { return m_bust; }
	virtual void killAllContained(void) { ulog("killall %d", (int)m_owner->m_oid); }
	virtual void harmAndForceExitAllContained(DamageInfo *info)
	{
		ulog("harmall %d %d %d %d %u %08x", (int)m_owner->m_oid, (int)info->in.m_damageType, (int)info->in.m_deathType, (int)info->in.m_sourceID,
			(unsigned)info->in.m_sourcePlayerMask, realBits(info->in.m_amount));
	}
	Bool m_bust = false;
	Object *m_owner = 0;
};

Object *g_r54Objs[4];
Object *r54Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 4) ? g_r54Objs[(int)id - 1] : 0; }
void r54FxObj(const Object *a, const Object *) { ulog("fxobj %d", a ? (int)a->m_oid : 0); }
std::vector<unsigned char> g_r54Saved;
UpgradeMaskType r54Mask(unsigned bits)
{
	UpgradeMaskType m;
	for (int k = 0; k < 8; ++k) if (bits >> k & 1) {
		char name[8];
		sprintf(name, "Up%d", k);
		const UpgradeTemplate *u = TheUpgradeCenter->findUpgrade(AsciiString(name));
		if (u) m.set(u->getUpgradeMask());
	}
	return m;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a BunkerBusterBehavior and make it on object 1 of four at frame cfg[0], then onObjectCreated. `cfg`
** (8 words; 1-4 also settable by op 4): 1 the bomb's player + 1 (0 none), 2 the players' completed upgrades (byte p: bit k Up<k>), 3 the bomb:
** bit 0 an AI, 1 MISSILE_KILLING_SELF, bits 4-7 the AI's current victim (objects 1-4, else NULL), 4 the targets: nibble k-2 for object k (bit 0 a
** contain module, 1 bustable), 5 the two weapons' damage types (bytes 0, 1) and death types (bytes 2, 3). The data dump goes first (the
** upgrade's name, the two FX lists, the frequency, the two seismic numbers, the two weapons), then onObjectCreated's state. Then `n` ops of 5
** words [op, a, b, c, d]: 0 update()  1 the frame = a  2 the frame += 1  3 onDie  4 cfg[a] = b  5 xfer save  6 xfer load of the last save
** 7 xfer CRC  8 onObjectCreated. Each op appends: the CATCH_ALL status, the result, the effect log ("fx <id>", "fxobj <object>", "killall
** <object>", "harmall ...", "weapon <name> <object> <position>"), the xfer status, bytes and CRC, the state (the victim, whether the upgrade
** was found). */
static int l9r54RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r54Saved.clear();
	*len = 0;
	unsigned cfg[8];
	memcpy(cfg, cfg_in, sizeof(cfg));
	for (int w = 0; w < 2; ++w) {
		g_r54Weapons[w].m_damageTypeT = (int)(cfg[5] >> (8 * w) & 255);
		g_r54Weapons[w].m_deathTypeT = (int)(cfg[5] >> (16 + 8 * w) & 255);
	}
	ref_weapon_lookup_hook = r54WeaponLookup;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r54Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = 0;
	if (status) return status;
	{
		const BunkerBusterBehaviorModuleData *bd = (const BunkerBusterBehaviorModuleData *)d;
		putStr(bd->m_upgradeRequired.str(), (int)bd->m_upgradeRequired.getLength());
		putI32(fxId(bd->m_detonationFX));
		putI32(fxId(bd->m_crashThroughBunkerFX));
		putU32(bd->m_crashThroughBunkerFXFrequency);
		putU32(realBits(bd->m_seismicEffectRadius));
		putU32(realBits(bd->m_seismicEffectMagnitude));
		putI32(r54WeaponCode(bd->m_shockwaveWeaponTemplate));
		putI32(r54WeaponCode(bd->m_occupantDamageWeaponTemplate));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheWeaponStore = &g_r54Store;
	g_tempWeaponName = r54WeaponName;
	g_fxObjHook = r54FxObj;
	g_modFindHook = r54Find;
	Player players[3];
	for (int p = 0; p < 3; ++p) players[p].m_idx = p;
	AIUpdateInterface ai;
	R54Contain contains[4];
	for (int k = 0; k < 4; ++k) {
		g_r54Objs[k] = newObject();
		g_r54Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r54Objs[k]->m_pos.x = 10.0f * (k + 1);
		g_r54Objs[k]->m_pos.y = 20.0f * (k + 1);
		g_r54Objs[k]->m_pos.z = (Real)(k + 1);
		contains[k].m_owner = g_r54Objs[k];
	}
	Object *me = g_r54Objs[0];
	auto scene = [&]() {
		unsigned tp = cfg[1];
		me->m_player = (tp >= 1 && tp <= 3) ? &players[tp - 1] : 0;
		for (int p = 0; p < 3; ++p) players[p].m_upgradesCompleted = r54Mask(cfg[2] >> (8 * p) & 255);
		me->m_ai = (cfg[3] & 1) ? &ai : 0;
		me->m_status.set(OBJECT_STATUS_MISSILE_KILLING_SELF, (cfg[3] >> 1 & 1) ? 1 : 0);
		unsigned v = cfg[3] >> 4 & 15;
		ai.m_victim = (v >= 1 && v <= 4) ? g_r54Objs[v - 1] : 0;
		for (int k = 1; k < 4; ++k) {
			unsigned f = cfg[4] >> (4 * (k - 1)) & 15;
			g_r54Objs[k]->m_contain = (f & 1) ? &contains[k] : 0;
			contains[k].m_bust = (f >> 1 & 1) != 0;
		}
	};
	scene();
	BunkerBusterBehavior *m = 0;
	try { m = newInstance(BunkerBusterBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; TheWeaponStore = 0; g_tempWeaponName = 0; g_fxObjHook = 0; g_modFindHook = 0;
		for (int k = 0; k < 4; ++k) { g_r54Objs[k]->m_ai = 0; g_r54Objs[k]->m_contain = 0; g_r54Objs[k]->m_player = 0; delete g_r54Objs[k]; g_r54Objs[k] = 0; }
		delete d;
		return 5;
	}
	m->onObjectCreated();
	putU32((unsigned)m->m_victimID);
	putU32(m->m_upgradeRequired ? 1 : 0);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 2) gl.m_frame += 1;
		if (o[0] == 4 && o[1] >= 1 && o[1] <= 4) cfg[o[1]] = o[2];
		scene();
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
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: { DamageInfo di; m->onDie(&di); break; }
			case 5: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r54Saved.assign(bytes, bytes + blen); break;
			case 6: {
				std::vector<unsigned char> b = g_r54Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 7: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 8: m->onObjectCreated(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1 && e.p) { char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)m->m_victimID);
		putU32(m->m_upgradeRequired ? 1 : 0);
	}
	*len = finish(buf, cap);
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 4; ++k) {
		Object *x = g_r54Objs[k];
		x->m_ai = 0; x->m_contain = 0; x->m_player = 0;
		delete x;
		g_r54Objs[k] = 0;
	}
	g_modFindHook = 0;
	g_fxObjHook = 0;
	g_tempWeaponName = 0;
	TheWeaponStore = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r54_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r54RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
