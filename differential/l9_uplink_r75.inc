/* Differential-test driver for ParticleUplinkCannonUpdate (p1w13-sonnet, batch R75; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ParticleUplinkCannonUpdate.cpp   (#included, unchanged)
** with its header (less one line, below), in wrap_module.cpp's unit, at its end (batch R50's destroyObject rename; SpecialPowerUpdateModule.cpp,
** its base, is batch R53's), on the module build's stand-ins: the cannon (id 1: a team of three, each with its player, maybe the local one;
** maybe a drawable, whose model conditions are batch R43's lines and whose current client bones are the art's; maybe a REAL special power
** module of SP_0..SP_3 whose isReady and getReadyFrame are the test's and whose setReadyFrame and markSpecialPowerTriggered are lines (batch
** R72's); sold, under construction, effectively dead or not; underpowered, EMP'd, subdued or hacked; its shroud status for the local player; a
** position and an orientation, Thing's pose, batch R28's), the art's bones (Node01..Node06, Connector, FireBone: a table, a bone's pose turned
** about z; Object::getMultiLogicalBonePosition through batch R's hook; Object::convertBonePosToWorldPos batch R8's), four other objects (ids
** 2-5: effectively dead or not, maybe a body (batch M's MBody, attemptDamage a line), a position), the scan (the test's list, the filters
** applied for real; a line), the waypoints (four nodes, ids 11-14, the test's links), the thing factory (LaserA, whose drawables have a
** LaserUpdate, LaserB, without, Remnant; newObject makes ids 6, 7, ...; newDrawable batch R12's, ids from 1000), the lasers' LaserUpdate
** (initLaser and setDecayFrames lines, getCurrentLaserRadius the test's), the particle systems (batch M's; setLocalTransform batch R70's line),
** the FX lists (die batch J's doFXPos), the scorches (batch R32's line), the shroud reveals (lines), the audio (addAudioEvent and
** removeAudioEvent lines), the REAL special power store, UpdateModule, INI, Xfer and the logic and client random values; the game logic (its
** frame). */
/* (the header forward-declares `enum ParticleSystemID;`, which the shim's ParticleSys.h has as a typedef, and the file uses the type: the
** header without that line, mod_gen/ParticleUplinkCannonUpdate_r75.h (build.rs), comes first, and the file's own include stops at its guard) */
#include "ParticleUplinkCannonUpdate_r75.h"
#define destroyObject r50DestroyObject
/* (BaseType.h's min and max, lines 101 and 105, the compiler's __min and __max, and BaseType.h's TWO_PI, line 87, which the module build's
** BaseType.h does not define: for the file alone) */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
#define __min(x,y) (((x)<(y)) ? (x) : (y))
#define __max(x,y) (((x)>(y)) ? (x) : (y))
#define TWO_PI 6.28318530718f
/* (Damage.h's TheDeathNames, lines 201-228, verbatim: the unit has included Damage.h before the file defines DEFINE_DEATH_NAMES) */
static const char *TheDeathNames[] =
{
	"NORMAL",
	"NONE",
	"CRUSHED",
	"BURNED",
	"EXPLODED",
	"POISONED",
	"TOPPLED",
	"FLOODED",
	"SUICIDED",
	"LASERED",
	"DETONATED",
	"SPLATTED",
	"POISONED_BETA",
	"EXTRA_2",
	"EXTRA_3",
	"EXTRA_4",
	"EXTRA_5",
	"EXTRA_6",
	"EXTRA_7",
	"EXTRA_8",
	"POISONED_GAMMA",

	NULL
};
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ParticleUplinkCannonUpdate.cpp"
#undef TWO_PI
#undef __max
#undef __min
#undef max
#undef min
#undef destroyObject

