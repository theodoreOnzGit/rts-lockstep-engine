/* Differential-test driver for the rebuild hole (p1w13-sonnet, batch R15; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/RebuildHoleExposeDie.cpp
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/RebuildHoleBehavior.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its end, on the module build's stand-ins: four objects (ids 1 to 4: the module's object, an attacker, a building
** under construction, a mine with a sticky bomb), each maybe with an AI, in the game logic's list (newest first, the objects the factory makes
** in front); three players (0 the neutral one), each maybe an observer or dead; a thing factory that knows U1 and U2 and makes a bare object (ids 5
** up) with, as the test says, a body, a RebuildHoleBehavior stand-in and an AI whose `construct` returns the object the test names. Every call
** is a line of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/RebuildHoleExposeDie.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/RebuildHoleBehavior.cpp"

Object *g_r15Construct = 0;   /* what an AI's construct returns */
Object *AIUpdateInterface::construct(const ThingTemplate *what, const Coord3D *pos, Real angle, Player *owner, Bool isRebuild)
{
	ulog("construct %d %s %08x %08x %08x %08x %d %d", (int)m_owner->m_oid, what ? what->getName().str() : "-", realBits(pos->x), realBits(pos->y),
		realBits(pos->z), realBits(angle), owner ? (int)owner->m_idx : -1, isRebuild ? 1 : 0);
	return g_r15Construct;
}
void AIUpdateInterface::aiResumeConstruction(Object *obj, CommandSourceType cmdSource)
{
	ulog("resume %d %d %d", (int)m_owner->m_oid, obj ? (int)obj->m_oid : 0, (int)cmdSource);
}
void Object::maskObject(Bool mask) { ulog("mask %d %d", (int)m_oid, mask ? 1 : 0); }
void Object::setProducer(const Object *obj) { m_producerID = obj ? obj->getID() : INVALID_ID; }

namespace {

/* a body: health and maximum, healing as the Rust probe does it (a line of the log) */
class R15Body : public InactiveBody {
public:
	R15Body(Thing *t, const ModuleData *d) : InactiveBody(t, d) {}
	virtual Real getHealth() const { return m_health; }
	virtual Real getMaxHealth() const { return m_max; }
	virtual void setMaxHealth(Real maxHealth, MaxHealthChangeType) { m_max = maxHealth; if (m_health > maxHealth) m_health = maxHealth; }
	virtual void attemptHealing(DamageInfo *info)
	{
		const int oid = (int)AsObject(const_cast<Thing *>(static_cast<const Thing *>(getObject())))->m_oid;
		ulog("heal %d %08x %d", oid, realBits(info->in.m_amount), (int)info->in.m_sourceID);
		Real room = m_max - m_health;
		m_health += (info->in.m_damageType == DAMAGE_HEALING && info->in.m_amount < room) ? info->in.m_amount : room;
	}
	Real m_health = 0, m_max = 0;
};

/* a hole's RebuildHoleBehavior: the interface the expose die tells */
class R15HoleIf : public RebuildHoleBehaviorInterface {
public:
	virtual void startRebuildProcess(const ThingTemplate *rebuild, ObjectID spawnerID)
	{
		ulog("startrebuild %d %s %d", m_oid, rebuild ? rebuild->getName().str() : "-", (int)spawnerID);
	}
	virtual ObjectID getSpawnerID() { return INVALID_ID; }
	virtual ObjectID getReconstructedBuildingID() { return INVALID_ID; }
	virtual const ThingTemplate *getRebuildTemplate() const { return 0; }
	int m_oid = 0;
};
class R15HoleBehavior : public TestBehavior {
public:
	R15HoleBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d) {}
	virtual RebuildHoleBehaviorInterface *getRebuildHoleBehaviorInterface() { return &m_if; }
	R15HoleIf m_if;
};

