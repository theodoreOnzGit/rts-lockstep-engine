/* Differential-test driver for StealthDetectorUpdate (p1w13-sonnet, batch R27; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthDetectorUpdate.cpp   (#included from build.rs's mod_gen copy: its field table's
**   INI::parseParticleSystemTemplate is this unit's l9ahParsePsTemplate, as for AutoHealBehavior)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the detector (id 1: a player of three or none, a drawable, effectively dead or
** not, under construction, sold, stealthed, a position, a vision range, contained by object 2 or not, whose contain module is garrisonable or not, the
** local player's shroud of it); the scan (the height-die driver's, objects 3 to 6 as the test lists them, filtered by the module's own
** PartitionFilterStealthedOrStealthGarrisoned; the partition manager's three filters are the scan's); objects 3 to 6 with their status bits, kinds,
** players, drawables, positions, a stand-in StealthUpdate (its markAsDetected a line, its Eva events the test's) and a contain module (garrisonable,
** stealth units, riders 7 and 8 with stand-in stealth updates); the REAL Object::getRelationship on the teams' relationships; the radar (tryEvent a
** line and the test's answer, createEvent a line), the misc audio's two stealth sounds, the in-game UI's message, the Eva, the particle systems
** (lines), the drawables' second pass; the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them); and the
** REAL UpdateModule, INI, Xfer and logic random values. */
#include "StealthDetectorUpdate_edit.cpp"

void InGameUI::message( const UnicodeString &text ) { if (g_uiMessageHook) g_uiMessageHook(text.m_label); }
void ParticleSystem::attachToDrawable( const Drawable *draw ) { ulog("psattachdraw %u %d", m_id, draw && draw->m_ownerObject ? (int)draw->m_ownerObject->m_oid : 0); }

namespace {

ModuleData *l9r27Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = StealthDetectorUpdate::friend_newModuleData(&ini);
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

/* a contain module: garrisonable or not, its stealth units and its riders, from the test */
class R27Contain : public UpgContain
{
public:
	virtual Bool isGarrisonable() const { return m_garrisonable; }
	virtual UnsignedInt getStealthUnitsContained() const { return m_stealthUnits; }
	virtual const ContainedItemsList *getContainedItemsList() const { return &m_list; }
	Bool m_garrisonable = false;
	UnsignedInt m_stealthUnits = 0;
	ContainedItemsList m_list;
};

std::vector<unsigned char> g_r27Saved;
unsigned g_r27TryAnswers = 0;   /* bit 0: tryEvent's answer for a discovery, bit 1: for a neutralisation */
Team g_r27Teams[3];
int g_r27Rel[3][3];   /* team i + 1's relationship to team j + 1 (0 enemies, 1 neutral, 2 allies) */
Relationship r27TeamRel(const Team *a, const Team *b) { return (Relationship)g_r27Rel[a->m_dummy - 1][b->m_dummy - 1]; }
UpdateModule *g_r27Updating = 0;
/* GameLogic::friend_awakenUpdateModule (GameLogic.cpp lines 2983-3046): the updating module's call is ignored, an equal frame returns early, so does
** the "already awake" case; otherwise the frame is set */
void r27Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r27Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
void r27Message(const char *label) { ulog("message %s", label); }

}  // namespace

