/* Differential-test driver for SpectreGunshipUpdate (p1w13-sonnet, batch R72; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipUpdate.cpp   (#included, unchanged but for the particle system
**   parser: mod_gen/SpectreGunshipUpdate_edit.cpp, build.rs)
** with its header unchanged, in wrap_module.cpp's unit, at its end (batch R50's destroyObject rename; SpecialPowerUpdateModule.cpp, its base,
** is batch R53's), on the module build's stand-ins: the gunship (id 1: a team of three, each with its player (human or computer, maybe the
** local one); maybe an AI whose orders are lines and whose current locomotor (the AI itself) may be missing, its three calls lines; maybe a
** drawable (model conditions: batch R43's lines); maybe a contain module whose addToContain is a line; maybe a REAL special power module of
** SP_0..SP_3 whose setReadyFrame and markSpecialPowerTriggered are lines (batch R60's); disabled, effectively dead, off the map or not; its
** shroud status for the local player; a position and an orientation, Thing's pose, batch R28's), four other objects (ids 2-5: a team,
** effectively dead, off the map or not, a position), the gattlings (the thing factory: GattlingA has an AI, GattlingB none; newObject makes
** ids 6, 7, ...; firing or not), Object::getSpecialPowerModule (batch R53's, verbatim), the teams' relationships (the REAL
** Object::getRelationship), the scan (the objects of the test's list whose centres are within the range in 2D, nearest first; the first
** filter, the file's own PartitionFilterLiveMapEnemies, applied for real; the call a line), the terrain's extent and ground heights (batch
** O's cells), the weapon store (WEAP_A, WEAP_B: createAndFireTempWeapon a line), the particle systems (batch M's), the decals (batch O's
** RadiusDecal stand-ins), the audio (addAudioEvent and removeAudioEvent lines; isCurrentlyPlaying the test's, a line: batch R34's),
** setDisabled, clearDisabled and destroyObject (lines), the REAL special power store, UpdateModule, INI, Xfer and the logic and client
** random values; the game logic (its frame). */
/* (the header forward-declares `enum ParticleSystemID;`, which the shim's ParticleSys.h has as a typedef: renamed by macro for the include, as
** batch R60 does; the file names the type nowhere else) */
enum R72ParticleSystemIDFwd { R72_PARTICLE_SYSTEM_ID_FWD };
#define ParticleSystemID R72ParticleSystemIDFwd
#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc: the inline one loses to other units' out-of-line ones) */
/* (BaseType.h's min and max, lines 101 and 105, which the module build's BaseType.h does not define: for the file alone) */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
#include "SpectreGunshipUpdate_edit.cpp"   /* (mod_gen: INI::parseParticleSystemTemplate is this unit's l9ahParsePsTemplate, build.rs) */
#undef max
#undef min
#undef destroyObject
#undef ParticleSystemID

void Locomotor::setAllowInvalidPosition(Bool allow)
{
	ulog("locoinvalid %d %d", (int)((const AIUpdateInterface *)(const void *)this)->m_owner->m_oid, allow ? 1 : 0);
}
/* Player.cpp's, verbatim */
Bool Player::isLocalPlayer() const { return this == ThePlayerList->getLocalPlayer(); }
extern int (*g_locoOwnerHook)(const Locomotor *);
extern const AudioEventRTS *(*g_perUnitSoundHook)(const AsciiString &);

