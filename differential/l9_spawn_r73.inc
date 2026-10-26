/* Differential-test driver for SpawnBehavior (p1w13-sonnet, batch R73; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpawnBehavior.h
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SpawnBehavior.cpp   (both #included, unchanged)
** in wrap_module.cpp's unit, at its end (batch R50's destroyObject rename). The module build has a stand-in SpawnBehaviorInterface (for the
** modules that call one; module_prelude.h names it ModTestSpawnBehaviorInterface), so while the two files are included the real interface is
** R73SpawnBehaviorInterface and BehaviorModule's getSpawnBehaviorInterface, which returns the stand-in's type, is r73GetSpawnBehaviorInterface
** (a new virtual of the class, which nothing here calls). On the module build's stand-ins: the spawner (id 1: a team of three, each with its
** player, player 0 the neutral one; maybe an AI whose last command source is the test's, an exit interface (an update module whose four exit
** calls are lines, reserveDoorForExit the test's door), a body (batch M's MBody, setInitialHealth a line), a drawable (selected or not);
** reconstructing, under construction, sold or not; its producer the barracks or none; its veterancy; a position), the barracks (id 2: a
** structure or not, maybe an exit interface), the spawn (the thing factory: SpawnA has a slaved update (its four calls lines, isSelfTasking
** the test's), an AI, a body, a drawable and a stealth update (allowedToStealth the test's); SpawnB an AI, a body and a drawable; SpawnC
** nothing; newObject makes ids 3, 4, ...; each spawn's health, maximum health, selection, veterancy, death, position, NO_ATTACK and the two
** attack answers the test's), the player's onUnitCreated (a line), the AI's orders, aiIdle (the active-body driver's line: no source),
** setDisabledUntil, clearDisabled, setStatus, scoreTheKill, the experience tracker's setVeterancyLevel, getAbleToAttackSpecificObject (a
** hook on the active-body driver's) and getAbleToUseWeaponAgainstTarget (lines), the message stream and the UI's selection (batch R5's
** lines), getDistanceSquared (lines; FROM_CENTER_2D's 2D distance), destroyObject (a line), the REAL Object::kill, DieMuxData, UpdateModule,
** INI and Xfer; the game logic (its frame). */
#undef SpawnBehaviorInterface
#define SpawnBehaviorInterface R73SpawnBehaviorInterface
#define getSpawnBehaviorInterface r73GetSpawnBehaviorInterface
#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc: the inline one loses to other units' out-of-line ones) */
#include "../upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpawnBehavior.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SpawnBehavior.cpp"
#undef destroyObject
#undef getSpawnBehaviorInterface
#undef SpawnBehaviorInterface
#define SpawnBehaviorInterface ModTestSpawnBehaviorInterface

extern CanAttackResult (*g_canAttackHook)(const Object *, AbleToAttackType, const Object *, CommandSourceType);
extern Real (*g_distDcHook)(const Object *, const Object *, DistanceCalculationType);
extern Real (*g_distPosDcHook)(const Object *, const Coord3D *, DistanceCalculationType);

