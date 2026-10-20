/* Differential-test wrapper for the layer-9 create modules (p1l9-creates). The unchanged upstream files
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/PreorderCreate.cpp
**   .../Create/SpecialPowerCreate.cpp, SupplyWarehouseCreate.cpp, SupplyCenterCreate.cpp
**   .../Create/LockWeaponCreate.cpp, VeterancyGainCreate.cpp, GrantUpgradeCreate.cpp
** are #included below. CreateModule.cpp is NOT: it is compiled by differential/wrap_module.cpp (the layer-5 base
** fixture) and linked, so the base-class symbols are that build's. Its header is the real one, on the prelude
** differential/shim_creates/create_prelude.h (forced with -include), which renames what this unit stands in for.
**
** Stand-ins (the classes the modules call, defined here, each action logged as one line through crt_log):
** CrtObject (GameLogic/Object.h), CrtPlayer and CrtPlayerList (Common/Player.h, PlayerList.h), CrtRGM,
** CrtAcademyStats, CrtExperienceTracker, the upgrade center CrtUpgradeCenter (the real Upgrade.h, with
** findUpgrade defined here over a table of three upgrades), and TheScienceStore's hook.
**
** Entry point (extern "C", tests/differential_create_modules.rs): ref_create_run reads a block of INI (the
** block `D`, as the module fixture reads its data) and a program of state changes and calls, runs them, and
** writes what happened as text lines. The Rust side writes the same lines for the same program. */

#define DEFINE_OBJECT_STATUS_NAMES
#define DEFINE_VETERANCY_NAMES
#define DEFINE_WEAPONSLOTTYPE_NAMES

#include <stdarg.h>
#include <string>
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/Xfer.h"
#include "Common/ModelState.h"
#include "Common/Upgrade.h"
#include "Common/Player.h"
#include "Common/PlayerList.h"
#include "Common/ResourceGatheringManager.h"
#include "GameLogic/ExperienceTracker.h"
#include "GameLogic/Object.h"
#include "GameLogic/WeaponSet.h"
#include "GameLogic/Module/CreateModule.h"
#include "GameLogic/Module/PreorderCreate.h"
#include "GameLogic/Module/SpecialPowerCreate.h"
#include "GameLogic/Module/SupplyWarehouseCreate.h"
#include "GameLogic/Module/SupplyCenterCreate.h"
#include "GameLogic/Module/LockWeaponCreate.h"
#include "GameLogic/Module/VeterancyGainCreate.h"
#include "GameLogic/Module/GrantUpgradeCreate.h"

/* the upstream files, unchanged */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/PreorderCreate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/SpecialPowerCreate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/SupplyWarehouseCreate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/SupplyCenterCreate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/LockWeaponCreate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/VeterancyGainCreate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/GrantUpgradeCreate.cpp"

/* ---- the log ------------------------------------------------------------------------ */
static std::string g_out;
void crt_log(const char *fmt, ...)
{
	char b[512];
	va_list ap; va_start(ap, fmt); vsnprintf(b, sizeof b, fmt, ap); va_end(ap);
	g_out += b; g_out += '\n';
}

/* ---- the stand-in world ------------------------------------------------------------- */
#define CRT_UPGRADES 3
static unsigned g_upgradeExists[CRT_UPGRADES];
static unsigned g_upgradePlayerType[CRT_UPGRADES];
static const char *g_upgradeNames[CRT_UPGRADES] = { "U0", "U1", "U2" };
/* a real UpgradeTemplate is never constructed here (its constructor is in Upgrade.cpp, not linked): the three
** templates are zeroed storage with m_type set, which is all getUpgradeType reads */
alignas(16) static unsigned char g_templateStore[CRT_UPGRADES][sizeof(CrtUpgradeTemplate)];
static unsigned char g_centerStore[sizeof(CrtUpgradeCenter)];
CrtUpgradeCenter *CrtTheUpgradeCenter = (CrtUpgradeCenter *)g_centerStore;

static int upgradeIndex(const UpgradeTemplate *t)
{
	for (int i = 0; i < CRT_UPGRADES; ++i)
		if (t == (const UpgradeTemplate *)g_templateStore[i]) return i;
	return -1;
}