Object *g_r15Objs[4];
std::vector<Object *> g_r15Made;
std::vector<R15Body *> g_r15Bodies;
std::vector<R15HoleBehavior *> g_r15Holes;
std::vector<BehaviorModule **> g_r15Lists;
std::vector<AIUpdateInterface *> g_r15Ais;
int g_r15NextId = 5;
unsigned g_r15MakeFlags = 0;   /* bit 0 a body, 1 a RebuildHoleBehavior, 2 an AI */
Real g_r15MadeMax = 0;
ModuleData g_r15Plain;
bool g_r15Gone[4];
ThingTemplate g_r15Templates[2];
Object *r15Find(ObjectID id)
{
	for (Object *o = g_ddFirst; o; o = o->m_next) if ((ObjectID)o->m_oid == id) return o;
	return 0;
}
const ThingTemplate *r15FindTemplate(const AsciiString &name)
{
	for (int k = 0; k < 2; ++k) if (g_r15Templates[k].m_name.compare(name) == 0) return &g_r15Templates[k];
	return 0;
}
Object *r15NewObject(const ThingTemplate *tmplate, Team *team)
{
	ulog("newobject %s %d", tmplate->getName().str(), team ? (int)team->m_dummy : -1);
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r15NextId++;
	o->m_team = team;
	o->m_tmpl.m_name = tmplate->getName();
	o->m_next = g_ddFirst;
	g_ddFirst = o;
	if (g_r15MakeFlags & 1) {
		R15Body *b = new R15Body(o, &g_r15Plain);
		b->m_max = b->m_health = g_r15MadeMax;
		o->m_body = b;
		g_r15Bodies.push_back(b);
	}
	/* a real object's list is never NULL: empty, or the hole's behaviour */
	BehaviorModule **list = new BehaviorModule *[2];
	list[0] = list[1] = 0;
	if (g_r15MakeFlags & 2) {
		R15HoleBehavior *h = new R15HoleBehavior(o, &g_r15Plain);
		h->m_if.m_oid = (int)o->m_oid;
		list[0] = h;
		g_r15Holes.push_back(h);
	}
	o->m_behaviors = list;
	g_r15Lists.push_back(list);
	if (g_r15MakeFlags & 4) {
		AIUpdateInterface *ai = new AIUpdateInterface;
		ai->m_owner = o;
		o->m_ai = ai;
		g_r15Ais.push_back(ai);
	}
	g_r15Made.push_back(o);
	return o;
}
/* the mine's sticky bomb: a stand-in the object list answers findUpdateModule with; its target is a word of the test */
unsigned char g_r15BombBuf[16] = { 0 };
ObjectID g_r15BombTarget = INVALID_ID;
UpdateModule *r15FindUpdate(const Object *o, NameKeyType key)
{
	return (o == g_r15Objs[3] && !g_r15Gone[3] && key == TheNameKeyGenerator->nameToKey("StickyBombUpdate")) ? (UpdateModule *)(void *)g_r15BombBuf : 0;
}
int g_r15Rel = 0;

ModuleData *l9r15Read(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			if (kind == 0) data = RebuildHoleExposeDie::friend_newModuleData(&ini);
			else data = RebuildHoleBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r15Saved;

/* takes object `id` out of the game logic's list (nothing if it is not there) */
void r15Remove(ObjectID id)
{
	Object **p = &g_ddFirst;
	while (*p && (*p)->getID() != id) p = &(*p)->m_next;
	if (*p) *p = (*p)->m_next;
	if (id >= 1 && id <= 4) g_r15Gone[id - 1] = true;
}

}  // namespace

