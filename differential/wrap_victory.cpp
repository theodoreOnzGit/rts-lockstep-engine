/* Differential-test wrapper for VictoryConditions.cpp (upstream commit 0a05454d), p1w13-sonnet, layer 11 batch V1:
**   GeneralsMD/Code/GameEngine/Include/GameLogic/VictoryConditions.h            (unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/VictoryConditions.cpp (unchanged, #included below)
** on the stand-ins of differential/shim_victory (vc_world.h: the players answer from the configuration, every call with an effect
** writes a line), all inside namespace vc. Entry point (extern "C", tests/differential_victory_v1.rs): ref_victory_run. */
#include <string>
#include <vector>
#include <cstdio>
#include <cstring>
#include <cstdarg>
#include <cwchar>
#include <cstdlib>

namespace vc {
#define private public
#define protected public
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/VictoryConditions.cpp"
#undef protected
#undef private

std::vector<std::string> g_log;
void vcLog(const char *fmt, ...)
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
PlayerTemplateStore *ThePlayerTemplateStore = 0;
PlayerList *ThePlayerList = 0;
Recorder *TheRecorder = 0;
GameLogic *TheGameLogic = 0;
PartitionManager *ThePartitionManager = 0;
GameClient *TheGameClient = 0;
InGameUI *TheInGameUI = 0;
GameAudio *TheAudio = 0;
Radar *TheRadar = 0;
GameInfo *TheGameInfo = 0;

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

}  // namespace vc

