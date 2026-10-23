/* Differential-test driver for the eight sabotage crates (p1w13-sonnet, batch R13; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/Sabotage{CommandCenter,InternetCenter,MilitaryFactory,PowerPlant,Superweapon,
**   SupplyCenter,SupplyDropzone}CrateCollide.cpp and SabotageFakeBuilding.cpp   (#included, unchanged; the two thieves with Lib/BaseType.h's `min` macro,
**   which the module build leaves out)
** in wrap_module.cpp's unit, at its end (CrateCollide.cpp is compiled by l9_crates_f.inc), on the module build's stand-ins: a saboteur (id 1) with the
** crate module and maybe an AI with a goal, a victim building (id 2) with the test's kinds, player, drawable, body, special power modules, a REAL
** OCLUpdate, REAL SpyVisionUpdates and a contain holding two hackers (ids 3 and 4); three players (money, energy, brown-out, score) and the local
** player; a radar, an Eva, an in-game UI and an audio manager that log; the teams' relationship the test gives. Every call is a line of the effect
** log (ulog). */
#define min(x,y) (((x)<(y)) ? (x) : (y))   /* Lib/BaseType.h:101, verbatim (the module build leaves the macro out) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageCommandCenterCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageFakeBuilding.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageInternetCenterCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageMilitaryFactoryCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotagePowerPlantCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageSuperweaponCrateCollide.cpp"
#define min(x,y) (((x)<(y)) ? (x) : (y))   /* Lib/BaseType.h:101, verbatim (the module build leaves the macro out) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageSupplyCenterCrateCollide.cpp"
#undef min
#define min(x,y) (((x)<(y)) ? (x) : (y))   /* Lib/BaseType.h:101, verbatim (the module build leaves the macro out) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SabotageSupplyDropzoneCrateCollide.cpp"
#undef min
#undef min

Radar *TheRadar = 0;
void Radar::tryInfiltrationEvent(const Object *obj) { ulog("infiltrate %d", obj ? (int)obj->m_oid : 0); }
UnsignedInt Money::withdraw(UnsignedInt amountToWithdraw, Bool playSound) { ulog("withdraw %d %u %d", (int)m_idx, amountToWithdraw, (int)playSound); return amountToWithdraw; }
void Energy::setPowerSabotagedTillFrame(UnsignedInt frame) { ulog("powersabotage %d %u", (int)m_idx, frame); }
void Player::onPowerBrownOutChange(Bool brownOut) { ulog("brownout %d %d", (int)m_idx, (int)brownOut); }
void Object::setDisabledUntil(DisabledType type, UnsignedInt frame) { ulog("disableduntil %d %d %u", (int)m_oid, (int)type, frame); }