namespace {

ModuleData *l9r73Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = SpawnBehavior::friend_newModuleData(&ini);
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

/* an exit interface: an update module whose exit calls are lines and whose door is the test's */
class R73Exit : public UpdateModule, public ExitInterface
{
public:
	R73Exit(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual ExitInterface *getUpdateExitInterface() { return this; }
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual Bool isExitBusy() const { return FALSE; }
	virtual ExitDoorType reserveDoorForExit( const ThingTemplate *, Object * ) { ulog("reservedoor %d", (int)getObject()->getID()); return (ExitDoorType)m_door; }
	virtual void exitObjectViaDoor( Object *newObj, ExitDoorType exitDoor ) { ulog("exitdoor %d %d %d", (int)getObject()->getID(), (int)newObj->getID(), (int)exitDoor); }
	virtual void exitObjectByBudding( Object *newObj, Object *budHost )
	{
		ulog("exitbud %d %d %d", (int)getObject()->getID(), (int)newObj->getID(), budHost ? (int)budHost->getID() : 0);
	}
	virtual void unreserveDoorForExit( ExitDoorType exitDoor ) { ulog("unreservedoor %d %d", (int)getObject()->getID(), (int)exitDoor); }
	virtual void setRallyPoint( const Coord3D * ) {}
	virtual const Coord3D *getRallyPoint( void ) const { return 0; }
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *) {}
	virtual void loadPostProcess() {}
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }   /* (no lookup by name reaches it) */
	int m_door = 0;
};
/* a slaved update: its four calls are lines, isSelfTasking the test's */
class R73Slaved : public UpdateModule, public SlavedUpdateInterface
{
public:
	R73Slaved(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual SlavedUpdateInterface *getSlavedUpdateInterface() { return this; }
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual ObjectID getSlaverID() const { return INVALID_ID; }
	virtual void onEnslave( const Object *slaver ) { ulog("enslave %d %d", (int)getObject()->getID(), (int)slaver->getID()); }
	virtual void onSlaverDie( const DamageInfo * ) { ulog("slaverdie %d", (int)getObject()->getID()); }
	virtual void onSlaverDamage( const DamageInfo *info )
	{
		ulog("slaverdamage %d %d %d %d %d %08x %08x %08x %d %d %d", (int)getObject()->getID(), (int)info->in.m_sourceID, (int)info->in.m_damageType,
			(int)info->in.m_deathType, (int)info->in.m_damageFXOverride, realBits(info->in.m_amount), realBits(info->out.m_actualDamageDealt),
			realBits(info->out.m_actualDamageClipped), info->out.m_noEffect ? 1 : 0, info->in.m_kill ? 1 : 0, (int)info->in.m_damageStatusType);
	}
	virtual Bool isSelfTasking() const { return m_selfTasking; }
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *) {}
	virtual void loadPostProcess() {}
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }
	Bool m_selfTasking = false;
};
/* the spawner's body: setInitialHealth a line */
class R73Body : public MBody
{
public:
	void setInitialHealth( Int initialPercent ) { ulog("initialhealth %d %d", (int)m_owner->m_oid, (int)initialPercent); }
};

/* one object of the scene: the object and its parts */
struct R73Obj {
	Object *obj = 0;
	AIUpdateInterface ai;
	R73Body body;
	Drawable draw;
	ExperienceTracker xp;
	StealthUpdate stealth;
	R73Exit *exit = 0;
	R73Slaved *slaved = 0;
	BehaviorModule *behaviors[4] = { 0, 0, 0, 0 };
	int able = 0, ableWeapon = 0;
};

