/* Differential-test driver for four more layer-9 upgrade modules (p1w11-sonnet, upgrade batch C; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CostModifierUpgrade.cpp, PowerPlantUpgrade.cpp, RadarUpgrade.cpp,
**   UnpauseSpecialPowerUpgrade.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_upgrades.inc, whose effect log and
** helpers it uses) on the stand-ins of differential/shim_module: a Player that logs what the modules do to it (tagged with its index), an
** Object that has the behaviour modules the test gives it (a null-terminated array, as upstream's m_behaviors) and a disabled flag, and the
** REAL UpgradeCenter, INI and (for the special power template field) the REAL SpecialPowerStore of wrap_upgrade.cpp, reached through the
** hook of wrap_ini.cpp. Every call a module makes on its object, its players or its behaviour modules is a line of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CostModifierUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/PowerPlantUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/RadarUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UnpauseSpecialPowerUpgrade.cpp"

extern "C" const void *ref_sp_find_raw(const char *name);
extern "C" unsigned ref_sp_raw_id(const void *t);
extern "C" void ref_ini_sp_bind(const void *(*hook)(const char *));

namespace {

/* a kind-of mask as the indices of its set bits, comma separated */
std::string kindStr(const KindOfMaskType &m)
{
	std::string s;
	for (int i = 0; i < KINDOF_COUNT; ++i) if (m.test((KindOfType)i)) { char b[16]; sprintf(b, "%d,", i); s += b; }
	return s;
}

int g_hasRadar = 0;
UpdateModule *g_radarMod = 0;
Drawable g_radarDrawable;
int g_nameKeyRadar = -1;

}  // namespace

void Player::addKindOfProductionCostChange(KindOfMaskType kindOf, Real percent) { ulog("addcost %d [%s] %u", (int)m_idx, kindStr(kindOf).c_str(), realBits(percent)); }
void Player::removeKindOfProductionCostChange(KindOfMaskType kindOf, Real percent) { ulog("removecost %d [%s] %u", (int)m_idx, kindStr(kindOf).c_str(), realBits(percent)); }
void Player::addPowerBonus(Object *obj) { ulog("addpower %d", (int)m_idx); }
void Player::removePowerBonus(Object *obj) { ulog("removepower %d", (int)m_idx); }
void Player::addRadar(Bool disableProof) { ulog("addradar %d %d", (int)m_idx, (int)disableProof); }
void Player::removeRadar(Bool disableProof) { ulog("removeradar %d %d", (int)m_idx, (int)disableProof); }
/* the object's radar update: the REAL RadarUpdate (batch E compiles RadarUpdate.cpp), which the test gives the object (g_radarMod) and finds by the key of
** its name. Its extendRadar() gives the object's drawable MODELCONDITION_RADAR_EXTENDING: the drawable is the stand-in that logs "mc <flag>", and the driver
** shows that line as "extendradar" (what the Rust probe module logs) */
UpdateModule *(*g_findUpdateHook)(const Object *, NameKeyType) = 0;   /* special power batch H finds its own update modules */
UpdateModule *Object::findUpdateModule(NameKeyType key) const
{
	if (g_findUpdateHook) return g_findUpdateHook(this, key);
	if (g_radarMod && key == TheNameKeyGenerator->nameToKey("RadarUpdate")) return g_radarMod;
	return 0;
}