const CrtUpgradeTemplate *CrtUpgradeCenter::findUpgrade(const AsciiString &name) const
{
	for (int i = 0; i < CRT_UPGRADES; ++i)
		if (g_upgradeExists[i] && name.str() && strcmp(name.str(), g_upgradeNames[i]) == 0)
			return (const CrtUpgradeTemplate *)g_templateStore[i];
	return NULL;
}

static CrtPlayerList g_players;
CrtPlayerList *CrtThePlayerList = &g_players;
static CrtExperienceTracker g_tracker;
static CrtObject g_object;

Player *PlayerList::getNthPlayer(Int i)
{
	if (i < 0 || i >= 16) return NULL;
	return &m_players[i];
}

Player *CrtObject::getControllingPlayer() const { return m_controller; }
void CrtObject::setModelConditionState(ModelConditionFlagType a) { crt_log("set %u %d", m_id, (int)a); }
void CrtObject::clearModelConditionState(ModelConditionFlagType a) { crt_log("clear %u %d", m_id, (int)a); }
Bool CrtObject::setWeaponLock(WeaponSlotType weaponSlot, WeaponLockType lockType)
{
	crt_log("lock %u %d %d", m_id, (int)weaponSlot, (int)lockType);
	return TRUE;
}
void CrtObject::giveUpgrade(const UpgradeTemplate *upgradeT) { crt_log("object-upgrade %u %d", m_id, upgradeIndex(upgradeT)); }

Bool CrtExperienceTracker::isTrainable() const { return m_trainable; }
void CrtExperienceTracker::setMinVeterancyLevel(VeterancyLevel newLevel)
{
	crt_log("minvet %u %d", g_object.m_id, (int)newLevel);
	if (newLevel > m_level) m_level = newLevel;
}

void CrtRGM::addSupplyCenter(Object *newCenter) { crt_log("center %d %u", m_id, newCenter->m_id); }
void CrtRGM::addSupplyWarehouse(Object *newWarehouse) { crt_log("warehouse %d %u", m_id, newWarehouse->m_id); }

Bool CrtPlayer::hasScience(ScienceType t) const
{
	int bit = (int)t - 1; /* SCIENCE_A = 1, SCIENCE_B = 2, SCIENCE_C = 3 */
	return bit >= 0 && bit < 32 ? (m_sciences >> bit) & 1u : 0;
}
Upgrade *CrtPlayer::addUpgrade(const UpgradeTemplate *upgradeTemplate, UpgradeStatusType status)
{
	(void)status;
	crt_log("player-upgrade %d %d", m_index, upgradeIndex(upgradeTemplate));
	return NULL;
}
/* AcademyStats.cpp is not linked here; the stats are only written by recordUpgrade below, so an empty constructor is all
** the player array needs */
CrtAcademyStats::CrtAcademyStats() {}
/* the Snapshot interface of AcademyStats (its key function, so the vtable is emitted here); not called by the create modules */
void CrtAcademyStats::crc(Xfer *) {}
void CrtAcademyStats::xfer(Xfer *) {}
void CrtAcademyStats::loadPostProcess(void) {}
void CrtAcademyStats::recordUpgrade(const UpgradeTemplate *upgrade, Bool granted)
{
	/* the player whose AcademyStats this is: its offset in the player array */
	int index = (int)(((const char *)this - (const char *)&g_players.m_players[0].m_academyStats) / (long)sizeof(CrtPlayer));
	crt_log("academy %d %d %d", index, upgradeIndex(upgrade), granted ? 1 : 0);
}

/* the science names of the INI block: SCIENCE_A = 1, SCIENCE_B = 2, SCIENCE_C = 3; anything else is unknown */
static ScienceType crt_science_lookup(const ScienceStore *, const char *name)
{
	if (strcmp(name, "SCIENCE_A") == 0) return (ScienceType)1;
	if (strcmp(name, "SCIENCE_B") == 0) return (ScienceType)2;
	if (strcmp(name, "SCIENCE_C") == 0) return (ScienceType)3;
	return SCIENCE_INVALID;
}

BehaviorModule **CrtObject::getBehaviorModules() const
{
	crt_log("behaviors %u", m_id);
	return m_behaviors;
}

/* the INI fixture (wrap_ini.cpp) and the science hook (wrap_ini.cpp, ScienceStore::friend_lookupScience) */
extern "C" {
void ref_ini_reset(void);
void ref_ini_add_file(const char *name, const char *data, int len);
extern ScienceType (*ref_science_lookup_hook)(const ScienceStore *, const char *);
}