std::vector<unsigned char> g_r73Saved;
std::vector<R73Obj *> g_r73Objs;   /* [0] the spawner, [1] the barracks, then the spawn in the order they were made */
UpdateModuleData *g_r73ModuleData = 0;
ThingTemplate g_r73Tmpl[3];   /* (named: the xfer saves the spawn template's name) */
const char *const g_r73TmplNames[3] = { "SpawnA", "SpawnB", "SpawnC" };
Bool g_r73Stealthy[64];
Real r73F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
R73Obj *r73Of(const Object *o) { for (size_t i = 0; i < g_r73Objs.size(); ++i) if (g_r73Objs[i]->obj == o) return g_r73Objs[i]; return 0; }
Object *r73Find(ObjectID id)
{
	for (size_t i = 0; i < g_r73Objs.size(); ++i) if (g_r73Objs[i]->obj->m_oid == id) return g_r73Objs[i]->obj;
	return 0;
}
void r73Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
int r73Code(const ThingTemplate *t) { for (int i = 0; i < 3; ++i) if (t == &g_r73Tmpl[i]) return i + 1; return 0; }
const ThingTemplate *r73FindTemplate(const AsciiString &name)
{
	for (int i = 0; i < 3; ++i) if (strcmp(name.str(), g_r73TmplNames[i]) == 0) return &g_r73Tmpl[i];
	return 0;
}
R73Obj *r73Make(ObjectID id)
{
	R73Obj *s = new R73Obj;
	s->obj = newObject();
	s->obj->m_oid = id;
	s->obj->m_behaviors = s->behaviors;
	s->xp.m_owner = s->obj;
	s->obj->m_xp = &s->xp;
	s->ai.m_owner = s->obj;
	s->body.m_owner = s->obj;
	s->draw.m_ownerObject = s->obj;
	s->stealth.m_owner = s->obj;
	g_r73Objs.push_back(s);
	return s;
}
unsigned g_r73NextId = 3;
Object *r73NewObject(const ThingTemplate *tmplate, Team *team)
{
	int code = r73Code(tmplate);
	R73Obj *s = r73Make((ObjectID)g_r73NextId++);
	Object *o = s->obj;
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	int nb = 0;
	if (code == 1) {
		s->slaved = new R73Slaved(o, g_r73ModuleData);
		s->behaviors[nb++] = s->slaved;
		o->m_stealth = &s->stealth;
	}
	s->body.m_health = s->body.m_max = 100.0f;   /* (a new spawn's health: the Rust side's after the op) */
	if (code == 1 || code == 2) {
		o->m_ai = &s->ai;
		o->m_body = &s->body;
		o->m_drawable = &s->draw;
	}
	return o;
}
CanAttackResult r73CanAttack(const Object *obj, AbleToAttackType t, const Object *target, CommandSourceType src)
{
	ulog("canattack %d %d %d %d", (int)obj->m_oid, (int)t, target ? (int)target->m_oid : 0, (int)src);
	R73Obj *s = r73Of(obj);
	return (CanAttackResult)(s ? s->able : 0);
}
Real r73Dist(const Object *a, const Object *b, DistanceCalculationType dc)
{
	ulog("distsq %d %d %d", (int)a->m_oid, (int)b->m_oid, (int)dc);
	Real dx = b->m_pos.x - a->m_pos.x, dy = b->m_pos.y - a->m_pos.y;
	return dx * dx + dy * dy;
}
Real r73DistPos(const Object *a, const Coord3D *pos, DistanceCalculationType dc)
{
	Real dx = pos->x - a->m_pos.x, dy = pos->y - a->m_pos.y;
	return dx * dx + dy * dy;
}

}  // namespace

void AIUpdateInterface::aiForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource )
{
	ulog("aiforceattack %d %d %d %d", (int)m_owner->m_oid, victim ? (int)victim->m_oid : 0, (int)maxShotsToFire, (int)cmdSource);
}
CanAttackResult Object::getAbleToUseWeaponAgainstTarget( AbleToAttackType attackType, const Object *victim, const Coord3D *pos, CommandSourceType commandSource ) const
{
	if (pos) ulog("canuseweapon %d %d %d %08x %08x %08x %d", (int)m_oid, (int)attackType, victim ? (int)victim->m_oid : 0, realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)commandSource);
	else ulog("canuseweapon %d %d %d null %d", (int)m_oid, (int)attackType, victim ? (int)victim->m_oid : 0, (int)commandSource);
	R73Obj *s = r73Of(this);
	return (CanAttackResult)(s ? s->ableWeapon : 0);
}
void Player::onUnitCreated( Object *factory, Object *unit ) { ulog("unitcreated %d %d %d", (int)m_idx, (int)factory->m_oid, (int)unit->m_oid); }
Bool StealthUpdate::allowedToStealth( Object *stealthOwner ) const { return g_r73Stealthy[stealthOwner->m_oid & 63]; }