namespace {

class UpcPP : public PowerPlantUpdateInterface {
public:
	UpcPP() : m_idx(0) {}
	virtual void extendRods(Bool extend) { ulog("extendrods %d %d", m_idx, (int)extend); }
	int m_idx;
};

class UpcSP : public SpecialPowerModuleInterface {
public:
	UpcSP() : m_idx(0), m_template(0) {}
	virtual Bool isModuleForPower(const SpecialPowerTemplate *t) const { return false; }
	virtual Bool isReady(void) const { return false; }
	virtual Real getPercentReady(void) const { return 0; }
	virtual UnsignedInt getReadyFrame(void) const { return 0; }
	virtual AsciiString getPowerName(void) const { return AsciiString(); }
	virtual const SpecialPowerTemplate *getSpecialPowerTemplate(void) const { return m_template; }
	virtual ScienceType getRequiredScience(void) const { return SCIENCE_INVALID; }
	virtual void onSpecialPowerCreation(void) {}
	virtual void setReadyFrame(UnsignedInt frame) {}
	virtual void pauseCountdown(Bool pause) { ulog("pause %d %d", m_idx, (int)pause); }
	virtual void doSpecialPower(UnsignedInt commandOptions) {}
	virtual void doSpecialPowerAtObject(Object *obj, UnsignedInt commandOptions) {}
	virtual void doSpecialPowerAtLocation(const Coord3D *loc, Real angle, UnsignedInt commandOptions) {}
	virtual void doSpecialPowerUsingWaypoints(const Waypoint *way, UnsignedInt commandOptions) {}
	virtual void markSpecialPowerTriggered(const Coord3D *location) {}
	virtual void startPowerRecharge() {}
	virtual const AudioEventRTS &getInitiateSound() const { static AudioEventRTS a; return a; }
	virtual Bool isScriptOnly() const { return false; }
	virtual const ThingTemplate *getReferenceThingTemplate() const { return 0; }
	int m_idx;
	const SpecialPowerTemplate *m_template;
};

/* a behaviour module of the object: with a power plant update interface (kind 1) or a special power interface (kind 2) or neither (0) */
class UpcBehavior : public TestBehavior {
public:
	UpcBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d), m_kind(0) {}
	virtual PowerPlantUpdateInterface *getPowerPlantUpdateInterface() { return m_kind == 1 ? &m_pp : 0; }
	virtual SpecialPowerModuleInterface *getSpecialPower() { return m_kind == 2 ? &m_sp : 0; }
	int m_kind;
	UpcPP m_pp;
	UpcSP m_sp;
};

