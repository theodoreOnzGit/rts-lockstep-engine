/* Differential-test driver for SpecialAbilityUpdate (p1w13-sonnet, batch R81; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialAbilityUpdate.h          (mod_gen/SpecialAbilityUpdate_r81.h: its guard renamed, its
**     particle system parser this unit's l9ahParsePsTemplate; build.rs)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpecialAbilityUpdate.cpp  (mod_gen/SpecialAbilityUpdate_r81.cpp: its include of the
**     header edited; build.rs)
** in wrap_module.cpp's unit, at its end, on batch R53's real SpecialPowerUpdateModule and batch R50's real PhysicsBehavior. The module build's
** GameLogic/Module/SpecialAbilityUpdate.h is batch O's stand-in, so the class is compiled under the name R81SpecialAbilityUpdate (and its data
** R81SpecialAbilityUpdateModuleData); Object::findSpecialAbilityUpdate, which answers the stand-in's type, is Object.cpp's (verbatim below) under
** the name r81FindSpecialAbilityUpdate; StickyBombUpdate is a stand-in whose initStickyBomb and detonate are lines; PhysicsBehavior is R50's.
**
** The scene: four objects (ids 1-4) and the special objects the module makes (5, 6, ...). Object 1 is the unit: the module of the data the test
** reads (block D), maybe a second one of the same data for another special power (its type maybe exclusive with the first's), maybe a special
** power module (its template, readiness and ready frame the test's; markSpecialPowerTriggered and startPowerRecharge lines), maybe an AI (its
** idle, moving and last command source the test's; its orders lines), maybe R50's PhysicsBehavior, maybe a stealth update (markAsDetected and
** disguiseAsObject lines), maybe a drawable (model condition changes, setAnimationCompletionTime, lines), maybe the bone BONE_A, maybe weapons in
** its primary and secondary slots (setWeaponLock a line), an experience tracker (addExperiencePoints a line). Objects 2-3 are targets (kinds,
** a drawable, a contain that may be garrisonable (removeAllContained a line), effectively dead, stealthed, detected, booby trapped, a booby trap
** that goes off (checkAndDetonateBoobyTrap a line), a geometry); object 4 maybe a mine. Every object: a position, an angle, a team of three
** (each with a player: its money, colour; the relationships and the local player the test's). The thing factory: SO_LASER (a drawable with a
** LaserUpdate, initLaser a line), SO_PLAIN, SO_BOMB (a StickyBombUpdate stand-in), SO_PHYS (R50's PhysicsBehavior, centre of mass offset 1.5).
** The partition manager: the distances are PartitionManager.cpp's (batch R57's), its iterations (potential collisions, the line of sight scan,
** the closest object) the test's list (lines). The particle system manager, the Eva, the radar, the floating texts, the money and score, the
** academy, defect, setDisabledUntil, destroyObject and the audio: lines. The REAL UpdateModule, INI, Xfer, logic random values and
** Object::getRelationship. */

class R81SpecialAbilityUpdate;
const NameKeyType kR81KeySticky = (NameKeyType)0x7F810001;
const NameKeyType kR81KeyLaser = (NameKeyType)0x7F810002;
inline NameKeyType r81NameKey(const char *name)
{
	if (strcmp(name, "StickyBombUpdate") == 0) return kR81KeySticky;
	if (strcmp(name, "LaserUpdate") == 0) return kR81KeyLaser;
	return TheNameKeyGenerator->nameToKey(name);
}
/* a sticky bomb whose calls are lines */
class R81StickyBomb
{
public:
	void initStickyBomb( Object *object, const Object *bomber, const Coord3D *specificPos = NULL )
	{
		ulog("stickyinit %d %d %d", (int)m_owner->m_oid, object ? (int)object->m_oid : 0, bomber ? (int)bomber->m_oid : 0);
	}
	void detonate() { ulog("detonate %d", (int)m_owner->m_oid); }
	Object *m_owner = 0;
};
/* (BaseType.h's min, line 101, and the compiler's __max, which the module build's BaseType.h does not define: for the file alone) */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define __max(x,y) (((x)>(y)) ? (x) : (y))
#define NAMEKEY r81NameKey
#define StickyBombUpdate R81StickyBomb
#define SpecialAbilityUpdate R81SpecialAbilityUpdate
#define SpecialAbilityUpdateModuleData R81SpecialAbilityUpdateModuleData
#define findSpecialAbilityUpdate r81FindSpecialAbilityUpdate
#define getSpecialAbilityUpdateModuleData getR81SpecialAbilityUpdateModuleData
#define PhysicsBehavior R50PhysicsBehavior
#define getPhysics r50GetPhysics
#define destroyObject r50DestroyObject
#include "SpecialAbilityUpdate_r81.cpp"
#undef destroyObject
#undef getPhysics
#undef PhysicsBehavior
#undef getSpecialAbilityUpdateModuleData
#undef findSpecialAbilityUpdate
#undef SpecialAbilityUpdateModuleData
#undef SpecialAbilityUpdate
#undef StickyBombUpdate
#undef NAMEKEY
#undef __max
#undef min