extern "C" {

/* Read `filename` (block `D`) as the data of a SpawnBehavior and make the module on the spawner (id 1) at frame cfg[0]. `cfg` (32 words; 3-9
** also settable by op 2): 2 the spawner's parts (read once): bit 0 an AI, 1 an exit interface, 2 a body, 3 a drawable; 3 the spawner: bits
** 0-1 its team (0 none, else 1-3: player team - 1, player 0 the neutral one), 2 reconstructing, 3 under construction, 4 sold, 5 its producer
** the barracks, 6 selected, 8-9 its veterancy, 12-14 its AI's last command source; 4 its exit's door; 5 the barracks: bit 0 a structure, 1
** an exit interface (read once); 6 the barracks' door; 7-8 the spawner's x, y (float bits). The data dump goes first, then the constructor's
** state. Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 onDie(death type a, source b)  4
** onDamage(amount a bits, source b)  5 onSpawnDeath(id a, source b)  6 onDelete  7 xfer save  8 xfer load of the last save  9 xfer CRC
** 10 spawn a (the a-th made) field b = c: 0 health, 1 maximum health (float bits), 2 selected, 3 veterancy, 4 effectively dead, 5 x, 6 y
** (float bits), 7 NO_ATTACK, 8 the attack answer, 9 the weapon answer, 10 self-tasking, 11 allowed to stealth  11 the interface's call a
** (see below) with b, c, d. Each op appends: the CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, the state
** (as the Rust state_words, and the spawn template's name), the spawner's health box offset and status MASKED, the spawn made. */
static int l9r73RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r73Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r73Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const SpawnBehaviorModuleData *sd = (const SpawnBehaviorModuleData *)d;
		const Int ints[4] = { sd->m_spawnNumberData, sd->m_spawnStartNumberData, sd->m_spawnReplaceDelayData, sd->m_initialBurst };
		for (int i = 0; i < 4; ++i) putI32(ints[i]);
		const Bool bools[6] = { sd->m_isOneShotData, sd->m_canReclaimOrphans, sd->m_aggregateHealth, sd->m_exitByBudding, sd->m_spawnedRequireSpawner,
			sd->m_slavesHaveFreeWill };
		for (int i = 0; i < 6; ++i) putU32(bools[i] ? 1 : 0);
		for (int i = 0; i < DAMAGE_NUM_TYPES; ++i) if (getDamageTypeFlag(sd->m_damageTypesToPropagateToSlaves, (DamageType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		putU32((unsigned)sd->m_spawnTemplateNameData.size());
		for (size_t i = 0; i < sd->m_spawnTemplateNameData.size(); ++i) putStr(sd->m_spawnTemplateNameData[i].str(), sd->m_spawnTemplateNameData[i].getLength());
	}
	/* (no template name: the constructor's *begin() is undefined, and the Rust side excludes the case; not run) */
	if (((const SpawnBehaviorModuleData *)d)->m_spawnTemplateNameData.empty()) { delete d; *len = finish(buf, cap); return 0; }
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_r73NextId = 3;
	for (int i = 0; i < 3; ++i) g_r73Tmpl[i].m_name = AsciiString(g_r73TmplNames[i]);
	g_r73ModuleData = new UpdateModuleData;
	memset(g_r73Stealthy, 0, sizeof(g_r73Stealthy));
	Player players[4];
	Team teams[3];
	for (int p = 0; p < 4; ++p) players[p].m_idx = p;
	for (int t = 0; t < 3; ++t) {
		teams[t].m_dummy = t + 1;
		teams[t].m_controller = &players[t];
	}
	R73Obj *me = r73Make((ObjectID)1);
	R73Obj *barracks = r73Make((ObjectID)2);
	Object *meObj = me->obj;
	meObj->m_ai = (cfg[2] & 1) ? &me->ai : 0;
	int nb = 0;
	if (cfg[2] >> 1 & 1) { me->exit = new R73Exit(meObj, g_r73ModuleData); }
	meObj->m_body = (cfg[2] >> 2 & 1) ? &me->body : 0;
	meObj->m_drawable = (cfg[2] >> 3 & 1) ? &me->draw : 0;
	if (cfg[5] >> 1 & 1) { barracks->exit = new R73Exit(barracks->obj, g_r73ModuleData); barracks->behaviors[0] = barracks->exit; }
	if (cfg[5] & 1) { barracks->obj->m_kindOf.set(KINDOF_STRUCTURE); barracks->obj->m_tmpl.m_kindOf = barracks->obj->m_kindOf; }
	auto scene = [&]() {
		unsigned f = cfg[3];
		unsigned t = f & 3;
		meObj->m_team = t ? &teams[t - 1] : 0;
		meObj->m_player = t ? &players[t - 1] : 0;
		meObj->m_status.clear();
		if (f >> 2 & 1) meObj->m_status.set(OBJECT_STATUS_RECONSTRUCTING);
		if (f >> 3 & 1) meObj->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		if (f >> 4 & 1) meObj->m_status.set(OBJECT_STATUS_SOLD);
		meObj->m_producerID = (f >> 5 & 1) ? (ObjectID)2 : INVALID_ID;
		me->draw.m_selected = (f >> 6 & 1) != 0;
		meObj->m_level = (VeterancyLevel)(f >> 8 & 3);
		me->ai.m_lastCmdSource = (Int)(f >> 12 & 7);
		if (me->exit) me->exit->m_door = (int)cfg[4];
		if (barracks->exit) barracks->exit->m_door = (int)cfg[6];
		meObj->m_pos.x = r73F(cfg[7]); meObj->m_pos.y = r73F(cfg[8]); meObj->m_pos.z = 0.0f;
	};
	scene();
	g_modFindHook = r73Find;
	g_modDestroyHook = r73Destroy;
	g_findTemplateHook = r73FindTemplate;
	g_newObjectHook = r73NewObject;
	g_canAttackHook = r73CanAttack;
	g_distDcHook = r73Dist;
	g_distPosDcHook = r73DistPos;
	g_setStatusHook = r57SetStatus;
	g_r26DrawPool = true;   /* (ctDrawOwner names a drawable's owner) */
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	SpawnBehavior *m = 0;
	try { m = newInstance(SpawnBehavior)(meObj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	me->behaviors[nb++] = m;
	if (me->exit) me->behaviors[nb++] = me->exit;
	auto putState = [&]() {
		putI32(m->m_oneShotCountdown);
		putI32(m->m_framesToWait);
		putI32(m->m_firstBatchCount);
		putU32(m->m_active ? 1 : 0);
		putU32(m->m_aggregateHealth ? 1 : 0);
		putU32(m->m_initialBurstTimesInited ? 1 : 0);
		putI32(m->m_spawnCount);
		putU32(m->m_selfTaskingSpawnCount);
		putU32(m->m_initialBurstCountdown);
		const SpawnBehaviorModuleData *sd = (const SpawnBehaviorModuleData *)d;
		putU32((unsigned)(m->m_templateNameIterator - sd->m_spawnTemplateNameData.begin()));
		putU32((unsigned)m->m_replacementTimes.size());
		for (std::list<Int>::const_iterator it = m->m_replacementTimes.begin(); it != m->m_replacementTimes.end(); ++it) putI32(*it);
		putU32((unsigned)m->m_spawnIDs.size());
		for (std::list<ObjectID>::const_iterator it = m->m_spawnIDs.begin(); it != m->m_spawnIDs.end(); ++it) putU32((unsigned)*it);
		AsciiString name = m->m_spawnTemplate ? AsciiString(g_r73TmplNames[r73Code(m->m_spawnTemplate) - 1]) : AsciiString();
		putStr(name.str(), name.getLength());
		for (int k = 0; k < 3; ++k) putU32(realBits((&meObj->m_healthBoxOffset.x)[k]));
		putU32(meObj->m_status.test(OBJECT_STATUS_MASKED) ? 1 : 0);
		putU32((unsigned)(g_r73Objs.size() - 2));
		for (size_t k = 2; k < g_r73Objs.size(); ++k) putU32((unsigned)g_r73Objs[k]->obj->m_producerID);   /* (each spawn's producer) */
	};
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 3 && o[1] <= 9) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: {
				DamageInfo info;
				info.in.m_deathType = (DeathType)o[1];
				info.in.m_sourceID = (ObjectID)o[2];
				m->onDie(&info);
				break;
			}
			case 4: {
				DamageInfo info;
				info.in.m_amount = r73F(o[1]);
				info.in.m_sourceID = (ObjectID)o[2];
				m->onDamage(&info);
				break;
			}
			case 5: {
				DamageInfo info;
				info.in.m_sourceID = (ObjectID)o[2];
				m->onSpawnDeath((ObjectID)o[1], &info);
				break;
			}
			case 6: m->onDelete(); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r73Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r73Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 10: {
				size_t k = (size_t)o[1] + 2;
				if (k >= g_r73Objs.size()) break;
				R73Obj *s = g_r73Objs[k];
				Object *so = s->obj;
				switch (o[2]) {
				case 0: s->body.m_health = r73F(o[3]); break;
				case 1: s->body.m_max = r73F(o[3]); break;
				case 2: s->draw.m_selected = o[3] != 0; break;
				case 3: so->m_level = (VeterancyLevel)(o[3] & 3); break;
				case 4: so->m_effDead = o[3] != 0; break;
				case 5: so->m_pos.x = r73F(o[3]); break;
				case 6: so->m_pos.y = r73F(o[3]); break;
				case 7: if (o[3]) so->m_status.set(OBJECT_STATUS_NO_ATTACK); else so->m_status.set(OBJECT_STATUS_NO_ATTACK, 0); break;
				case 8: s->able = (int)o[3]; break;
				case 9: s->ableWeapon = (int)o[3]; break;
				case 10: if (s->slaved) s->slaved->m_selfTasking = o[3] != 0; break;
				case 11: g_r73Stealthy[so->m_oid & 63] = o[3] != 0; break;
				}
				break;
			}
			case 11: {
				Coord3D pos;
				pos.x = r73F(o[2]); pos.y = r73F(o[3]); pos.z = 0.0f;
				switch (o[1]) {
				case 0: res = m->maySpawnSelfTaskAI(r73F(o[2])) ? 1 : 0; break;
				case 1: { Object *c = m->getClosestSlave(&pos); res = c ? (unsigned)c->m_oid : 0; break; }
				case 2: m->orderSlavesToAttackTarget(r73Find((ObjectID)o[2]), (Int)o[3], (CommandSourceType)o[4]); break;
				case 3: m->orderSlavesToAttackPosition(&pos, 5, (CommandSourceType)o[4]); break;
				case 4: res = (unsigned)m->getCanAnySlavesAttackSpecificTarget((AbleToAttackType)o[2], r73Find((ObjectID)o[3]), (CommandSourceType)o[4]); break;
				case 5: {
					Coord3D p2; p2.x = r73F(o[3]); p2.y = 0.0f; p2.z = 0.0f;
					res = (unsigned)m->getCanAnySlavesUseWeaponAgainstTarget((AbleToAttackType)o[2], (o[3] & 1) ? r73Find((ObjectID)2) : 0, (o[3] & 2) ? &p2 : 0,
						(CommandSourceType)o[4]);
					break;
				}
				case 6: res = m->canAnySlavesAttack() ? 1 : 0; break;
				case 7: m->orderSlavesToGoIdle((CommandSourceType)o[2]); break;
				case 8: m->orderSlavesDisabledUntil((DisabledType)o[2], o[3]); break;
				case 9: m->orderSlavesToClearDisabled((DisabledType)o[2]); break;
				case 10: m->giveSlavesStealthUpgrade(o[2] != 0); break;
				case 11: res = m->areAllSlavesStealthed() ? 1 : 0; break;
				case 12: m->revealSlaves(); break;
				case 13: res = m->doSlavesHaveFreedom() ? 1 : 0; break;
				case 14: m->stopSpawning(); break;
				case 15: m->startSpawning(); break;
				}
				break;
			}
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
	}
	*len = finish(buf, cap);
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_canAttackHook = 0;
	g_distDcHook = 0;
	g_distPosDcHook = 0;
	g_setStatusHook = 0;
	g_r26DrawPool = false;
	delete m;
	delete d;
	for (size_t i = 0; i < g_r73Objs.size(); ++i) {
		R73Obj *s = g_r73Objs[i];
		Object *x = s->obj;
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_ai = 0; x->m_body = 0; x->m_drawable = 0; x->m_xp = 0; x->m_stealth = 0;
		delete x;
		delete s->exit;
		delete s->slaved;
		delete s;
	}
	g_r73Objs.clear();
	delete g_r73ModuleData;
	g_r73ModuleData = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r73_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r73RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
