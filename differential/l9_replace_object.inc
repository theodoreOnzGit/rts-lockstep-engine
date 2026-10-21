/* Differential-test driver for ReplaceObjectUpgrade (p1w11-sonnet; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ReplaceObjectUpgrade.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_upgrades.inc, whose effect log and helpers
** it uses) on the stand-ins of differential/shim_module: a thing factory that knows three templates (RO_A, RO_B, RO_C, each defined or not, each
** with up to four behaviour modules that have a create interface or not), whose newObject makes a stand-in Object with those modules, the team
** (NULL is the neutral default team, team 1) and the controlling player the team has; a pathfinder; a GameLogic::destroyObject that logs and marks
** the object; and Player::onStructureConstructionComplete. The REAL UpgradeCenter and INI. Every call is a line of the effect log (ulog), and the
** objects' state (destroyed, template, team, player, matrix) is dumped after every op. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ReplaceObjectUpgrade.cpp"

namespace {

/* the three templates: name, defined, number of modules, which of them have a create interface */
struct RoTmpl { const char *name; int defined; int n; int create[4]; };
RoTmpl g_roTmpl[3] = { { "RO_A", 0, 0, {0,0,0,0} }, { "RO_B", 0, 0, {0,0,0,0} }, { "RO_C", 0, 0, {0,0,0,0} } };
Player *g_roPlayers[3];
int g_roTeamPlayer[256];     /* the player index each team has, -1 none (index 1..3 for the replace driver, any team below 256 for the power driver) */
int g_roNextId = 0;
int g_roMeId = 0;
std::vector<Object *> g_roObjects;   /* in id order */
std::vector<BehaviorModule *> g_roModules;
std::vector<int> g_roTemplateOf;     /* per object: 0 the object under test, 1..3 the template */

class RoCreate : public CreateModuleInterface {
public:
	RoCreate() : m_oid(0), m_idx(0) {}
	virtual void onCreate() {}
	virtual void onBuildComplete() { ulog("buildcomplete %d %d", m_oid, m_idx); }
	virtual Bool shouldDoOnBuildComplete() const { return true; }
	int m_oid, m_idx;
};
class RoBehavior : public TestBehavior {
public:
	RoBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d), m_create(false) {}
	virtual CreateModuleInterface *getCreate() { return m_create ? &m_ci : 0; }
	bool m_create;
	RoCreate m_ci;
};
ModuleData g_roModuleData;

void roDestroyHook(Object *obj)
{
	ulog("destroy %d", (int)obj->m_oid);
	obj->m_destroyed = true;
}

}  // namespace

const ThingTemplate *ThingFactory::findTemplate(const AsciiString &name, Bool check)
{
	for (int i = 0; i < 3; ++i) if (g_roTmpl[i].defined && strcmp(name.str(), g_roTmpl[i].name) == 0) return (const ThingTemplate *)&g_roTmpl[i];
	return 0;
}
Object *ThingFactory::newObject(const ThingTemplate *tmplate, Team *team, ObjectStatusMaskType statusMask)
{
	const RoTmpl *t = (const RoTmpl *)tmplate;
	int ti = (int)(t - g_roTmpl);
	Object *o = new Object;
	o->m_oid = g_roNextId++;
	o->m_team = team ? team : (Team *)(size_t)1;
	int tp = g_roTeamPlayer[(int)(size_t)o->m_team];
	o->m_player = tp >= 0 ? g_roPlayers[tp] : 0;
	BehaviorModule **list = new BehaviorModule *[t->n + 1];
	for (int i = 0; i < t->n; ++i) {
		RoBehavior *b = new RoBehavior(o, &g_roModuleData);
		b->m_create = t->create[i] != 0;
		b->m_ci.m_oid = o->m_oid;
		b->m_ci.m_idx = i;
		list[i] = b;
		g_roModules.push_back(b);
	}
	list[t->n] = 0;
	o->m_behaviors = list;
	g_roObjects.push_back(o);
	g_roTemplateOf.push_back(ti + 1);
	ulog("newobject %d", (int)o->m_oid);
	return o;
}
void Pathfinder::addObjectToPathfindMap(Object *obj) { ulog("pathadd %d", (int)obj->m_oid); }
void Pathfinder::removeObjectFromPathfindMap(Object *obj) { ulog("pathremove %d", (int)obj->m_oid); }
void Player::onStructureConstructionComplete(Object *builder, Object *structure, Bool isRebuild)
{
	ulog("structdone %d %d %d %d", (int)m_idx, (int)builder->m_oid, (int)structure->m_oid, (int)isRebuild);
}
void Object::setTransformMatrix(const Matrix3D *mx) { m_matrix = *mx; ulog("settransform %d", (int)m_oid); }
void (*g_modDestroyHook)(Object *obj) = 0;
ThingFactory *TheThingFactory = 0;
AI *TheAI = 0;

