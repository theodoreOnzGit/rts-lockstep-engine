/* Differential-test wrapper for ScriptConditions.cpp (upstream commit 0a05454d), p1w13-sonnet, layer 11 batch C1:
**   GeneralsMD/Code/GameEngine/Include/GameLogic/ScriptConditions.h            (unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptConditions.cpp (SC_SOURCE: build.rs's copy, see there)
** on the stand-ins of differential/shim_conditions (sc_world.h: the world answers from the configuration, the calls with an effect write
** a line), all inside namespace scw. Entry point (extern "C", tests/differential_script_conditions_c1.rs): ref_conditions_run. */
#include <string>
#include <vector>
#include <list>
#include <cstdio>
#include <cstring>
#include <cstdarg>
#include <cstdlib>

namespace scw {
#define private public
#define protected public
#include SC_SOURCE
#undef protected
#undef private

unsigned g_cfg[SC_CFG];
std::vector<std::string> g_log;
void scLog(const char *fmt, ...)
{
	char buf[256];
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(buf, sizeof(buf), fmt, ap);
	va_end(ap);
	g_log.push_back(buf);
}
NameKeyGenerator g_keys;
NameKeyGenerator *TheNameKeyGenerator = &g_keys;
KindOfMaskType KINDOFMASK_NONE;
ThingTemplate g_templates[8];
ThingFactory g_factory;
ThingFactory *TheThingFactory = &g_factory;
PolygonTrigger g_triggers[8];
Player g_players[SC_PLAYERS];
PlayerList g_playerList;
PlayerList *ThePlayerList = &g_playerList;
TeamPrototype g_protos[SC_PROTOS];
Team g_teams[SC_TEAMS];
TeamFactory g_teamFactory;
TeamFactory *TheTeamFactory = &g_teamFactory;
Object g_objects[SC_OBJS];
ScriptEngine g_engine;
ScriptEngine *TheScriptEngine = &g_engine;
TerrainLogic g_terrain;
TerrainLogic *TheTerrainLogic = &g_terrain;
PartitionManager g_partition;
PartitionManager *ThePartitionManager = &g_partition;
SpecialPowerStore g_powers;
SpecialPowerStore *TheSpecialPowerStore = &g_powers;
ScienceStore g_sciences;
ScienceStore *TheScienceStore = &g_sciences;
UpgradeTemplate g_upgrade;
ControlBar g_controlBar;
ControlBar *TheControlBar = &g_controlBar;
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
GameEngine g_gameEngine;
GameEngine *TheGameEngine = &g_gameEngine;
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;
View g_view;
View *TheTacticalView = &g_view;
VictoryConditionsInterface g_victory;
VictoryConditionsInterface *TheVictoryConditions = &g_victory;
GameAudio g_audio;
GameAudio *TheAudio = &g_audio;

/* the players' team lists, the prototypes' instances, the teams' names and members, from the configuration */
void scene()
{
	for (int k = 0; k < 8; ++k) { g_templates[k].m_index = k; g_templates[k].m_name = scName("tmpl", k); g_triggers[k].m_index = k;
		g_powers.m_powers[k].m_index = k; g_controlBar.m_buttons[k].m_index = k; }
	for (int i = 0; i < SC_PLAYERS; ++i) { g_players[i].m_index = i; g_players[i].m_teams.clear(); }
	for (int t = 0; t < SC_TEAMS; ++t) { g_teams[t].m_index = t; g_teams[t].m_name = scName("lone", t); g_teams[t].m_members.clear();
		for (int j = 0; j < SC_OBJS; ++j) if (g_cfg[64 + t] >> (16 + j) & 1) g_teams[t].m_members.push_back(&g_objects[j]); }
	for (int j = 0; j < SC_OBJS; ++j) g_objects[j].m_index = j;
	unsigned taken = 0;
	for (int k = 0; k < SC_PROTOS; ++k) {
		TeamPrototype &p = g_protos[k];
		p.m_instances.clear();
		unsigned w = g_cfg[48 + k];
		for (int t = 0; t < SC_TEAMS; ++t) {
			if ((w >> (4 + t) & 1) && !(taken >> t & 1)) { taken |= 1u << t; p.m_instances.push_back(&g_teams[t]); g_teams[t].m_name = scName("team", k); }
		}
		int owner = (int)(w & 15);
		if (owner < scNumPlayers()) g_players[owner].m_teams.push_back(&p);
	}
}

std::vector<unsigned> g_out;
void put(unsigned u) { g_out.push_back(u); }
void putStr(const std::string &s)
{
	put((unsigned)s.size());
	for (size_t i = 0; i < s.size(); i += 4) {
		unsigned w = 0;
		for (size_t k = 0; k < 4 && i + k < s.size(); ++k) w |= (unsigned)(unsigned char)s[i + k] << (8 * k);
		put(w);
	}
}

}  // namespace scw