namespace {

ModuleData *l9r72Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SpectreGunshipUpdate::friend_newModuleData(&ini);
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

/* a REAL special power module (getSpecialPowerModule finds it by its template) whose two calls are lines */
class R72Power : public SpecialPowerModule {
public:
	R72Power(Thing *t, const ModuleData *d) : SpecialPowerModule(t, d) {}
	virtual ~R72Power() {}
	virtual void setReadyFrame(UnsignedInt frame) { ulog("readyframe %d %u", (int)getObject()->m_oid, frame); }
	virtual void markSpecialPowerTriggered(const Coord3D *l)
	{
		if (l) ulog("triggered %d %u,%u,%u", (int)getObject()->m_oid, realBits(l->x), realBits(l->y), realBits(l->z));
		else ulog("triggered %d null", (int)getObject()->m_oid);
	}
};
/* the gunship's contain module: addToContain a line */
class R72Contain : public OpenContain {
public:
	R72Contain(Thing *t, const ModuleData *d) : OpenContain(t, d) {}
	virtual ~R72Contain() {}
	virtual void addToContain(Object *obj) { ulog("addtocontain %d %d", (int)getObject()->m_oid, (int)obj->m_oid); }
};

/* the weapon store's templates */
WeaponTemplate g_r72Weapons[2];
const char *const g_r72WeaponNames[2] = { "WEAP_A", "WEAP_B" };
const void *r72WeaponLookup(const char *name)
{
	for (int i = 0; i < 2; ++i) if (strcasecmp(name, g_r72WeaponNames[i]) == 0) return &g_r72Weapons[i];
	return 0;   /* "None" and any other name: NULL */
}
const char *r72WeaponName(const WeaponTemplate *t) { return t == &g_r72Weapons[0] ? "WEAP_A" : "WEAP_B"; }
int r72WeaponCode(const WeaponTemplate *t) { return t == 0 ? 0 : (t == &g_r72Weapons[0] ? 1 : 2); }
WeaponStore g_r72Store;

std::vector<unsigned char> g_r72Saved;
Object *g_r72Objs[5];
std::vector<Object *> g_r72Gattlings;
std::vector<AIUpdateInterface *> g_r72GattlingAis;
unsigned g_r72NextId = 6;
int g_r72Tmpl[2];
const char *const g_r72TmplNames[2] = { "GattlingA", "GattlingB" };
int g_r72Rel[3][3];
unsigned g_r72Scan = 0;
Region3D g_r72Extent;
AudioEventRTS g_r72Burner, g_r72Howitzer;
Real r72F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
const SpecialPowerTemplate *r72Template(unsigned k)
{
	if (k == 0 || k > 4) return 0;
	char name[8];
	sprintf(name, "SP_%u", k - 1);
	return (const SpecialPowerTemplate *)ref_sp_find_raw(name);
}
Object *r72Find(ObjectID id)
{
	if ((int)id >= 1 && (int)id <= 5) return g_r72Objs[(int)id - 1];
	for (size_t i = 0; i < g_r72Gattlings.size(); ++i) if (g_r72Gattlings[i]->m_oid == id) return g_r72Gattlings[i];
	return 0;
}
void r72Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
int r72Code(const ThingTemplate *t) { for (int i = 0; i < 2; ++i) if (t == (const ThingTemplate *)&g_r72Tmpl[i]) return i + 1; return 0; }
const ThingTemplate *r72FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 2; ++i) if (strcmp(name.str(), g_r72TmplNames[i]) == 0) return (const ThingTemplate *)&g_r72Tmpl[i];
	return 0;
}
Object *r72NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r72NextId++;
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	static BehaviorModule *none[1] = { 0 };
	o->m_behaviors = none;
	if (r72Code(tmplate) == 1) {
		AIUpdateInterface *ai = new AIUpdateInterface;
		ai->m_owner = o;
		o->m_ai = ai;
		g_r72GattlingAis.push_back(ai);
	}
	g_r72Gattlings.push_back(o);
	return o;
}
Relationship r72TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r72Rel[from->m_dummy - 1][to->m_dummy - 1];
}
/* the scan: the test's list (nibbles of object numbers 2-5, 0 ends) within the range of the position (centres, 2D), nearest first (ties in
** list order), then the first filter (the file's PartitionFilterLiveMapEnemies) for real; the stealth, attack and fog filters are the test's
** list. A line: the position, the range, the number of filters, the fog filter's player and the distance calculation. */
void r72Scan(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0, fog = -1;
	while (filters && filters[n]) {
		if (PartitionFilterFreeOfFog *f = dynamic_cast<PartitionFilterFreeOfFog *>(filters[n])) fog = f->m_comparisonIndex;
		++n;
	}
	ulog("scan %08x %08x %08x %08x %d %d %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n, fog, g_ahRangeDc);
	std::vector<std::pair<Real, Object *> > found;
	for (int k = 0; k < 4; ++k) {
		unsigned id = g_r72Scan >> (4 * k) & 15;
		if (id < 2 || id > 5) break;
		Object *o = g_r72Objs[id - 1];
		Real dx = o->m_pos.x - pos->x, dy = o->m_pos.y - pos->y;
		Real dsq = dx * dx + dy * dy;
		if (dsq <= maxDist * maxDist) found.push_back(std::make_pair(dsq, o));
	}
	std::stable_sort(found.begin(), found.end(), [](const std::pair<Real, Object *> &a, const std::pair<Real, Object *> &b) { return a.first < b.first; });
	g_ahRange.clear();
	g_ahRangeNums.clear();
	for (size_t i = 0; i < found.size(); ++i) if (filters && filters[0] && filters[0]->allow(found[i].second)) g_ahRange.push_back(found[i].second);
}
int r72LocoOwner(const Locomotor *l) { return (int)((const AIUpdateInterface *)(const void *)l)->m_owner->m_oid; }
const AudioEventRTS *r72Sound(const AsciiString &name)
{
	if (name == "Afterburner") return &g_r72Burner;
	if (name == "HowitzerFire") return &g_r72Howitzer;
	return 0;
}

}  // namespace