namespace {

ModuleData *l9roRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ReplaceObjectUpgrade::friend_newModuleData(&ini);
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

void putMatrix(const Matrix3D &m)
{
	for (int r = 0; r < 3; ++r) { putU32(realBits(m.Row[r].X)); putU32(realBits(m.Row[r].Y)); putU32(realBits(m.Row[r].Z)); putU32(realBits(m.Row[r].W)); }
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a ReplaceObjectUpgrade and make the module on an Object (id cfg[0]) of team cfg[1] (1..3). `cfg`:
** [the object's id, its team, RO_A, RO_B, RO_C (bit 31 defined, bits 4-6 the number of modules, bits 0-3 which have a create interface), the
** player of team 1, 2, 3 (0 none, else the index + 1)]; three players with completed upgrades `player_mask`, the object's own are `obj_mask`.
** The data dump goes first (the mux, then the name). Then `n` ops of 5 words [op, a, b, c, d]: 0 wouldUpgrade  1 attemptUpgrade  2 resetUpgrade
** 3 forceRefreshUpgrade  4 isAlreadyUpgraded (m = a..d as 128 bits)  5 the object's completed upgrades = m  6 the players' = m  7 the object's
** matrix = identity with translation (a, b, c) as float bits. Each op appends: the CATCH_ALL status, the Bool result, isAlreadyUpgraded(), the
** effect log (count, then each as a string), and the objects (count, then for each: id, destroyed, template 0..3, team, player, 12 matrix words). */
int ref_l9ro_run(const char *filename, const unsigned *player_mask, const unsigned *obj_mask, const unsigned *cfg, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9roRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	putMux(((UpgradeModuleData *)d)->m_upgradeMuxData);
	putAscii(((ReplaceObjectUpgradeModuleData *)d)->m_replaceObjectName);
	Player players[3];
	for (int i = 0; i < 3; ++i) { players[i].m_idx = i; players[i].m_upgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, player_mask); g_roPlayers[i] = &players[i]; }
	for (int t = 1; t <= 3; ++t) g_roTeamPlayer[t] = (int)cfg[4 + t] - 1;
	for (int i = 0; i < 3; ++i) {
		g_roTmpl[i].defined = (cfg[2 + i] >> 31) & 1;
		g_roTmpl[i].n = (cfg[2 + i] >> 4) & 7;
		for (int k = 0; k < 4; ++k) g_roTmpl[i].create[k] = (cfg[2 + i] >> k) & 1;
	}
	g_roObjects.clear();
	g_roModules.clear();
	g_roTemplateOf.clear();
	g_roMeId = (int)cfg[0];
	g_roNextId = g_roMeId + 1;
	ThingFactory factory;
	TheThingFactory = &factory;
	AI ai;
	TheAI = &ai;
	g_modDestroyHook = roDestroyHook;
	Object *obj = newObject();
	obj->m_oid = g_roMeId;
	obj->m_team = (Team *)(size_t)cfg[1];
	{
		int tp = g_roTeamPlayer[cfg[1]];
		obj->m_player = tp >= 0 ? &players[tp] : 0;
	}
	obj->m_objectUpgradesCompleted = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, obj_mask);
	obj->m_matrix.Make_Identity();
	obj->m_xp = new ExperienceTracker;
	g_roObjects.push_back(obj);
	g_roTemplateOf.push_back(0);
	UpgradeModule *m = new ReplaceObjectUpgrade(obj, d);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		g_events.clear();
		g_ulog.clear();
		int st = 0, res = 0;
		unsigned c2 = 0;
		char m2[16];
		UpgradeMaskType mask = maskFromU32<UpgradeMaskType>(UPGRADE_MAX_COUNT, o + 1);
		try {
			switch (o[0]) {
			case 0: res = m->wouldUpgrade(mask) ? 1 : 0; break;
			case 1: res = m->attemptUpgrade(mask) ? 1 : 0; break;
			case 2: res = m->resetUpgrade(mask) ? 1 : 0; break;
			case 3: m->forceRefreshUpgrade(); break;
			case 4: res = m->isAlreadyUpgraded() ? 1 : 0; break;
			case 5: obj->m_objectUpgradesCompleted = mask; break;
			case 6: for (int p = 0; p < 3; ++p) players[p].m_upgradesCompleted = mask; break;
			case 7:
			{
				Matrix3D mx(1);
				float x, y, z;
				memcpy(&x, &o[1], 4); memcpy(&y, &o[2], 4); memcpy(&z, &o[3], 4);
				mx.Set_Translation(Vector3(x, y, z));
				obj->m_matrix = mx;   /* the object's own, as a test sets it: no log */
				break;
			}
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		g_dump.push_back((unsigned char)res);
		g_dump.push_back(m->isAlreadyUpgraded() ? 1 : 0);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
			else if (e.kind == 1) { char b[32]; sprintf(b, "fx%d", fxId((const FXList *)e.p)); log.push_back(b); }
			else if (e.kind == 2) log.push_back(std::string("remove ") + (e.p ? ((const UpgradeTemplate *)e.p)->getUpgradeName().str() : "(null)"));
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)g_roObjects.size());
		for (size_t j = 0; j < g_roObjects.size(); ++j) {
			const Object *ob = g_roObjects[j];
			putI32(ob->m_oid);
			putI32(ob->m_destroyed ? 1 : 0);
			putI32(g_roTemplateOf[j]);
			putI32((int)(size_t)ob->m_team);
			putI32(ob->m_player ? ob->m_player->m_idx : -1);
			putMatrix(ob->m_matrix);
		}
	}
	*len = finish(buf, cap);
	g_modDestroyHook = 0;
	TheThingFactory = 0;
	TheAI = 0;
	delete m;
	delete d;
	for (size_t j = 0; j < g_roModules.size(); ++j) delete g_roModules[j];
	for (size_t j = 1; j < g_roObjects.size(); ++j) { delete[] g_roObjects[j]->m_behaviors; g_roObjects[j]->m_behaviors = 0; delete g_roObjects[j]; }
	delete obj;
	g_roObjects.clear();
	g_roModules.clear();
	return 0;
}

}  /* extern "C" */