/* PartitionManager.cpp 5294-5297, verbatim */
Bool PartitionFilterAlive::allow( Object *objOther )
{
	return !objOther->isEffectivelyDead();
}
Real g_r75LaserRadius = 0.0f;
void LaserUpdate::setDecayFrames( UnsignedInt decayFrames ) { ulog("laserdecay %d %u", (int)m_r75Drawable, decayFrames); }
Real LaserUpdate::getCurrentLaserRadius() const { return g_r75LaserRadius; }
void PartitionManager::doShroudReveal( Real centerX, Real centerY, Real radius, PlayerMaskType playerMask )
{
	ulog("reveal %08x %08x %08x %u", realBits(centerX), realBits(centerY), realBits(radius), (unsigned)playerMask);
}
void PartitionManager::undoShroudReveal( Real centerX, Real centerY, Real radius, PlayerMaskType playerMask )
{
	ulog("unreveal %08x %08x %08x %u", realBits(centerX), realBits(centerY), realBits(radius), (unsigned)playerMask);
}
/* the waypoints: four nodes, ids 11-14, at the test's places, linked as the test says */
Waypoint g_r75Ways[4];
Waypoint *(*g_waypointByIdHook)(UnsignedInt id) = 0;   /* batch R80: the railroad test's waypoints */
Waypoint *TerrainLogic::getWaypointByID( UnsignedInt id )
{
	if (g_waypointByIdHook) return g_waypointByIdHook(id);
	for (int k = 0; k < 4; ++k) if ((UnsignedInt)g_r75Ways[k].m_id == id) return &g_r75Ways[k];
	return NULL;
}
/* the art's bones: bit k of the test's word, k < 6 Node01..Node06, 6 Connector, 7 FireBone; bone k at (30 + 9k, -20 + 6k, 12 + 0.5k),
** turned 0.4k - 1.2 about z */
static const char *const kR75Bones[8] = { "Node01", "Node02", "Node03", "Node04", "Node05", "Node06", "Connector", "FireBone" };
unsigned g_r75BoneBits = 0;
Object *g_r75Me = 0;
static int r75BoneIndex(const char *name)
{
	for (int k = 0; k < 8; ++k) if ((g_r75BoneBits >> k & 1) && strcmp(name, kR75Bones[k]) == 0) return k;
	return -1;
}
static void r75BonePose(int k, Coord3D *pos, Matrix3D *m)
{
	pos->x = 30.0f + 9.0f * (Real)k; pos->y = -20.0f + 6.0f * (Real)k; pos->z = 12.0f + 0.5f * (Real)k;
	if (m) { m->Make_Identity(); m->Rotate_Z(0.4f * (Real)k - 1.2f); m->Set_Translation(Vector3(pos->x, pos->y, pos->z)); }
}
/* the drawable's current client bones: the art's table (startIndex 0: the name itself) */
Int Drawable::getCurrentClientBonePositions(const char* boneNamePrefix, Int startIndex, Coord3D* positions, Matrix3D* transforms, Int maxBones) const
{
	if (startIndex != 0 || maxBones < 1) return 0;
	int k = r75BoneIndex(boneNamePrefix);
	if (k < 0) return 0;
	r75BonePose(k, positions, transforms);
	return 1;
}
/* the laser drawables: LaserA's have a LaserUpdate */
std::vector<LaserUpdate *> g_r75Lasers;