extern "C" {

/* `cfg` (40 words): 0 multiplayer, 1 the frame, 2 a civilian template exists, 3 the neutral player's slot (-1 none), 4 bit 0 a game info,
** bits 8-15 which of its slots are an AI's; 5-20 the player list's slots 0-15: bit 0 a player (its index the slot), bits 1-2 its template
** (0 none, 1 the civilian one, 2 another), 3 an observer, 4 the local player, 5 has objects, 6 units, 7 buildings that count, bits 8-11
** its name key (0-7 "playerN", 8 "FactionCivilian", else another), bits 16-31 the players whose default team it is allied to; 21-36 the
** players whose default team it is neutral to. `ops` [op, a, b]: 0 update  1 cachePlayerPtrs  2 init  3 setVictoryConditions(a)
** 4 the frame = a  5 cfg[a] = b  6 question a about player b (16: NULL). Each op appends its result, its lines and the state. */
int ref_victory_run(const unsigned *cfg_in, int n, const unsigned *ops, unsigned *out, int cap, int *len)
{
	using namespace vc;
	unsigned cfg[40];
	memcpy(cfg, cfg_in, sizeof(cfg));
	g_out.clear();
	g_log.clear();
	PlayerTemplateStore store;
	ThePlayerTemplateStore = &store;
	PlayerList list;
	ThePlayerList = &list;
	Recorder rec;
	TheRecorder = &rec;
	GameLogic gl;
	TheGameLogic = &gl;
	PartitionManager pm; ThePartitionManager = &pm;
	GameClient gc; TheGameClient = &gc;
	InGameUI ui; TheInGameUI = &ui;
	GameAudio audio; TheAudio = &audio;
	Radar radar; TheRadar = &radar;
	GameInfo info;
	Player players[MAX_PLAYER_COUNT];
	Team teams[MAX_PLAYER_COUNT];
	auto scene = [&]() {
		rec.m_multiplayer = cfg[0] != 0;
		gl.m_frame = cfg[1];
		store.m_hasCivilian = cfg[2] != 0;
		store.m_civilian.m_key = 500;
		store.m_other.m_key = 7;
		TheGameInfo = (cfg[4] & 1) ? &info : 0;
		for (int k = 0; k < MAX_SLOTS; ++k) { info.m_slots[k].m_ai = (cfg[4] >> (8 + k) & 1) != 0; info.m_slots[k].m_idx = k; }
		for (int i = 0; i < MAX_PLAYER_COUNT; ++i) {
			unsigned w = cfg[5 + i];
			Player &p = players[i];
			p.m_index = i;
			teams[i].m_owner = &p;
			p.m_defaultTeam = &teams[i];
			unsigned t = w >> 1 & 3;
			p.m_template = t == 1 ? &store.m_civilian : (t == 2 ? &store.m_other : 0);
			p.m_observer = (w >> 3 & 1) != 0;
			p.m_local = (w >> 4 & 1) != 0;
			p.m_objects = (w >> 5 & 1) != 0;
			p.m_units = (w >> 6 & 1) != 0;
			p.m_buildings = (w >> 7 & 1) != 0;
			unsigned nk = w >> 8 & 15;
			p.m_nameKey = nk < 8 ? 1000 + (int)nk : (nk == 8 ? 500 : 1);
			p.m_alliedTo = w >> 16;
			p.m_neutralTo = cfg[21 + i];
			list.m_slots[i] = (w & 1) ? &p : 0;
		}
		int neutral = (int)cfg[3];
		list.m_neutral = neutral >= 0 && neutral < MAX_PLAYER_COUNT ? list.m_slots[neutral] : 0;
	};
	scene();
	VictoryConditionsInterface *iface = createVictoryConditions();
	VictoryConditions *v = (VictoryConditions *)iface;
	auto putState = [&]() {
		put((unsigned)v->m_victoryConditions);
		for (int i = 0; i < MAX_PLAYER_COUNT; ++i) put(v->m_players[i] ? (unsigned)v->m_players[i]->m_index : 0xffffffffu);
		put((unsigned)v->m_localSlotNum);
		put(v->m_endFrame);
		for (int i = 0; i < MAX_PLAYER_COUNT; ++i) put(v->m_isDefeated[i] ? 1 : 0);
		put(v->m_localPlayerDefeated ? 1 : 0);
		put(v->m_singleAllianceRemaining ? 1 : 0);
		put(v->m_isObserver ? 1 : 0);
	};
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 3 * i;
		if (o[0] == 4) cfg[1] = o[1];
		if (o[0] == 5 && o[1] < 40) cfg[o[1]] = o[2];
		scene();
		g_log.clear();
		unsigned res = 0;
		Player *who = o[2] < (unsigned)MAX_PLAYER_COUNT ? &players[o[2]] : 0;
		switch (o[0]) {
		case 0: iface->update(); break;
		case 1: iface->cachePlayerPtrs(); break;
		case 2: iface->init(); break;
		case 3: iface->setVictoryConditions((Int)o[1]); break;
		case 6:
			switch (o[1]) {
			case 0: res = iface->hasAchievedVictory(who) ? 1 : 0; break;
			case 1: res = iface->hasBeenDefeated(who) ? 1 : 0; break;
			case 2: res = iface->hasSinglePlayerBeenDefeated(who) ? 1 : 0; break;
			case 3: res = iface->isLocalAlliedVictory() ? 1 : 0; break;
			case 4: res = iface->isLocalAlliedDefeat() ? 1 : 0; break;
			case 5: res = iface->isLocalDefeat() ? 1 : 0; break;
			case 6: res = iface->amIObserver() ? 1 : 0; break;
			case 7: res = iface->getEndFrame(); break;
			case 8: res = (unsigned)iface->getVictoryConditions(); break;
			}
			break;
		}
		put(res);
		put((unsigned)g_log.size());
		for (size_t j = 0; j < g_log.size(); ++j) putStr(g_log[j]);
		putState();
	}
	delete iface;
	ThePlayerTemplateStore = 0; ThePlayerList = 0; TheRecorder = 0; TheGameLogic = 0; ThePartitionManager = 0; TheGameClient = 0;
	TheInGameUI = 0; TheAudio = 0; TheRadar = 0; TheGameInfo = 0;
	*len = (int)g_out.size();
	if ((int)g_out.size() > cap) return 1;
	memcpy(out, &g_out[0], g_out.size() * 4);
	return 0;
}

}  /* extern "C" */