/* ---- the driver ---------------------------------------------------------------------- */

static BehaviorModule *g_behaviors[1] = { NULL };
static ResourceGatheringManager g_rgm[4];

static void resetWorld(void)
{
	g_out.clear();
	for (int i = 0; i < 16; ++i) {
		Player &p = g_players.m_players[i];
		p.m_index = i;
		p.m_preorder = false;
		p.m_sciences = 0;
		p.m_rgm = NULL;
	}
	for (int i = 0; i < 4; ++i) g_rgm[i].m_id = i;
	g_players.m_playerCount = 0;
	g_object.m_id = 7;
	g_object.m_controller = &g_players.m_players[0];
	g_object.m_status.clear();
	g_object.m_experience = NULL;
	g_tracker.m_trainable = false;
	g_tracker.m_level = LEVEL_REGULAR;
	g_object.m_behaviors = g_behaviors;
	for (int u = 0; u < CRT_UPGRADES; ++u) {
		g_upgradeExists[u] = 0;
		g_upgradePlayerType[u] = 0;
		((CrtUpgradeTemplate *)g_templateStore[u])->m_type = UPGRADE_TYPE_OBJECT;
	}
	CrtThePlayerList = &g_players;
}

/* One module of the kind, made on the block's data, as the module fixture makes its modules: the data is read
** with the real INI from the block (its first line is the header, read and discarded). Returns the kind's
** module through the typed pointers. */
struct CreateUnderTest {
	int kind;
	PreorderCreate *preorder;
	SpecialPowerCreate *special;
	SupplyWarehouseCreate *warehouse;
	SupplyCenterCreate *center;
	LockWeaponCreate *lock;
	VeterancyGainCreate *veterancy;
	GrantUpgradeCreate *grant;
};

static bool makeModule(int kind, CreateUnderTest &c)
{
	c.kind = kind;
	c.preorder = NULL; c.special = NULL; c.warehouse = NULL; c.center = NULL;
	c.lock = NULL; c.veterancy = NULL; c.grant = NULL;
	Thing *thing = &g_object;
	INI ini;
	ini.prepFile(AsciiString("create.ini"), INI_LOAD_OVERWRITE);
	bool ok = true;
	try {
		ini.readLine();
		switch (kind) {
		case 0: case 1: case 2: case 3: {
			CreateModuleData *d = new CreateModuleData;
			ini.initFromINIMultiProc(d, CreateModuleData::buildFieldParse);
			crt_log("data none");
			if (kind == 0) c.preorder = new PreorderCreate(thing, d);
			if (kind == 1) c.special = new SpecialPowerCreate(thing, d);
			if (kind == 2) c.warehouse = new SupplyWarehouseCreate(thing, d);
			if (kind == 3) c.center = new SupplyCenterCreate(thing, d);
			break;
		}
		case 4: {
			LockWeaponCreateModuleData *d = new LockWeaponCreateModuleData;
			ini.initFromINIMultiProc(d, LockWeaponCreateModuleData::buildFieldParse);
			crt_log("data slot %d", d->m_slotToLock);
			c.lock = new LockWeaponCreate(thing, d);
			break;
		}
		case 5: {
			VeterancyGainCreateModuleData *d = new VeterancyGainCreateModuleData;
			ini.initFromINIMultiProc(d, VeterancyGainCreateModuleData::buildFieldParse);
			crt_log("data level %d science %d", (int)d->m_startingLevel, (int)d->m_scienceRequired);
			c.veterancy = new VeterancyGainCreate(thing, d);
			break;
		}
		case 6: {
			GrantUpgradeCreateModuleData *d = new GrantUpgradeCreateModuleData;
			ini.initFromINIMultiProc(d, GrantUpgradeCreateModuleData::buildFieldParse);
			std::string bits;
			for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) bits += d->m_exemptStatus.test(i) ? '1' : '0';
			crt_log("data upgrade %s exempt %s", d->m_upgradeName.str(), bits.c_str());
			c.grant = new GrantUpgradeCreate(thing, d);
			break;
		}
		default: ok = false;
		}
	} catch (...) {
		ok = false;
	}
	ini.unPrepFile();
	return ok;
}