namespace {

ModuleData *l9r75Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ParticleUplinkCannonUpdate::friend_newModuleData(&ini);
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

/* a REAL special power module (getSpecialPowerModule finds it by its template): isReady and getReadyFrame the test's, two calls lines */
class R75Power : public SpecialPowerModule {
public:
	R75Power(Thing *t, const ModuleData *d) : SpecialPowerModule(t, d) {}
	virtual ~R75Power() {}
	virtual Bool isReady() const { return m_r75Ready; }
	virtual UnsignedInt getReadyFrame() const { return m_r75ReadyFrame; }
	virtual void setReadyFrame(UnsignedInt frame) { ulog("readyframe %d %u", (int)getObject()->m_oid, frame); }
	virtual void markSpecialPowerTriggered(const Coord3D *l)
	{
		if (l) ulog("triggered %d %u,%u,%u", (int)getObject()->m_oid, realBits(l->x), realBits(l->y), realBits(l->z));
		else ulog("triggered %d null", (int)getObject()->m_oid);
	}
	Bool m_r75Ready = false;
	UnsignedInt m_r75ReadyFrame = 0;
};

std::vector<unsigned char> g_r75Saved;
Object *g_r75Objs[5];
MBody g_r75Bodies[5];
std::vector<Object *> g_r75Made;
unsigned g_r75NextId = 6;
int g_r75Tmpl[3];
const char *const g_r75TmplNames[3] = { "LaserA", "LaserB", "Remnant" };
unsigned g_r75Scan = 0;
Real r75F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
const SpecialPowerTemplate *r75Template(unsigned k)
{
	if (k == 0 || k > 4) return 0;
	char name[8];
	sprintf(name, "SP_%u", k - 1);
	return (const SpecialPowerTemplate *)ref_sp_find_raw(name);
}
Object *r75Find(ObjectID id)
{
	if ((int)id >= 1 && (int)id <= 5) return g_r75Objs[(int)id - 1];
	for (size_t i = 0; i < g_r75Made.size(); ++i) if (g_r75Made[i]->m_oid == id) return g_r75Made[i];
	return 0;
}
int r75Code(const ThingTemplate *t) { for (int i = 0; i < 3; ++i) if (t == (const ThingTemplate *)&g_r75Tmpl[i]) return i + 1; return 0; }
const ThingTemplate *r75FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 3; ++i) if (strcmp(name.str(), g_r75TmplNames[i]) == 0) return (const ThingTemplate *)&g_r75Tmpl[i];
	return 0;
}
Object *r75NewObject(const ThingTemplate *tmplate, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)g_r75NextId++;
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	static BehaviorModule *none[1] = { 0 };
	o->m_behaviors = none;
	g_r75Made.push_back(o);
	return o;
}
void r75NewDraw(Drawable *d, const ThingTemplate *t)
{
	if (r75Code(t) != 1) return;
	LaserUpdate *l = new LaserUpdate;
	l->m_r75Drawable = (Int)d->m_effectId;
	d->m_r59Laser = (ClientUpdateModule *)(void *)l;
	g_r75Lasers.push_back(l);
}
Int r75MultiBone(const Object *obj, const char *prefix, Int maxBones, Coord3D *positions, Matrix3D *transforms, Bool convertToWorld)
{
	/* Object.cpp's getMultiLogicalBonePosition: the drawable's pristine bones prefix01, prefix02, ... while the art has them */
	if (obj != g_r75Me || !obj->m_drawable) return 0;
	Int count = 0;
	for (Int i = 1; i <= 99 && count < maxBones; ++i) {
		char name[256];
		snprintf(name, sizeof(name), "%s%02d", prefix, (int)i);
		int k = r75BoneIndex(name);
		if (k < 0) break;
		r75BonePose(k, &positions[count], transforms ? &transforms[count] : 0);
		++count;
	}
	if (count > 0 && convertToWorld) ulog("unexpected convertToWorld");
	return count;
}
/* the scan: the test's list (nibbles of object numbers 2-5, 0 ends), its filters applied for real; a line: the position, the range, the
** number of filters and the distance calculation */
void r75Scan(const Coord3D *pos, Real maxDist, PartitionFilter **filters)
{
	int n = 0;
	while (filters && filters[n]) ++n;
	ulog("scan %08x %08x %08x %08x %d %d", realBits(pos->x), realBits(pos->y), realBits(pos->z), realBits(maxDist), n, g_ahRangeDc);
	g_ahRange.clear();
	g_ahRangeNums.clear();
	for (int k = 0; k < 4; ++k) {
		unsigned id = g_r75Scan >> (4 * k) & 15;
		if (id < 2 || id > 5) break;
		Object *o = g_r75Objs[id - 1];
		bool ok = true;
		for (int f = 0; f < n; ++f) if (!filters[f]->allow(o)) ok = false;
		if (ok) g_ahRange.push_back(o);
	}
}

}  // namespace

extern void (*g_r75NewDrawHook)(Drawable *, const ThingTemplate *);
extern Int (*g_multiBoneHook)(const Object *obj, const char *boneNamePrefix, Int maxBones, Coord3D *positions, Matrix3D *transforms, Bool convertToWorld);
extern "C" const char *ref_armor_fx_name(const void *fx);
void ctClearEffects();

