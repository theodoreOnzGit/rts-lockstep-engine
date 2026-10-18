/* Differential-test wrapper for Energy (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_energy
** for Player (the brown-out notification, logged here), ThePlayerList
** (sixteen players), the template's energy numbers (ThingTemplate, Object) and
** TheGameLogic->getFrame(). The class names of the stand-ins are renamed below,
** so that no symbol of this archive clashes with another fixture's.
**
** Entry points (extern "C", tests/differential_energy.rs):
**   ref_energy_reset        a fresh Energy, no owner, frame 0
**   ref_energy_op           one call on the Energy, with the frame set first
**   ref_energy_save         XferSave of the current Energy to a file
**   ref_energy_load         XferLoad of a file into a fresh Energy
**   ref_energy_save_version a file of a version 1 or 2 Energy, for the load test */
#define Player EnergyTestPlayer
#define PlayerList EnergyTestPlayerList
#define ThePlayerList EnergyTestThePlayerList
#define ThingTemplate EnergyTestThingTemplate
#define Object EnergyTestObject
#define GameLogic EnergyTestGameLogic
#define TheGameLogic EnergyTestTheGameLogic

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"

/* The real header, with its members readable (the owner, for the check of
** the save): every other name is the upstream one. */
#define private public
#include "Common/Energy.h"
#undef private
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp"

/* ---- stand-ins ----------------------------------------------------------- */
static UnsignedInt g_frame = 0;
static int g_noticeCount = 0;
static int g_noticeBrown = 0;
static int g_noticePlayer = -1;
static EnergyTestPlayer g_players[16];
static EnergyTestPlayerList g_playerList;
static EnergyTestGameLogic g_gameLogic;
static EnergyTestObject g_object;
EnergyTestPlayerList *EnergyTestThePlayerList = &g_playerList;
EnergyTestGameLogic *EnergyTestTheGameLogic = &g_gameLogic;

UnsignedInt EnergyTestGameLogic::getFrame() const { return g_frame; }
Bool EnergyTestGameLogic::isScoringEnabled() const { return TRUE; }

EnergyTestPlayer *EnergyTestPlayerList::getNthPlayer(Int i)
{
	return (i >= 0 && i < 16) ? &g_players[i] : NULL;
}

void EnergyTestPlayer::onPowerBrownOutChange(Bool brownOut)
{
	++g_noticeCount;
	g_noticeBrown = brownOut ? 1 : 0;
	g_noticePlayer = m_index;
}

/* ---- the Energy under test ------------------------------------------------ */
static Energy *g_energy = NULL;

static void ensure_energy(void)
{
	if (!g_energy) {
		for (int i = 0; i < 16; ++i) g_players[i].m_index = i;
		g_energy = new Energy();
	}
}

extern "C" {

void ref_energy_reset(void)
{
	ensure_energy();
	delete g_energy;
	g_energy = new Energy();
	g_frame = 0;
}

/* One call on the Energy at `frame`. op 0 init(owner: a player index, or -1 for
** NULL); 1 adjustPower(a, b != 0); 2 objectEnteringInfluence(template energy a);
** 3 objectLeavingInfluence(a); 4 addPowerBonus(a); 5 removePowerBonus(a);
** 6 setPowerSabotagedTillFrame(a).
** out[0] getProduction(), out[1] getConsumption(), out[2] hasSufficientPower(),
** out[3] the bits of getEnergySupplyRatio(), out[4] getPowerSabotagedTillFrame(),
** out[5] the number of owner notifications, out[6] the last one's brown-out
** flag, out[7] the owner's player index (-1: none). */
void ref_energy_op(int op, int a, int b, unsigned frame, unsigned *out)
{
	ensure_energy();
	g_frame = frame;
	g_noticeCount = 0;
	g_noticeBrown = 0;
	g_noticePlayer = -1;
	switch (op) {
	case 0:
		g_energy->init(a >= 0 && a < 16 ? &g_players[a] : NULL);
		break;
	case 1:
		g_energy->adjustPower(a, b != 0);
		break;
	case 2:
		g_object.m_template.m_energyProduction = a;
		g_energy->objectEnteringInfluence(&g_object);
		break;
	case 3:
		g_object.m_template.m_energyProduction = a;
		g_energy->objectLeavingInfluence(&g_object);
		break;
	case 4:
		g_object.m_template.m_energyBonus = a;
		g_energy->addPowerBonus(&g_object);
		break;
	case 5:
		g_object.m_template.m_energyBonus = a;
		g_energy->removePowerBonus(&g_object);
		break;
	case 6:
		g_energy->setPowerSabotagedTillFrame((UnsignedInt)a);
		break;
	}
	float ratio = g_energy->getEnergySupplyRatio();
	unsigned bits;
	memcpy(&bits, &ratio, sizeof bits);
	out[0] = (unsigned)g_energy->getProduction();
	out[1] = (unsigned)g_energy->getConsumption();
	out[2] = g_energy->hasSufficientPower() ? 1u : 0u;
	out[3] = bits;
	out[4] = g_energy->getPowerSabotagedTillFrame();
	out[5] = (unsigned)g_noticeCount;
	out[6] = (unsigned)g_noticeBrown;
	out[7] = (unsigned)(g_energy->m_owner ? g_energy->m_owner->getPlayerIndex() : -1);
}

/* XferSave of the current Energy to `path`: 0, or the XferStatus + 1. */
int ref_energy_save(const char *path)
{
	ensure_energy();
	XferSave x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_energy);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
	}
	return status;
}

/* XferLoad of `path` into a fresh Energy: out[0] the status (0 none, else the
** XferStatus + 1), out[1] the owner's index (-1 none), out[2] the sabotage
** frame, out[3] getProduction() at frame 0, out[4] getConsumption(). */
void ref_energy_load(const char *path, unsigned *out)
{
	ensure_energy();
	delete g_energy;
	g_energy = new Energy();
	g_frame = 0;
	XferLoad x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_energy);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x.close(); } catch (...) {}
	}
	out[0] = (unsigned)status;
	out[1] = (unsigned)(g_energy->m_owner ? g_energy->m_owner->getPlayerIndex() : -1);
	out[2] = g_energy->getPowerSabotagedTillFrame();
	out[3] = (unsigned)g_energy->getProduction();
	out[4] = (unsigned)g_energy->getConsumption();
}

} /* extern "C" */