/* Object.cpp 5981-5997, verbatim but for the names */
R81SpecialAbilityUpdate* Object::r81FindSpecialAbilityUpdate( SpecialPowerType type ) const
{
	for( BehaviorModule** u = m_behaviors; *u; ++u )
	{
		SpecialPowerUpdateInterface *spInterface = (*u)->getSpecialPowerUpdateInterface();
		if( spInterface && spInterface->isSpecialAbility() )
		{
			R81SpecialAbilityUpdate *spUpdate = (R81SpecialAbilityUpdate*)spInterface;
			if( spUpdate->getSpecialPowerType() == type )
			{
				return spUpdate;
			}
		}
	}  // end for

	return NULL;
}
/* Drawable.cpp 322-334, verbatim */
void Drawable::saturateRGB(RGBColor& color, Real factor)
{
	color.red *= factor;
	color.green *= factor;
	color.blue *= factor;

	Real halfFactor = factor * 0.5f;

	color.red -= halfFactor;
	color.green -= halfFactor;
	color.blue -= halfFactor;

}
void Drawable::setAnimationCompletionTime(UnsignedInt numFrames) { ulog("animdone %d %u", m_ownerObject ? (int)m_ownerObject->m_oid : 0, numFrames); }
void AIUpdateInterface::aiMoveToObject( Object *obj, CommandSourceType cmdSource ) { ulog("aimoveobj %d %d %d", (int)m_owner->m_oid, obj ? (int)obj->m_oid : 0, (int)cmdSource); }
void AIUpdateInterface::aiFaceObject( Object *obj, CommandSourceType cmdSource ) { ulog("aifaceobj %d %d %d", (int)m_owner->m_oid, obj ? (int)obj->m_oid : 0, (int)cmdSource); }
void AIUpdateInterface::aiFacePosition( const Coord3D *pos, CommandSourceType cmdSource )
{
	ulog("aifacepos %d %08x %08x %08x %d", (int)m_owner->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)cmdSource);
}
void StealthUpdate::disguiseAsObject( const Object *target ) { ulog("disguise %d %d", (int)m_owner->m_oid, target ? (int)target->m_oid : 0); }
void ExperienceTracker::addExperiencePoints( Int experienceGain, Bool canScaleForBonus ) { ulog("xpadd %d %d %d", (int)m_owner->m_oid, (int)experienceGain, (int)canScaleForBonus); }
Bool Player::addSkillPoints( Int delta ) { ulog("skill %d %d", (int)m_idx, (int)delta); return FALSE; }
void PowerAcademyStats::recordBuildingCapture() { ulog("captured %d", (int)((Player *)this)->m_idx); }

namespace {

ModuleData *l9r81Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = R81SpecialAbilityUpdate::friend_newModuleData(&ini);
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

/* a special power module whose template, readiness and ready frame are the test's; the trigger and the recharge are lines */
class R81Power : public SpecialPowerModuleInterface {
public:
	virtual Bool isModuleForPower(const SpecialPowerTemplate *t) const { return t == m_template; }
	virtual Bool isReady(void) const { return m_ready; }
	virtual Real getPercentReady(void) const { return 0; }
	virtual UnsignedInt getReadyFrame(void) const { return m_readyFrame; }
	virtual AsciiString getPowerName(void) const { return AsciiString(); }
	virtual const SpecialPowerTemplate *getSpecialPowerTemplate(void) const { return m_template; }
	virtual ScienceType getRequiredScience(void) const { return SCIENCE_INVALID; }
	virtual void onSpecialPowerCreation(void) {}
	virtual void setReadyFrame(UnsignedInt) {}
	virtual void pauseCountdown(Bool) {}
	virtual void doSpecialPower(UnsignedInt) {}
	virtual void doSpecialPowerAtObject(Object *, UnsignedInt) {}
	virtual void doSpecialPowerAtLocation(const Coord3D *, Real, UnsignedInt) {}
	virtual void doSpecialPowerUsingWaypoints(const Waypoint *, UnsignedInt) {}
	virtual void markSpecialPowerTriggered(const Coord3D *loc)
	{
		if (loc) ulog("triggered 1 %u,%u,%u", realBits(loc->x), realBits(loc->y), realBits(loc->z));
		else ulog("triggered 1 null");
	}
	virtual void startPowerRecharge() { ulog("recharge 1"); }
	virtual const AudioEventRTS &getInitiateSound() const { static AudioEventRTS a; return a; }
	virtual Bool isScriptOnly() const { return false; }
	virtual const ThingTemplate *getReferenceThingTemplate() const { return 0; }
	const SpecialPowerTemplate *m_template = 0;
	Bool m_ready = false;
	UnsignedInt m_readyFrame = 0;
	int m_index = 0;
};
class R81PowerBehavior : public TestBehavior {
public:
	R81PowerBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d) {}
	virtual SpecialPowerModuleInterface *getSpecialPower() { return &m_sp; }
	R81Power m_sp;
};
/* a contain whose garrisonability is the test's and whose removeAllContained is a line */
class R81Contain : public UpgContain
{
public:
	Object *m_r81Owner = 0;
	Bool m_garrisonable = false;
	virtual Bool isGarrisonable() const { return m_garrisonable; }
	virtual void removeAllContained(Bool exposeStealthUnits) { ulog("removeall %d %d", (int)m_r81Owner->m_oid, (int)exposeStealthUnits); }
};
/* a special object made by the factory */
struct R81Made {
	Object *obj = 0;
	Drawable *draw = 0;
	LaserUpdate *laser = 0;
	R81StickyBomb *bomb = 0;
	R50PhysicsBehavior *phys = 0;
	ExperienceTracker *xp = 0;
	BehaviorModule **list = 0;
};