void TerrainLogic::getExtentIncludingBorder( Region3D *extent ) const
{
	extent->lo.x = g_r72Extent.lo.x; extent->lo.y = g_r72Extent.lo.y;
	extent->hi.x = g_r72Extent.hi.x; extent->hi.y = g_r72Extent.hi.y;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a SpectreGunshipUpdate and make the module on the gunship (id 1) at frame cfg[0], then
** onObjectCreated. `cfg` (32 words; 4-25 also settable by op 2): 1 the logic random seed, 2 the client's; 3 the gunship (read once): bit 0
** an AI, 1 its current locomotor, 2 a drawable, 3 a contain module, 4-6 its special power module (0 none, else SP_(k-1)); 4 the gunship:
** bits 0-1 its team (0 none, else 1-3), 2 disabled, 3 effectively dead, 4 off the map, 5-7 its shroud status for the local player, 8 the
** gattlings are firing, 9 the audio is playing; 5-7 its position, 8 its orientation (float bits); 9 objects 2-5, a byte each: bits 0-1 the
** team, 2 effectively dead, 3 off the map; 10-17 their x and y (float bits; z 0); 18 the scan's list (nibbles of object numbers, 0 ends);
** 19 bits 0-2 player p is human, 4-5 the local player (3 none); 20 the teams' relationships (2 bits each, from-major); 21-24 the extent's
** lo.x, lo.y, hi.x, hi.y (float bits); 25 the eight cells' ground heights (nibbles, halves). The data dump goes first (the template's
** id, the weapon's code, the gattling's name, the three frames, the six reals, the particle system's name, the two decal templates), then
** the disabled types the module processes and its state. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] =
** b  3 initiateIntentToDoSpecialPower(SP_(a >> 8 - 1), else NULL; NULL; a bit 0 ? (b, c, d) : NULL; NULL; (a bit 1 ?
** COMMAND_FIRED_BY_SCRIPT : 0) | (a & 0xff) >> 2)  4 setSpecialPowerOverridableDestination((a, b, c))  5 xfer save  6 xfer load of the
** last save  7 xfer CRC  8 the queries. Each op appends: the CATCH_ALL status, the result, the effect log (but the orientation lines), the
** xfer status, bytes and CRC, the state, the number of gattlings and the last one's team. */
static int l9r72RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r72Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	TheParticleSystemManager = &g_ahPsm;
	ref_ini_sp_bind(ref_sp_find_raw);   /* (INI::parseSpecialPowerTemplate reads the real store) */
	ref_weapon_lookup_hook = r72WeaponLookup;
	try { d = l9r72Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_weapon_lookup_hook = 0;
	ref_ini_sp_bind(0);
	if (status) { TheParticleSystemManager = 0; return status; }
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const SpectreGunshipUpdateModuleData *sd = (const SpectreGunshipUpdateModuleData *)d;
		putU32(ref_sp_raw_id(sd->m_specialPowerTemplate));
		putU32((unsigned)r72WeaponCode(sd->m_howitzerWeaponTemplate));
		putStr(sd->m_gattlingTemplateName.str(), (int)sd->m_gattlingTemplateName.getLength());
		putU32(sd->m_orbitFrames);
		putU32(sd->m_howitzerFiringRate);
		putU32(sd->m_howitzerFollowLag);
		const Real reals[6] = { sd->m_attackAreaRadius, sd->m_targetingReticleRadius, sd->m_gunshipOrbitRadius, sd->m_strafingIncrement,
			sd->m_orbitInsertionSlope, sd->m_randomOffsetForHowitzer };
		for (int i = 0; i < 6; ++i) putU32(realBits(reals[i]));
		std::string ps = sd->m_gattlingStrafeFXParticleSystem ? g_ahPsNames[(size_t)sd->m_gattlingStrafeFXParticleSystem->m_idx] : std::string();
		putStr(ps.c_str(), (int)ps.size());
		const RadiusDecalTemplate *ts[2] = { &sd->m_attackAreaDecalTemplate, &sd->m_targetingReticleDecalTemplate };
		for (int i = 0; i < 2; ++i) {
			const RadiusDecalTemplate &t = *ts[i];
			putStr(t.m_name.str(), t.m_name.getLength());
			putU32((unsigned)t.m_shadowType);
			putU32(realBits(t.m_minOpacity));
			putU32(realBits(t.m_maxOpacity));
			putU32(t.m_opacityThrobTime);
			putU32((unsigned)t.m_color);
			putU32(t.m_onlyVisibleToOwningPlayer ? 1 : 0);
		}
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[2]);
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	TheTerrainLogic = &g_ocTerrain;
	ThePlayerList = &g_spList;   /* (batch H's: the local player is g_spLocal) */
	TheWeaponStore = &g_r72Store;
	g_tempWeaponName = r72WeaponName;
	AbAudio audio;   /* (batch P's: addAudioEvent is die batch D's line) */
	TheAudio = &audio;
	g_audioHandleLast = 0;
	g_ahPsLastId = 0;
	g_r72NextId = 6;
	g_r28ThingPose = true;   /* (setOrientation keeps the angle, batch R28's way) */
	g_objXformSyncPos = true;
	g_r72Burner.setEventName(AsciiString("R72Burner"));
	g_r72Howitzer.setEventName(AsciiString("R72Howitzer"));
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
	}
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 5; ++k) {
		g_r72Objs[k] = newObject();
		g_r72Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r72Objs[k]->m_behaviors = none;
	}
	Object *me = g_r72Objs[0];
	me->m_tmpl.m_voiceAttack.setEventName(AsciiString("R72Voice"));
	AIUpdateInterface ai;
	ai.m_owner = me;
	ai.m_hasLoco = (cfg[3] >> 1 & 1) != 0;
	me->m_ai = (cfg[3] & 1) ? &ai : 0;
	Drawable draw;
	draw.m_ownerObject = me;
	me->m_drawable = (cfg[3] >> 2 & 1) ? &draw : 0;
	OpenContainModuleData cd;
	R72Contain *contain = (cfg[3] >> 3 & 1) ? new R72Contain(me, &cd) : 0;
	me->m_contain = contain;
	auto scene = [&]() {
		unsigned f = cfg[4];
		unsigned t = f & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		me->m_disabled = (f >> 2 & 1) != 0;
		me->m_effDead = (f >> 3 & 1) != 0;
		me->m_offMap = (f >> 4 & 1) != 0;
		me->m_shroudR27 = (Int)(f >> 5 & 7);
		for (size_t i = 0; i < g_r72Gattlings.size(); ++i) {
			g_r72Gattlings[i]->m_status.clear();
			if (f >> 8 & 1) g_r72Gattlings[i]->m_status.set(OBJECT_STATUS_IS_FIRING_WEAPON);
		}
		g_r34Playing = (f >> 9 & 1) != 0;
		me->m_pos.x = r72F(cfg[5]); me->m_pos.y = r72F(cfg[6]); me->m_pos.z = r72F(cfg[7]);
		me->setOrientation(r72F(cfg[8]));
		me->poseMatrix();
		for (int k = 1; k < 5; ++k) {
			Object *o = g_r72Objs[k];
			unsigned b = cfg[9] >> (8 * (k - 1)) & 255;
			unsigned ot = b & 3;
			o->m_team = ot ? &teams[ot - 1] : 0;
			o->m_player = ot ? &players[ot - 1] : 0;
			o->m_effDead = (b >> 2 & 1) != 0;
			o->m_offMap = (b >> 3 & 1) != 0;
			o->m_pos.x = r72F(cfg[10 + 2 * (k - 1)]); o->m_pos.y = r72F(cfg[11 + 2 * (k - 1)]); o->m_pos.z = 0.0f;
		}
		g_r72Scan = cfg[18];
		for (int p = 0; p < 3; ++p) players[p].m_type = (cfg[19] >> p & 1) ? PLAYER_HUMAN : PLAYER_COMPUTER;
		unsigned local = cfg[19] >> 4 & 3;
		g_spLocal = local < 3 ? &players[local] : 0;
		for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r72Rel[a][b] = (int)(cfg[20] >> (2 * (3 * a + b)) & 3);
		g_r72Extent.lo.x = r72F(cfg[21]); g_r72Extent.lo.y = r72F(cfg[22]); g_r72Extent.hi.x = r72F(cfg[23]); g_r72Extent.hi.y = r72F(cfg[24]);
		for (int k = 0; k < 8; ++k) g_hdGround[k] = (Real)(cfg[25] >> (4 * k) & 15) * 0.5f;
	};
	scene();
	g_modFindHook = r72Find;
	g_modDestroyHook = r72Destroy;
	g_findTemplateHook = r72FindTemplate;
	g_newObjectHook = r72NewObject;
	g_teamRelHook = r72TeamRel;
	g_ahRangeHook = r72Scan;
	g_locoOwnerHook = r72LocoOwner;
	g_perUnitSoundHook = r72Sound;
	g_mcHook = r43Mc;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 10, "setorient ") == 0 || l.compare(0, 13, "settransform ") == 0) continue;
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	R72Power *power = 0;
	SpecialPowerModuleData pd;
	if (r72Template(cfg[3] >> 4 & 7)) {
		pd.m_specialPowerTemplate = r72Template(cfg[3] >> 4 & 7);
		power = new R72Power(me, &pd);
	}
	SpectreGunshipUpdate *m = 0;
	try { m = newInstance(SpectreGunshipUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	g_dsFirst = &m->m_attackAreaDecal;   /* (the attack area is decal 0, the reticle decal 1) */
	BehaviorModule *behaviors[3] = { power ? (BehaviorModule *)power : (BehaviorModule *)m, power ? (BehaviorModule *)m : 0, 0 };
	me->m_behaviors = behaviors;
	g_events.clear();
	g_ulog.clear();
	m->onObjectCreated();
	auto putState = [&]() {
		putU32((unsigned)m->m_status);
		const Coord3D *ps[5] = { &m->m_initialTargetPosition, &m->m_overrideTargetDestination, &m->m_satellitePosition, &m->m_gattlingTargetPosition,
			&m->m_positionToShootAt };
		for (int i = 0; i < 5; ++i) for (int k = 0; k < 3; ++k) putU32(realBits((&ps[i]->x)[k]));
		putU32(m->m_okToFireHowitzerCounter);
		putU32(m->m_orbitEscapeFrame);
		putU32((unsigned)m->m_gattlingID);
		putU32(m->m_attackAreaDecal.isEmpty() ? 1 : 0);
		putU32(m->m_targetingReticleDecal.isEmpty() ? 1 : 0);
		putU32(m->m_afterburnerSound.getPlayingHandle());
	};
	{
		DisabledMaskType mask = m->getDisabledTypesToProcess();
		unsigned bits = 0;
		for (int t = 0; t < DISABLED_COUNT; ++t) if (mask.test((DisabledType)t)) bits |= 1u << t;
		putU32(bits);
	}
	putLog();
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 4 && o[1] <= 25) cfg[o[1]] = o[2];
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
				pos.x = r72F(o[2]); pos.y = r72F(o[3]); pos.z = r72F(o[4]);
				UnsignedInt opts = ((o[1] >> 1 & 1) ? COMMAND_FIRED_BY_SCRIPT : 0) | ((o[1] & 0xff) >> 2);
				res = m->initiateIntentToDoSpecialPower(r72Template(o[1] >> 8), 0, (o[1] & 1) ? &pos : 0, 0, opts) ? 1 : 0;
				break;
			}
			case 4: {
				Coord3D pos;
				pos.x = r72F(o[1]); pos.y = r72F(o[2]); pos.z = r72F(o[3]);
				m->setSpecialPowerOverridableDestination(&pos);
				break;
			}
			case 5: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r72Saved.assign(bytes, bytes + blen); break;
			case 6: {
				std::vector<unsigned char> b = g_r72Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 7: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 8:
				res = (m->doesSpecialPowerHaveOverridableDestinationActive() ? 1u : 0u) | (m->isActive() ? 2u : 0u) | (m->isPowerCurrentlyInUse() ? 4u : 0u)
					| (m->doesSpecialPowerHaveOverridableDestination() ? 8u : 0u) | ((unsigned)m->getCommandOption() << 4);
				break;
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
		putState();
		putU32((unsigned)g_r72Gattlings.size());
		putU32(g_r72Gattlings.empty() || !g_r72Gattlings.back()->m_team ? 0u : (unsigned)g_r72Gattlings.back()->m_team->m_dummy);
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_teamRelHook = 0;
	g_ahRangeHook = 0;
	g_locoOwnerHook = 0;
	g_perUnitSoundHook = 0;
	g_mcHook = 0;
	g_dsFirst = 0;
	g_spLocal = 0;
	g_r34Playing = false;
	g_r28ThingPose = false;
	g_objXformSyncPos = false;
	g_tempWeaponName = 0;
	g_ahRange.clear();
	me->m_behaviors = 0;
	me->m_ai = 0;
	me->m_drawable = 0;
	me->m_contain = 0;
	delete m;
	delete power;
	delete contain;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r72Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0;
		delete x;
		g_r72Objs[k] = 0;
	}
	for (size_t i = 0; i < g_r72Gattlings.size(); ++i) { g_r72Gattlings[i]->m_ai = 0; g_r72Gattlings[i]->m_team = 0; g_r72Gattlings[i]->m_player = 0; g_r72Gattlings[i]->m_behaviors = 0; delete g_r72Gattlings[i]; }
	for (size_t i = 0; i < g_r72GattlingAis.size(); ++i) delete g_r72GattlingAis[i];
	g_r72Gattlings.clear();
	g_r72GattlingAis.clear();
	ThePlayerList = 0;
	TheAudio = 0;
	TheWeaponStore = 0;
	TheTerrainLogic = 0;
	ThePartitionManager = 0;
	TheParticleSystemManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r72_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r72RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