Bool Radar::tryEvent( RadarEventType event, const Coord3D *pos )
{
	ulog("radartry %d %08x %08x %08x", (int)event, realBits(pos->x), realBits(pos->y), realBits(pos->z));
	return (g_r27TryAnswers >> (event == RADAR_EVENT_STEALTH_DISCOVERED ? 0 : 1) & 1) != 0;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a StealthDetectorUpdate and make the module on object 1 at frame cfg[0] (logic seed cfg[1]). `cfg` (14
** words; each of 2-13 also settable by op 2):
**   2 the detector: 0 drawable, 1-2 player (3 none), 3 effectively dead, 4 under construction, 5 sold, 6 stealthed, 7 contained by object 2, 8 object
**     2's contain module garrisonable, 9 object 2 has a contain module
**   3 the detector's vision range   4 its position's z   5 0-1 the local player (3 none), 4-6 the local player's shroud of the detector
**   6 the teams' relationships (team i + 1 to team j + 1 at 2 (3i + j))   7 the scan (nibbles of object numbers 3-6, 0 ends)
**   8-11 objects 3-6: 0 stealthed, 1 detected, 2 effectively dead, 3 KINDOF_MINE, 4 KINDOF_BOOBY_TRAP, 5 KINDOF_DEMOTRAP, 6 a stealth update, 7 a
**     contain module, 8 garrisonable, 9 stealth units in it, 10 rider 7 in it, 11 rider 8 in it, 12-13 player (3 none), 14 drawable, 15-16 the
**     enemy detection Eva event (0 EVA_Invalid, else 29 + k), 17-18 the own (the same), 20-27 the position's x
**   12 the riders: 0 rider 7 has a stealth update, 1-2 its player, 4 rider 8 has one, 5-6 its player
**   13 the radar's tryEvent answers: 0 a discovery, 1 a neutralisation
** The data dump goes first, then the constructor's wake calls. Then `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 setSDEnabled(a & 1)
** 2 cfg[a] = b  3 xfer save  4 xfer load of the last save  5 xfer CRC  6 the frame = a. Each op appends: the CATCH_ALL status, the result, the effect
** log, the wake calls (count, last frame), the xfer status, bytes and CRC, m_enabled and the next call frame. */
int ref_l9r27_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r27Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r27Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[14];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const StealthDetectorUpdateModuleData *sd = (const StealthDetectorUpdateModuleData *)d;
		putU32(sd->m_updateRate);
		putU32(realBits(sd->m_detectionRange));
		putU32(sd->m_initiallyDisabled ? 1 : 0);
		putStr(sd->m_pingSound.getEventName().str(), (int)sd->m_pingSound.getEventName().getLength());
		putStr(sd->m_loudPingSound.getEventName().str(), (int)sd->m_loudPingSound.getEventName().getLength());
		const ParticleSystemTemplate *ps[4] = { sd->m_IRBeaconParticleSysTmpl, sd->m_IRParticleSysTmpl, sd->m_IRBrightParticleSysTmpl, sd->m_IRGridParticleSysTmpl };
		for (int k = 0; k < 4; ++k) { const char *nm = ps[k] ? g_ahPsNames[(size_t)ps[k]->m_idx].c_str() : ""; putStr(nm, (int)strlen(nm)); }
		putStr(sd->m_IRParticleSysBone.str(), (int)sd->m_IRParticleSysBone.getLength());
		for (int i = 0; i < KINDOF_COUNT; ++i) putU32(sd->m_extraDetectKindof.test(i) ? 1 : 0);
		for (int i = 0; i < KINDOF_COUNT; ++i) putU32(sd->m_extraDetectKindofNot.test(i) ? 1 : 0);
		putU32((sd->m_canDetectWhileGarrisoned ? 1 : 0) | (sd->m_canDetectWhileTransported ? 2 : 0));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	g_ahPsLastId = 0;
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		g_r27Teams[i].m_dummy = i + 1;
		g_r27Teams[i].m_controller = &players[i];
	}
	ThePlayerList = &g_spList;
	g_teamRelHook = r27TeamRel;
	g_hdApplyFilters = true;
	g_uiMessageHook = r27Message;
	Radar radar;
	TheRadar = &radar;
	TheInGameUI = &g_spUi;
	TheAudio = &g_ddAudio;
	g_cfMisc.m_stealthDiscoveredSound = AudioEventRTS(AsciiString("StealthDiscoveredSnd"), (ObjectID)0);
	g_cfMisc.m_stealthNeutralizedSound = AudioEventRTS(AsciiString("StealthNeutralizedSnd"), (ObjectID)0);
	TheEva = &g_spEva;
	PartitionManager pm;
	ThePartitionManager = &pm;
	TheGameText = &g_spText;
	Object *objs[8];
	Drawable draws[8];
	R27Contain contains[8];
	StealthUpdate stealths[8];
	for (int k = 0; k < 8; ++k) {
		objs[k] = newObject();
		objs[k]->m_oid = (ObjectID)(k + 1);
		draws[k].m_ownerObject = objs[k];
		stealths[k].m_owner = objs[k];
	}
	Object *me = objs[0];
	auto player = [&](unsigned p) { return p < 3 ? &players[p] : (Player *)0; };
	auto team = [&](unsigned p) { return p < 3 ? &g_r27Teams[p] : (Team *)0; };
	auto scene = [&]() {
		unsigned f = cfg[2];
		me->m_drawable = (f & 1) ? &draws[0] : 0;
		me->m_player = player(f >> 1 & 3);
		me->m_team = team(f >> 1 & 3);
		me->m_effDead = (f >> 3 & 1) != 0;
		me->m_status = ObjectStatusMaskType();
		if (f >> 4 & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		if (f >> 5 & 1) me->m_status.set(OBJECT_STATUS_SOLD);
		if (f >> 6 & 1) me->m_status.set(OBJECT_STATUS_STEALTHED);
		me->m_containedBy = (f >> 7 & 1) ? objs[1] : 0;
		contains[1].m_garrisonable = (f >> 8 & 1) != 0;
		objs[1]->m_contain = (f >> 9 & 1) ? &contains[1] : 0;
		me->m_vision = (Real)cfg[3];
		me->m_pos.x = 0.0f;
		me->m_pos.y = 0.0f;
		me->m_pos.z = (Real)cfg[4];
		unsigned local = cfg[5] & 3;
		g_spLocal = player(local);
		me->m_shroudR27 = (Int)(cfg[5] >> 4 & 7);
		for (int i = 0; i < 3; ++i) for (int j = 0; j < 3; ++j) g_r27Rel[i][j] = (int)(cfg[6] >> (2 * (3 * i + j)) & 3) % 3;
		g_hdScan.clear();
		for (int k = 0; k < 4; ++k) { unsigned id = cfg[7] >> (4 * k) & 15; if (id == 0) break; if (id >= 3 && id <= 6) g_hdScan.push_back(objs[id - 1]); }
		for (int k = 2; k < 6; ++k) {
			unsigned g = cfg[8 + (k - 2)];
			Object *ob = objs[k];
			ob->m_status = ObjectStatusMaskType();
			if (g & 1) ob->m_status.set(OBJECT_STATUS_STEALTHED);
			if (g >> 1 & 1) ob->m_status.set(OBJECT_STATUS_DETECTED);
			ob->m_effDead = (g >> 2 & 1) != 0;
			ob->m_kindOf = KindOfMaskType();
			if (g >> 3 & 1) ob->m_kindOf.set(KINDOF_MINE);
			if (g >> 4 & 1) ob->m_kindOf.set(KINDOF_BOOBY_TRAP);
			if (g >> 5 & 1) ob->m_kindOf.set(KINDOF_DEMOTRAP);
			ob->m_tmpl.m_kindOf = ob->m_kindOf;
			ob->m_stealth = (g >> 6 & 1) ? &stealths[k] : 0;
			ob->m_contain = (g >> 7 & 1) ? &contains[k] : 0;
			contains[k].m_garrisonable = (g >> 8 & 1) != 0;
			contains[k].m_stealthUnits = (g >> 9 & 1) ? 1 : 0;
			contains[k].m_list.clear();
			if (g >> 10 & 1) contains[k].m_list.push_back(objs[6]);
			if (g >> 11 & 1) contains[k].m_list.push_back(objs[7]);
			ob->m_player = player(g >> 12 & 3);
			ob->m_team = team(g >> 12 & 3);
			ob->m_drawable = (g >> 14 & 1) ? &draws[k] : 0;
			unsigned ee = g >> 15 & 3, oe = g >> 17 & 3;
			stealths[k].m_enemyEva = ee ? (int)(29 + ee) : -1;
			stealths[k].m_ownEva = oe ? (int)(29 + oe) : -1;
			ob->m_pos.x = (Real)(g >> 20 & 255);
			ob->m_pos.y = 5.0f;
			ob->m_pos.z = 0.0f;
		}
		for (int k = 0; k < 2; ++k) {
			unsigned r = cfg[12] >> (4 * k);
			objs[6 + k]->m_stealth = (r & 1) ? &stealths[6 + k] : 0;
			objs[6 + k]->m_player = player(r >> 1 & 3);
			objs[6 + k]->m_team = team(r >> 1 & 3);
		}
		g_r27TryAnswers = cfg[13];
	};
	scene();
	gl.m_awakenHook = r27Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	StealthDetectorUpdate *m = new StealthDetectorUpdate(me, d);
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	/* registerObject's "a frame of 0 is now, and now is at least 1" (GameLogic.cpp lines 3896-3923) */
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] < 14) cfg[o[1]] = o[2];
		if (o[0] == 6) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		gl.m_awakenCalls = 0;
		gl.m_awakenFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: g_r27Updating = m; res = (unsigned)m->update(); break;
			case 1: m->setSDEnabled((o[1] & 1) != 0); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r27Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r27Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r27Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_enabled ? 1 : 0);
		putU32(m->friend_getNextCallFrame());
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_hdApplyFilters = false;
	g_hdScan.clear();
	g_teamRelHook = 0;
	g_uiMessageHook = 0;
	delete m;
	delete d;
	for (int k = 0; k < 8; ++k) {
		Object *ob = objs[k];
		ob->m_drawable = 0; ob->m_contain = 0; ob->m_stealth = 0; ob->m_team = 0; ob->m_player = 0; ob->m_containedBy = 0;
		delete ob;
	}
	TheRadar = 0;
	TheInGameUI = 0;
	TheAudio = 0;
	TheEva = 0;
	ThePartitionManager = 0;
	TheGameText = 0;
	ThePlayerList = 0;
	g_spLocal = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