extern "C" {

/* `cfg` (SC_CFG words, see the test); `conds`: the count, then per condition its type, its parameter count and per parameter the Int,
** the Real's bits, the status mask's two words, the string's length and its bytes packed 4 to a word. `ops` [op, a, b]: 0 evaluate
** condition a, 1 cfg[a] = b, 2 init, 3 evaluateTeamIsContained(condition a's parameter 0, b), 4 evaluateSkirmishCommandButtonIsReady(
** condition a's parameters 0-2, b) (the script engine's direct calls; a condition with too few parameters is not asked). Each op appends its result, its lines, condition a's custom data, custom frame and parameter Ints
** (op 0), and the transports seen. */
int ref_conditions_run(const unsigned *cfg_in, const unsigned *conds, const unsigned *ops, int n, unsigned *out, int cap, int *len)
{
	using namespace scw;
	memcpy(g_cfg, cfg_in, sizeof(g_cfg));
	g_out.clear();
	g_log.clear();
	g_engine.m_counts.clear();
	std::vector<Condition> cs;
	std::vector<Parameter> ps(12 * 8);
	unsigned nc = *conds++;
	cs.resize(nc);
	for (unsigned c = 0; c < nc; ++c) {
		Condition &cd = cs[c];
		cd.m_conditionType = (Condition::ConditionType)*conds++;
		cd.m_numParms = (Int)*conds++;
		cd.m_customData = 0;
		cd.m_customFrame = 0;
		for (int i = 0; i < MAX_PARMS; ++i) cd.m_parms[i] = NULL;
		for (int i = 0; i < cd.m_numParms; ++i) {
			Parameter &p = ps[c * 12 + i];
			p.m_type = 0;
			p.m_int = (Int)*conds++;
			unsigned rb = *conds++;
			memcpy(&p.m_real, &rb, 4);
			unsigned lo = *conds++, hi = *conds++;
			p.m_status.m_bits = (unsigned long long)lo | ((unsigned long long)hi << 32);
			unsigned sl = *conds++;
			std::string s;
			for (unsigned k = 0; k < sl; k += 4) {
				unsigned w = *conds++;
				for (unsigned b = 0; b < 4 && k + b < sl; ++b) s.push_back((char)(w >> (8 * b) & 255));
			}
			p.m_string.m_s = s;
			cd.m_parms[i] = &p;
		}
	}
	ScriptConditions *sc = new ScriptConditions;
	sc->init();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 3 * i;
		if (o[0] == 1 && o[1] < SC_CFG) g_cfg[o[1]] = o[2];
		scene();
		g_log.clear();
		unsigned res = 0;
		if (o[0] == 0 && o[1] < nc) res = sc->evaluateCondition(&cs[o[1]]) ? 1 : 0;
		if (o[0] == 2) sc->init();
		if (o[0] == 3 && o[1] < nc) res = sc->evaluateTeamIsContained(cs[o[1]].getParameter(0), o[2] != 0) ? 1 : 0;
		if (o[0] == 4 && o[1] < nc) res = sc->evaluateSkirmishCommandButtonIsReady(cs[o[1]].getParameter(0), cs[o[1]].getParameter(1),
			cs[o[1]].getParameter(2), o[2] != 0) ? 1 : 0;
		put(res);
		put((unsigned)g_log.size());
		for (size_t j = 0; j < g_log.size(); ++j) putStr(g_log[j]);
		if (o[0] == 0 && o[1] < nc) {
			Condition &cd = cs[o[1]];
			put((unsigned)cd.m_customData);
			put(cd.m_customFrame);
			for (int k = 0; k < cd.m_numParms; ++k) put((unsigned)cd.m_parms[k]->m_int);
		}
		unsigned count = 0;
		for (TransportStatus *t = s_transportStatuses; t; t = t->m_nextStatus) ++count;
		put(count);
		for (TransportStatus *t = s_transportStatuses; t; t = t->m_nextStatus) { put((unsigned)t->m_objID); put(t->m_frameNumber); put((unsigned)t->m_unitCount); }
	}
	delete sc;
	*len = (int)g_out.size();
	if ((int)g_out.size() > cap) return 1;
	memcpy(out, &g_out[0], g_out.size() * 4);
	return 0;
}

}  /* extern "C" */