Object *StickyBombUpdate::getTargetObject() const { return r15Find(g_r15BombTarget); }
void StickyBombUpdate::setTargetObject(Object *obj)
{
	ulog("bombtarget %d %d", (int)g_r15Objs[3]->m_oid, obj ? (int)obj->m_oid : 0);
	g_r15BombTarget = obj ? obj->getID() : INVALID_ID;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a RebuildHoleExposeDie (`kind` 0) or a RebuildHoleBehavior (`kind` 1) and make the module on object
** 1 (team 1). `cfg` (12 words): [0 object 1's player (0xFFFFFFFF none; player 0 is neutral), 1 flags (bits 0-3 objects 1 to 4 have an AI, 4 object
** 1 has a body, 5 object 3 is under construction, 6-8 players 0 to 2 are observers, 9-11 dead), 2 what the factory gives an object it makes (bit 0
** a body, 1 a RebuildHoleBehavior, 2 an AI), 3 that body's health (float bits), 4 the object an AI's construct returns (0 none, else an id), 5
** the sticky bomb's target (an id), 6 object 1's health, 7 its maximum (float bits), 8-9 its position, 10 its orientation, 11 its geometry's
** major radius (float bits)]. The data dump goes first (the die mux and the three fields, or the three fields). Then `n` ops of 6 words [op, a, b,
** c, d, e]: 0 onDie (death type a, veterancy b, status words c and d, damage source e) 1 update() (kind 1) 2 flags = a 3 startRebuildProcess(a ?
** U<a> : NULL, b) (kind 1) 4 object a (2 to 5) is gone 5 object 1's health = a 6 what construct returns = a 7 xfer save 8 xfer load of the last
** save 9 xfer CRC. Each op appends: the CATCH_ALL status, the effect log (count, then each), the xfer status, bytes and CRC, the module's state
** (kind 1: worker, reconstruction, spawner, wait, the two template names; kind 0: nothing), object 1's health and maximum, object 3's producer,
** then the objects made so far (count, then each one's id, team, position, major radius, its body's maximum health or 0, its UNSELECTABLE bit). */
int ref_l9r15_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r15Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r15Read(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	if (kind == 0) {
		const RebuildHoleExposeDieModuleData *ed = (const RebuildHoleExposeDieModuleData *)d;
		putDieMux(ed->m_dieMuxData);
		putStr(ed->m_holeName.str(), ed->m_holeName.getLength());
		putU32(realBits(ed->m_holeMaxHealth));
		putU32(ed->m_transferAttackers ? 1 : 0);
	} else {
		const RebuildHoleBehaviorModuleData *bd = (const RebuildHoleBehaviorModuleData *)d;
		putStr(bd->m_workerTemplateName.str(), bd->m_workerTemplateName.getLength());
		putU32(realBits(bd->m_workerRespawnDelay));
		putU32(realBits(bd->m_holeHealthRegenPercentPerSecond));
	}
	GameLogic gl;
	gl.m_frame = 0;
	TheGameLogic = &gl;
	unsigned flags = cfg[1];
	Team defaults[3];
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		defaults[i].m_dummy = 11 + i;
		players[i].m_defaultTeam = &defaults[i];
		players[i].m_type = PLAYER_HUMAN;   /* the stand-in has no constructor */
	}
	g_tbNeutral = &players[0];
	g_r15Templates[0].m_name = "U1";
	g_r15Templates[1].m_name = "U2";
	g_findTemplateHook = r15FindTemplate;
	g_newObjectHook = r15NewObject;
	g_findUpdateHook = r15FindUpdate;
	g_r15Made.clear();
	g_r15NextId = 5;
	g_r15MakeFlags = cfg[2];
	g_r15MadeMax = ctF(cfg[3]);
	g_r15BombTarget = (ObjectID)cfg[5];
	Team teams[4];
	AIUpdateInterface ais[4];
	g_ddFirst = 0;
	for (int k = 0; k < 4; ++k) {
		g_r15Objs[k] = newObject();
		g_r15Objs[k]->m_oid = (ObjectID)(k + 1);
		teams[k].m_dummy = k + 1;
		g_r15Objs[k]->m_team = &teams[k];
		g_r15Objs[k]->m_next = g_ddFirst;
		g_ddFirst = g_r15Objs[k];
		ais[k].m_owner = g_r15Objs[k];
		g_r15Gone[k] = false;
	}
	Object *obj = g_r15Objs[0];
	obj->m_player = cfg[0] == 0xFFFFFFFFu ? 0 : &players[cfg[0]];
	obj->m_name = "Hole1";
	obj->m_tmpl.m_name = "U2";
	obj->m_pos.x = ctF(cfg[8]);
	obj->m_pos.y = ctF(cfg[9]);
	obj->m_orientation = ctF(cfg[10]);
	obj->m_geometry = GeometryInfo(GEOMETRY_BOX, false, 10.0f, ctF(cfg[11]), ctF(cfg[11]));
	g_r15Objs[3]->m_kindOf.set(KINDOF_MINE);
	g_r15Objs[3]->m_tmpl.m_kindOf.set(KINDOF_MINE);
	R15Body body(obj, &g_r15Plain);
	body.m_health = ctF(cfg[6]);
	body.m_max = ctF(cfg[7]);
	g_r15Construct = cfg[4] ? r15Find((ObjectID)cfg[4]) : 0;
	g_modFindHook = r15Find;
	g_modDestroyHook = cfDestroyHook;
	DieModule *ed = 0;
	RebuildHoleBehavior *hb = 0;
	Snapshot *snap = 0;
	if (kind == 0) { ed = new RebuildHoleExposeDie(obj, d); snap = ed; }
	else { hb = new RebuildHoleBehavior(obj, d); hb->friend_setNextCallFrame(1); snap = hb; }
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 2) flags = o[1];
		if (o[0] == 4) r15Remove((ObjectID)o[1]);
		if (o[0] == 5) body.m_health = ctF(o[1]);
		if (o[0] == 6) g_r15Construct = o[1] ? r15Find((ObjectID)o[1]) : 0;
		for (int k = 0; k < 4; ++k) g_r15Objs[k]->m_ai = (flags >> k & 1) ? &ais[k] : 0;
		obj->m_body = (flags & 16) ? &body : 0;
		if (flags & 32) g_r15Objs[2]->m_status.set(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_UNDER_CONSTRUCTION));
		else g_r15Objs[2]->m_status.clear(MAKE_OBJECT_STATUS_MASK(OBJECT_STATUS_UNDER_CONSTRUCTION));
		for (int p = 0; p < 3; ++p) {
			players[p].m_observer = (flags >> (6 + p) & 1) != 0;
			players[p].m_dead = (flags >> (9 + p) & 1) != 0;
		}
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		try {
			switch (o[0]) {
			case 0: {
				DamageInfo info;
				info.in.m_deathType = (DeathType)o[1];
				info.in.m_sourceID = (ObjectID)o[5];
				obj->m_level = (VeterancyLevel)o[2];
				obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, o + 3);
				if (kind == 0) dynamic_cast<DieModuleInterface *>(ed)->onDie(&info);
				else hb->onDie(&info);
				break;
			}
			case 1: if (hb) hb->update(); break;
			case 3: if (hb) hb->startRebuildProcess(o[1] ? &g_r15Templates[o[1] - 1] : 0, (ObjectID)o[2]); break;
			case 7: xst = runXfer(snap, 0, xfer_path, bytes, 512, &blen, &crc); g_r15Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r15Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(snap, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(snap, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		if (hb) {
			putU32((unsigned)hb->m_workerID);
			putU32((unsigned)hb->m_reconstructingID);
			putU32((unsigned)hb->m_spawnerObjectID);
			putU32(hb->m_workerWaitCounter);
			AsciiString w = hb->m_workerTemplate ? hb->m_workerTemplate->getName() : AsciiString::TheEmptyString;
			AsciiString r = hb->m_rebuildTemplate ? hb->m_rebuildTemplate->getName() : AsciiString::TheEmptyString;
			putStr(w.str(), w.getLength());
			putStr(r.str(), r.getLength());
		}
		putU32(realBits(body.m_health));
		putU32(realBits(body.m_max));
		putU32(g_r15Gone[2] ? 0u : (unsigned)g_r15Objs[2]->m_producerID);   /* 0 for an object that is gone, as the Rust side */
		std::vector<const Object *> made;   /* the objects made that are still in the game */
		for (size_t j = 0; j < g_r15Made.size(); ++j) if (r15Find(g_r15Made[j]->getID())) made.push_back(g_r15Made[j]);
		putU32((unsigned)made.size());
		for (size_t j = 0; j < made.size(); ++j) {
			const Object *x = made[j];
			putU32((unsigned)x->m_oid);
			putI32(x->m_team ? (int)x->m_team->m_dummy : -1);
			putU32(realBits(x->m_pos.x));
			putU32(realBits(x->m_pos.y));
			putU32(realBits(x->m_pos.z));
			putU32(realBits(x->m_geometry.getMajorRadius()));
			putU32(x->m_body ? realBits(((R15Body *)x->m_body)->m_max) : 0);
			putU32(x->m_status.test(OBJECT_STATUS_UNSELECTABLE) ? 1 : 0);
		}
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_findUpdateHook = 0;
	g_tbNeutral = 0;
	g_ddFirst = 0;
	delete ed;
	delete hb;
	delete d;
	for (size_t j = 0; j < g_r15Made.size(); ++j) { g_r15Made[j]->m_body = 0; g_r15Made[j]->m_ai = 0; g_r15Made[j]->m_behaviors = 0; delete g_r15Made[j]; }
	g_r15Made.clear();
	for (size_t j = 0; j < g_r15Bodies.size(); ++j) delete g_r15Bodies[j];
	g_r15Bodies.clear();
	for (size_t j = 0; j < g_r15Holes.size(); ++j) delete g_r15Holes[j];
	g_r15Holes.clear();
	for (size_t j = 0; j < g_r15Lists.size(); ++j) delete[] g_r15Lists[j];
	g_r15Lists.clear();
	for (size_t j = 0; j < g_r15Ais.size(); ++j) delete g_r15Ais[j];
	g_r15Ais.clear();
	for (int k = 0; k < 4; ++k) { g_r15Objs[k]->m_ai = 0; g_r15Objs[k]->m_body = 0; delete g_r15Objs[k]; g_r15Objs[k] = 0; }
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
