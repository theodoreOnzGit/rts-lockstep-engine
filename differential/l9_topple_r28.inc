/* Differential-test driver for ToppleUpdate (p1w13-sonnet, batch R28; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ToppleUpdate.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: the toppling object (id 1: a body, the module build's MBody, whose
** attemptDamage is a line; a drawable whose calls are lines; effectively dead or not; a name; a position and an orientation, and the transform Thing
** keeps: setTransformMatrix stores it and takes the position and the z angle from it, setOrientation poses it from the angle and the position), a
** crusher (id 2: a crusher level, a physics behaviour with a velocity, a position), the stumps the thing factory makes (ids 3, 4, ...; no body, no
** drawable) of the one template it knows, the script engine's topple directions, the pathfinder, the FX lists, the REAL Object::kill (Object.cpp),
** the game logic (its frame, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them), and the REAL UpdateModule, INI, Xfer and
** Matrix3D. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ToppleUpdate.cpp"

namespace {

ModuleData *l9r28Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ToppleUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r28Saved;
UpdateModule *g_r28Updating = 0;
void r28Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r28Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
/* Thing::setTransformMatrix (Thing.cpp): the transform, and the position and the z angle cached from it */
void r28Xform(Object *o, const Matrix3D *mx)
{
	o->m_pos.x = mx->Get_X_Translation();
	o->m_pos.y = mx->Get_Y_Translation();
	o->m_pos.z = mx->Get_Z_Translation();
	o->m_orientation = mx->Get_Z_Rotation();
}
/* the script engine's topple directions (ScriptEngine::m_toppleDirections, by object name) */
std::vector<std::pair<AsciiString, Coord3D> > g_r28ToppleDirs;
/* the thing factory: the template "Stump" makes object 3 */
ThingTemplate g_r28StumpTmpl;
std::vector<Object *> g_r28Stumps;
const ThingTemplate *r28FindTemplate(const AsciiString &name) { return strcmp(name.str(), "Stump") == 0 ? &g_r28StumpTmpl : 0; }
Object *r28NewObject(const ThingTemplate *, Team *)
{
	/* (no body, no drawable; ids 3, 4, ... as the game logic gives them: every topple makes one) */
	Object *o = newObject();
	o->m_oid = 3 + (ObjectID)g_r28Stumps.size();
	g_r28Stumps.push_back(o);
	return o;
}

}  // namespace

/* the drawable's client modules: none (the call is a line) */
ClientUpdateModule **Drawable::getClientUpdateModules() { ulog("clientmodules %d", m_ownerObject ? (int)m_ownerObject->m_oid : 0); return 0; }
/* Object.cpp lines 1195-1208, verbatim but for findModule (the first behaviour module of that name) */
void Object::topple( const Coord3D *toppleDirection, Real toppleSpeed, UnsignedInt options )
{
	static NameKeyType key_ToppleUpdate = NAMEKEY("ToppleUpdate");

	ToppleUpdate* toppleUpdate = 0;
	for (BehaviorModule **b = m_behaviors; b && *b; ++b) if ((*b)->getModuleNameKey() == key_ToppleUpdate) { toppleUpdate = (ToppleUpdate *)*b; break; }
	if( toppleUpdate && toppleUpdate->isAbleToBeToppled() )
	{

		// apply the topple force
		toppleUpdate->applyTopplingForce( toppleDirection, toppleSpeed, options );

	}  // end if

}  // end topple

/* ScriptEngine.cpp lines 7561-7577, verbatim on the driver's list */
void ScriptEngine::adjustToppleDirection( Object *object, Coord3D *direction)
{
	AsciiString objName = object->getName();
	if (objName.isEmpty() || !direction) {
		return;
	}
	for (size_t i = 0; i < g_r28ToppleDirs.size(); ++i) {
		if (g_r28ToppleDirs[i].first == objName) {
			(*direction) = g_r28ToppleDirs[i].second;
			(*direction).normalize();
			return;
		}
	}
}

