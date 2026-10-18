/* Differential-test wrapper for ScoreKeeper (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/ScoreKeeper.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_score
** for Player, ThingTemplate (name, kind-of mask, equivalence), ThingFactory
** (findTemplate), Object and TheGameLogic->isScoringEnabled(). The stand-ins
** are renamed below, so that no symbol of this archive clashes with another
** fixture's.
**
** A fixed table of NT_TPL templates is set up per case (ref_score_setup):
** each has a name, the kind bits it has, an equivalence group, and whether
** the factory knows its name (an unknown one is a load error, as upstream).
**
** Entry points (extern "C", tests/differential_score_keeper.rs):
**   ref_score_setup    the template table
**   ref_score_fresh    a fresh ScoreKeeper (constructor: reset(0))
**   ref_score_op       one call on it; then the readout (OUT_LEN values)
**   ref_score_save     XferSave of the ScoreKeeper to a file
**   ref_score_load     XferLoad of a file into a fresh ScoreKeeper, readout */
#define Player ScoreTestPlayer
#define ThingTemplate ScoreTestThingTemplate
#define ThingFactory ScoreTestThingFactory
#define TheThingFactory ScoreTestTheThingFactory
#define Object ScoreTestObject
#define GameLogic ScoreTestGameLogic
#define TheGameLogic ScoreTestTheGameLogic

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <bitset>
#include <map>
#include <string>
#include <vector>
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/KindOf.h"
#include "Common/AsciiString.h"
#ifndef PI
#define PI 3.14159265358979323846f /* Lib/trig.h, not in this build (GameCommon.h uses it) */
#endif
#include "Common/GameCommon.h"

/* The real header, with its members readable (the per-owner arrays and the
** current score are read by the readout). */
#define private public
#include "Common/ScoreKeeper.h"
#undef private
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/ScoreKeeper.cpp"

enum { NT_TPL = 12, KIND_BITS = 6, OUT_LEN = 14 + NT_TPL };

/* ---- stand-ins ----------------------------------------------------------- */
static ScoreTestGameLogic g_gameLogic;
ScoreTestGameLogic *ScoreTestTheGameLogic = &g_gameLogic;
static bool g_scoring = true;
Bool ScoreTestGameLogic::isScoringEnabled() const { return g_scoring ? TRUE : FALSE; }

static ScoreTestPlayer g_players[16] = {
	ScoreTestPlayer(0), ScoreTestPlayer(1), ScoreTestPlayer(2), ScoreTestPlayer(3),
	ScoreTestPlayer(4), ScoreTestPlayer(5), ScoreTestPlayer(6), ScoreTestPlayer(7),
	ScoreTestPlayer(8), ScoreTestPlayer(9), ScoreTestPlayer(10), ScoreTestPlayer(11),
	ScoreTestPlayer(12), ScoreTestPlayer(13), ScoreTestPlayer(14), ScoreTestPlayer(15)};

struct TplRow { std::string name; int group; bool known; };
static TplRow g_rows[NT_TPL];
static ScoreTestThingTemplate g_templates[NT_TPL];

AsciiString ScoreTestThingTemplate::getName() const { return m_name; }
Bool ScoreTestThingTemplate::isEquivalentTo(const ScoreTestThingTemplate *tt) const
{
	return m_group == tt->m_group ? TRUE : FALSE;
}

static ScoreTestThingFactory g_factory;
ScoreTestThingFactory *ScoreTestTheThingFactory = &g_factory;
const ScoreTestThingTemplate *ScoreTestThingFactory::findTemplate(AsciiString name) const
{
	for (int i = 0; i < NT_TPL; ++i)
		if (g_rows[i].known && g_rows[i].name == std::string(name.str())) return &g_templates[i];
	return NULL;
}

/* The ScoreKeeper under test. */
static ScoreKeeper *g_sk = NULL;

static void ensure_sk(void)
{
	if (!g_sk) g_sk = new ScoreKeeper();
}