ModuleData *l9upcRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = CostModifierUpgrade::friend_newModuleData(&ini); break;
			case 1: data = PowerPlantUpgrade::friend_newModuleData(&ini); break;
			case 2: data = RadarUpgrade::friend_newModuleData(&ini); break;
			default: data = UnpauseSpecialPowerUpgrade::friend_newModuleData(&ini); break;
			}
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

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of upgrade module `kind` (0 CostModifier, 1 PowerPlant, 2 Radar, 3 UnpauseSpecialPower) and make the
** module on an Object whose controlling player is player `ctrl` of three (-1: none), each with completed upgrades `player_mask`, and whose own
** are `obj_mask`. `cfg`: [disabled, number of behaviour modules (0..4), the kind of each of the four (0 none, 1 power plant update interface,
** 2 special power interface), has a radar update, the special power template of each of the four (the index of the name SP_<i> in the real
** store, 99 for a NULL template)]. The data dump goes first (the mux, then the module's own fields).
** Then `n` ops of 5 words [op, a, b, c, d]: 0 wouldUpgrade  1 attemptUpgrade  2 resetUpgrade  3 forceRefreshUpgrade  4 isAlreadyUpgraded
** (m = a..d as 128 bits)  5 the object's completed upgrades = m  6 the three players' completed upgrades = m  7 the object disabled = a & 1
** 8 onDelete  9 onCapture(player a - 1, player b - 1; 0 is NULL)  10 loadPostProcess  11 the controlling player = a - 1 (0 none). Each op
** appends: the CATCH_ALL status, the Bool result, isAlreadyUpgraded(), the effect log (count, then each as a string). */
int ref_l9upc_run(int kind, const char *filename, int ctrl, const unsigned *player_mask, const unsigned *obj_mask, const unsigned *cfg,
	int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	if (kind == 3) ref_ini_sp_bind(ref_sp_find_raw);
	try { d = l9upcRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (kind == 3) ref_ini_sp_bind(0);
	if (status) return status;
	putMux(((UpgradeModuleData *)d)->m_upgradeMuxData);
	switch (kind) {
	case 0:
	{
		const CostModifierUpgradeModuleData *c = (CostModifierUpgradeModuleData *)d;
		std::string s = kindStr(c->m_kindOf);
		putStr(s.c_str(), (int)s.size());
		putU32(realBits(c->m_percentage));
		break;
	}
	case 2: g_dump.push_back(((RadarUpgradeModuleData *)d)->m_isDisableProof ? 1 : 0); break;
	case 3: putU32(ref_sp_raw_id(((UnpauseSpecialPowerUpgradeModuleData *)d)->m_specialPower)); break;
	default: break;
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) { players[i].m_idx = i; players[i].m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, player_mask); }
	Object *obj = newObject();
	obj->m_player = ctrl >= 0 ? &players[ctrl] : 0;
	obj->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, obj_mask);
	obj->m_disabled = cfg[0] != 0;
	g_hasRadar = (int)cfg[6];
	GameLogic radarGl;
	RadarUpdateModuleData radarData;
	if (g_hasRadar) {
		TheGameLogic = &radarGl;
		obj->m_drawable = &g_radarDrawable;
		g_radarMod = new RadarUpdate(obj, &radarData);
	}
	ModuleData mdata;
	UpcBehavior *behaviors[4] = { 0, 0, 0, 0 };
	BehaviorModule *list[5];
	unsigned nmods = cfg[1] > 4 ? 4 : cfg[1];
	for (unsigned i = 0; i < nmods; ++i) {
		behaviors[i] = new UpcBehavior(obj, &mdata);
		behaviors[i]->m_kind = (int)cfg[2 + i];
		behaviors[i]->m_pp.m_idx = (int)i;
		behaviors[i]->m_sp.m_idx = (int)i;
		if (cfg[7 + i] != 99) {
			char nm[16];
			sprintf(nm, "SP_%u", cfg[7 + i]);
			behaviors[i]->m_sp.m_template = (const SpecialPowerTemplate *)ref_sp_find_raw(nm);
		}
		list[i] = behaviors[i];
	}
	list[nmods] = 0;
	obj->m_behaviors = list;
	obj->m_xp = new ExperienceTracker;
	UpgradeModule *m = 0;
	switch (kind) {
	case 0: m = new CostModifierUpgrade(obj, d); break;
	case 1: m = new PowerPlantUpgrade(obj, d); break;
	case 2: m = new RadarUpgrade(obj, d); break;
	default: m = new UnpauseSpecialPowerUpgrade(obj, d); break;
	}
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		g_ulog.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 1: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 2: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 3: m->forceRefreshUpgrade(); break;
			case 4: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 5: obj->m_objectUpgradesCompleted = mask; break;
			case 6: for (int p = 0; p < 3; ++p) players[p].m_upgradesCompleted = mask; break;
			case 7: obj->m_disabled = (o[1] & 1) != 0; break;
			case 8: m->onDelete(); break;
			case 9: m->onCapture(o[1] ? &players[o[1] - 1] : 0, o[2] ? &players[o[2] - 1] : 0); break;
			case 10: m->loadPostProcess(); break;
			case 11: obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		g_dump.push_back((unsigned char)res);
		g_dump.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				/* the radar update's drawable call, as the Rust probe logs the call itself */
				char radarMc[32];
				sprintf(radarMc, "mc %d", (int)MODELCONDITION_RADAR_EXTENDING);
				log.push_back(g_hasRadar && g_ulog[(size_t)e.p] == radarMc ? std::string("extendradar") : g_ulog[(size_t)e.p]);
			}
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	}
	*len = finish(buf, cap);
	obj->m_behaviors = 0;
	g_hasRadar = 0;
	delete g_radarMod;
	g_radarMod = 0;
	obj->m_drawable = 0;
	TheGameLogic = 0;
	delete m;
	delete d;
	for (unsigned i = 0; i < nmods; ++i) delete behaviors[i];
	delete obj;
	return 0;
}

}  /* extern "C" */
