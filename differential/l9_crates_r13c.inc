/* Differential-test driver for SalvageCrateCollide (p1w13-sonnet, batch R13c; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SalvageCrateCollide.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end (CrateCollide.cpp is compiled by l9_crates_f.inc), on the module build's stand-ins: the crate (id 1, at the test's
** height) and the collector (id 2: kinds, player, level, whether it trains, a body holding armor set flags, the weapon set flags, maybe a drawable); three
** players (money, score, academy stats, colours); the logic random; an in-game UI, a game text and an audio manager that log. Every call is a line of the
** effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SalvageCrateCollide.cpp"

/* Object.cpp:1211-1226, verbatim: the armor set flags are the body's, read unchecked */
void Object::setArmorSetFlag(ArmorSetType ast)
{
	m_body->setArmorSetFlag(ast);
}
void Object::clearArmorSetFlag(ArmorSetType ast)
{
	m_body->clearArmorSetFlag(ast);
}
Bool Object::testArmorSetFlag(ArmorSetType ast) const
{
	return m_body->testArmorSetFlag(ast);
}
void PowerAcademyStats::recordSalvageCollected() { ulog("salvage %d", (int)((Player *)this)->m_idx); }

namespace {

/* the kinds the test gives the collector, by bit (the same list in the Rust test) */
const KindOfType g_r13cKinds[8] = { KINDOF_SALVAGER, KINDOF_WEAPON_SALVAGER, KINDOF_ARMOR_SALVAGER, KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_STRUCTURE,
	KINDOF_PARACHUTE, KINDOF_HERO };

/* the collector's body: the armor set flags, as a body module keeps them */
class R13cBody : public InactiveBody {
public:
	R13cBody(Thing *t, const ModuleData *d) : InactiveBody(t, d), m_armor(0) {}
	virtual void setArmorSetFlag(ArmorSetType ast) { m_armor |= 1u << ast; }
	virtual void clearArmorSetFlag(ArmorSetType ast) { m_armor &= ~(1u << ast); }
	virtual Bool testArmorSetFlag(ArmorSetType ast) { return (m_armor >> ast & 1) != 0; }
	unsigned m_armor;
};

void r13cSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 8; ++i) if (bits >> i & 1) o->m_kindOf.set(g_r13cKinds[i]);
	o->m_tmpl.m_kindOf = o->m_kindOf;
}

