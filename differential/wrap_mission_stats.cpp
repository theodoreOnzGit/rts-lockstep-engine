/* Differential-test wrapper for MissionStats (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/MissionStats.cpp
** (#included below, unchanged). The file includes Common/Player.h, which the
** shim_energy stand-in answers (MissionStats.cpp never calls into a player).
** The header is included with its members readable (`private` made public),
** as the other wrappers do.
**
** Entry points (extern "C", tests/differential_mission_stats.rs):
**   ref_mission_op     one call: 0 init(); 1..4 set one counter; out gets all 34 counters
**   ref_mission_save   XferSave of the MissionStats to a file
**   ref_mission_load   XferLoad of a file into a fresh MissionStats, out gets the counters */
#define Player MissionTestPlayer

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#ifndef PI
#define PI 3.14159265358979323846f /* Lib/trig.h, not in this build (GameCommon.h uses it) */
#endif

#define private public
#include "Common/MissionStats.h"
#undef private
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/MissionStats.cpp"

static MissionStats *g_stats = NULL;

static void ensure(void)
{
	if (!g_stats) g_stats = new MissionStats();
}

static void readout(unsigned *out)
{
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i) out[i] = (unsigned)g_stats->m_unitsKilled[i];
	out[MAX_PLAYER_COUNT] = (unsigned)g_stats->m_unitsLost;
	for (int i = 0; i < MAX_PLAYER_COUNT; ++i) out[MAX_PLAYER_COUNT + 1 + i] = (unsigned)g_stats->m_buildingsKilled[i];
	out[2 * MAX_PLAYER_COUNT + 1] = (unsigned)g_stats->m_buildingsLost;
}

extern "C" {

void ref_mission_op(int op, int idx, int value, unsigned *out)
{
	ensure();
	switch (op) {
	case 0: g_stats->init(); break;
	case 1: g_stats->m_unitsKilled[idx] = value; break;
	case 2: g_stats->m_buildingsKilled[idx] = value; break;
	case 3: g_stats->m_unitsLost = value; break;
	case 4: g_stats->m_buildingsLost = value; break;
	}
	readout(out);
}

int ref_mission_save(const char *path)
{
	ensure();
	XferSave x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_stats);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
	}
	return status;
}

void ref_mission_load(const char *path, unsigned *out)
{
	delete g_stats;
	g_stats = new MissionStats();
	XferLoad x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(g_stats);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x.close(); } catch (...) {}
	}
	readout(out + 1);
	out[0] = (unsigned)status;
}

} /* extern "C" */
