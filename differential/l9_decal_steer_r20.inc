/* Differential-test driver for RadiusDecalUpdate and AnimationSteeringUpdate (p1w13-sonnet, batch R20; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/RadiusDecalUpdate.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AnimationSteeringUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the module's object (id 1: maybe a controlling player, maybe a drawable whose
** model condition calls are lines, the attacking status, maybe a physics behaviour, the contain driver's physics memory, whose turning the test sets);
** the dynamic shroud driver's RadiusDecal stand-ins (the calls are lines; m_empty as RadiusDecal.cpp keeps it; xferRadiusDecal verbatim); the game
** logic, whose wake calls the test counts; and the REAL UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/RadiusDecalUpdate.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AnimationSteeringUpdate.cpp"

namespace {

ModuleData *l9r20Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = kind ? AnimationSteeringUpdate::friend_newModuleData(&ini) : RadiusDecalUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r20Saved;
/* the decal textures the test names, by index (0: none) */
const char *const g_r20Names[3] = { "", "RingA", "RingB" };
Real r20F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a RadiusDecalUpdate (`kind` 0) or an AnimationSteeringUpdate (1) and make the module on object 1 at
** frame 0. `cfg` (1 word): the flags [bit 0 a controlling player, 1 a drawable, 2 attacking, 3 a physics behaviour, 4-5 its turning (0 none,
** 1 negative, 2 positive)]. The data dump goes first (AnimationSteeringUpdate: the transition frames; RadiusDecalUpdate: nothing). Then `n` ops of
** 6 words [op, a, b, c, d, e]: 0 update()  1 flags = a  2 the frame = a  3 createRadiusDecal(the template of texture a (g_r20Names) and colour
** b, radius c (float bits), position (d, e, 0) (float bits))  4 killRadiusDecal()  5 killWhenNoLongerAttacking(a & 1)  6 the turning = a & 3  7 xfer save  8 xfer load
** of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the result (update's sleep time), the effect log (count, then each), the
** wake calls (count, the last frame), the xfer status, bytes and CRC, and two state words (RadiusDecalUpdate: the decal is empty, the kill
** flag; AnimationSteeringUpdate: the turn animation, the next transition frame). */
int ref_l9r20_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r20Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r20Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	if (kind) putU32(((const AnimationSteeringUpdateModuleData *)d)->m_transitionFrames);
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	Player player;
	player.m_idx = 2;
	Drawable draw;
	Object *me = newObject();
	me->m_oid = 1;
	draw.m_ownerObject = me;
	memset(g_ctPhysMem[0], 0, sizeof(g_ctPhysMem[0]));
	PhysicsBehavior *phys = (PhysicsBehavior *)g_ctPhysMem[0];
	g_dsFirst = 0;
	gl.m_awakenCalls = 0;
	UpdateModule *m = 0;
	if (kind) m = new AnimationSteeringUpdate(me, d); else m = new RadiusDecalUpdate(me, d);
	/* GameLogic::friend_awakenUpdateModule for an object not yet in the list (GameLogic.cpp:3035-3046), and the Rust world scheduling an update
	** module when it makes the object (frame 1 at frame 0) for a constructor that sets no wake frame (the batch R13e correction) */
	m->friend_setNextCallFrame(gl.m_awakenCalls ? gl.m_awakenFrame : 1);
	unsigned flags = cfg[0];
	RadiusDecalTemplate tmpl;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 1) flags = o[1];
		if (o[0] == 2) gl.m_frame = o[1];
		if (o[0] == 6) flags = (flags & ~0x30u) | (o[1] & 3) << 4;
		me->m_player = (flags & 1) ? &player : 0;
		me->m_drawable = (flags & 2) ? &draw : 0;
		if (flags & 4) me->m_status.set(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_IS_ATTACKING)); else me->m_status.clear(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_IS_ATTACKING));
		me->m_physics = (flags & 8) ? phys : 0;
		unsigned turn = (flags >> 4) & 3;
		phys->m_turning = turn == 1 ? TURN_NEGATIVE : turn == 2 ? TURN_POSITIVE : TURN_NONE;
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
			case 0: res = (unsigned)m->update(); break;
			case 3:
				if (!kind) {
					tmpl.m_name = g_r20Names[o[1] % 3];
					tmpl.m_color = (Color)o[2];
					Coord3D pos;
					pos.x = r20F(o[4]);
					pos.y = r20F(o[5]);
					pos.z = 0.0f;
					((RadiusDecalUpdate *)m)->createRadiusDecal(tmpl, r20F(o[3]), pos);
				}
				break;
			case 4: if (!kind) ((RadiusDecalUpdate *)m)->killRadiusDecal(); break;
			case 5: if (!kind) ((RadiusDecalUpdate *)m)->killWhenNoLongerAttacking((o[1] & 1) != 0); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r20Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r20Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		/* the op's wake call (createRadiusDecal, killRadiusDecal) as GameLogic::friend_awakenUpdateModule applies it (GameLogic.cpp:2983-3024, the
		** module's own next call frame; the Rust world's sleepy list does the same): the stand-in game logic only records it */
		if (gl.m_awakenCalls) {
			UnsignedInt now = gl.m_frame, when = gl.m_awakenFrame, next = m->friend_getNextCallFrame();
			if (when != next && !(now > 0 && next == now && when == now + 1)) m->friend_setNextCallFrame(when);
		}
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (kind) {
			AnimationSteeringUpdate *a = (AnimationSteeringUpdate *)m;
			putU32((unsigned)a->m_currentTurnAnim);
			putU32(a->m_nextTransitionFrame);
		} else {
			RadiusDecalUpdate *r = (RadiusDecalUpdate *)m;
			putU32(r->m_deliveryDecal.isEmpty() ? 1 : 0);
			putU32(r->m_killWhenNoLongerAttacking ? 1 : 0);
		}
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	me->m_physics = 0;
	me->m_drawable = 0;
	me->m_player = 0;
	delete me;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