ModuleData *l9r13cRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SalvageCrateCollide::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r13cSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SalvageCrateCollide and make the module on the crate (id 1). `cfg` (12 words): [0 the crate's player
** (0xFFFFFFFF none), 1 the collector's player, 2 the collector's kinds (bits of g_r13cKinds), 3 flags (bit 0 the collector has a drawable, 1 it is
** effectively dead, 2 it trains, 3-4 its level, 5-7 players 0 to 2 are computers, 8 the collector has an AI), 4 the body's armor set flags (bit 0 CRATE_UPGRADE_ONE, 1 _TWO), 5 the
** weapon set flags (bit 0 CRATEUPGRADE_ONE, 1 _TWO), 6 the logic random seed, 7 the players' colour base (player i has base + i), 8 the crate's z (float
** bits)]. The data dump goes first (the crate base and the class's five fields). Then `n` ops of 6 words [op, a, b, c]: 0 onCollide(a ? collector : NULL)
** 1 wouldLikeToCollideWith(a ? collector : NULL)  2 the collector's player = a - 1 (0 none)  3 flags bits 0-4 and 8 = a  4 the collector's kinds = a  5 armor
** set flags = a, weapon set flags = b  6 the crate's z = a (float bits)  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the
** CATCH_ALL status, the result, the effect log (count, then each), the xfer status, bytes and CRC, then the armor set flags and the weapon set flags. */
int ref_l9r13c_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r13cSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r13cRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	cfPutBase((CrateCollideModuleData *)d, scienceKeys);
	const SalvageCrateCollideModuleData *sd = (const SalvageCrateCollideModuleData *)d;
	putU32(realBits(sd->m_weaponChance));
	putU32(realBits(sd->m_levelChance));
	putU32(realBits(sd->m_moneyChance));
	putI32(sd->m_minimumMoney);
	putI32(sd->m_maximumMoney);
	GameLogic gl;
	gl.m_frame = 0;
	gl.m_drawIconUI = false;
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[6]);
	unsigned flags = cfg[3];
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_money.m_idx = i;
		players[i].m_score.m_idx = i;
		players[i].m_color = (Color)(cfg[7] + i);
		players[i].m_type = (flags >> (5 + i) & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;   /* the stand-in has no constructor */
	}
	g_spPlayerPtrs[0] = &players[0]; g_spPlayerPtrs[1] = &players[1]; g_spPlayerPtrs[2] = &players[2];
	g_spPlayerCount = 3;
	g_spLocal = 0;
	ThePlayerList = &g_spList;
	TheAudio = &g_ddAudio;
	TheInGameUI = &g_cfUi;
	TheGameText = &g_sabText;
	g_modDestroyHook = cfDestroyHook;
	g_cfMisc.m_crateSalvage.setEventName(AsciiString("crateSalvage"));
	g_cfMisc.m_crateMoney.setEventName(AsciiString("crateMoney"));
	Team teams[2];
	Object *obj[2];
	ExperienceTracker xps[2];
	for (int k = 0; k < 2; ++k) {
		obj[k] = newObject();
		obj[k]->m_oid = (ObjectID)(k + 1);
		teams[k].m_dummy = k + 1;
		obj[k]->m_team = &teams[k];
		obj[k]->m_next = k ? obj[k - 1] : 0;
		xps[k].m_owner = obj[k];
		obj[k]->m_xp = &xps[k];
	}
	Object *crate = obj[0], *other = obj[1];
	crate->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	other->m_player = cfg[1] == 0xFFFFFFFFu ? 0 : &players[cfg[1]];
	crate->m_pos.z = ctF(cfg[8]);
	r13cSetKinds(other, cfg[2]);
	ModuleData plain;
	R13cBody body(other, &plain);
	body.m_armor = (cfg[4] & 1 ? 1u << ARMORSET_CRATE_UPGRADE_ONE : 0) | (cfg[4] & 2 ? 1u << ARMORSET_CRATE_UPGRADE_TWO : 0);
	other->m_body = &body;
	other->m_weaponSetFlags[0] = (cfg[5] & 1 ? 1ULL << WEAPONSET_CRATEUPGRADE_ONE : 0) | (cfg[5] & 2 ? 1ULL << WEAPONSET_CRATEUPGRADE_TWO : 0);
	Drawable draw;
	draw.m_ownerObject = other;
	int otherAi = 0;
	CrateCollide *m = new SalvageCrateCollide(crate, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (i == 0 || o[0] == 3) {
			if (o[0] == 3) flags = (flags & ~0x11Fu) | (o[1] & 0x11Fu);
			other->m_ai = (flags & 256) ? (AIUpdateInterface *)&otherAi : 0;
			other->m_drawable = (flags & 1) ? &draw : 0;
			other->m_effDead = (flags & 2) != 0;
			xps[1].m_trainable = (flags & 4) != 0;
			other->m_level = (VeterancyLevel)((flags >> 3) & 3);
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
			case 0: m->onCollide(o[1] ? other : 0, 0, 0); break;
			case 1: res = m->wouldLikeToCollideWith(o[1] ? other : 0) ? 1 : 0; break;
			case 2: other->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 3: break;
			case 4: r13cSetKinds(other, o[1]); break;
			case 5:
				body.m_armor = (o[1] & 1 ? 1u << ARMORSET_CRATE_UPGRADE_ONE : 0) | (o[1] & 2 ? 1u << ARMORSET_CRATE_UPGRADE_TWO : 0);
				other->m_weaponSetFlags[0] = (o[2] & 1 ? 1ULL << WEAPONSET_CRATEUPGRADE_ONE : 0) | (o[2] & 2 ? 1ULL << WEAPONSET_CRATEUPGRADE_TWO : 0);
				break;
			case 6: crate->m_pos.z = ctF(o[1]); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r13cSaved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r13cSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
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
		putU32((body.m_armor >> ARMORSET_CRATE_UPGRADE_ONE & 1) | (body.m_armor >> ARMORSET_CRATE_UPGRADE_TWO & 1) << 1);
		putU32((unsigned)((other->m_weaponSetFlags[0] >> WEAPONSET_CRATEUPGRADE_ONE & 1) | (other->m_weaponSetFlags[0] >> WEAPONSET_CRATEUPGRADE_TWO & 1) << 1));
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	delete m;
	delete d;
	for (int k = 0; k < 2; ++k) { obj[k]->m_xp = 0; obj[k]->m_body = 0; obj[k]->m_drawable = 0; obj[k]->m_ai = 0; delete obj[k]; }
	TheGameLogic = 0;
	TheAudio = 0;
	TheInGameUI = 0;
	TheGameText = 0;
	ThePlayerList = 0;
	return 0;
}

}  /* extern "C" */
