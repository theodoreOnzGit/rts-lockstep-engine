/* Differential-test driver for the layer-9 crate modules (p1w11-sonnet, crate batch F; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/{CrateCollide, HealCrateCollide, ShroudCrateCollide}.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_dies_d.inc, whose audio manager and whose
** effect log it uses) on the stand-ins of differential/shim_module: three players (type, sciences, a healAllObjects that logs), objects with the
** test's kinds, position, AI, death and player, a partition manager that logs the reveal, a miscellaneous audio whose events are named after their
** fields, a 2D animation collection that logs the lookup, an in-game UI that logs the world animation, a game logic whose destroyObject logs, and
** the REAL CollideModule, INI, FXList parse (wrap_armor.cpp's findFXList) and ScienceStore lookup (the hook of ref_up_reset). Every call a module
** makes is a line of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/CrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/HealCrateCollide.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ShroudCrateCollide.cpp"
/* MoneyCrateCollide.cpp with its function-local `static` made a global and a flag the driver resets (build.rs, mod_gen/: mechanical, asserted to apply
** once); parseUpgradePair is AutoDepositUpdate.cpp's, compiled by differential/l9_updates_l.inc */
namespace { const UpgradeTemplate *g_moneyStatic = 0; bool g_moneyStaticInit = false; }
#include "GameLogic/Module/AutoDepositUpdate.h"
#include "MoneyCrateCollide_static.cpp"

namespace {

const char *const SCIENCE_NAMES_CF[5] = { "SCIENCE_Paradrop", "SCIENCE_Rank3", "SCIENCE_NapalmStrike", "science_lower", "SCIENCE_X" };   /* tests/up_common SCIENCES */
MiscAudio g_cfMisc;
Anim2DTemplate g_cfAnimTemplate;
Anim2DCollection g_cfAnims;
InGameUI g_cfUi;
PartitionManager g_cfPartition;
int g_cfDummyAi = 0;

/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_cfKinds[8] = { KINDOF_STRUCTURE, KINDOF_PARACHUTE, KINDOF_VEHICLE, KINDOF_INFANTRY, KINDOF_AIRCRAFT, KINDOF_SELECTABLE, KINDOF_CAN_ATTACK, KINDOF_HUGE_VEHICLE };

void cfDestroyHook(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }

}  // namespace

Bool Object::isNeutralControlled() const { return m_player && m_player->m_idx == 0; }
void Drawable::flashAsSelected(const RGBColor *color)
{
	if (color) ulog("flashcolor %u,%u,%u", realBits(color->red), realBits(color->green), realBits(color->blue));
	else ulog("flash");
}
void Player::healAllObjects() { ulog("heal %d", (int)m_idx); }
void Money::deposit(UnsignedInt amountToDeposit, Bool playSound) { ulog("deposit %d %u %d", (int)m_idx, amountToDeposit, (int)playSound); }
void ScoreKeeper::addMoneyEarned(Int money) { ulog("earned %d %d", (int)m_idx, (int)money); }
void PartitionManager::revealMapForPlayer(Int playerIndex) { ulog("reveal %d", (int)playerIndex); }
const MiscAudio *AudioManager::getMiscAudio(void) const { return &g_cfMisc; }
Anim2DTemplate *Anim2DCollection::findTemplate(const AsciiString &name) { ulog("findanim %s", name.str()); return &g_cfAnimTemplate; }
void InGameUI::addWorldAnimation(Anim2DTemplate *animTemplate, const Coord3D *pos, WorldAnimationOptions options, Real durationInSeconds, Real zRisePerSecond)
{
	ulog("anim %u,%u,%u %d %u %u", realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)options, realBits(durationInSeconds), realBits(zRisePerSecond));
}
PartitionManager *ThePartitionManager = 0;
Anim2DCollection *TheAnim2DCollection = 0;
InGameUI *TheInGameUI = 0;

