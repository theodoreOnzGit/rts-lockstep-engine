/* Differential-test driver for the eight layer-9 upgrade modules of p1l9-upgrades (upstream commit 0a05454d), verified by p1w10-sonnet:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/WeaponSetUpgrade.cpp, WeaponBonusUpgrade.cpp, ArmorUpgrade.cpp,
**   ModelConditionUpgrade.cpp, ExperienceScalarUpgrade.cpp, ActiveShroudUpgrade.cpp, MaxHealthUpgrade.cpp, GrantScienceUpgrade.cpp
**   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, in its marked block) on the stand-ins
** of differential/shim_module (an Object with a body, a drawable, an experience tracker, weapon set flags, a weapon bonus condition and a
** shroud range; a Player that logs the science it is granted) and the REAL UpgradeCenter and INI. The pattern is l9_example.inc's. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/WeaponSetUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/WeaponBonusUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ArmorUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ModelConditionUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ExperienceScalarUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ActiveShroudUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/MaxHealthUpgrade.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/GrantScienceUpgrade.cpp"

namespace {
/* the effect log of one op: the calls the modules make on their object, in order */
std::vector<std::string> g_ulog;
void ulog(const char *fmt, ...)
{
	char b[160];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(b, sizeof b, fmt, ap);
	va_end(ap);
	g_ulog.push_back(b);
	event(100, (const void *)(size_t)(g_ulog.size() - 1));   /* chronological with the FX and the removals */
}

}  // namespace

void Object::setWeaponSetFlag(WeaponSetType wst)
{
	/* Object.cpp:3054, the lines that reach outside the object: the flag set, the weapon set told, the drawable's model condition */
	m_weaponSetFlags[wst / 64] |= 1ULL << (wst % 64);
	ulog("wset");
	if (m_drawable) m_drawable->setModelConditionState(TheWeaponSetTypeToModelConditionTypeMap[wst]);
}
void modhelp_bonus(int b);   /* l9_helpers.inc: TempWeaponBonusHelper's tests read the call from a log */
void Object::setWeaponBonusCondition(WeaponBonusConditionType wst) { m_weaponBonusCondition |= (1 << (wst & 31)); modhelp_bonus((int)wst); }
void Object::setShroudRange(Real r) { m_shroudRange = r; }
void Object::handlePartitionCellMaintenance() { ulog("partition"); }
void Object::setModelConditionState(ModelConditionFlagType a) { if (m_drawable) m_drawable->setModelConditionState(a); }
void Drawable::setTerrainDecal(TerrainDecalType t) { ulog("decal %d", (int)t); m_terrainDecalType = t; }
void Drawable::setModelConditionState(ModelConditionFlagType a) { ulog("mc %d", (int)a); }
void Player::grantScience(ScienceType st) { ulog("science %d", (int)st); }
/* ScienceStore::getScienceFromInternalName is the REAL one (Science.cpp, in the Science archive; it reads nothing of the store); the shim
** Science.h declares the class without its data, so `TheScienceStore` points at an empty object of that class */

