/* Differential-test driver for NeutronBlastBehavior and BridgeScaffoldBehavior (p1w13-sonnet, batch R35; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/NeutonBlastBehavior.cpp    (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeScaffoldBehavior.cpp (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: five objects (ids 1-5; kinds, the airborne status bit, maybe a contain
** module whose killAllContained is a line, maybe an AI whose aiIdle is a line (the ActiveBody driver's), maybe a drawable whose setTerrainDecal
** is a line, maybe a team, maybe the module build's MBody, whose attemptDamage is a line; a position), the teams' relationships, the neutral
** player's default team (team 9), the scan (the objects the test lists, which pass the filters; the call a line with the number of filters and
** the distance type), setDisabled, deselectObject (the ActiveBody driver's lines), setTeam (a line, the object left alone), destroyObject (a
** line), Thing::setPosition (the shim's: the position kept), the REAL Object::kill, UpdateModule, INI and Xfer; the game logic (its frame, its
** wake calls applied as GameLogic::friend_awakenUpdateModule applies them). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/NeutonBlastBehavior.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeScaffoldBehavior.cpp"

namespace {

ModuleData *l9r35Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind ? BridgeScaffoldBehavior::friend_newModuleData(&ini) : NeutronBlastBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r35Saved;
UpdateModule *g_r35Updating = 0;
void r35Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r35Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
void r35ScanHook(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n, g_ahRangeDc);
}
Real r35F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
/* the kinds the test gives an object, by bit (the same list in the Rust test) */
const KindOfType g_r35Kinds[5] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_DRONE, KINDOF_CLIFF_JUMPER, KINDOF_AIRCRAFT };
Object *g_r35Objs[5];
Team g_r35Teams[3];
Player g_r35Players[3];
int g_r35Rel[3][3];
Object *r35Find(ObjectID id) { return (id >= 1 && id <= 5) ? g_r35Objs[id - 1] : 0; }
Relationship r35TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r35Rel[from->m_dummy - 1][to->m_dummy - 1];
}
void r35SetTeam(Object *, Team *team) { ulog("setteam %d", team ? team->m_dummy : -1); }
void r35Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
class R35Contain : public UpgContain
{
public:
	Object *m_owner = 0;
	virtual void killAllContained(void) { ulog("killall %d", (int)m_owner->m_oid); }
};

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a NeutronBlastBehavior (`kind` 0) or a BridgeScaffoldBehavior (1) and make the module on object 1 of
** five at frame cfg[0]. `cfg` (24 words, 2-22 also settable by the cfg op): kind 0: 2-6 objects 1-5: bits 0-4 their kinds (of g_r35Kinds), 5
** airborne, 6 a contain module, 7 an AI, 8 a drawable, 9-10 their team (0 none, else 1-3) (every object has a body); 7 the teams' relationships (2 bits each,
** row-major, team a to team b: 0 enemies, 1 neutral, 2 allies); 8 the scan (nibbles of object numbers 1-5, 0 ends); 9-13 the objects' x (float
** bits; y 0, z 0). Kind 1: 2-10 the create, rise-to and build positions (float bits), 11-13 object 1's position. The data dump goes first, then the
** constructor's effects and wake calls. Then `n` ops of 4 words [op, a, b, c]. Kind 0: 0 update()  1 onDie  2 the frame = a  3 cfg[a] = b  4 xfer
** save  5 xfer load of the last save  6 xfer CRC. Kind 1: 0 update()  1 the frame = a  2 setPositions (cfg 2-10)  3 setMotion(a)  4
** getCurrentMotion  5 reverseMotion  6 setLateralSpeed(a bits)  7 setVerticalSpeed(a bits)  8 xfer save  9 load  10 CRC  11 cfg[a] = b  12
** getBridgeScaffoldBehaviorInterfaceFromObject(object a) != NULL  13 object 1's position = cfg 11-13. Each op appends: the CATCH_ALL status,
** the result, the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the module's state words (kind 1) and object
** 1's position. */
int ref_l9r35_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r35Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r35Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[24];
	memcpy(cfg, cfg_in, sizeof(cfg));
	if (!kind) {
		const NeutronBlastBehaviorModuleData *nd = (const NeutronBlastBehaviorModuleData *)d;
		putU32(realBits(nd->m_blastRadius));
		putU32(nd->m_isAffectAirborne ? 1 : 0);
		putU32(nd->m_affectAllies ? 1 : 0);
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	ThePartitionManager = &g_ahPm;
	MBody bodies[5];
	Drawable draws[5];
	R35Contain contains[5];
	AIUpdateInterface ais[5];
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 5; ++k) {
		g_r35Objs[k] = newObject();
		g_r35Objs[k]->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = g_r35Objs[k];
		draws[k].m_ownerObject = g_r35Objs[k];
		contains[k].m_owner = g_r35Objs[k];
		ais[k].m_owner = g_r35Objs[k];
		g_r35Objs[k]->m_behaviors = none;
	}
	Team neutralTeam;
	neutralTeam.m_dummy = 9;
	Player neutral;
	neutral.m_defaultTeam = &neutralTeam;
	g_tbNeutral = &neutral;
	PlayerList playerList;
	ThePlayerList = &playerList;
	for (int t = 0; t < 3; ++t) {
		g_r35Players[t].m_idx = t;
		g_r35Teams[t].m_dummy = t + 1;
		g_r35Teams[t].m_controller = &g_r35Players[t];
	}
	Object *me = g_r35Objs[0];
	auto scene = [&]() {
		if (kind) return;
		for (int k = 0; k < 5; ++k) {
			Object *o = g_r35Objs[k];
			unsigned f = cfg[2 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 5; ++b) if (f >> b & 1) o->m_kindOf.set(g_r35Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			o->m_status.clear();
			if (f >> 5 & 1) o->m_status.set(OBJECT_STATUS_AIRBORNE_TARGET);
			o->m_contain = (f >> 6 & 1) ? &contains[k] : 0;
			o->m_ai = (f >> 7 & 1) ? &ais[k] : 0;
			o->m_drawable = (f >> 8 & 1) ? &draws[k] : 0;
			unsigned t = f >> 9 & 3;
			o->m_team = t ? &g_r35Teams[t - 1] : 0;
			o->m_player = t ? &g_r35Players[t - 1] : 0;
			o->m_body = &bodies[k];   /* always: Object::kill reads the body unchecked */
			memcpy(&o->m_pos.x, &cfg[9 + k], 4);
			o->m_pos.y = 0.0f;
			o->m_pos.z = 0.0f;
		}
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r35Rel[a][b] = (int)(cfg[7] >> (2 * (3 * a + b)) & 3);
		g_ahRange.clear();
		for (int k = 0; k < 5; ++k) { unsigned id = cfg[8] >> (4 * k) & 15; if (id == 0) break; if (id <= 5) g_ahRange.push_back(g_r35Objs[id - 1]); }
	};
	auto placeMe = [&]() { memcpy(&me->m_pos.x, &cfg[11], 4); memcpy(&me->m_pos.y, &cfg[12], 4); memcpy(&me->m_pos.z, &cfg[13], 4); };
	scene();
	if (kind) placeMe();
	g_ahRangeHook = r35ScanHook;
	g_teamRelHook = r35TeamRel;
	g_modFindHook = r35Find;
	g_setTeamHook = r35SetTeam;
	g_modDestroyHook = r35Destroy;
	g_events.clear();
	g_ulog.clear();
	gl.m_awakenHook = r35Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	UpdateModule *m = kind ? (UpdateModule *)new BridgeScaffoldBehavior(me, d) : (UpdateModule *)new NeutronBlastBehavior(me, d);
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	putLog();
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 4 * i;
		unsigned cfgOp = kind ? 11 : 3, frameOp = kind ? 1 : 2;
		if (o[0] == cfgOp && o[1] >= 2 && o[1] <= 22) cfg[o[1]] = o[2];
		if (o[0] == frameOp) gl.m_frame = o[1];
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
		int xferOp = kind ? 8 : 4;
		try {
			if ((int)o[0] >= xferOp && (int)o[0] <= xferOp + 2) {
				int how = (int)o[0] - xferOp;
				if (how == 0) { xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r35Saved.assign(bytes, bytes + blen); }
				else if (how == 1) {
					std::vector<unsigned char> b = g_r35Saved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				} else xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc);
			} else if (o[0] == 0) {
				g_r35Updating = m;
				res = (unsigned)m->update();
			} else if (!kind && o[0] == 1) {
				DamageInfo di;
				((NeutronBlastBehavior *)m)->onDie(&di);
			} else if (kind) {
				BridgeScaffoldBehavior *s = (BridgeScaffoldBehavior *)m;
				Coord3D p[3];
				for (int k = 0; k < 3; ++k) { p[k].x = r35F(cfg[2 + 3 * k]); p[k].y = r35F(cfg[3 + 3 * k]); p[k].z = r35F(cfg[4 + 3 * k]); }
				switch (o[0]) {
				case 2: s->setPositions(&p[0], &p[1], &p[2]); break;
				case 3: s->setMotion((ScaffoldTargetMotion)o[1]); break;
				case 4: res = (unsigned)s->getCurrentMotion(); break;
				case 5: s->reverseMotion(); break;
				case 6: s->setLateralSpeed(r35F(o[1])); break;
				case 7: s->setVerticalSpeed(r35F(o[1])); break;
				case 12: res = BridgeScaffoldBehavior::getBridgeScaffoldBehaviorInterfaceFromObject(g_r35Objs[o[1] % 5]) ? 1 : 0; break;
				case 13: placeMe(); break;
				}
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r35Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind) {
			BridgeScaffoldBehavior *s = (BridgeScaffoldBehavior *)m;
			putU32((unsigned)s->m_targetMotion);
			const Coord3D *cs[4] = { &s->m_createPos, &s->m_riseToPos, &s->m_buildPos, &s->m_targetPos };
			for (int k = 0; k < 3; ++k) { putU32(realBits(cs[k]->x)); putU32(realBits(cs[k]->y)); putU32(realBits(cs[k]->z)); }
			putU32(realBits(s->m_lateralSpeed));
			putU32(realBits(s->m_verticalSpeed));
			putU32(realBits(cs[3]->x)); putU32(realBits(cs[3]->y)); putU32(realBits(cs[3]->z));
		}
		putU32(realBits(me->m_pos.x)); putU32(realBits(me->m_pos.y)); putU32(realBits(me->m_pos.z));
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_ahRangeHook = 0;
	g_ahRange.clear();
	g_teamRelHook = 0;
	g_modFindHook = 0;
	g_setTeamHook = 0;
	g_modDestroyHook = 0;
	g_tbNeutral = 0;
	ThePlayerList = 0;
	me->m_behaviors = 0;
	delete m;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r35Objs[k];
		x->m_drawable = 0; x->m_body = 0; x->m_player = 0; x->m_team = 0; x->m_ai = 0; x->m_contain = 0; x->m_behaviors = 0;
		delete x;
		g_r35Objs[k] = 0;
	}
	TheGameLogic = 0;
	ThePartitionManager = 0;
	return 0;
}

}  /* extern "C" */