extern "C" {

/* Read `filename` (block `D`) as the data of a ParticleUplinkCannonUpdate and make the module on the cannon (id 1) at frame cfg[0], then
** onObjectCreated. `cfg` (32 words; 5-22 also settable by op 2): 1 the logic random seed, 2 the client's; 3 the cannon (read once): bit 0 a
** drawable, 1-3 its special power module (0 none, else SP_(k-1)); 4 the waypoints (read once): node k's links, bits 4k..4k+3 (bit j: to node
** j); 5 the cannon: bits 0-1 its team (0 none, else 1-3), 2 sold, 3 under construction, 4 effectively dead, 5-8 underpowered, EMP'd, subdued,
** hacked, 9-11 its shroud status for the local player, 12-13 the local player (3 none), 14 its power isReady; 6 the power's ready frame; 7-9
** the cannon's x, y and orientation (float bits); 10 the art's bones (see r75BonePose); 11 objects 2-5, a byte each: bit 0 effectively dead, 1
** a body; 12-19 their x and y (float bits; z 0); 20 the scan's list (nibbles of object numbers, 0 ends); 21 the lasers' radius (float bits);
** 22 the eight cells' ground heights (nibbles, halves). The data dump goes first, then the disabled types the module processes, the effect log
** of onObjectCreated, and the state. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b
** 3 initiateIntentToDoSpecialPower(SP_(a >> 8 - 1) or NULL; the object a >> 5 & 7 (2-5, else NULL); a bit 0 ? (b, c, d) : NULL; node
** (a >> 2 & 7) - 1 (else NULL); a bit 1 ? COMMAND_FIRED_BY_SCRIPT : 0)  4 setSpecialPowerOverridableDestination((a, b, c))  5 xfer save
** 6 xfer load of the last save  7 xfer CRC  8 the queries  9 onObjectCreated. Each op appends: the CATCH_ALL status, the result, the effect
** log (but the orientation lines), the xfer status, bytes and CRC, the state, the number of objects made and the last one's position. */
static int l9r75RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r75Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	TheParticleSystemManager = &g_ahPsm;
	ref_ini_sp_bind(ref_sp_find_raw);   /* (INI::parseSpecialPowerTemplate reads the real store) */
	try { d = l9r75Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_ini_sp_bind(0);
	if (status) { TheParticleSystemManager = 0; return status; }
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const ParticleUplinkCannonUpdateModuleData *pd = (const ParticleUplinkCannonUpdateModuleData *)d;
		putU32(ref_sp_raw_id(pd->m_specialPowerTemplate));
		const UnsignedInt frames[11] = { pd->m_beginChargeFrames, pd->m_raiseAntennaFrames, pd->m_readyDelayFrames, pd->m_widthGrowFrames,
			pd->m_beamTravelFrames, pd->m_totalFiringFrames, pd->m_framesBetweenLaunchFXRefresh, pd->m_outerEffectNumBones, pd->m_totalScorchMarks,
			pd->m_totalDamagePulses, pd->m_doubleClickToFastDriveDelay };
		for (int i = 0; i < 11; ++i) putU32(frames[i]);
		const Real reals[9] = { pd->m_swathOfDeathDistance, pd->m_swathOfDeathAmplitude, pd->m_scorchMarkScalar, pd->m_damagePerSecond,
			pd->m_damageRadiusScalar, pd->m_revealRange, pd->m_manualDrivingSpeed, pd->m_manualFastDrivingSpeed, 0.0f };
		for (int i = 0; i < 8; ++i) putU32(realBits(reals[i]));
		putU32((unsigned)pd->m_damageType);
		putU32((unsigned)pd->m_deathType);
		const AsciiString *names[21] = { &pd->m_outerEffectBaseBoneName, &pd->m_outerNodesLightFlareParticleSystemName,
			&pd->m_outerNodesMediumFlareParticleSystemName, &pd->m_outerNodesIntenseFlareParticleSystemName, &pd->m_connectorBoneName,
			&pd->m_connectorMediumLaserNameName, &pd->m_connectorIntenseLaserNameName, &pd->m_connectorMediumFlareParticleSystemName,
			&pd->m_connectorIntenseFlareParticleSystemName, &pd->m_laserBaseLightFlareParticleSystemName, &pd->m_laserBaseMediumFlareParticleSystemName,
			&pd->m_laserBaseIntenseFlareParticleSystemName, &pd->m_fireBoneName, &pd->m_particleBeamLaserName, &pd->m_powerupSoundName,
			&pd->m_unpackToReadySoundName, &pd->m_firingToIdleSoundName, &pd->m_annihilationSoundName, &pd->m_damagePulseRemnantObjectName, 0, 0 };
		for (int i = 0; i < 19; ++i) putStr(names[i]->str(), names[i]->getLength());
		const char *fx[2] = { pd->m_groundHitFX ? ref_armor_fx_name(pd->m_groundHitFX) : "", pd->m_beamLaunchFX ? ref_armor_fx_name(pd->m_beamLaunchFX) : "" };
		for (int i = 0; i < 2; ++i) putStr(fx[i], (int)strlen(fx[i]));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[2]);
	InitGameLogicRandom(cfg[1]);
	ThePartitionManager = &g_ahPm;
	TheTerrainLogic = &g_ocTerrain;
	ThePlayerList = &g_spList;   /* (batch H's: the local player is g_spLocal) */
	AbAudio audio;   /* (batch P's: addAudioEvent is die batch D's line) */
	TheAudio = &audio;
	g_audioHandleLast = 0;
	g_ahPsLastId = 0;
	g_r75NextId = 6;
	ctClearEffects();
	g_r28ThingPose = true;   /* (setOrientation keeps the angle, batch R28's way) */
	g_objXformSyncPos = true;
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
	}
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 5; ++k) {
		g_r75Objs[k] = newObject();
		g_r75Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r75Objs[k]->m_behaviors = none;
		g_r75Bodies[k].m_owner = g_r75Objs[k];
		g_r75Bodies[k].m_health = g_r75Bodies[k].m_max = 100.0f;
	}
	Object *me = g_r75Objs[0];
	g_r75Me = me;
	Drawable draw;
	draw.m_ownerObject = me;
	me->m_drawable = (cfg[3] & 1) ? &draw : 0;
	for (int k = 0; k < 4; ++k) {
		Waypoint &w = g_r75Ways[k];
		w.m_id = 11 + k;
		w.m_location.x = -50.0f + 40.0f * (Real)k; w.m_location.y = 25.0f * (Real)(k & 1) - 10.0f; w.m_location.z = 0.0f;
		w.m_numLinks = 0;
		for (int j = 0; j <= Waypoint::MAX_LINKS; ++j) w.m_links[j] = 0;
		/* (TerrainLogic::addWaypointLink links no waypoint to itself) */
		for (int j = 0; j < 4; ++j) if (j != k && (cfg[4] >> (4 * k + j) & 1)) w.m_links[w.m_numLinks++] = &g_r75Ways[j];
	}
	R75Power *power = 0;
	SpecialPowerModuleData spd;
	if (r75Template(cfg[3] >> 1 & 7)) {
		spd.m_specialPowerTemplate = r75Template(cfg[3] >> 1 & 7);
		power = new R75Power(me, &spd);
	}
	auto scene = [&]() {
		unsigned f = cfg[5];
		unsigned t = f & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		me->m_status.clear();
		if (f >> 2 & 1) me->m_status.set(OBJECT_STATUS_SOLD);
		if (f >> 3 & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		me->m_effDead = (f >> 4 & 1) != 0;
		me->m_disabledBits = 0;
		const DisabledType kinds[4] = { DISABLED_UNDERPOWERED, DISABLED_EMP, DISABLED_SUBDUED, DISABLED_HACKED };
		for (int i = 0; i < 4; ++i) if (f >> (5 + i) & 1) me->m_disabledBits |= 1u << kinds[i];
		me->m_disabled = me->m_disabledBits != 0;
		me->m_shroudR27 = (Int)(f >> 9 & 7);
		unsigned local = f >> 12 & 3;
		g_spLocal = local < 3 ? &players[local] : 0;
		if (power) {
			power->m_r75Ready = (f >> 14 & 1) != 0;
			power->m_r75ReadyFrame = cfg[6];
		}
		me->m_pos.x = r75F(cfg[7]); me->m_pos.y = r75F(cfg[8]); me->m_pos.z = 0.0f;
		me->setOrientation(r75F(cfg[9]));
		me->poseMatrix();
		g_r75BoneBits = cfg[10];
		for (int k = 1; k < 5; ++k) {
			Object *o = g_r75Objs[k];
			unsigned b = cfg[11] >> (8 * (k - 1)) & 255;
			o->m_effDead = (b & 1) != 0;
			o->m_body = (b >> 1 & 1) ? &g_r75Bodies[k] : 0;
			o->m_pos.x = r75F(cfg[12 + 2 * (k - 1)]); o->m_pos.y = r75F(cfg[13 + 2 * (k - 1)]); o->m_pos.z = 0.0f;
		}
		g_r75Scan = cfg[20];
		g_r75LaserRadius = r75F(cfg[21]);
		for (int k = 0; k < 8; ++k) g_hdGround[k] = (Real)(cfg[22] >> (4 * k) & 15) * 0.5f;
	};
	scene();
	g_modFindHook = r75Find;
	g_findTemplateHook = r75FindTemplate;
	g_newObjectHook = r75NewObject;
	g_ahRangeHook = r75Scan;
	g_mcHook = r43Mc;
	g_r75NewDrawHook = r75NewDraw;
	g_multiBoneHook = r75MultiBone;
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
	ParticleUplinkCannonUpdate *m = 0;
	try { m = newInstance(ParticleUplinkCannonUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[3] = { power ? (BehaviorModule *)power : (BehaviorModule *)m, power ? (BehaviorModule *)m : 0, 0 };
	me->m_behaviors = behaviors;
	auto putState = [&]() {
		putU32((unsigned)m->m_status);
		putU32((unsigned)m->m_laserStatus);
		putU32(m->m_frames);
		for (int i = 0; i < MAX_OUTER_NODES; ++i) putU32((unsigned)m->m_outerSystemIDs[i]);
		for (int i = 0; i < MAX_OUTER_NODES; ++i) putU32((unsigned)m->m_laserBeamIDs[i]);
		putU32((unsigned)m->m_groundToOrbitBeamID);
		putU32((unsigned)m->m_orbitToTargetBeamID);
		putU32((unsigned)m->m_connectorSystemID);
		putU32((unsigned)m->m_laserBaseSystemID);
		for (int i = 0; i < MAX_OUTER_NODES; ++i) for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_outerNodePositions[i].x)[k]));
		for (int i = 0; i < MAX_OUTER_NODES; ++i) for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits(m->m_outerNodeOrientations[i][r][c]));
		const Coord3D *ps[5] = { &m->m_connectorNodePosition, &m->m_laserOriginPosition, &m->m_initialTargetPosition, &m->m_currentTargetPosition,
			&m->m_overrideTargetDestination };
		for (int i = 0; i < 5; ++i) for (int k = 0; k < 3; ++k) putU32(realBits((&ps[i]->x)[k]));
		const UnsignedInt us[10] = { m->m_scorchMarksMade, m->m_nextScorchMarkFrame, m->m_nextLaunchFXFrame, m->m_damagePulsesMade,
			m->m_nextDamagePulseFrame, m->m_startAttackFrame, m->m_startDecayFrame, m->m_lastDrivingClickFrame, m->m_2ndLastDrivingClickFrame,
			m->m_nextDestWaypointID };
		for (int i = 0; i < 10; ++i) putU32(us[i]);
		const Bool bs[6] = { m->m_upBonesCached, m->m_defaultInfoCached, m->m_invalidSettings, m->m_manualTargetMode, m->m_scriptedWaypointMode,
			m->m_clientShroudedLastFrame };
		for (int i = 0; i < 6; ++i) putU32(bs[i] ? 1u : 0u);
		putU32(m->m_powerupSound.getPlayingHandle());
		putU32(m->m_unpackToReadySound.getPlayingHandle());
		putU32(m->m_firingToIdleSound.getPlayingHandle());
		putU32(m->m_annihilationSound.getPlayingHandle());
	};
	{
		DisabledMaskType mask = m->getDisabledTypesToProcess();
		unsigned bits = 0;
		for (int t = 0; t < DISABLED_COUNT; ++t) if (mask.test((DisabledType)t)) bits |= 1u << t;
		putU32(bits);
	}
	g_events.clear();
	g_ulog.clear();
	m->onObjectCreated();
	putLog();
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 5 && o[1] <= 22) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[2048];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: {
				Coord3D pos;
				pos.x = r75F(o[2]); pos.y = r75F(o[3]); pos.z = r75F(o[4]);
				unsigned obj = o[1] >> 5 & 7, node = o[1] >> 2 & 7;
				Object *target = (obj >= 2 && obj <= 5) ? g_r75Objs[obj - 1] : 0;
				Waypoint *way = (node >= 1 && node <= 4) ? &g_r75Ways[node - 1] : 0;
				UnsignedInt opts = (o[1] >> 1 & 1) ? COMMAND_FIRED_BY_SCRIPT : 0;
				res = m->initiateIntentToDoSpecialPower(r75Template(o[1] >> 8), target, (o[1] & 1) ? &pos : 0, way, opts) ? 1 : 0;
				break;
			}
			case 4: {
				Coord3D pos;
				pos.x = r75F(o[1]); pos.y = r75F(o[2]); pos.z = r75F(o[3]);
				m->setSpecialPowerOverridableDestination(&pos);
				break;
			}
			case 5: xst = runXfer(m, 0, xfer_path, bytes, 2048, &blen, &crc); g_r75Saved.assign(bytes, bytes + blen); break;
			case 6: {
				std::vector<unsigned char> b = g_r75Saved;
				if (b.size() > 2048) b.resize(2048);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 7: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 8:
				res = (m->doesSpecialPowerHaveOverridableDestinationActive() ? 1u : 0u) | (m->isActive() ? 2u : 0u) | (m->isPowerCurrentlyInUse() ? 4u : 0u)
					| (m->doesSpecialPowerHaveOverridableDestination() ? 8u : 0u) | ((unsigned)m->getCommandOption() << 4);
				break;
			case 9: m->onObjectCreated(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		if (st) res = 0;
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putState();
		putU32((unsigned)g_r75Made.size());
		Coord3D last; last.x = last.y = last.z = 0.0f;
		if (!g_r75Made.empty()) last = g_r75Made.back()->m_pos;
		for (int k = 0; k < 3; ++k) putU32(realBits((&last.x)[k]));
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_ahRangeHook = 0;
	g_mcHook = 0;
	g_r75NewDrawHook = 0;
	g_multiBoneHook = 0;
	g_spLocal = 0;
	g_r75Me = 0;
	g_r28ThingPose = false;
	g_objXformSyncPos = false;
	g_ahRange.clear();
	ctClearEffects();
	for (size_t i = 0; i < g_r75Lasers.size(); ++i) delete g_r75Lasers[i];
	g_r75Lasers.clear();
	me->m_behaviors = 0;
	me->m_drawable = 0;
	delete m;
	delete power;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r75Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_body = 0;
		delete x;
		g_r75Objs[k] = 0;
	}
	for (size_t i = 0; i < g_r75Made.size(); ++i) { g_r75Made[i]->m_team = 0; g_r75Made[i]->m_player = 0; g_r75Made[i]->m_behaviors = 0; delete g_r75Made[i]; }
	g_r75Made.clear();
	ThePlayerList = 0;
	TheAudio = 0;
	TheTerrainLogic = 0;
	ThePartitionManager = 0;
	TheParticleSystemManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r75_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r75RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