namespace {
class UpgBody : public TestBody {
public:
	UpgBody(Thing *t, const ModuleData *d) : TestBody(t, d), m_maxHealth(0), m_armor(0) {}
	virtual Real getMaxHealth() const { return m_maxHealth; }
	virtual void setMaxHealth(Real maxHealth, MaxHealthChangeType type)
	{
		unsigned bits;
		memcpy(&bits, &maxHealth, 4);
		ulog("maxhealth %u %d", bits, (int)type);
		m_maxHealth = maxHealth;
	}
	virtual void setArmorSetFlag(ArmorSetType ast) { m_armor |= 1u << ast; }
	Real m_maxHealth;
	unsigned m_armor;
};

ModuleData *l9upRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = WeaponSetUpgrade::friend_newModuleData(&ini); break;
			case 1: data = WeaponBonusUpgrade::friend_newModuleData(&ini); break;
			case 2: data = ArmorUpgrade::friend_newModuleData(&ini); break;
			case 3: data = ModelConditionUpgrade::friend_newModuleData(&ini); break;
			case 4: data = ExperienceScalarUpgrade::friend_newModuleData(&ini); break;
			case 5: data = ActiveShroudUpgrade::friend_newModuleData(&ini); break;
			case 6: data = MaxHealthUpgrade::friend_newModuleData(&ini); break;
			default: data = GrantScienceUpgrade::friend_newModuleData(&ini); break;
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

unsigned realBits(Real r) { unsigned b; memcpy(&b, &r, 4); return b; }
}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of upgrade module `kind` (0 WeaponSet, 1 WeaponBonus, 2 Armor, 3 ModelCondition,
** 4 ExperienceScalar, 5 ActiveShroud, 6 MaxHealth, 7 GrantScience) and make the module on an Object whose controlling player is a Player with
** completed upgrades `player_mask` (none if `has_player` is 0) and whose own are `obj_mask`. `cfg`: [has body, has drawable]. The data dump goes first (the mux, then the module's own fields). Then `n` ops of 5 words [op, a, b, c, d]:
**   0 wouldUpgrade(m)  1 attemptUpgrade(m)  2 resetUpgrade(m)  3 forceRefreshUpgrade()  4 isAlreadyUpgraded()  (m = a..d as 128 bits)
**   5 the object's completed upgrades = m  6 the player's completed upgrades = m
**   7 the object's state: weapon set flags and bonus cleared, the body's armor flags cleared, experience scalar = bits a, shroud range =
**     bits b, body max health = bits c
** Each op appends: the CATCH_ALL status, the Bool result, isAlreadyUpgraded(), the effect log (count, then each as a string), then the
** state: the two words of the weapon set flags, the bonus condition, the body's armor flags, the experience scalar, the shroud range
** and the body's max health (as bit patterns), and the cached science of a GrantScienceUpgrade. */
int ref_l9up_run(int kind, const char *filename, int has_player, const unsigned *player_mask, const unsigned *obj_mask, const unsigned *cfg,
	int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9upRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putMux(((UpgradeModuleData *)d)->m_upgradeMuxData);
	switch (kind) {
	case 3: putI32((int)((ModelConditionUpgradeModuleData *)d)->m_conditionFlag); break;
	case 4: putU32(realBits(((ExperienceScalarUpgradeModuleData *)d)->m_addXPScalar)); break;
	case 5: putU32(realBits(((ActiveShroudUpgradeModuleData *)d)->m_newShroudRange)); break;
	case 6: putU32(realBits(((MaxHealthUpgradeModuleData *)d)->m_addMaxHealth)); putI32((int)((MaxHealthUpgradeModuleData *)d)->m_maxHealthChangeType); break;
	case 7: putAscii(((GrantScienceUpgradeModuleData *)d)->m_grantScienceName); break;
	default: break;
	}
	static ScienceStore stand_in_store;
	if (!TheScienceStore) TheScienceStore = &stand_in_store;
	Player *player = has_player ? new Player : 0;
	if (player) player->m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, player_mask);
	Object *obj = newObject();
	obj->m_player = player;
	obj->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, obj_mask);
	UpgBody *body = 0;
	if (cfg[0]) { body = new UpgBody(obj, new BodyModuleData); obj->m_body = body; }
	if (cfg[1]) obj->m_drawable = new Drawable;
	obj->m_xp = new ExperienceTracker;   /* Object's constructor makes one for every object */
	UpgradeModule *m = 0;
	switch (kind) {
	case 0: m = new WeaponSetUpgrade(obj, d); break;
	case 1: m = new WeaponBonusUpgrade(obj, d); break;
	case 2: m = new ArmorUpgrade(obj, d); break;
	case 3: m = new ModelConditionUpgrade(obj, d); break;
	case 4: m = new ExperienceScalarUpgrade(obj, d); break;
	case 5: m = new ActiveShroudUpgrade(obj, d); break;
	case 6: m = new MaxHealthUpgrade(obj, d); break;
	default: m = new GrantScienceUpgrade(obj, d); break;
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
			case 6: if (player) player->m_upgradesCompleted = mask; break;
			case 7:
				obj->m_weaponSetFlags[0] = obj->m_weaponSetFlags[1] = 0;
				obj->m_weaponBonusCondition = 0;
				if (body) { body->m_armor = 0; memcpy(&body->m_maxHealth, &o[3], 4); }
				if (obj->m_xp) memcpy(&obj->m_xp->m_scalar, &o[1], 4);
				memcpy(&obj->m_shroudRange, &o[2], 4);
				break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		g_dump.push_back((unsigned char)res);
		g_dump.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU64(obj->m_weaponSetFlags[0]);
		putU64(obj->m_weaponSetFlags[1]);
		putU32(obj->m_weaponBonusCondition);
		putU32(body ? body->m_armor : 0);
		putU32(obj->m_xp ? realBits(obj->m_xp->m_scalar) : 0);
		putU32(realBits(obj->m_shroudRange));
		putU32(body ? realBits(body->m_maxHealth) : 0);
		putI32(kind == 7 ? (int)((GrantScienceUpgrade *)m)->m_scienceType : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	delete player;
	return 0;
}

}  /* extern "C" */