namespace {

Radar g_sabRadar;
GameTextInterface g_sabText;

/* the kinds the test gives the victim, by bit (the same list in the Rust test) */
const KindOfType g_sabKinds[16] = { KINDOF_STRUCTURE, KINDOF_PARACHUTE, KINDOF_COMMANDCENTER, KINDOF_FS_POWER, KINDOF_FS_INTERNET_CENTER, KINDOF_FS_BARRACKS,
	KINDOF_FS_WARFACTORY, KINDOF_FS_AIRFIELD, KINDOF_AIRCRAFT_CARRIER, KINDOF_FS_SUPERWEAPON, KINDOF_FS_STRATEGY_CENTER, KINDOF_FS_SUPPLY_CENTER,
	KINDOF_FS_SUPPLY_DROPZONE, KINDOF_FS_FAKE, KINDOF_INFANTRY, KINDOF_VEHICLE };

/* a special power module: its recharge is a line naming the object and the module's index */
class SabSP : public UpcSP {
public:
	virtual void startPowerRecharge() { ulog("recharge %d %d", m_oid, m_idx); }
	int m_oid = 0;
};
class SabSPBehavior : public TestBehavior {
public:
	SabSPBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d) {}
	virtual SpecialPowerModuleInterface *getSpecialPower() { return &m_sp; }
	SabSP m_sp;
};
/* the victim's contain: the hackers it holds */
class SabContain : public UpgContain {
public:
	virtual void iterateContained(ContainIterateFunc func, void *userData, Bool reverse) { for (size_t i = 0; i < m_list.size(); ++i) func(m_list[i], userData); }
	std::vector<Object *> m_list;
};
/* the victim's body: the maximum health the test gives; a damage is a line */
class SabBody : public InactiveBody {
public:
	SabBody(Thing *t, const ModuleData *d) : InactiveBody(t, d), m_max(0) {}
	virtual Real getMaxHealth() const { return m_max; }
	virtual void attemptDamage(DamageInfo *info)
	{
		ulog("damage %d %d %d %d %08x", (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid, (int)info->in.m_damageType,
			(int)info->in.m_deathType, (int)info->in.m_sourceID, realBits(info->in.m_amount));
	}
	Real m_max;
};

int g_sabRel = 0;
OCLUpdate *g_sabOcl = 0;
Relationship sabTeamRel(const Team *a, const Team *b) { return (a->m_dummy == 1 && b->m_dummy == 2) ? (Relationship)g_sabRel : ALLIES; }
UpdateModule *sabFindUpdate(const Object *, NameKeyType key) { return (g_sabOcl && key == TheNameKeyGenerator->nameToKey("OCLUpdate")) ? g_sabOcl : 0; }

ModuleData *l9sabRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = SabotageCommandCenterCrateCollide::friend_newModuleData(&ini); break;
			case 1: data = SabotageFakeBuildingCrateCollide::friend_newModuleData(&ini); break;
			case 2: data = SabotageInternetCenterCrateCollide::friend_newModuleData(&ini); break;
			case 3: data = SabotageMilitaryFactoryCrateCollide::friend_newModuleData(&ini); break;
			case 4: data = SabotagePowerPlantCrateCollide::friend_newModuleData(&ini); break;
			case 5: data = SabotageSuperweaponCrateCollide::friend_newModuleData(&ini); break;
			case 6: data = SabotageSupplyCenterCrateCollide::friend_newModuleData(&ini); break;
			default: data = SabotageSupplyDropzoneCrateCollide::friend_newModuleData(&ini); break;
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

CrateCollide *sabMake(int kind, Object *o, ModuleData *d)
{
	switch (kind) {
	case 0: return new SabotageCommandCenterCrateCollide(o, d);
	case 1: return new SabotageFakeBuildingCrateCollide(o, d);
	case 2: return new SabotageInternetCenterCrateCollide(o, d);
	case 3: return new SabotageMilitaryFactoryCrateCollide(o, d);
	case 4: return new SabotagePowerPlantCrateCollide(o, d);
	case 5: return new SabotageSuperweaponCrateCollide(o, d);
	case 6: return new SabotageSupplyCenterCrateCollide(o, d);
	default: return new SabotageSupplyDropzoneCrateCollide(o, d);
	}
}

void sabSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 16; ++i) if (bits >> i & 1) o->m_kindOf.set(g_sabKinds[i]);
}