namespace {

ModuleData *l9cfRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = HealCrateCollide::friend_newModuleData(&ini); break;
			case 1: data = ShroudCrateCollide::friend_newModuleData(&ini); break;
			default: data = MoneyCrateCollide::friend_newModuleData(&ini); break;
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

std::vector<unsigned char> g_cfSaved;

/* the kinds of an object as a mask of the g_cfKinds bits */
void cfSetKinds(Object *o, unsigned bits)
{
	o->m_kindOf = KindOfMaskType();
	for (int i = 0; i < 8; ++i) if (bits >> i & 1) o->m_kindOf.set(g_cfKinds[i]);
}

/* the data of the crate base, as the Rust test dumps it */
void cfPutBase(const CrateCollideModuleData *c, const unsigned *scienceKeys)
{
	for (int i = 0; i < KINDOF_COUNT; ++i) putU32(c->m_kindof.test((KindOfType)i) ? 1 : 0);
	for (int i = 0; i < KINDOF_COUNT; ++i) putU32(c->m_kindofnot.test((KindOfType)i) ? 1 : 0);
	putU32(c->m_isForbidOwnerPlayer ? 1 : 0);
	putU32(c->m_isBuildingPickup ? 1 : 0);
	putU32(c->m_isHumanOnlyPickup ? 1 : 0);
	int sci = -1;                                        /* the index of the pickup science in the test's list, -1 for none */
	if (c->m_pickupScience != SCIENCE_INVALID) for (int i = 0; i < 5; ++i) if ((unsigned)c->m_pickupScience == scienceKeys[i]) sci = i;
	putI32(sci);
	putI32(fxId(c->m_executeFX));
	putStr(c->m_executionAnimationTemplate.str(), (int)c->m_executionAnimationTemplate.getLength());
	putU32(realBits(c->m_executeAnimationDisplayTimeInSeconds));
	putU32(realBits(c->m_executeAnimationZRisePerSecond));
	putU32(c->m_executeAnimationFades ? 1 : 0);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of crate module `kind` (0 HealCrateCollide, 1 ShroudCrateCollide, 2 MoneyCrateCollide) and make the module on a crate Object
** (id 1) whose controlling player is cfg[0] of three (0xFFFFFFFF none), with a second object (id 2) that collides with it. `cfg`: [the crate's player,
** the other's player (0xFFFFFFFF none), the other's kinds (bits of g_cfKinds), flags (bit 0 the other has an AI, 1 it is effectively dead, 2 the
** animation collection exists, 3 the game logic draws icons), the crate's z (float bits), the other's x, y, z (float bits), the types of the three
** players (bit i set: player i is a computer), the sciences of the three players (5 bits each: bit k of player i's group is the test's science k), the
** completed upgrades of the three players (four words each, from cfg[10])].
** The data dump goes first (the crate base). Then `n` ops of 6 words [op, a, b, c]: 0 onCollide(other: a = 0 NULL, 1 the other object)
** 1 wouldLikeToCollideWith(a)  2 the other's player = a - 1 (0 none)  3 the other has an AI = a  4 the other is effectively dead = a  5 the other's kinds = a
** 6 the other's position = (a, b, c) float bits  7 the crate's z = a  8 flags = a (as cfg[3])  9 xfer save  10 xfer load of the last save  11 xfer CRC
** 12 the crate's player = a - 1 (0 none)
** 13 the players' completed upgrades = (a, 0, 0, 0), (b, 0, 0, 0), (c, 0, 0, 0). Each op appends: the CATCH_ALL status, the Bool result of op 1, the effect log (count, then each as a
** string), the xfer status, its bytes and CRC. */
int ref_l9cf_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_cfSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9cfRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned scienceKeys[5];
	for (int i = 0; i < 5; ++i) scienceKeys[i] = (unsigned)NAMEKEY(SCIENCE_NAMES_CF[i]);
	cfPutBase((CrateCollideModuleData *)d, scienceKeys);
	if (kind == 2) {
		const MoneyCrateCollideModuleData *md = (const MoneyCrateCollideModuleData *)d;
		putU32(md->m_moneyProvided);
		putU32((unsigned)md->m_upgradeBoost.size());
		for (std::list<upgradePair>::const_iterator it = md->m_upgradeBoost.begin(); it != md->m_upgradeBoost.end(); ++it) {
			putStr(it->type.c_str(), (int)it->type.size());
			putI32(it->amount);
		}
	}
	g_moneyStaticInit = false;
	g_moneyStatic = 0;
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_money.m_idx = i;
		players[i].m_score.m_idx = i;
		players[i].m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, cfg + 10 + 4 * i);
		players[i].m_type = (cfg[8] >> i & 1) ? PLAYER_COMPUTER : PLAYER_HUMAN;
		for (int k = 0; k < 5; ++k) if (cfg[9] >> (5 * i + k) & 1) players[i].m_sciences.push_back((ScienceType)scienceKeys[k]);
	}
	Object *crate = newObject();
	Object *other = newObject();
	crate->m_oid = 1;
	other->m_oid = 2;
	crate->m_next = 0;
	other->m_next = crate;
	crate->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	other->m_player = cfg[1] == 0xFFFFFFFFu ? 0 : &players[cfg[1]];
	cfSetKinds(other, cfg[2]);
	other->m_ai = cfg[3] & 1 ? (AIUpdateInterface *)&g_cfDummyAi : 0;
	other->m_effDead = (cfg[3] & 2) != 0;
	memcpy(&crate->m_pos.z, &cfg[4], 4);
	memcpy(&other->m_pos.x, &cfg[5], 4);
	memcpy(&other->m_pos.y, &cfg[6], 4);
	memcpy(&other->m_pos.z, &cfg[7], 4);
	GameLogic gl;
	gl.m_drawIconUI = (cfg[3] & 8) != 0;
	TheGameLogic = &gl;
	TheAudio = &g_ddAudio;
	ThePartitionManager = &g_cfPartition;
	TheInGameUI = &g_cfUi;
	TheAnim2DCollection = (cfg[3] & 4) ? &g_cfAnims : 0;
	g_modDestroyHook = cfDestroyHook;
	g_cfMisc.m_crateHeal.setEventName(AsciiString("crateHeal"));
	g_cfMisc.m_crateShroud.setEventName(AsciiString("crateShroud"));
	g_cfMisc.m_crateMoney.setEventName(AsciiString("crateMoney"));
	CrateCollide *m = 0;
	if (kind == 0) m = new HealCrateCollide(crate, d);
	else if (kind == 1) m = new ShroudCrateCollide(crate, d);
	else m = new MoneyCrateCollide(crate, d);
	unsigned flags = cfg[3];
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
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
			case 3: other->m_ai = o[1] ? (AIUpdateInterface *)&g_cfDummyAi : 0; break;
			case 4: other->m_effDead = o[1] != 0; break;
			case 5: cfSetKinds(other, o[1]); break;
			case 6: memcpy(&other->m_pos.x, &o[1], 4); memcpy(&other->m_pos.y, &o[2], 4); memcpy(&other->m_pos.z, &o[3], 4); break;
			case 7: memcpy(&crate->m_pos.z, &o[1], 4); break;
			case 8: flags = o[1]; gl.m_drawIconUI = (flags & 8) != 0; TheAnim2DCollection = (flags & 4) ? &g_cfAnims : 0; break;
			case 9: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_cfSaved.assign(bytes, bytes + blen); break;
			case 10: {
				std::vector<unsigned char> b = g_cfSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 11: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 12: crate->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 13: for (int p = 0; p < 3; ++p) { unsigned w[4] = { o[1 + p], 0, 0, 0 }; players[p].m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, w); } break;
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
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	delete m;
	delete d;
	delete crate;
	delete other;
	TheGameLogic = 0;
	TheAudio = 0;
	ThePartitionManager = 0;
	TheInGameUI = 0;
	TheAnim2DCollection = 0;
	return 0;
}

}  /* extern "C" */
