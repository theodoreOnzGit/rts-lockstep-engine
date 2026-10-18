/* Differential-test wrapper for AcademyStats (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/AcademyStats.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_academy
** for the player (its template side, money, energy, objects, its own
** statistics), the local and neutral players, the templates and objects the
** advice inspects, the game text (a tip's label is its text), the global data
** and the frame. The class names of the stand-ins are renamed below.
**
** The real AcademyStats.h is included with its members readable (`private`
** made public, for the probe and the advice inputs), as the Science and
** ScoreKeeper wrappers do.
**
** Entry points (extern "C", tests/differential_academy_stats.rs):
**   ref_academy_init      init(player) for a side (0 none, 1 USA, 2 China, 3 GLA, 4 Civilian)
**   ref_academy_update    update() with money, power and the player's objects
**   ref_academy_record    one record call (code: see the function)
**   ref_academy_advice    calculateAcademyAdvice: the tips' labels
**   ref_academy_probe     four readouts
**   ref_academy_save / ref_academy_load  the xfer of the statistics */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <bitset>
#include <map>
#include <string>
#include <vector>
/* Lib/trig.h is not in this build; GameCommon.h (LOGICFRAMES_PER_SECOND) uses its PI */
#ifndef PI
#define PI 3.14159265358979323846f
#endif
#include "Common/GameCommon.h"

/* the names table is also cut into the upgrade build (build.rs): this archive renames its own copy */
#define TheAcademyClassificationTypeNames AcadTestTheAcademyClassificationTypeNames
#define Player AcadTestPlayer
#define PlayerList AcadTestPlayerList
#define ThePlayerList AcadTestThePlayerList
#define PlayerTemplate AcadTestPlayerTemplate
#define Energy AcadTestEnergy
#define Money AcadTestMoney
#define SpecialPowerTemplate AcadTestSpecialPowerTemplate
#define UpgradeTemplate AcadTestUpgradeTemplate
#define ThingTemplate AcadTestThingTemplate
#define CommandSet AcadTestCommandSet
#define CommandButton AcadTestCommandButton
#define ControlBar AcadTestControlBar
#define TheControlBar AcadTestTheControlBar
#define GameTextInterface AcadTestGameTextInterface
#define TheGameText AcadTestTheGameText
#define GameLogic AcadTestGameLogic
#define TheGameLogic AcadTestTheGameLogic
#define Object AcadTestObject
#define ContainModuleInterface AcadTestContainModuleInterface
#define GlobalData AcadTestGlobalData
#define TheGlobalData AcadTestTheGlobalData
#define UnicodeString AcadTestUnicodeString

#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/KindOf.h"
#include "Common/GlobalData.h"
#include "Common/AcademyStats.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/AcademyStats.cpp"

/* ---- stand-ins ----------------------------------------------------------- */
static AcadTestGameLogic g_gameLogic;
AcadTestGameLogic *AcadTestTheGameLogic = &g_gameLogic;
static UnsignedInt g_frame = 0;
UnsignedInt AcadTestGameLogic::getFrame() const { return g_frame; }

static AcadTestPlayer g_player;   /* the player under test: its AcademyStats is g_player.m_academy */
static AcadTestPlayer g_local;    /* the local player (its mines are read) */
static AcadTestPlayer g_neutral;  /* the neutral player (its vehicles sniped are read) */
static AcadTestPlayerList g_playerList;
AcadTestPlayerList *AcadTestThePlayerList = &g_playerList;
AcadTestPlayer *AcadTestPlayerList::getLocalPlayer() const { return &g_local; }
AcadTestPlayer *AcadTestPlayerList::getNeutralPlayer() const { return &g_neutral; }

static AcadTestMoney g_money;
static AcadTestEnergy g_energy;
static AcadTestObject g_objects[64];
void AcadTestPlayer::iterateObjects(void (*fn)(AcadTestObject *, void *), void *userData) const
{
	for (int i = 0; i < m_nobj; ++i) fn(m_objects[i], userData);
}

Int AcadTestThingTemplate::calcCostToBuild(const AcadTestPlayer *) const { return 777; }

static AcadTestGlobalData g_globals;
AcadTestGlobalData *AcadTestTheGlobalData = &g_globals;

static AcadTestGameTextInterface g_text;
AcadTestGameTextInterface *AcadTestTheGameText = &g_text;
AcadTestUnicodeString AcadTestGameTextInterface::fetch(const char *label) const
{
	WideChar buf[256];
	int n = 0;
	for (const char *p = label; *p && n < 255; ++p) buf[n++] = (WideChar)(unsigned char)*p;
	buf[n] = 0;
	return AcadTestUnicodeString(buf);
}