extern "C" {

/* Read `filename` (block `D`) as the data of a ToppleUpdate and make the module on object 1 at frame cfg[0]. `cfg` (12 words; each of 1-11 also
** settable by op 6): 1 object 1: 0 drawable, 1 effectively dead, 2 named "Tree1", 3 unused; 2 its x, 3 its y (float bits), 4 its
** orientation (float bits), 5 its health (and maximum, float bits); 6 the crusher: 0-3 its crusher level, 4 a physics behaviour; 7 the crusher's x,
** 8 its y (float bits), 9 the velocity's x, 10 its y (float bits); 11 the script's topple direction for "Tree1": 0 none, else the angle k / 8 of a
** turn. The data dump goes first, then the constructor's wake calls. Then `n` ops of 6 words [op, a, b, c, d, e]: 0 update()  1 isAbleToBeToppled
** 2 applyTopplingForce(direction (a, b) float bits, speed c float bits, options d)  3 onCollide(object 2 if a & 1, else NULL)  4 Object::topple(the
** same as 2)  5 the frame = a  6 cfg[a] = b  7 xfer save  8 xfer load of the last save  9 xfer CRC. Each op appends: the CATCH_ALL status, the
** result, the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, the state, object 1's transform (12 floats), its
** position and orientation, its health and how many stumps were made. */
int ref_l9r28_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r28Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r28Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[12];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const ToppleUpdateModuleData *td = (const ToppleUpdateModuleData *)d;
		putI32(fxId(td->m_toppleFX));
		putI32(fxId(td->m_bounceFX));
		putStr(td->m_stumpName.str(), (int)td->m_stumpName.getLength());
		putU32(realBits(td->m_initialVelocityPercent));
		putU32(realBits(td->m_initialAccelPercent));
		putU32(realBits(td->m_bounceVelocityPercent));
		putU32((td->m_killWhenToppled ? 1 : 0) | (td->m_killWhenStartToppled ? 2 : 0) | (td->m_killStumpWhenToppled ? 4 : 0) | (td->m_toppleLeftOrRightOnly ? 8 : 0)
			| (td->m_reorientToppledRubble ? 16 : 0));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	AI ai;
	TheAI = &ai;
	ScriptEngine script;
	TheScriptEngine = &script;
	ThingFactory factory;
	TheThingFactory = &factory;
	g_findTemplateHook = r28FindTemplate;
	g_newObjectHook = r28NewObject;
	g_xformHook = r28Xform;
	g_r28ThingPose = true;
	g_r28Stumps.clear();
	Object *me = newObject(), *crusher = newObject();
	me->m_oid = 1;
	crusher->m_oid = 2;
	MBody body;
	body.m_owner = me;
	Drawable draw;
	draw.m_ownerObject = me;
	alignas(16) static unsigned char physBuf[sizeof(PhysicsBehavior) + 16];
	memset(physBuf, 0, sizeof(physBuf));
	PhysicsBehavior *physics = (PhysicsBehavior *)(void *)physBuf;
	me->m_body = &body;
	auto scene = [&](bool pose) {
		unsigned f = cfg[1];
		me->m_drawable = (f & 1) ? &draw : 0;
		me->m_effDead = (f >> 1 & 1) != 0;
		me->m_name = (f >> 2 & 1) ? AsciiString("Tree1") : AsciiString::TheEmptyString;
		if (pose) {
			memcpy(&me->m_pos.x, &cfg[2], 4);
			memcpy(&me->m_pos.y, &cfg[3], 4);
			me->m_pos.z = 0.0f;
			memcpy(&me->m_orientation, &cfg[4], 4);
			me->poseMatrix();
			memcpy(&body.m_health, &cfg[5], 4);
			body.m_max = body.m_health;
		}
		crusher->m_crusher = (UnsignedByte)(cfg[6] & 15);
		crusher->m_physics = (cfg[6] >> 4 & 1) ? physics : 0;
		memcpy(&crusher->m_pos.x, &cfg[7], 4);
		memcpy(&crusher->m_pos.y, &cfg[8], 4);
		crusher->m_pos.z = 0.0f;
		memcpy(&physics->m_vel.x, &cfg[9], 4);
		memcpy(&physics->m_vel.y, &cfg[10], 4);
		physics->m_vel.z = 0.0f;
		g_r28ToppleDirs.clear();
		if (cfg[11]) {
			Coord3D dir;
			Real a = (Real)(cfg[11] % 8) * (2.0f * PI / 8.0f);
			dir.x = 3.0f * cosf(a);
			dir.y = 3.0f * sinf(a);
			dir.z = 0.0f;
			g_r28ToppleDirs.push_back(std::make_pair(AsciiString("Tree1"), dir));
		}
	};
	scene(true);
	gl.m_awakenHook = r28Awaken;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	ToppleUpdate *m = new ToppleUpdate(me, d);
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	BehaviorModule *behaviors[2] = { m, 0 };
	me->m_behaviors = behaviors;
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		bool pose = false;
		if (o[0] == 6 && o[1] >= 1 && o[1] < 12) { cfg[o[1]] = o[2]; pose = o[1] >= 2 && o[1] <= 5; }
		if (o[0] == 5) gl.m_frame = o[1];
		scene(pose);
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
			case 0: g_r28Updating = m; res = (unsigned)m->update(); break;
			case 1: res = m->isAbleToBeToppled() ? 1 : 0; break;
			case 2: case 4: {
				Coord3D dir;
				memcpy(&dir.x, &o[1], 4);
				memcpy(&dir.y, &o[2], 4);
				dir.z = 0.0f;
				Real speed;
				memcpy(&speed, &o[3], 4);
				if (o[0] == 2) m->applyTopplingForce(&dir, speed, o[4]);
				else if (m->isAbleToBeToppled()) m->applyTopplingForce(&dir, speed, o[4]);   /* Object::topple (Object.cpp lines 1195-1208): findModule finds it */
				break;
			}
			case 3: { Coord3D loc, normal; loc.zero(); normal.zero(); m->onCollide((o[1] & 1) ? crusher : 0, &loc, &normal); break; }
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r28Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r28Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r28Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) {
				const std::string &l = g_ulog[(size_t)e.p];
				if (l.compare(0, 13, "settransform ") == 0 || l.compare(0, 10, "setorient ") == 0) continue;   /* the transforms are the dump's */
				log.push_back(l);
			} else if (e.kind == 1 && e.p) {
				char b[32]; sprintf(b, "fx %d", fxId((const FXList *)e.p)); log.push_back(b);
			}
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(realBits(m->m_angularVelocity));
		putU32(realBits(m->m_angularAcceleration));
		putU32(realBits(m->m_toppleDirection.x));
		putU32(realBits(m->m_toppleDirection.y));
		putU32(realBits(m->m_toppleDirection.z));
		putU32((unsigned)m->m_toppleState);
		putU32(realBits(m->m_angularAccumulation));
		putU32(realBits(m->m_angleDeltaX));
		putU32((unsigned)m->m_numAngleDeltaX);
		putU32(m->m_doBounceFX ? 1 : 0);
		putU32(m->m_options);
		putU32((unsigned)m->m_stumpID);
		const Matrix3D &mx = me->m_matrix;
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits(mx[r][c]));
		putU32(realBits(me->m_pos.x));
		putU32(realBits(me->m_pos.y));
		putU32(realBits(me->m_pos.z));
		putU32(realBits(me->m_orientation));
		putU32(realBits(body.m_health));
		putU32((unsigned)g_r28Stumps.size());
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_xformHook = 0;
	g_r28ThingPose = false;
	me->m_behaviors = 0;
	delete m;
	delete d;
	me->m_body = 0; me->m_drawable = 0;
	crusher->m_physics = 0;
	delete me;
	delete crusher;
	for (Object *s : g_r28Stumps) { s->m_body = 0; s->m_drawable = 0; delete s; }
	g_r28Stumps.clear();
	TheAI = 0;
	TheScriptEngine = 0;
	TheThingFactory = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