/* The program: see the comment above ref_create_run's words. */
extern "C" int ref_create_run(const char *text, int textLen, const unsigned *prog, int nProg, char *out, int cap, int *outLen)
{
	(void)nProg;
	resetWorld();
	if (!TheScienceStore) TheScienceStore = new ScienceStore;
	ref_science_lookup_hook = crt_science_lookup;
	ref_ini_reset();
	ref_ini_add_file("create.ini", text, textLen);

	int r = 0;
	int kind = (int)prog[r++];
	unsigned nPlayers = prog[r++];
	for (int p = 0; p < 4; ++p) {
		g_players.m_players[p].m_preorder = prog[r++] != 0;
		g_players.m_players[p].m_sciences = prog[r++];
		g_players.m_players[p].m_rgm = prog[r++] != 0 ? &g_rgm[p] : NULL;
	}
	g_players.m_playerCount = nPlayers == 0xffffffffu ? 0 : (int)nPlayers;
	CrtThePlayerList = nPlayers == 0xffffffffu ? NULL : &g_players;
	g_object.m_controller = &g_players.m_players[prog[r++] & 3];
	if (prog[r++]) g_object.m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
	bool tracker = prog[r++] != 0;
	g_tracker.m_trainable = prog[r++] != 0;
	g_object.m_experience = tracker ? &g_tracker : NULL;
	for (int u = 0; u < CRT_UPGRADES; ++u) {
		g_upgradeExists[u] = prog[r++] != 0;
		g_upgradePlayerType[u] = prog[r++] != 0;
		((CrtUpgradeTemplate *)g_templateStore[u])->m_type = g_upgradePlayerType[u] ? UPGRADE_TYPE_PLAYER : UPGRADE_TYPE_OBJECT;
	}

	CreateUnderTest c;
	if (!makeModule(kind, c)) {
		crt_log("data error");
	} else {
		for (;;) {
			unsigned op = prog[r++];
			if (op == 0) break;
			switch (op) {
			case 1: /* onCreate */
				if (c.preorder) c.preorder->onCreate();
				if (c.special) c.special->onCreate();
				if (c.warehouse) c.warehouse->onCreate(); /* SupplyWarehouseCreate::onCreate, via the on_create of the Rust */
				if (c.center) c.center->onCreate();
				if (c.lock) c.lock->onCreate();
				if (c.veterancy) c.veterancy->onCreate();
				if (c.grant) c.grant->onCreate();
				break;
			case 2: /* onBuildComplete */
				if (c.preorder) c.preorder->onBuildComplete();
				if (c.special) c.special->onBuildComplete();
				if (c.center) c.center->onBuildComplete();
				if (c.lock) c.lock->onBuildComplete();
				if (c.grant) c.grant->onBuildComplete();
				break;
			case 3: { unsigned p = prog[r++]; g_players.m_players[p & 3].m_preorder = prog[r++] != 0; break; }
			case 4: { unsigned v = prog[r++]; g_object.m_controller = &g_players.m_players[v & 3]; break; }
			case 5: { unsigned v = prog[r++]; g_object.m_status.clear(); if (v) g_object.m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1); break; }
			case 6: { bool present = prog[r++] != 0; g_tracker.m_trainable = prog[r++] != 0; g_object.m_experience = present ? &g_tracker : NULL; break; }
			case 7: { unsigned n = prog[r++]; g_players.m_playerCount = n == 0xffffffffu ? 0 : (int)n; CrtThePlayerList = n == 0xffffffffu ? NULL : &g_players; break; }
			case 8: { unsigned p = prog[r++]; g_players.m_players[p & 3].m_sciences = prog[r++]; break; }
			case 9: { unsigned p = prog[r++]; g_players.m_players[p & 3].m_rgm = prog[r++] != 0 ? &g_rgm[p & 3] : NULL; break; }
			case 10: { unsigned u = prog[r++] % CRT_UPGRADES; g_upgradeExists[u] = prog[r++] != 0; g_upgradePlayerType[u] = prog[r++] != 0;
				((CrtUpgradeTemplate *)g_templateStore[u])->m_type = g_upgradePlayerType[u] ? UPGRADE_TYPE_PLAYER : UPGRADE_TYPE_OBJECT; break; }
			default: break;
			}
		}
	}
	int n = (int)g_out.size();
	if (outLen) *outLen = n;
	if (n > cap) n = cap;
	memcpy(out, g_out.data(), (size_t)n);
	return 0;
}