/* the dozer's command set: its buttons name a supply center and a command center */
static AcadTestThingTemplate g_supplyCenter, g_commandCenter;
static AcadTestCommandSet g_dozerSet;
static void set_center_kinds(void)
{
	g_supplyCenter.m_kindOf.set(KINDOF_FS_SUPPLY_CENTER);
	g_commandCenter.m_kindOf.set(KINDOF_COMMANDCENTER);
}
static AcadTestControlBar g_controlBar;
AcadTestControlBar *AcadTestTheControlBar = &g_controlBar;
const AcadTestCommandSet *AcadTestControlBar::findCommandSet(AsciiString) const { return &g_dozerSet; }

/* the side names: index 0 is "no player template" (see ref_academy_init) */
static const char *g_sideNames[5] = {"", "USA", "China", "GLA", "Civilian"};

/* the advice's labels, joined by '\n' into the caller's buffer */
static void append_label(const AcadTestUnicodeString &text, std::string &out)
{
	const std::basic_string<WideChar> &u = text.units();
	/* the advice's header ("\n\n") is the client's formatting: skip it */
	size_t start = 0;
	while (start < u.size() && (u[start] == '\n')) ++start;
	if (!out.empty()) out.push_back('\n');
	for (size_t i = start; i < u.size(); ++i) out.push_back((char)u[i]);
}

