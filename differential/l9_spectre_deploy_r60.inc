/* Differential-test driver for SpectreGunshipDeploymentUpdate (p1w13-sonnet, batch R60; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipDeploymentUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end (SpecialPowerUpdateModule.cpp, its base, is batch R53's), on the module
** build's stand-ins: the command centre (id 1: maybe a special power module, a REAL SpecialPowerModule of SP_0..SP_3 whose setReadyFrame,
** markSpecialPowerTriggered and doSpecialPowerAtLocation are lines; sold, under construction, effectively dead or not; a team of three, each
** with its player, whose sciences are the test's), Object::getSpecialPowerModule (batch R53's, verbatim), the thing factory (GunshipA: an
** AI and a special power module; GunshipB: an AI; GunshipC: neither; newObject makes gunships 2, 3, ...; the AI's current locomotor and its
** preferred height the test's), the terrain's findClosestEdgePoint and findFarthestEdgePoint (offsets: batch N's and batch R22's), the game
** logic's selectObject (a line), the science store (a name is its NAMEKEY), the special power store (the REAL one the test loaded), the REAL
** UpdateModule, INI and Xfer; Thing's orientation (batch R28's pose). */
/* (the header forward-declares `enum ParticleSystemID;`, which the shim's ParticleSys.h has as a typedef: renamed by macro for the include, to
** an enum defined first, as clang refuses a forward-declared enum; the files use the name nowhere else) */
enum R60ParticleSystemIDFwd { R60_PARTICLE_SYSTEM_ID_FWD };
#define ParticleSystemID R60ParticleSystemIDFwd
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipDeploymentUpdate.cpp"
#undef ParticleSystemID

Real Locomotor::getPreferredHeight() const { return ((const AIUpdateInterface *)(const void *)this)->m_r60Height; }
void GameLogic::selectObject(Object *obj, Bool createNewSelection, PlayerMaskType playerMask, Bool affectClient)
{
	ulog("select %d %d %u %d", (int)obj->m_oid, (int)createNewSelection, (unsigned)playerMask, (int)affectClient);
}
Coord3D *r22FarOffsetPtr();   /* l9_ocl_power_r22.inc, later in this unit */
extern "C" ScienceType (*ref_science_lookup_hook)(const ScienceStore *, const char *);

