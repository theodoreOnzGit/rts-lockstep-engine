/* Differential-test driver for TechBuildingBehavior (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/TechBuildingBehavior.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with a team and a controlling player (the team's) and a
** drawable that logs the model-condition calls, three players (the first is the neutral player) each with a team and a playable flag, FXList::doFXObj (the list,
** logged as "fx<id>"), Object::setTeam (below) and the game logic whose frame the test sets and whose wake recorder counts the calls setWakeFrame makes, and the REAL
** UpdateModule, INI, FXList parse and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/TechBuildingBehavior.cpp"

namespace {

ModuleData *l9tbRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = TechBuildingBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_tbSaved;
Player *g_tbNeutral = 0;
TechBuildingBehavior *g_tbModule = 0;

}  // namespace

Player *PlayerList::getNeutralPlayer() { return g_tbNeutral; }
/* Object.cpp's setTeam -> setOrRestoreTeam, reduced to what a module can see: a team that is the same changes nothing; the object's controlling player is the team's;
** the modules are told onCapture(old owner, new owner) when both teams exist (Object.cpp setOrRestoreTeam, "This can't just go in ::defect") */
/* --- p1w13-sonnet --- (batch P: ActiveBody's test logs the team it is given and leaves the object alone; null for TechBuildingBehavior's) */
void (*g_setTeamHook)(Object *, Team *) = 0;
/* --- end p1w13-sonnet --- */
void Object::setTeam( Team *team )
{
	if (g_setTeamHook) { g_setTeamHook(this, team); return; }
	Team *old = m_team;
	if (old == team) return;
	m_team = team;
	m_player = team ? team->m_controller : 0;
	ulog("setteam %d", team ? team->m_dummy : -1);
	if (old && team && g_tbModule) g_tbModule->onCapture(old->m_controller, team->m_controller);
}

extern "C" {

/* Read `filename` (block `D`) as the data of a TechBuildingBehavior and make the module on an Object (id 1) at the frame cfg[0], controlled by player cfg[1] - 1 of three (0:
** none: no team), with the playable flags cfg[2] (bit i: player i is a playable side). The data dump goes first (the pulse FX list as an id, the rate). Then the
** constructor's wake calls. Then `n` ops of 5 words [op, a, b, ...]: 0 update()  1 onDie  2 onCapture(player a, player b)  3 the controlling player = a - 1 (0 none,
** team and player)  4 the frame = a  5 the playable flags = a  6 xfer save  7 xfer load of the last save  8 xfer CRC. Each op appends: the CATCH_ALL status, the sleep time
** update() returned (0 for the others), the effect log (count, then each as a string: "fx<id>", "mc <n>", "mcclear <n>", "setteam <team>"), the number of wake calls and
** the frame of the last, the xfer status, bytes and CRC, the object's team (-1 none). */
static int l9tbRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_tbSaved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9tbRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const TechBuildingBehaviorModuleData *td = (const TechBuildingBehaviorModuleData *)d;
		putI32(fxId(td->m_pulseFX));
		putU32(td->m_pulseFXRate);
	}
	Player players[3];
	Team teams[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_playable = (cfg[2] >> i & 1) != 0;
		players[i].m_defaultTeam = &teams[i];
		teams[i].m_dummy = i;
		teams[i].m_controller = &players[i];
	}
	g_tbNeutral = &players[0];
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_drawable = &g_cpDrawable;
	obj->m_team = cfg[1] ? &teams[cfg[1] - 1] : 0;
	obj->m_player = cfg[1] ? &players[cfg[1] - 1] : 0;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	TechBuildingBehavior *m = 0;
	status = 0;
	try { m = new TechBuildingBehavior(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0;
		return 5;
	}
	g_tbModule = m;
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		int sleep = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			gl.m_awakenCalls = 0;
			gl.m_awakenFrame = 0;
			try {
				switch (o[0]) {
				case 0: sleep = (int)m->update(); break;
				case 1: { DamageInfo info; m->onDie(&info); break; }
				case 2: m->onCapture(&players[o[1] % 3], &players[o[2] % 3]); break;
				case 3: obj->m_team = o[1] ? &teams[o[1] - 1] : 0; obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
				case 4: gl.m_frame = o[1]; break;
				case 5: for (int i = 0; i < 3; ++i) players[i].m_playable = (o[1] >> i & 1) != 0; break;
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_tbSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_tbSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		if (phase) { putI32(st); putI32(sleep); }
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
		putI32(obj->m_team ? obj->m_team->m_dummy : -1);
	}
	*len = finish(buf, cap);
	g_tbModule = 0;
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

int ref_l9tb_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9tbRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