const int kR81Templates = 13;
const char *const kR81Powers[kR81Templates] = { "SP_0", "SP_1", "SP_2", "SP_3", "SP_4", "SP_5", "SP_6", "SP_7", "SP_8", "SP_9", "SP_10", "SP_11", "SP_12" };
const Real kR81Radii[8] = { 1.0f, 2.5f, 5.0f, 8.0f, 12.0f, 0.5f, 3.0f, 15.0f };
std::vector<unsigned char> g_r81Saved;
Object *g_r81Objs[4];
std::vector<R81Made> g_r81Made;
std::set<int> g_r81Gone;
Team g_r81Teams[3];
Player g_r81Players[3];
int g_r81Rel[3][3];
ThingTemplate g_r81Templates[4];
R50PhysicsBehaviorModuleData *g_r81PhysData = 0;
bool g_r81Booby[5];
bool g_r81Bone = false;
const char *const kR81TemplateNames[4] = { "SO_LASER", "SO_PLAIN", "SO_BOMB", "SO_PHYS" };

const SpecialPowerTemplate *r81Template(unsigned k)
{
	if (k >= (unsigned)kR81Templates) return 0;
	return (const SpecialPowerTemplate *)ref_sp_find_raw(kR81Powers[k]);
}
Object *r81Find(ObjectID id)
{
	if (g_r81Gone.count((int)id)) return 0;
	if ((int)id >= 1 && (int)id <= 4) return g_r81Objs[(int)id - 1];
	size_t k = (size_t)((int)id - 5);
	if ((int)id >= 5 && k < g_r81Made.size()) return g_r81Made[k].obj;
	return 0;
}
R81Made *r81MadeOf(const Object *o)
{
	for (size_t k = 0; k < g_r81Made.size(); ++k) if (g_r81Made[k].obj == o) return &g_r81Made[k];
	return 0;
}
UpdateModule *r81FindUpdate(const Object *obj, NameKeyType key)
{
	if (key == kR81KeySticky) {
		R81Made *m = r81MadeOf(obj);
		if (m && m->bomb) return (UpdateModule *)(void *)m->bomb;
	}
	return 0;
}
Relationship r81TeamRel(const Team *from, const Team *to)
{
	if (!from || !to) return NEUTRAL;
	return (Relationship)g_r81Rel[from->m_dummy - 1][to->m_dummy - 1];
}
std::vector<Object *> r81List(unsigned w)
{
	std::vector<Object *> v;
	for (int k = 0; k < 8; ++k) {
		unsigned id = w >> (4 * k) & 15;
		if (id == 0) break;
		Object *o = r81Find((ObjectID)id);
		if (o) v.push_back(o);
	}
	return v;
}
std::vector<Object *> g_r81Scan;
/* the closest object: the first of the test's list that the caller's two filters (finishAbility's PartitionFilterAcceptByKindOf(MINE) and
** PartitionFilterSamePlayer(the unit's player)) allow, by PartitionManager.cpp's allow functions (lines 5217-5223, 5479-5482); the module
** build's stand-ins of the two filters have no allow of their own (and PartitionFilterSamePlayer's name is another build's too) */
Object *r81Closest(PartitionFilter **)
{
	const Player *player = g_r81Objs[0]->getControllingPlayer();
	for (size_t i = 0; i < g_r81Scan.size(); ++i) {
		Object *c = g_r81Scan[i];
		if (!r81Find((ObjectID)c->m_oid)) continue;   /* (gone since the list was given: no longer in the partition) */
		if (c->isKindOf(KINDOF_MINE) && player == c->getControllingPlayer()) return c;
	}
	return 0;
}
const ThingTemplate *r81FindTemplate(const AsciiString &name)
{
	for (int k = 0; k < 4; ++k) if (g_r81Templates[k].m_name.compare(name) == 0) return &g_r81Templates[k];
	return 0;
}
int r81Bone(const Object *o, const char *name, Coord3D *pos, Matrix3D *)
{
	if (o == g_r81Objs[0] && g_r81Bone && strcmp(name, "BONE_A") == 0) {
		if (pos) { *pos = o->m_pos; pos->x += 1.0f; pos->y += 2.0f; pos->z += 3.0f; }
		return 1;
	}
	if (pos) *pos = o->m_pos;
	return 0;
}
Bool r81BoobyHook(const Object *self, const Object *)
{
	int id = (int)self->m_oid;
	return id >= 1 && id <= 4 && g_r81Booby[id] ? TRUE : FALSE;
}
void r81Defect(Object *o, Team *t, UnsignedInt time)
{
	ulog("defect %d %d %u", (int)o->m_oid, t ? t->m_dummy : 0, time);
}
UpdateModule *g_r81Updating = 0;
void r81Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r81Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
Object *r81NewObject(const ThingTemplate *tmplate, Team *team)
{
	R81Made m;
	Object *o = m.obj = newObject();
	o->m_oid = (ObjectID)(5 + g_r81Made.size());
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	o->m_xp = m.xp = new ExperienceTracker;
	m.xp->m_owner = o;
	m.draw = new Drawable;   /* (ThingFactory::newObject makes the object's drawable) */
	m.draw->m_ownerObject = o;
	o->m_drawable = m.draw;
	m.list = new BehaviorModule *[2];
	m.list[0] = m.list[1] = 0;
	if (tmplate == &g_r81Templates[0]) {
		m.laser = new LaserUpdate;
		m.laser->m_r59Owner = o;
		m.draw->m_r59Laser = (ClientUpdateModule *)(void *)m.laser;
	} else if (tmplate == &g_r81Templates[2]) {
		m.bomb = new R81StickyBomb;
		m.bomb->m_owner = o;
	} else if (tmplate == &g_r81Templates[3]) {
		m.phys = newInstance(R50PhysicsBehavior)(o, g_r81PhysData);
		m.list[0] = m.phys;
		o->m_r50Physics = m.phys;
	}
	o->m_behaviors = m.list;
	g_r81Made.push_back(m);
	return o;
}
Real r81F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
/* the unit's three per-unit voices, each named for its key */
const char *const kR81Voices[3] = { "VoiceCaptureBuildingComplete", "VoiceDisableVehicleComplete", "VoiceStealCashComplete" };
AudioEventRTS g_r81Voices[3];
const AudioEventRTS *r81PerUnitSound(const AsciiString &name)
{
	for (int k = 0; k < 3; ++k) if (strcmp(name.str(), kR81Voices[k]) == 0) return &g_r81Voices[k];
	return 0;
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a SpecialAbilityUpdate, make the scene and the module on object 1 (and, if asked, a second of the
** same data with another template). `cfg` (32 words; * settable by op 1): 0 the frame, 1 the logic random seed, 2 the unit (bits 0 a drawable,
** 1 an AI, 2 a PhysicsBehavior, 3 a stealth update, 4 a special power module, 5-8 its template (SP_k, 13+ none), 9 a second module, 10-13 its
** template, 14 a primary weapon, 15 a secondary weapon, 16-17 the team (0 none), 18-20 the radius (of kR81Radii), 21 the bone BONE_A), 3* the
** unit's changing state (bits 0 effectively dead, 1 the AI idle, 2 moving, 3-4 its last command source, 5 the power ready), 4* the power's
** ready frame, 5-6 objects 2-3 (bits 0 STRUCTURE, 1 MINE, 2 a drawable, 3 a contain, 4 garrisonable, 5-6 the team, 7-9 the radius, 10 a box
** (else a sphere)), 7-8* their changing state (bits 0 effectively dead, 1 stealthed, 2 detected, 3 booby trapped, 4 a booby trap goes off),
** 9 object 4 (bits 0 MINE, 1-2 the team, 3-5 the radius), 10-13* objects 1-4's x, 14-17* their y (float bits; z 0), 18* their angles
** (nibbles: 0.4 k - 3.0), 19* the partition manager's list (nibbles of object numbers, 0 ends), 20 the players' money (11 bits each), 21 the
** teams' relationships (2 bits each, row-major), 22 the local player (3: none). The data dump goes first. Then `n` ops of 5 words
** [op, a, b, c, d] ("module a": 1 the first, 2 the second; none, nothing): 0 module a's update()  1 cfg[a] = b  2 the frame = a  3 xfer save
** (the first's)  4 xfer load of the last save, then loadPostProcess  5 xfer CRC  6 module (a >> 8)'s initiateIntentToDoSpecialPower(SP_(a & 255)
** or NULL, object b or NULL, c != 0 ? (x, y, 0) of d's signed bytes * 4 : NULL)  7 module a's isPowerCurrentlyInUse()  8 module a's
** getSpecialObjectCount, getSpecialObjectMax, findSpecialObjectWithProducerID(object b)  9 module a's isActive()  10 module a's
** getSpecialPowerType()  11 object b is gone (findObjectByID no longer finds it). Each op appends: the CATCH_ALL status, the result, the effect
** log, the xfer status, bytes and CRC; then each module (0, or 1 and its state), and each object found: its id, position, angle, its
** IS_USING_ABILITY status, its producer, its experience sink, its physics (0, or 1 then its velocity, rates and flags). */
static int l9r81RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r81Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	ModuleData *d2 = 0;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	ref_ini_sp_bind(ref_sp_find_raw);
		TheParticleSystemManager = &g_ahPsm;
	g_ahPsLastId = 0;
	try { d = l9r81Read(filename); d2 = l9r81Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_ini_sp_bind(0);
	if (status) { delete d; TheParticleSystemManager = 0; return status; }
	((R81SpecialAbilityUpdateModuleData *)d2)->m_specialPowerTemplate = r81Template(cfg[2] >> 10 & 15);
	{
		const R81SpecialAbilityUpdateModuleData *md = (const R81SpecialAbilityUpdateModuleData *)d;
		putU32(ref_sp_raw_id(md->m_specialPowerTemplate));
		putStr(md->m_specialObjectName.str(), (int)md->m_specialObjectName.getLength());
		putStr(md->m_specialObjectAttachToBoneName.str(), (int)md->m_specialObjectAttachToBoneName.getLength());
		const Real reals[4] = { md->m_startAbilityRange, md->m_abilityAbortRange, md->m_packUnpackVariationFactor, md->m_fleeRangeAfterCompletion };
		for (int k = 0; k < 4; ++k) putU32(realBits(reals[k]));
		const Int ints[3] = { md->m_effectValue, md->m_awardXPForTriggering, md->m_skillPointsForTriggering };
		for (int k = 0; k < 3; ++k) putU32((unsigned)ints[k]);
		const UnsignedInt uints[7] = { md->m_preparationFrames, md->m_persistentPrepFrames, md->m_effectDuration, md->m_maxSpecialObjects, md->m_packTime,
			md->m_unpackTime, md->m_preTriggerUnstealthFrames };
		for (int k = 0; k < 7; ++k) putU32(uints[k]);
		const Bool bools[14] = { md->m_skipPackingWithNoTarget, md->m_specialObjectsPersistent, md->m_uniqueSpecialObjectTargets,
			md->m_specialObjectsPersistWhenOwnerDies, md->m_flipObjectAfterPacking, md->m_flipObjectAfterUnpacking, md->m_alwaysValidateSpecialObjects,
			md->m_doCaptureFX, md->m_loseStealthOnTrigger, md->m_approachRequiresLOS, md->m_needToFaceTarget, md->m_persistenceRequiresRecharge, 0, 0 };
		unsigned flags = 0;
		for (int k = 0; k < 14; ++k) if (bools[k]) flags |= 1u << k;
		putU32(flags);
		const char *ps = md->m_disableFXParticleSystem ? g_ahPsNames[(size_t)md->m_disableFXParticleSystem->m_idx].c_str() : "-";
		putStr(ps, (int)strlen(ps));
		const AudioEventRTS *sounds[4] = { &md->m_packSound, &md->m_unpackSound, &md->m_prepSoundLoop, &md->m_triggerSound };
		for (int k = 0; k < 4; ++k) putStr(sounds[k]->getEventName().str(), (int)sounds[k]->getEventName().getLength());
	}
	initKindOfMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	R34Audio audio;
	TheAudio = &audio;
	g_audioHandleLast = 0;
	g_cfMisc.m_defectorTimerTickSound.setEventName(AsciiString("defectorTick"));
	ThePartitionManager = &g_ahPm;
	ThePlayerList = &g_spList;
	TheEva = &g_spEva;
	TheRadar = &g_sabRadar;
	TheInGameUI = &g_spUi;
	TheGameText = &g_spText;
	ModTestPowGlobalData globals;
	globals.m_selectionFlashSaturationFactor = 0.5f;
	TheGlobalData = &globals;
	g_modDestroyHook = r50Destroy;
	g_modFindHook = r81Find;
	g_findUpdateHook = r81FindUpdate;
	g_teamRelHook = r81TeamRel;
	g_findTemplateHook = r81FindTemplate;
	g_newObjectHook = r81NewObject;
	g_closestAtHook = r81Closest;
	g_distDcHook = r57Dist;
	g_distPosDcHook = r57DistPos;
	g_r74BoneHook = r81Bone;
	g_boobyHook = r81BoobyHook;
	g_defectHook = r81Defect;
	g_mcHook = r43Mc;
	g_r28ThingPose = true;
	g_perUnitSoundHook = r81PerUnitSound;
	for (int k = 0; k < 3; ++k) g_r81Voices[k].setEventName(AsciiString(kR81Voices[k]));
	gl.m_awakenHook = r81Awaken;
	g_r81Made.clear();
	g_r81Gone.clear();
	R50PhysicsBehaviorModuleData physData;
	R50PhysicsBehaviorModuleData soPhysData;
	soPhysData.m_centerOfMassOffset = 1.5f;
	g_r81PhysData = &soPhysData;
	for (int k = 0; k < 4; ++k) g_r81Templates[k].m_name = AsciiString(kR81TemplateNames[k]);
	for (int t = 0; t < 3; ++t) {
		g_r81Players[t].m_idx = t;
		g_r81Players[t].m_money.m_idx = t;
		g_r81Players[t].m_score.m_idx = t;
		g_r81Players[t].m_money.m_count = cfg[20] >> (11 * t) & 0x7FF;
		g_r81Players[t].m_color = (Color)(0xFF102030u + 0x00203040u * (unsigned)t);
		g_r81Players[t].m_defaultTeam = &g_r81Teams[t];
		g_r81Teams[t].m_dummy = t + 1;
		g_r81Teams[t].m_controller = &g_r81Players[t];
	}
	for (int a = 0; a < 3; ++a) for (int b = 0; b < 3; ++b) g_r81Rel[a][b] = (int)(cfg[21] >> (2 * (3 * a + b)) & 3);
	g_spLocal = (cfg[22] & 3) < 3 ? &g_r81Players[cfg[22] & 3] : 0;
	/* the objects */
	Drawable draws[4];
	R81Contain contains[2];
	AIUpdateInterface ai;
	StealthUpdate stealth;
	ExperienceTracker xps[4];
	Weapon weapon;
	R81PowerBehavior *power = 0;
	R50PhysicsBehavior *unitPhys = 0;
	UpdateModuleData behaviorData;
	BehaviorModule *lists[4][6] = { { 0 } };
	for (int k = 0; k < 4; ++k) {
		Object *o = g_r81Objs[k] = newObject();
		o->m_oid = (ObjectID)(k + 1);
		o->m_behaviors = lists[k];
		xps[k].m_owner = o;
		o->m_xp = &xps[k];
		unsigned f = k == 0 ? cfg[2] : k < 3 ? cfg[4 + k] : cfg[9];
		unsigned team = k == 0 ? (f >> 16 & 3) : k < 3 ? (f >> 5 & 3) : (f >> 1 & 3);
		o->m_team = team ? &g_r81Teams[team - 1] : 0;
		o->m_player = team ? &g_r81Players[team - 1] : 0;
		Real r = k == 0 ? kR81Radii[f >> 18 & 7] : k < 3 ? kR81Radii[f >> 7 & 7] : kR81Radii[f >> 3 & 7];
		o->m_geometry = (k == 1 || k == 2) && (f >> 10 & 1) ? GeometryInfo(GEOMETRY_BOX, false, 5.0f, r, r) : GeometryInfo(GEOMETRY_SPHERE, false, r, r, r);
		o->m_kindOf.clear();
		draws[k].m_ownerObject = o;
		if (k == 0) {
			o->m_drawable = (f & 1) ? &draws[0] : 0;
			ai.m_owner = o;
			o->m_ai = (f >> 1 & 1) ? &ai : 0;
			stealth.m_owner = o;
			o->m_stealth = (f >> 3 & 1) ? &stealth : 0;
			o->m_slotWeapon[PRIMARY_WEAPON] = (f >> 14 & 1) ? &weapon : 0;
			o->m_slotWeapon[SECONDARY_WEAPON] = (f >> 15 & 1) ? &weapon : 0;
			g_r81Bone = (f >> 21 & 1) != 0;
		} else if (k < 3) {
			if (f & 1) o->m_kindOf.set(KINDOF_STRUCTURE);
			if (f >> 1 & 1) o->m_kindOf.set(KINDOF_MINE);
			o->m_drawable = (f >> 2 & 1) ? &draws[k] : 0;
			contains[k - 1].m_r81Owner = o;
			contains[k - 1].m_garrisonable = (f >> 4 & 1) != 0;
			o->m_contain = (f >> 3 & 1) ? &contains[k - 1] : 0;
		} else {
			if (f & 1) o->m_kindOf.set(KINDOF_MINE);
		}
		o->m_tmpl.m_kindOf = o->m_kindOf;
	}
	Object *me = g_r81Objs[0];
	me->m_tmpl.m_voiceTaskComplete.setEventName(AsciiString("VoiceTaskComplete"));
	auto apply = [&](unsigned a) {
		if (a == 3) {
			unsigned f = cfg[3];
			me->m_effDead = (f & 1) != 0;
			ai.m_idle = (f >> 1 & 1) != 0;
			ai.m_moving = (f >> 2 & 1) != 0;
			ai.m_lastCmdSource = (Int)(f >> 3 & 3);
			if (power) power->m_sp.m_ready = (f >> 5 & 1) != 0;
		} else if (a == 4) {
			if (power) power->m_sp.m_readyFrame = cfg[4];
		} else if (a == 7 || a == 8) {
			Object *o = g_r81Objs[a - 6];
			unsigned f = cfg[a];
			o->m_effDead = (f & 1) != 0;
			o->m_status.set(OBJECT_STATUS_STEALTHED, (f >> 1 & 1) ? 1 : 0);
			o->m_status.set(OBJECT_STATUS_DETECTED, (f >> 2 & 1) ? 1 : 0);
			o->m_status.set(OBJECT_STATUS_BOOBY_TRAPPED, (f >> 3 & 1) ? 1 : 0);
			g_r81Booby[a - 5] = (f >> 4 & 1) != 0;
		} else if (a >= 10 && a <= 18) {
			for (int k = 0; k < 4; ++k) {
				Object *o = g_r81Objs[k];
				o->m_pos.x = r81F(cfg[10 + k]);
				o->m_pos.y = r81F(cfg[14 + k]);
				o->m_pos.z = 0.0f;
				o->m_orientation = (Real)(cfg[18] >> (4 * k) & 15) * 0.4f - 3.0f;
				o->poseMatrix();
			}
		} else if (a == 19) {
			g_r81Scan = r81List(cfg[19]);
			g_ahRange = g_r81Scan;
			g_hdScan = g_r81Scan;
		}
	};
	R81SpecialAbilityUpdate *m = 0;
	R81SpecialAbilityUpdate *m2 = 0;
	try {
		int at = 0;
		m = newInstance(R81SpecialAbilityUpdate)(me, d);
		lists[0][at++] = m;
		if (cfg[2] >> 9 & 1) { m2 = newInstance(R81SpecialAbilityUpdate)(me, d2); lists[0][at++] = m2; }
		if (cfg[2] >> 4 & 1) {
			power = new R81PowerBehavior(me, &behaviorData);
			power->m_sp.m_template = r81Template(cfg[2] >> 5 & 15);
			power->m_sp.m_index = at;
			lists[0][at++] = power;
		}
		if (cfg[2] >> 2 & 1) {
			unitPhys = newInstance(R50PhysicsBehavior)(me, &physData);
			lists[0][at++] = unitPhys;
			me->m_r50Physics = unitPhys;
		}
	}
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	for (unsigned a = 3; a <= 19; ++a) apply(a);
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind != 100) continue;
			const std::string &l = g_ulog[(size_t)g_events[j].p];
			if (l.compare(0, 13, "settransform ") == 0 || l.compare(0, 10, "setorient ") == 0 || l == "partition") continue;   /* the dump's */
			log.push_back(l);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	auto putModule = [&](R81SpecialAbilityUpdate *sa) {
		putU32(sa ? 1 : 0);
		if (!sa) return;
		putU32(sa->friend_getNextCallFrame());
		putU32(sa->m_prepSoundLoop.getPlayingHandle());
		putU32(sa->m_prepFrames);
		putU32(sa->m_animFrames);
		putU32((unsigned)sa->m_targetID);
		putU32(realBits(sa->m_targetPos.x));
		putU32(realBits(sa->m_targetPos.y));
		putU32(realBits(sa->m_targetPos.z));
		putU32((unsigned)sa->m_locationCount);
		putU32(sa->m_specialObjectEntries);
		putU32((unsigned)sa->m_specialObjectIDList.size());
		for (std::list<ObjectID>::const_iterator it = sa->m_specialObjectIDList.begin(); it != sa->m_specialObjectIDList.end(); ++it) putU32((unsigned)*it);
		putU32(realBits(sa->m_captureFlashPhase));
		putU32((unsigned)sa->m_packingState);
		const Bool fl[6] = { sa->m_active, sa->m_noTargetCommand, sa->m_facingInitiated, sa->m_facingComplete, sa->m_withinStartAbilityRange,
			sa->m_doDisableFXParticles };
		unsigned flags = 0;
		for (int j = 0; j < 6; ++j) if (fl[j]) flags |= 1u << j;
		putU32(flags);
	};
	auto putObject = [&](Object *x) {
		putU32((unsigned)x->m_oid);
		putU32(realBits(x->m_pos.x));
		putU32(realBits(x->m_pos.y));
		putU32(realBits(x->m_pos.z));
		putU32(realBits(x->m_orientation));
		putU32(x->m_status.test(OBJECT_STATUS_IS_USING_ABILITY) ? 1 : 0);
		putU32((unsigned)x->getProducerID());
		putU32((unsigned)(x->m_xp ? x->m_xp->m_sink : INVALID_ID));
		R50PhysicsBehavior *p = x->m_r50Physics;
		putU32(p ? 1 : 0);
		if (p) {
			const unsigned pw[7] = { realBits(p->m_vel.x), realBits(p->m_vel.y), realBits(p->m_vel.z), realBits(p->m_yawRate), realBits(p->m_pitchRate),
				realBits(p->m_rollRate), (unsigned)p->m_flags };
			for (int j = 0; j < 7; ++j) putU32(pw[j]);
		}
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1 && o[1] < 32) { cfg[o[1]] = o[2]; apply(o[1]); }
		if (o[0] == 2) gl.m_frame = o[1];
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2s[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		R81SpecialAbilityUpdate *ma = (o[0] == 6 ? (o[1] >> 8) : o[1]) == 2 ? m2 : (o[0] == 6 ? (o[1] >> 8) : o[1]) == 1 ? m : 0;
		Object *objB = (o[2] >= 1) ? r81Find((ObjectID)o[2]) : 0;
		try {
			switch (o[0]) {
			case 0: if (ma) { g_r81Updating = ma; res = (unsigned)ma->update(); } break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r81Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r81Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: {
				Coord3D pos;
				pos.x = (Real)(signed char)(o[4] & 255) * 4.0f;
				pos.y = (Real)(signed char)(o[4] >> 8 & 255) * 4.0f;
				pos.z = 0.0f;
				if (ma) res = ma->initiateIntentToDoSpecialPower(r81Template(o[1] & 255), objB, o[3] ? &pos : 0, 0, 0) ? 1 : 0;
				break;
			}
			case 7: if (ma) res = ma->isPowerCurrentlyInUse() ? 1 : 0; break;
			case 8: if (ma) {
				Object *found = objB ? ma->findSpecialObjectWithProducerID(objB) : 0;
				res = ma->getSpecialObjectCount() | ma->getSpecialObjectMax() << 8 | (found ? (unsigned)found->m_oid : 0u) << 16;
				break;
			}
			case 9: if (ma) res = ma->isActive() ? 1 : 0; break;
			case 10: if (ma) res = (unsigned)ma->getSpecialPowerType(); break;
			case 11: if (o[2] >= 2 && objB) g_r81Gone.insert((int)o[2]); break;   /* (an object not yet made is not gone) */
			}
		}
		CATCH_ALL(st, &c2, m2s, 16)
		g_r81Updating = 0;
		putI32(st);
		putU32(res);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putModule(m);
		putModule(m2);
		std::vector<Object *> all;
		for (int k = 0; k < 4; ++k) all.push_back(g_r81Objs[k]);
		for (size_t k = 0; k < g_r81Made.size(); ++k) all.push_back(g_r81Made[k].obj);
		unsigned found = 0;
		for (size_t k = 0; k < all.size(); ++k) if (r81Find((ObjectID)all[k]->m_oid)) ++found;
		putU32(found);
		for (size_t k = 0; k < all.size(); ++k) if (r81Find((ObjectID)all[k]->m_oid)) putObject(all[k]);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modDestroyHook = 0;
	g_modFindHook = 0;
	g_findUpdateHook = 0;
	g_teamRelHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_closestAtHook = 0;
	g_distDcHook = 0;
	g_distPosDcHook = 0;
	g_r74BoneHook = 0;
	g_boobyHook = 0;
	g_defectHook = 0;
	g_mcHook = 0;
	g_r28ThingPose = false;
	g_perUnitSoundHook = 0;
	g_spLocal = 0;
	g_r81Scan.clear();
	g_ahRange.clear();
	g_hdScan.clear();
	/* (the destructor runs onExit(TRUE), which reads the template's type: a module of no template is left, as upstream's would crash) */
	if (((R81SpecialAbilityUpdateModuleData *)d)->m_specialPowerTemplate) delete m;
	if (m2 && ((R81SpecialAbilityUpdateModuleData *)d2)->m_specialPowerTemplate) delete m2;
	delete power;
	delete unitPhys;
	for (int k = 0; k < 4; ++k) {
		Object *x = g_r81Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0; x->m_contain = 0; x->m_ai = 0; x->m_stealth = 0; x->m_xp = 0;
		x->m_r50Physics = 0; x->m_slotWeapon[0] = x->m_slotWeapon[1] = x->m_slotWeapon[2] = 0;
		delete x;
		g_r81Objs[k] = 0;
	}
	for (size_t k = 0; k < g_r81Made.size(); ++k) {
		R81Made &mm = g_r81Made[k];
		Object *x = mm.obj;
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_drawable = 0; x->m_xp = 0; x->m_r50Physics = 0;
		delete x;
		delete mm.draw;
		delete mm.laser;
		delete mm.bomb;
		delete mm.phys;
		delete mm.xp;
		delete[] mm.list;
	}
	g_r81Made.clear();
	g_r81Gone.clear();
	delete d;
	delete d2;
	g_r81PhysData = 0;
	TheGameLogic = 0;
	TheAudio = 0;
	ThePartitionManager = 0;
	ThePlayerList = 0;
	TheEva = 0;
	TheRadar = 0;
	TheInGameUI = 0;
	TheGameText = 0;
	TheGlobalData = 0;
	TheParticleSystemManager = 0;
	return 0;
}

int ref_l9r81_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r81RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