extern "C" {

/* The player's objects: nobj objects of 6 kind bits (-1 unused) and a tunnel flag. */
static void set_objects(const int *kinds, const int *tunnels, int nobj)
{
	static AcadTestContainModuleInterface tunnel;
	tunnel.m_tunnel = true;
	for (int i = 0; i < nobj && i < 64; ++i) {
		g_objects[i].m_kindOf.clear();
		for (int b = 0; b < 6; ++b) {
			int bit = kinds[i * 6 + b];
			if (bit >= 0) g_objects[i].m_kindOf.set(bit);
		}
		g_objects[i].m_contain = tunnels[i] ? &tunnel : NULL;
		g_player.m_objects[i] = &g_objects[i];
	}
	g_player.m_nobj = nobj < 64 ? nobj : 64;
}

/* init(player): the frame, the side (0 none, 1 USA, 2 China, 3 GLA, 4 Civilian)
** and the player's objects (set first, as init iterates them). */
void ref_academy_init(unsigned frame, int side, const int *kinds, const int *tunnels, int nobj)
{
	g_frame = frame;
	g_player.m_hasTemplate = side != 0;
	g_player.m_template.m_baseSide = AsciiString(g_sideNames[side]);
	g_player.m_money = &g_money;
	g_player.m_energy = &g_energy;
	set_objects(kinds, tunnels, nobj);
	g_dozerSet.m_buttons[0].m_thing = &g_supplyCenter;
	g_dozerSet.m_buttons[1].m_thing = &g_commandCenter;
	set_center_kinds();
	g_player.m_academy.init(&g_player);
}

/* update(): frame, money (-1: no money object), power (-1: no energy object, 0/1),
** and the player's objects (as set_objects). */
void ref_academy_update(unsigned frame, int money, int power, const int *kinds, const int *tunnels, int nobj)
{
	g_frame = frame;
	set_objects(kinds, tunnels, nobj);
	g_player.m_money = money >= 0 ? &g_money : NULL;
	if (money >= 0) g_money.m_amount = (UnsignedInt)money;
	g_player.m_energy = power >= 0 ? &g_energy : NULL;
	if (power >= 0) g_energy.m_sufficient = power != 0;
	g_player.m_academy.update();
}

/* One record call. code 0 recordProduction(object of kinds, tunnel a);
** 1 recordUpgrade(classification a, granted b); 2 recordSpecialPowerUsed(a);
** 3 recordIncome(); 4 recordBuildingCapture(); 5 recordGeneralsPointsSpent(a);
** 6 recordBuildingGarrisoned(); 7 recordDragSelection(); 8 recordStrategyCenter();
** 9 recordBattlePlanSelected(); 10 recordUnitEnteredTunnelNetwork();
** 11 recordControlGroupsUsed(); 12 recordClearedGarrisonedBuilding();
** 13 recordVehicleDisguised(); 14 recordFirestormCreated(); 15 recordGuardAbilityUsed();
** 16 recordSalvageCollected(); 17 recordDoubleClickAttackMoveOrderGiven();
** 18 recordMineCleared(); 19 recordVehicleSniped(); 20 recordMine(). */
void ref_academy_record(int code, unsigned frame, int a, int b, const int *kinds, int tunnel)
{
	g_frame = frame;
	AcadTestObject o;
	o.m_kindOf.clear();
	for (int k = 0; k < 6; ++k) {
		if (kinds[k] >= 0) o.m_kindOf.set(kinds[k]);
	}
	static AcadTestContainModuleInterface tunnelContain;
	tunnelContain.m_tunnel = tunnel != 0;
	o.m_contain = tunnel ? &tunnelContain : NULL;
	AcadTestUpgradeTemplate upgrade;
	upgrade.m_academyClassificationType = (AcademyClassificationType)a;
	AcadTestSpecialPowerTemplate power;
	power.m_academyClassificationType = (AcademyClassificationType)a;
	AcademyStats &s = g_player.m_academy;
	switch (code) {
	case 0: s.recordProduction(&o, NULL); break;
	case 1: s.recordUpgrade(&upgrade, b != 0); break;
	case 2: s.recordSpecialPowerUsed(&power); break;
	case 3: s.recordIncome(); break;
	case 4: s.recordBuildingCapture(); break;
	case 5: s.recordGeneralsPointsSpent(a); break;
	case 6: s.recordBuildingGarrisoned(); break;
	case 7: s.recordDragSelection(); break;
	case 8: s.recordStrategyCenter(); break;
	case 9: s.recordBattlePlanSelected(); break;
	case 10: s.recordUnitEnteredTunnelNetwork(); break;
	case 11: s.recordControlGroupsUsed(); break;
	case 12: s.recordClearedGarrisonedBuilding(); break;
	case 13: s.recordVehicleDisguised(); break;
	case 14: s.recordFirestormCreated(); break;
	case 15: s.recordGuardAbilityUsed(); break;
	case 16: s.recordSalvageCollected(); break;
	case 17: s.recordDoubleClickAttackMoveOrderGiven(); break;
	case 18: s.recordMineCleared(); break;
	case 19: s.recordVehicleSniped(); break;
	case 20: s.recordMine(); break;
	}
}

/* calculateAcademyAdvice at `frame`, with the local player's mines, the neutral
** player's vehicles sniped and the alternate-mouse option. Returns numTips; the
** tips' labels go to `labels` ('\n'-separated, NUL-terminated, at most cap bytes). */
int ref_academy_advice(unsigned frame, int localMines, int neutralSniped, int altMouse, char *labels, int cap)
{
	g_frame = frame;
	/* the two statistics the advice reads are set by the calls that count them: init
	** clears them, then the counts are recorded */
	g_local.m_academy.init(&g_local);
	for (int i = 0; i < localMines; ++i) g_local.m_academy.recordMine();
	g_neutral.m_academy.init(&g_neutral);
	for (int i = 0; i < neutralSniped; ++i) g_neutral.m_academy.recordVehicleSniped();
	g_globals.m_useAlternateMouse = altMouse != 0;
	AcademyAdviceInfo info;
	info.numTips = 0;
	g_player.m_academy.calculateAcademyAdvice(&info);
	std::string text;
	for (UnsignedInt i = 0; i < info.numTips && i < MAX_ADVICE_TIPS; ++i) append_label(info.advice[i], text);
	if ((int)text.size() + 1 <= cap) memcpy(labels, text.c_str(), text.size() + 1);
	return (int)info.numTips;
}

/* out: isFirstUpdate, hadASupplyCenter, getMines(), getVehiclesSniped(),
** getCommandCenterTemplate() != NULL */
void ref_academy_probe(unsigned *out)
{
	AcademyStats &s = g_player.m_academy;
	out[0] = s.isFirstUpdate() ? 1u : 0u;
	out[1] = s.hadASupplyCenter() ? 1u : 0u;
	out[2] = s.getMines();
	out[3] = s.getVehiclesSniped();
	out[4] = s.getCommandCenterTemplate() != NULL ? 1u : 0u;
}

/* XferSave of the statistics to `path`: 0, or the XferStatus + 1. */
int ref_academy_save(const char *path)
{
	XferSave x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(&g_player.m_academy);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
	}
	return status;
}

/* XferLoad of `path` into fresh statistics: out[0] the status, then the probe (out[1..6]). */
void ref_academy_load(const char *path, unsigned *out)
{
	g_player.m_academy = AcademyStats();
	XferLoad x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(&g_player.m_academy);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x.close(); } catch (...) {}
	}
	out[0] = (unsigned)status;
	ref_academy_probe(out + 1);
}

} /* extern "C" */