std::vector<unsigned char> g_sabSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of sabotage crate `kind` (0 command centre, 1 fake building, 2 internet centre, 3 military factory, 4 power
** plant, 5 superweapon, 6 supply centre, 7 drop zone) and make the module on the saboteur (id 1). `cfg` (12 words): [0 the saboteur's player (0xFFFFFFFF
** none), 1 the victim's player, 2 the victim's kinds (bits of g_sabKinds), 3 flags (bit 0 the saboteur has an AI, 1 its goal is the victim, 2-3 the
** saboteur team's relationship to the victim's: 0 enemies, 1 neutral, 2 allies, 4 the victim has an AI, 5 it is effectively dead, 6 it has a drawable,
** 7 a body, 8 a contain with the hackers, 9-10 its special power modules, 11 an OCLUpdate, 12-13 its SpyVisionUpdates, 14-15 the local player (3
** none), 16 the saboteur is above the terrain), 4-6 the players' money, 7 the victim's maximum health (float bits), 8 the frame, 9 the hackers' players
** (2 bits each, 3 none), 10 the logic random seed, 11 the OCL delay]. The data dump goes first (the crate base and the class's field). Then `n` ops of 6 words
** [op, a, b, c]: 0 onCollide(a ? victim : NULL)  1 wouldLikeToCollideWith(a ? victim : NULL)  2 the victim's player = a - 1 (0 none)  3 flags bits 0-8 and
** 14-16 = a (as cfg[3])  4 the victim's kinds = a  5 player a's money = b  6 the frame = a  7 xfer save  8 xfer load of the last save  9 xfer CRC
** 10 isSabotageBuildingCrateCollide. Each op appends: the CATCH_ALL status, the result, the effect log (count, then each), the xfer status, bytes and
** CRC, then the OCLUpdate's remaining frames and each SpyVisionUpdate's reset flag and disabled-until frame. */
int ref_l9sab_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_sabSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9sabRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	cfPutBase((CrateCollideModuleData *)d, scienceKeys);
	switch (kind) {
	case 2: putU32(((const SabotageInternetCenterCrateCollideModuleData *)d)->m_sabotageFrames); break;
	case 3: putU32(((const SabotageMilitaryFactoryCrateCollideModuleData *)d)->m_sabotageFrames); break;
	case 4: putU32(((const SabotagePowerPlantCrateCollideModuleData *)d)->m_powerSabotageFrames); break;
	case 6: putU32(((const SabotageSupplyCenterCrateCollideModuleData *)d)->m_stealCashAmount); break;
	case 7: putU32(((const SabotageSupplyDropzoneCrateCollideModuleData *)d)->m_stealCashAmount); break;
	default: putU32(0); break;
	}
	GameLogic gl;
	gl.m_frame = cfg[8];
	gl.m_drawIconUI = false;
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[10]);
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_money.m_idx = i;
		players[i].m_money.m_count = cfg[4 + i];
		players[i].m_score.m_idx = i;
	}
	g_spPlayerPtrs[0] = &players[0]; g_spPlayerPtrs[1] = &players[1]; g_spPlayerPtrs[2] = &players[2];
	g_spPlayerCount = 3;
	unsigned flags = cfg[3];
	g_spLocal = ((flags >> 14) & 3) == 3 ? 0 : &players[(flags >> 14) & 3];
	ThePlayerList = &g_spList;
	TheEva = &g_spEva;
	TheRadar = &g_sabRadar;
	TheAudio = &g_ddAudio;
	TheInGameUI = &g_cfUi;
	TheGameText = &g_sabText;
	TheAnim2DCollection = 0;
	g_modDestroyHook = cfDestroyHook;
	g_teamRelHook = sabTeamRel;
	g_findUpdateHook = sabFindUpdate;
	g_sabRel = (int)((flags >> 2) & 3);
	g_cfMisc.m_moneyWithdrawSound.setEventName(AsciiString("moneyWithdraw"));
	g_cfMisc.m_sabotageShutDownBuilding.setEventName(AsciiString("sabotageShutDown"));
	g_cfMisc.m_sabotageResetTimerBuilding.setEventName(AsciiString("sabotageResetTimer"));
	Team teams[4];
	Object *obj[4];
	for (int k = 0; k < 4; ++k) {
		obj[k] = newObject();
		obj[k]->m_oid = (ObjectID)(k + 1);
		teams[k].m_dummy = k + 1;
		obj[k]->m_team = &teams[k];
		obj[k]->m_next = k ? obj[k - 1] : 0;
	}
	Object *saboteur = obj[0], *victim = obj[1];
	saboteur->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	victim->m_player = cfg[1] == 0xFFFFFFFFu ? 0 : &players[cfg[1]];
	for (int h = 0; h < 2; ++h) { const unsigned p = (cfg[9] >> (2 * h)) & 3; obj[2 + h]->m_player = p == 3 ? 0 : &players[p]; }
	for (int p = 0; p < 3; ++p) { players[p].m_objects.clear(); for (int k = 0; k < 4; ++k) if (obj[k]->m_player == &players[p]) players[p].m_objects.push_back(obj[k]); }
	sabSetKinds(victim, cfg[2]);
	AIUpdateInterface sabAi;
	sabAi.m_owner = saboteur;
	int victimAi = 0;
	Drawable victimDraw;
	victimDraw.m_ownerObject = victim;
	if (flags & 65536) saboteur->m_pos.z = 5.0f;
	/* the victim's modules: body, special powers, OCLUpdate, SpyVisionUpdates, contain */
	BehaviorModule *mods[10];
	int nm = 0;
	ModuleData plain;
	SabBody *body = 0;
	if (flags & 128) { body = new SabBody(victim, &plain); body->m_max = ctF(cfg[7]); victim->m_body = body; mods[nm++] = body; }
	std::vector<SabSPBehavior *> sps;
	for (unsigned i = 0; i < ((flags >> 9) & 3); ++i) { SabSPBehavior *b = new SabSPBehavior(victim, &plain); b->m_sp.m_oid = 2; b->m_sp.m_idx = nm; sps.push_back(b); mods[nm++] = b; }
	OCLUpdateModuleData oclData;
	oclData.m_minDelay = oclData.m_maxDelay = cfg[11];
	g_sabOcl = 0;
	if (flags & 2048) { g_sabOcl = new OCLUpdate(victim, &oclData); mods[nm++] = g_sabOcl; }
	SpyVisionUpdateModuleData spyData;
	std::vector<SpyVisionUpdate *> spies;
	for (unsigned i = 0; i < ((flags >> 12) & 3); ++i) { SpyVisionUpdate *s = new SpyVisionUpdate(victim, &spyData); spies.push_back(s); mods[nm++] = s; }
	SabContain contain;
	contain.m_list.push_back(obj[2]);
	contain.m_list.push_back(obj[3]);
	victim->m_contain = (flags & 256) ? &contain : 0;
	mods[nm] = 0;
	victim->m_behaviors = mods;
	CrateCollide *m = sabMake(kind, saboteur, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (i == 0 || o[0] == 3) {
			saboteur->m_ai = (flags & 1) ? &sabAi : 0;
			sabAi.m_goal = (flags & 2) ? victim : 0;
			g_sabRel = (int)((flags >> 2) & 3);
			victim->m_ai = (flags & 16) ? (AIUpdateInterface *)&victimAi : 0;
			victim->m_effDead = (flags & 32) != 0;
			victim->m_drawable = (flags & 64) ? &victimDraw : 0;
			g_spLocal = ((flags >> 14) & 3) == 3 ? 0 : &players[(flags >> 14) & 3];
			saboteur->m_pos.z = (flags & 65536) ? 5.0f : 0.0f;
		}
		g_events.clear();
		g_ulog.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		try {
			switch (o[0]) {
			case 0: m->onCollide(o[1] ? victim : 0, 0, 0); break;
			case 1: res = m->wouldLikeToCollideWith(o[1] ? victim : 0) ? 1 : 0; break;
			case 2: victim->m_player = o[1] ? &players[o[1] - 1] : 0;
				for (int p = 0; p < 3; ++p) { players[p].m_objects.clear(); for (int k = 0; k < 4; ++k) if (obj[k]->m_player == &players[p]) players[p].m_objects.push_back(obj[k]); }
				break;
			case 3: flags = (flags & ~0x1c1ffu) | (o[1] & 0x1c1ffu); break;
			case 4: sabSetKinds(victim, o[1]); break;
			case 5: players[o[1] % 3].m_money.m_count = o[2]; break;
			case 6: gl.m_frame = o[1]; break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_sabSaved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_sabSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 10: res = m->isSabotageBuildingCrateCollide() ? 1 : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		if (o[0] == 3) {
			/* the flags take effect for the next op too */
			saboteur->m_ai = (flags & 1) ? &sabAi : 0;
			sabAi.m_goal = (flags & 2) ? victim : 0;
			g_sabRel = (int)((flags >> 2) & 3);
			victim->m_ai = (flags & 16) ? (AIUpdateInterface *)&victimAi : 0;
			victim->m_effDead = (flags & 32) != 0;
			victim->m_drawable = (flags & 64) ? &victimDraw : 0;
			g_spLocal = ((flags >> 14) & 3) == 3 ? 0 : &players[(flags >> 14) & 3];
			saboteur->m_pos.z = (flags & 65536) ? 5.0f : 0.0f;
		}
		putI32(st);
		putI32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(g_sabOcl ? g_sabOcl->getRemainingFrames() : 0xFFFFFFFFu);
		putU32((unsigned)spies.size());
		for (size_t j = 0; j < spies.size(); ++j) { putU32(spies[j]->m_resetTimersNextUpdate ? 1 : 0); putU32(spies[j]->m_disabledUntilFrame); }
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	g_teamRelHook = 0;
	g_findUpdateHook = 0;
	delete m;
	delete d;
	for (size_t j = 0; j < spies.size(); ++j) delete spies[j];
	for (size_t j = 0; j < sps.size(); ++j) delete sps[j];
	delete g_sabOcl;
	g_sabOcl = 0;
	delete body;
	for (int k = 0; k < 4; ++k) { obj[k]->m_behaviors = 0; obj[k]->m_body = 0; obj[k]->m_contain = 0; obj[k]->m_drawable = 0; obj[k]->m_ai = 0; delete obj[k]; }
	TheGameLogic = 0;
	TheAudio = 0;
	TheInGameUI = 0;
	TheRadar = 0;
	TheEva = 0;
	ThePlayerList = 0;
	TheGameText = 0;
	g_spLocal = 0;
	return 0;
}

}  /* extern "C" */