namespace {

ModuleData *l9r60Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SpectreGunshipDeploymentUpdate::friend_newModuleData(&ini);
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

/* a REAL special power module (upstream casts the interface to SpecialPowerModule) whose three calls are lines */
class R60Power : public SpecialPowerModule {
public:
	R60Power(Thing *t, const ModuleData *d) : SpecialPowerModule(t, d) {}
	virtual ~R60Power() {}
	virtual void setReadyFrame(UnsignedInt frame) { ulog("readyframe %d %u", (int)getObject()->m_oid, frame); }
	virtual void markSpecialPowerTriggered(const Coord3D *l)
	{
		if (l) ulog("triggered %d %u,%u,%u", (int)getObject()->m_oid, realBits(l->x), realBits(l->y), realBits(l->z));
		else ulog("triggered %d null", (int)getObject()->m_oid);
	}
	virtual void doSpecialPowerAtLocation(const Coord3D *l, Real angle, UnsignedInt opts)
	{
		ulog("poweratloc %d %u,%u,%u %u %u", (int)getObject()->m_oid, realBits(l->x), realBits(l->y), realBits(l->z), realBits(angle), opts);
	}
};

std::vector<unsigned char> g_r60Saved;
Object *g_r60Me;
int g_r60Tmpl[3];
const char *const g_r60TmplNames[3] = { "GunshipA", "GunshipB", "GunshipC" };
std::vector<Object *> g_r60Ships;
std::vector<AIUpdateInterface *> g_r60Ais;
std::vector<SpecialPowerModuleData *> g_r60PowerData;
std::vector<R60Power *> g_r60Powers;
std::vector<BehaviorModule **> g_r60Behaviors;
unsigned g_r60NextId = 2;
unsigned g_r60Cfg[12];
Object *r60Find(ObjectID id)
{
	if ((int)id == 1) return g_r60Me;
	for (size_t i = 0; i < g_r60Ships.size(); ++i) if (g_r60Ships[i]->m_oid == id) return g_r60Ships[i];
	return 0;
}
int r60Code(const ThingTemplate *t) { for (int i = 0; i < 3; ++i) if (t == (const ThingTemplate *)&g_r60Tmpl[i]) return i + 1; return 0; }
const ThingTemplate *r60FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 3; ++i) if (strcmp(name.str(), g_r60TmplNames[i]) == 0) return (const ThingTemplate *)&g_r60Tmpl[i];
	return 0;
}
const SpecialPowerTemplate *r60Template(unsigned k)
{
	if (k > 3) return 0;
	char name[8];
	sprintf(name, "SP_%u", k);
	return (const SpecialPowerTemplate *)ref_sp_find_raw(name);
}
R60Power *r60NewPower(Object *o, unsigned k)
{
	SpecialPowerModuleData *pd = new SpecialPowerModuleData;
	pd->m_specialPowerTemplate = r60Template(k);
	g_r60PowerData.push_back(pd);
	R60Power *p = new R60Power(o, pd);
	g_r60Powers.push_back(p);
	return p;
}
/* the gunship: cfg[2] bit 0 its AI has a current locomotor, bits 1-3 its power's template (GunshipA; 0-3, else SP_0); cfg[3] the height */
Object *r60NewObject(const ThingTemplate *tmplate, Team *team)
{
	int code = r60Code(tmplate);
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r60NextId++;
	o->m_team = team;
	BehaviorModule **b = new BehaviorModule *[2];
	b[0] = 0; b[1] = 0;
	if (code == 1) { unsigned k = g_r60Cfg[2] >> 1 & 7; b[0] = r60NewPower(o, k <= 3 ? k : 0); }
	o->m_behaviors = b;
	g_r60Behaviors.push_back(b);
	if (code == 1 || code == 2) {
		AIUpdateInterface *ai = new AIUpdateInterface;
		ai->m_owner = o;
		ai->m_hasLoco = (g_r60Cfg[2] & 1) != 0;
		memcpy(&ai->m_r60Height, &g_r60Cfg[3], 4);
		o->m_ai = ai;
		g_r60Ais.push_back(ai);
	}
	g_r60Ships.push_back(o);
	return o;
}
Real r60F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
ScienceType r60Science(const ScienceStore *, const char *name) { return (ScienceType)NAMEKEY(name); }

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SpectreGunshipDeploymentUpdate and make the module on the command centre (id 1) at frame cfg[0],
** then onObjectCreated. `cfg` (12 words; 1-11 also settable by op 2, cfg[1]'s bits 0-3 kept): 1 the centre: bit 0 a special power module,
** 1-3 its template (SP_k; 0-3, else SP_0), 4 sold, 5 under construction, 6 effectively dead, 7-8 its team (0 none, else 1-3, with its
** player), 9 and 10 the players have SCIENCE_A, SCIENCE_B; 2 the new gunship (see r60NewObject); 3 its preferred height; 4, 5 the centre's x,
** y; 6-8 the closest edge point's offset, 9-11 the farthest's (float bits). The data dump goes first (the special power template's id, the
** science's name, the gunship's name, the attack radius, the orbit radius, the create location). Then `n` ops of 5 words [op, a, b, c, d]:
** 0 update()  1 the frame = a  2 cfg[a] = b  3 initiateIntentToDoSpecialPower(SP_(a >> 8), else NULL; NULL; a bit 0 ? (b, c, 0) : NULL;
** NULL; (a bit 1 ? COMMAND_FIRED_BY_SCRIPT : 0) | (a & 0xff) >> 2)  4 doesSpecialPowerUpdatePassScienceTest()  5 xfer save  6 xfer load of the last save
** 7 xfer CRC. Each op appends: the CATCH_ALL status, the result, the effect log (but the orientation lines), the xfer status, bytes and CRC,
** the state (the gunship's id, the target position), the gunships made and the last one's position, orientation, producer and team. */
static int l9r60RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r60Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	static ScienceStore stand_in_store;
	ScienceStore *oldStore = TheScienceStore;
	if (!TheScienceStore) TheScienceStore = &stand_in_store;
	ScienceType (*oldLookup)(const ScienceStore *, const char *) = ref_science_lookup_hook;
	ref_science_lookup_hook = r60Science;
	ref_ini_sp_bind(ref_sp_find_raw);
	try { d = l9r60Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_ini_sp_bind(0);
	ref_science_lookup_hook = oldLookup;
	TheScienceStore = oldStore;
	if (status) return status;
	memcpy(g_r60Cfg, cfg_in, sizeof(g_r60Cfg));
	unsigned *cfg = g_r60Cfg;
	{
		const SpectreGunshipDeploymentUpdateModuleData *sd = (const SpectreGunshipDeploymentUpdateModuleData *)d;
		putU32(ref_sp_raw_id(sd->m_specialPowerTemplate));
		AsciiString sci = sd->m_extraRequiredScience == SCIENCE_INVALID ? AsciiString() : TheNameKeyGenerator->keyToName((NameKeyType)sd->m_extraRequiredScience);
		putStr(sci.str(), (int)sci.getLength());
		putStr(sd->m_gunshipTemplateName.str(), (int)sd->m_gunshipTemplateName.getLength());
		putU32(realBits(sd->m_attackAreaRadius));
		putU32(realBits(sd->m_gunshipOrbitRadius));
		putI32((int)sd->m_createLoc);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheTerrainLogic = &g_ocTerrain;
	g_r60NextId = 2;
	g_r28ThingPose = true;
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
		players[p].m_defaultTeam = &teams[p];
	}
	BehaviorModule *none[1] = { 0 };
	g_r60Me = newObject();
	Object *me = g_r60Me;
	me->m_oid = 1;
	me->m_behaviors = none;
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_status.clear();
		if (f >> 4 & 1) me->m_status.set(OBJECT_STATUS_SOLD);
		if (f >> 5 & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		me->m_effDead = (f >> 6 & 1) != 0;
		unsigned t = f >> 7 & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		for (int p = 0; p < 3; ++p) {
			players[p].m_sciences.clear();
			if (f >> 9 & 1) players[p].m_sciences.push_back((ScienceType)NAMEKEY("SCIENCE_A"));
			if (f >> 10 & 1) players[p].m_sciences.push_back((ScienceType)NAMEKEY("SCIENCE_B"));
		}
		me->m_pos.x = r60F(cfg[4]);
		me->m_pos.y = r60F(cfg[5]);
		me->m_pos.z = 0.0f;
		g_ocEdgeOffset.x = r60F(cfg[6]); g_ocEdgeOffset.y = r60F(cfg[7]); g_ocEdgeOffset.z = r60F(cfg[8]);
		Coord3D *far = r22FarOffsetPtr();
		far->x = r60F(cfg[9]); far->y = r60F(cfg[10]); far->z = r60F(cfg[11]);
	};
	scene();
	g_modFindHook = r60Find;
	g_findTemplateHook = r60FindTemplate;
	g_newObjectHook = r60NewObject;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 10, "setorient ") == 0) continue;
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	R60Power *own = 0;
	if (cfg[1] & 1) { unsigned k = cfg[1] >> 1 & 7; own = r60NewPower(me, k <= 3 ? k : 0); }
	SpectreGunshipDeploymentUpdate *m = 0;
	try { m = newInstance(SpectreGunshipDeploymentUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[3] = { own ? (BehaviorModule *)own : (BehaviorModule *)m, own ? (BehaviorModule *)m : 0, 0 };
	me->m_behaviors = behaviors;
	m->onObjectCreated();
	{
		putU32(m->m_gunshipID);
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_initialTargetPosition.x)[k]));
	}
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 1 && o[1] <= 11) cfg[o[1]] = o[1] == 1 ? (o[2] & ~15u) | (cfg[1] & 15u) : o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
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
			case 3: {
				Coord3D pos;
				pos.x = r60F(o[2]); pos.y = r60F(o[3]); pos.z = 0.0f;
				unsigned k = o[1] >> 8;
				UnsignedInt opts = ((o[1] >> 1 & 1) ? COMMAND_FIRED_BY_SCRIPT : 0) | ((o[1] & 0xff) >> 2);
				res = m->initiateIntentToDoSpecialPower(r60Template(k), 0, (o[1] & 1) ? &pos : 0, 0, opts) ? 1 : 0;
				break;
			}
			case 4: res = m->doesSpecialPowerUpdatePassScienceTest() ? 1 : 0; break;
			case 5: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r60Saved.assign(bytes, bytes + blen); break;
			case 6: {
				std::vector<unsigned char> b = g_r60Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 7: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32(m->m_gunshipID);
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_initialTargetPosition.x)[k]));
		putU32((unsigned)g_r60Ships.size());
		if (!g_r60Ships.empty()) {
			Object *s = g_r60Ships.back();
			putU32(realBits(s->m_pos.x)); putU32(realBits(s->m_pos.y)); putU32(realBits(s->m_pos.z));
			putU32(realBits(s->m_orientation));
			putU32((unsigned)s->m_producerID);
			putU32(s->m_team ? (unsigned)s->m_team->m_dummy : 0);
		}
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_r28ThingPose = false;
	me->m_behaviors = 0;
	delete m;
	delete own;
	delete d;
	me->m_team = 0; me->m_player = 0;
	delete me;
	g_r60Me = 0;
	for (size_t i = 0; i < g_r60Ships.size(); ++i) { g_r60Ships[i]->m_ai = 0; g_r60Ships[i]->m_team = 0; g_r60Ships[i]->m_behaviors = 0; delete g_r60Ships[i]; }
	for (size_t i = 0; i < g_r60Ais.size(); ++i) delete g_r60Ais[i];
	for (size_t i = 0; i < g_r60Powers.size(); ++i) if (g_r60Powers[i] != own) delete g_r60Powers[i];
	for (size_t i = 0; i < g_r60PowerData.size(); ++i) delete g_r60PowerData[i];
	for (size_t i = 0; i < g_r60Behaviors.size(); ++i) delete[] g_r60Behaviors[i];
	g_r60Ships.clear(); g_r60Ais.clear(); g_r60Powers.clear(); g_r60PowerData.clear(); g_r60Behaviors.clear();
	TheGameLogic = 0;
	TheTerrainLogic = 0;
	return 0;
}

int ref_l9r60_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r60RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