static void readout(unsigned *out)
{
	ensure_sk();
	ScoreKeeper &s = *g_sk;
	out[0] = (unsigned)s.getTotalMoneyEarned();
	out[1] = (unsigned)s.getTotalMoneySpent();
	out[2] = (unsigned)s.getTotalUnitsDestroyed();
	out[3] = (unsigned)s.getTotalUnitsBuilt();
	out[4] = (unsigned)s.getTotalUnitsLost();
	out[5] = (unsigned)s.getTotalBuildingsDestroyed();
	out[6] = (unsigned)s.getTotalBuildingsBuilt();
	out[7] = (unsigned)s.getTotalBuildingsLost();
	out[8] = (unsigned)s.getTotalTechBuildingsCaptured();
	out[9] = (unsigned)s.getTotalFactionBuildingsCaptured();
	out[10] = (unsigned)s.m_currentScore;
	/* getTotalUnitsBuilt(valid, invalid) for three mask pairs */
	KindOfMaskType v0, i0, v1, i1, v2, i2;
	v0.set(KINDOF_VEHICLE); i0.set(KINDOF_SCORE);
	v1.set(KINDOF_STRUCTURE); i1.set(KINDOF_SCORE_DESTROY);
	v2.set(KINDOF_INFANTRY); i2 = KindOfMaskType();
	out[11] = (unsigned)s.getTotalUnitsBuilt(v0, i0);
	out[12] = (unsigned)s.getTotalUnitsBuilt(v1, i1);
	out[13] = (unsigned)s.getTotalUnitsBuilt(v2, i2);
	/* getTotalObjectsBuilt(template k) for each template */
	for (int k = 0; k < NT_TPL; ++k)
		out[14 + k] = (unsigned)s.getTotalObjectsBuilt(&g_templates[k]);
}

extern "C" {

void ref_score_setup(const char *names, const int *groups, const int *known, const int *kindBits)
{
	/* names: NT_TPL NUL-terminated names in a row; kindBits: NT_TPL x KIND_BITS, -1 unused */
	const char *p = names;
	for (int i = 0; i < NT_TPL; ++i) {
		g_rows[i].name = p;
		p += strlen(p) + 1;
		g_rows[i].group = groups[i];
		g_rows[i].known = known[i] != 0;
		g_templates[i].m_name = AsciiString(g_rows[i].name.c_str());
		g_templates[i].m_group = groups[i];
		g_templates[i].m_kindOf.clear();
		for (int b = 0; b < KIND_BITS; ++b) {
			int bit = kindBits[i * KIND_BITS + b];
			if (bit >= 0) g_templates[i].m_kindOf.set(bit);
		}
	}
}

void ref_score_fresh(void)
{
	delete g_sk;
	g_sk = new ScoreKeeper();
}

/* One call on the ScoreKeeper. op 0 reset(amount); 1 addMoneySpent(amount);
** 2 addMoneyEarned(amount); 3 addObjectBuilt; 4 removeObjectBuilt;
** 5 addObjectCaptured; 6 addObjectDestroyed; 7 addObjectLost (each on the
** object of template t, controller c, under construction uc, with the
** scoring flag); 8 calculateScore(). `out` gets the readout (OUT_LEN). */
void ref_score_op(int op, int t, int c, int uc, int scoring, int amount, unsigned *out)
{
	ensure_sk();
	g_scoring = scoring != 0;
	ScoreTestObject o;
	o.m_template = &g_templates[t];
	o.m_controller = &g_players[c];
	o.m_underConstruction = uc != 0;
	switch (op) {
	case 0: g_sk->reset(amount); break;
	case 1: g_sk->addMoneySpent(amount); break;
	case 2: g_sk->addMoneyEarned(amount); break;
	case 3: g_sk->addObjectBuilt(&o); break;
	case 4: g_sk->removeObjectBuilt(&o); break;
	case 5: g_sk->addObjectCaptured(&o); break;
	case 6: g_sk->addObjectDestroyed(&o); break;
	case 7: g_sk->addObjectLost(&o); break;
	case 8: g_sk->calculateScore(); break;
	}
	g_scoring = true;
	readout(out);
}

/* XferSave of the ScoreKeeper to `path`: 0, or 1 when it threw. */
int ref_score_save(const char *path)
{
	ensure_sk();
	XferSave x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_sk);
		x.close();
	} catch (...) {
		status = 1;
		try { x.close(); } catch (...) {}
	}
	return status;
}

/* XferLoad of `path` into a fresh ScoreKeeper: out[0] the status (0 none, 1 it
** threw), then the readout (OUT_LEN values) when it did not throw. */
void ref_score_load(const char *path, unsigned *out)
{
	ref_score_fresh();
	XferLoad x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_sk);
		x.close();
	} catch (...) {
		status = 1;
		try { x.close(); } catch (...) {}
	}
	out[0] = (unsigned)status;
	if (status == 0) readout(out + 1);
	else memset(out + 1, 0, sizeof(unsigned) * OUT_LEN);
}

} /* extern "C" */
