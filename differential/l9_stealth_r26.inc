/* Differential-test driver for StealthUpdate (p1w13-sonnet, batch R26; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StealthUpdate.h       (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthUpdate.cpp (#included, unchanged)
** in wrap_module.cpp's unit, at its end. The unit already has a stand-in StealthUpdate (the containers' drivers), so the real class is compiled as
** R26StealthUpdate, and the two accessors the file uses on an Object, getStealth and getPhysics, are the shim Object's r26GetStealth and r26GetPhysics
** while it is included (a physics behaviour of this driver's, whose velocity is the test's and whose resetDynamicPhysics is a line).
**
** The scene, on the module build's stand-ins: the module's object (id 1: kinds, status bits, the script's unstealthed bit, a controlling player of
** three or none, maybe a drawable whose calls are lines and whose selection and hidden state the test gives, maybe an AI with a current victim and a
** last command source, maybe a physics behaviour, maybe a body with a last damage, maybe a contain module (rider change or not, its rider, its
** passengers, whether they attack), maybe a spawn behaviour whose slaves stealth or not, the three weapon slots and their last shots, maybe contained
** by object 2, whose contain module is garrisonable or not and logs recalcApparentControllingPlayer); object 3, a rider with its own
** R26StealthUpdate (from a second block D); object 4, a disguise target; objects 5 and 6, the players 1 and 2's, with an AI whose
** wakeUpAndAttemptToTarget is a line, a position and a vision range; object 7, a black market (kinds and states) in the object's player's list;
** three players (their default teams, colours, active or not, the local one, their relationships to each other's default team and the object's
** team's to them); the radar, the audio, the FX, the in-game UI, the control bar, the game client and the thing factory (a pool that gives a destroyed
** drawable back, as the game's memory pool does), the partition manager's distance (the ActiveBody driver's table), the game logic (its frame, its
** wake calls applied as GameLogic::friend_awakenUpdateModule applies them), and the REAL UpdateModule, INI, Xfer and random values (logic and
** client). */

/* a physics behaviour: PhysicsUpdate.cpp's getVelocityMagnitude (lines 949-956, without the cache) on the test's velocity */
class R26PhysicsBehavior
{
public:
	Real getVelocityMagnitude() const { return (Real)sqrtf( sqr(m_vel.x) + sqr(m_vel.y) + sqr(m_vel.z) ); }
	void resetDynamicPhysics() { ulog("resetphysics %d", m_owner); }
	Coord3D m_vel;
	int m_owner = 0;
};

#define DEFINE_STEALTHLEVEL_NAMES
#define StealthUpdate R26StealthUpdate
#define StealthUpdateModuleData R26StealthUpdateModuleData
#define PhysicsBehavior R26PhysicsBehavior
#define getStealth r26GetStealth
#define getPhysics r26GetPhysics
#define getStealthUpdateModuleData getR26StealthUpdateModuleData   /* the name MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA makes of the renamed class */
#include "../upstream/GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StealthUpdate.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthUpdate.cpp"
#undef getStealthUpdateModuleData
#undef getPhysics
#undef getStealth
#undef PhysicsBehavior
#undef StealthUpdateModuleData
#undef StealthUpdate

/* Eva.cpp lines 41-102 (the names) and 322-338 (nameToMessage), verbatim apart from the stand-in's count, and parseEvaMessageFromIni (lines
** 554-566): the stand-in's EvaMessage keeps the index */
static const char *const g_r26EvaNames[] =
{
	"LOWPOWER", "INSUFFICIENTFUNDS",
	"SUPERWEAPONDETECTED_OWN_PARTICLECANNON", "SUPERWEAPONDETECTED_OWN_NUKE", "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
	"SUPERWEAPONDETECTED_ALLY_PARTICLECANNON", "SUPERWEAPONDETECTED_ALLY_NUKE", "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",
	"SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON", "SUPERWEAPONDETECTED_ENEMY_NUKE", "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",
	"SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON", "SUPERWEAPONLAUNCHED_OWN_NUKE", "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM",
	"SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON", "SUPERWEAPONLAUNCHED_ALLY_NUKE", "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM",
	"SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON", "SUPERWEAPONLAUNCHED_ENEMY_NUKE", "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM",
	"SUPERWEAPONREADY_OWN_PARTICLECANNON", "SUPERWEAPONREADY_OWN_NUKE", "SUPERWEAPONREADY_OWN_SCUDSTORM",
	"SUPERWEAPONREADY_ALLY_PARTICLECANNON", "SUPERWEAPONREADY_ALLY_NUKE", "SUPERWEAPONREADY_ALLY_SCUDSTORM",
	"SUPERWEAPONREADY_ENEMY_PARTICLECANNON", "SUPERWEAPONREADY_ENEMY_NUKE", "SUPERWEAPONREADY_ENEMY_SCUDSTORM",
	"BUILDINGLOST", "BASEUNDERATTACK", "ALLYUNDERATTACK", "BEACONDETECTED",
	"ENEMYBLACKLOTUSDETECTED", "ENEMYJARMENKELLDETECTED", "ENEMYCOLONELBURTONDETECTED",
	"OWNBLACKLOTUSDETECTED", "OWNJARMENKELLDETECTED", "OWNCOLONELBURTONDETECTED",
	"UNITLOST", "GENERALLEVELUP", "VEHICLESTOLEN", "BUILDINGSTOLEN", "CASHSTOLEN", "UPGRADECOMPLETE", "BUILDINGBEINGSTOLEN", "BUILDINGSABOTAGED",
	"SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER", "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER", "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER",
	"SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK", "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK", "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK",
	"EVA_INVALID",
};
void Eva::parseEvaMessageFromIni( INI *ini, void *instance, void *store, const void *userData )
{
	const char *token = ini->getNextToken();
	EvaMessage message = EVA_Invalid;
	for (Int i = 0; i < (Int)(sizeof(g_r26EvaNames) / sizeof(g_r26EvaNames[0])) - 1; ++i) {
		if (AsciiString(token).compareNoCase(g_r26EvaNames[i]) == 0) { message = (EvaMessage)i; break; }
	}
	if ( message == EVA_Invalid )
	{
		// debug message already displayed
		throw ERROR_BAD_INI;
	}
	*((EvaMessage *)store) = message;
}

/* the calls of the stand-ins this file adds, lines of the effect log */
void Drawable::setEffectiveOpacity( Real pulseFactor, Real explicitOpacity ) { ulog("opacity %08x %08x", realBits(pulseFactor), realBits(explicitOpacity)); }
void Drawable::setSecondMaterialPassOpacity( Real op ) { ulog("secondpass %d %08x", m_ownerObject ? (int)m_ownerObject->m_oid : 0, realBits(op)); }
void Drawable::setStealthLook( StealthLookType look ) { ulog("look %d", (int)look); }
void Drawable::setModelConditionFlags( const ModelConditionFlags & ) {}
void Drawable::updateDrawable() {}
void GameLogic::bindObjectAndDrawable( Object *obj, Drawable *draw ) { ulog("bind %d", (int)obj->m_oid); draw->m_ownerObject = obj; obj->m_drawable = draw; }
void SpawnBehaviorInterface::revealSlaves() { ulog("revealslaves %d", m_ownerId); }
void Object::forceRefreshSubObjectUpgradeStatus() { ulog("refreshsub %d", (int)m_oid); }
int g_r26Disguised[3];
void PowerAcademyStats::recordVehicleDisguised() { ++g_r26Disguised[((Player *)this)->m_idx]; }
/* Object.cpp lines 1645-1690, verbatim */
Color Object::getIndicatorColor() const
{
	if (m_indicatorColor == 0)
	{
		const Team *myTeam = getTeam();
		if (myTeam)
		{
			const Player* p = myTeam->getControllingPlayer();
			if (p)
			{
				return p->getPlayerColor();
			}
		}
		return GameMakeColor(0, 0, 0, 255);
	}
	return m_indicatorColor;
}
Color Object::getNightIndicatorColor() const
{
	if (m_indicatorColor == 0)
	{
		const Team *myTeam = getTeam();
		if (myTeam)
		{
			const Player* p = myTeam->getControllingPlayer();
			if (p)
			{
				return p->getPlayerNightColor();
			}
		}
		return GameMakeColor(0, 0, 0, 255);
	}
	return m_indicatorColor;
}

namespace {

ModuleData *l9r26Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = R26StealthUpdate::friend_newModuleData(&ini);
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

/* a contain module: the answers StealthUpdate asks for, from the test; recalcApparentControllingPlayer is a line */
class R26Contain : public UpgContain
{
public:
	virtual Bool isGarrisonable() const { return m_garrisonable; }
	virtual Bool isRiderChangeContain() const { return m_riderChange; }
	virtual const Object *friend_getRider() const { return m_rider; }
	virtual const ContainedItemsList *getContainedItemsList() const { return &m_list; }
	virtual Bool isPassengerAllowedToFire(ObjectID) const { return m_mayFire; }
	virtual Bool isAnyRiderAttacking() const { return m_attacking; }
	virtual void recalcApparentControllingPlayer() { ulog("recalcapparent %d", m_owner); }
	Bool m_garrisonable = false, m_riderChange = false, m_mayFire = false, m_attacking = false;
	const Object *m_rider = 0;
	ContainedItemsList m_list;
	int m_owner = 0;
};

/* a body whose last damage is the test's */
class R26Body : public MBody
{
public:
	const DamageInfo *getLastDamageInfo() const { return &m_info; }
	UnsignedInt getLastDamageTimestamp() const { return m_stamp; }
	DamageInfo m_info;
	UnsignedInt m_stamp = 0;
};

std::vector<unsigned char> g_r26Saved;
/* the status bits the test sets and dumps, by bit (the same list in the Rust test) */
const ObjectStatusTypes g_r26Status[8] = { OBJECT_STATUS_IS_FIRING_WEAPON, OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_STEALTHED,
	OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD };
const char *const g_r26TmplNames[7] = { "R26Me", "R26Box", "R26Rider", "R26Tank", "R26Enemy", "R26Enemy", "R26Market" };
Team g_r26Teams[3];
int g_r26TeamRel[3];   /* the relationship of the object's team to player j's default team (0 enemies, 1 neutral, 2 allies) */
Relationship r26TeamRel(const Team *, const Team *b)
{
	int t = (int)(size_t)b;
	if (t >= 101 && t <= 103) return (Relationship)g_r26TeamRel[t - 101];
	return NEUTRAL;
}
Object *g_r26Objs[7];
const ThingTemplate *r26FindTemplate(const AsciiString &name)
{
	for (int k = 0; k < 7; ++k) if (strcmp(name.str(), g_r26TmplNames[k]) == 0) return g_r26Objs[k]->getTemplate();
	return 0;
}
bool g_r26InConstruction = false;
/* GameLogic::friend_awakenUpdateModule (GameLogic.cpp lines 2983-3046) for the driver's modules: the call made by the module being updated is
** ignored, an equal frame returns early, and so does the "already awake" case; otherwise the frame is set (during the constructors the objects are not
** in the list yet, which sets it the same way) */
UpdateModule *g_r26Updating = 0;
void r26Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r26Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of the object's StealthUpdate and `filename2` as the rider's, and make the two modules on objects 1 and 3 at
** frame cfg[0] (logic seed cfg[1], client seed cfg[2]). `cfg` (20 words, each of 3-19 also settable by op 6):
**   3 the object: 0 drawable, 1 AI, 2 physics, 3 body, 4 contain module, 5 spawn behaviour, 6 KINDOF_MINE, 7 KINDOF_SPAWNS_ARE_THE_WEAPONS,
**     8 contained by object 2, 9-10 its player (3 none), 11 effectively dead, 12 script-unstealthed, 13 drawable selected, 14 drawable hidden,
**     15 the rider (object 3) in its contain list, 16 the rider its friend_getRider
**   4 the object's status bits (by g_r26Status)   5 the AI: 0-2 the current victim (0 none, else an object), 3-5 the last command source
**   6 the velocity's x (float bits)   7 the body's last damage frame   8 its damage type
**   9 the weapons: 0-2 a weapon in the slot, 8-15, 16-23, 24-31 their last shot frames
**   10 the contain module: 0 rider change, 1 passengers may fire, 2 a rider attacks   11 object 2's contain module: 0 garrisonable
**   12 the spawn behaviour: 0 all slaves stealthed
**   13 relationships: 0-5 the object's team to each player's default team, 8-25 player i to player j's default team (at 8 + 2 (3i + j))
**   14 0-1 the local player (3 none), 4-6 player k inactive, 8 night
**   15 object 7: 0 in the object's player's list, 1 effectively dead, 2 under construction, 3 sold, 4 KINDOF_FS_BLACK_MARKET
**   16 object 4: 0-1 its player (3 none)   17 objects 5 and 6: x and vision range of each (bytes 0-1 and 2-3)
**   18 the rider: 0 its stealth update is its getStealth(), 8-15 its status bits   19 the distance squared of the object to its victim (float bits)
** The data dumps go first, then the constructors' wake calls. Then `n` ops of 6 words [op, a, b, c, d, e]: 0 the object's update()  1 the rider's
** update()  2 markAsDetected(a)  3 receiveGrant(a & 1, b)  4 disguiseAsObject(object a, 0 none)  5 the rider's disguiseAsObject(object a)  6 cfg[a] =
** b  7 xfer save  8 xfer load of the last save  9 xfer CRC  10 loadPostProcess  11 the frame = a. Each op appends: the CATCH_ALL status, the result,
** the effect log, the wake calls (count, last frame), the xfer status, bytes and CRC, both modules' state, the object's status bits, the rider's, the
** academy counts and whether the object has a drawable. */
int ref_l9r26_run(const char *filename, const char *filename2, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r26Saved.clear();
	g_fxIds.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0, *d2 = 0;
	try { d = l9r26Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	try { d2 = l9r26Read(filename2); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { delete d; return status; }
	unsigned cfg[20];
	memcpy(cfg, cfg_in, sizeof(cfg));
	for (int which = 0; which < 2; ++which) {
		const R26StealthUpdateModuleData *sd = (const R26StealthUpdateModuleData *)(which ? d2 : d);
		for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) putU32(sd->m_hintDetectableStates.test(i) ? 1 : 0);
		for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) putU32(sd->m_requiredStatus.test(i) ? 1 : 0);
		for (int i = 0; i < OBJECT_STATUS_COUNT; ++i) putU32(sd->m_forbiddenStatus.test(i) ? 1 : 0);
		putI32(fxId(sd->m_disguiseFX));
		putI32(fxId(sd->m_disguiseRevealFX));
		putU32(realBits(sd->m_stealthSpeed));
		putU32(realBits(sd->m_friendlyOpacityMin));
		putU32(realBits(sd->m_friendlyOpacityMax));
		putU32(realBits(sd->m_revealDistanceFromTarget));
		putU32(sd->m_disguiseTransitionFrames);
		putU32(sd->m_disguiseRevealTransitionFrames);
		putU32(sd->m_pulseFrames);
		putU32(sd->m_stealthDelay);
		putU32(sd->m_stealthLevel);
		putU32(sd->m_blackMarketCheckFrames);
		putI32((int)sd->m_enemyDetectionEvaEvent);
		putI32((int)sd->m_ownDetectionEvaEvent);
		putU32((sd->m_innateStealth ? 1 : 0) | (sd->m_orderIdleEnemiesToAttackMeUponReveal ? 2 : 0) | (sd->m_teamDisguised ? 4 : 0) | (sd->m_useRiderStealth ? 8 : 0)
			| (sd->m_grantedBySpecialPower ? 16 : 0));
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitRandom(cfg[2]);
	InitGameLogicRandom(cfg[1]);
	Player players[3];
	for (int i = 0; i < 3; ++i) {
		players[i].m_idx = i;
		players[i].m_defaultTeam = (Team *)(size_t)(101 + i);
		players[i].m_color = 0x100 + i;
		players[i].m_nightColor = 0x200 + i;
		g_spPlayerPtrs[i] = &players[i];
		g_r26Teams[i].m_dummy = i + 1;
		g_r26Teams[i].m_controller = &players[i];
		g_r26Disguised[i] = 0;
	}
	g_spPlayerCount = 3;
	ThePlayerList = &g_spList;
	g_teamRelHook = r26TeamRel;
	g_findTemplateHook = r26FindTemplate;
	g_r26DrawPool = true;
	g_r26Freed = 0;
	GlobalData globals;
	TheGlobalData = &globals;
	Radar radar;
	TheRadar = &radar;
	TheInGameUI = &g_spUi;
	ControlBar controlBar;
	TheControlBar = &controlBar;
	TheAudio = &g_ddAudio;
	PartitionManager pm;
	ThePartitionManager = &pm;
	for (int k = 0; k < 7; ++k) {
		g_r26Objs[k] = newObject();
		g_r26Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r26Objs[k]->m_tmpl.m_name = g_r26TmplNames[k];
	}
	Object *me = g_r26Objs[0], *box = g_r26Objs[1], *rider = g_r26Objs[2], *target = g_r26Objs[3];
	me->m_pos.x = 10.0f;
	Drawable draw;
	AIUpdateInterface ai, enemyAi[2];
	R26PhysicsBehavior physics;
	R26Body body;
	R26Contain contain, boxContain;
	SpawnBehaviorInterface spawn;
	ModuleData plain;
	ai.m_owner = me;
	physics.m_owner = 1;
	contain.m_owner = 1;
	boxContain.m_owner = 2;
	spawn.m_ownerId = 1;
	Weapon weapons[3];
	for (int k = 0; k < 2; ++k) { enemyAi[k].m_owner = g_r26Objs[4 + k]; g_r26Objs[4 + k]->m_ai = &enemyAi[k]; g_r26Objs[4 + k]->m_team = &g_r26Teams[1 + k]; g_r26Objs[4 + k]->m_player = &players[1 + k]; }
	rider->m_team = &g_r26Teams[1];
	rider->m_player = &players[1];
	/* the scene, before the constructors and before every op */
	auto scene = [&]() {
		unsigned f = cfg[3];
		me->m_drawable = (f & 1) ? &draw : 0;
		draw.m_ownerObject = me;
		draw.m_selected = (f >> 13 & 1) != 0;
		draw.m_hiddenR26 = (f >> 14 & 1) != 0;
		me->m_ai = (f >> 1 & 1) ? &ai : 0;
		me->m_r26Physics = (f >> 2 & 1) ? &physics : 0;
		me->m_body = (f >> 3 & 1) ? &body : 0;
		me->m_contain = (f >> 4 & 1) ? &contain : 0;
		me->m_spawn = (f >> 5 & 1) ? &spawn : 0;
		me->m_kindOf = KindOfMaskType();
		if (f >> 6 & 1) me->m_kindOf.set(KINDOF_MINE);
		if (f >> 7 & 1) me->m_kindOf.set(KINDOF_SPAWNS_ARE_THE_WEAPONS);
		me->m_tmpl.m_kindOf = me->m_kindOf;
		me->m_containedBy = (f >> 8 & 1) ? box : 0;
		unsigned p = f >> 9 & 3;
		me->m_player = p < 3 ? &players[p] : 0;
		me->m_team = p < 3 ? &g_r26Teams[p] : 0;
		me->m_effDead = (f >> 11 & 1) != 0;
		me->m_scriptStatus = (f >> 12 & 1) ? OBJECT_STATUS_SCRIPT_UNSTEALTHED : 0;
		contain.m_list.clear();
		if (f >> 15 & 1) contain.m_list.push_back(rider);
		contain.m_rider = (f >> 16 & 1) ? rider : 0;
		me->m_status = ObjectStatusMaskType();
		for (int b = 0; b < 8; ++b) if (cfg[4] >> b & 1) me->m_status.set(g_r26Status[b]);
		unsigned v = cfg[5] & 7;
		ai.m_victim = v ? g_r26Objs[v - 1] : 0;
		ai.m_lastCmdSource = (Int)(cfg[5] >> 3 & 7);
		memcpy(&physics.m_vel.x, &cfg[6], 4);
		physics.m_vel.y = physics.m_vel.z = 0.0f;
		body.m_stamp = cfg[7];
		body.m_info.in.m_damageType = (DamageType)cfg[8];
		for (int k = 0; k < 3; ++k) {
			me->m_slotWeapon[k] = (cfg[9] >> k & 1) ? &weapons[k] : 0;
			weapons[k].m_last = cfg[9] >> (8 + 8 * k) & 255;
		}
		contain.m_riderChange = (cfg[10] & 1) != 0;
		contain.m_mayFire = (cfg[10] >> 1 & 1) != 0;
		contain.m_attacking = (cfg[10] >> 2 & 1) != 0;
		box->m_contain = &boxContain;
		boxContain.m_garrisonable = (cfg[11] & 1) != 0;
		spawn.m_allStealthed = (cfg[12] & 1) != 0;
		for (int j = 0; j < 3; ++j) g_r26TeamRel[j] = (int)(cfg[13] >> (2 * j) & 3) % 3;
		for (int i = 0; i < 3; ++i) for (int j = 0; j < 3; ++j) g_spMatrix[i][j] = (int)(cfg[13] >> (8 + 2 * (3 * i + j)) & 3) % 3;
		unsigned local = cfg[14] & 3;
		g_spLocal = local < 3 ? &players[local] : 0;
		for (int i = 0; i < 3; ++i) players[i].m_dead = (cfg[14] >> (4 + i) & 1) != 0;
		globals.m_timeOfDay = (cfg[14] >> 8 & 1) ? TIME_OF_DAY_NIGHT : TIME_OF_DAY_AFTERNOON;
		Object *market = g_r26Objs[6];
		market->m_effDead = (cfg[15] >> 1 & 1) != 0;
		market->m_status = ObjectStatusMaskType();
		if (cfg[15] >> 2 & 1) market->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		if (cfg[15] >> 3 & 1) market->m_status.set(OBJECT_STATUS_SOLD);
		market->m_kindOf = KindOfMaskType();
		if (cfg[15] >> 4 & 1) market->m_kindOf.set(KINDOF_FS_BLACK_MARKET);
		for (int i = 0; i < 3; ++i) {
			players[i].m_objects.clear();
			if (i == 1) players[i].m_objects.push_back(g_r26Objs[4]);
			if (i == 2) players[i].m_objects.push_back(g_r26Objs[5]);
			if ((cfg[15] & 1) && p == (unsigned)i) players[i].m_objects.push_back(market);
		}
		unsigned tp = cfg[16] & 3;
		target->m_player = tp < 3 ? &players[tp] : 0;
		target->m_team = tp < 3 ? &g_r26Teams[tp] : 0;
		for (int k = 0; k < 2; ++k) {
			g_r26Objs[4 + k]->m_pos.x = (Real)(cfg[17] >> (16 * k) & 255);
			g_r26Objs[4 + k]->m_vision = (Real)(cfg[17] >> (16 * k + 8) & 255);
		}
		rider->m_status = ObjectStatusMaskType();
		for (int b = 0; b < 8; ++b) if (cfg[18] >> (8 + b) & 1) rider->m_status.set(g_r26Status[b]);
		memcpy(&g_abDist[0][v ? v - 1 : 0], &cfg[19], 4);
	};
	scene();
	gl.m_awakenHook = r26Awaken;
	g_r26InConstruction = true;
	gl.m_awakenCalls = 0;
	gl.m_awakenFrame = 0;
	R26StealthUpdate *m = new R26StealthUpdate(me, d);
	R26StealthUpdate *rm = new R26StealthUpdate(rider, d2);
	g_r26InConstruction = false;
	putU32((unsigned)gl.m_awakenCalls);
	putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
	/* registerObject's "a frame of 0 is now, and now is at least 1" (GameLogic.cpp lines 3896-3923) */
	{
		UnsignedInt now = gl.m_frame ? gl.m_frame : 1;
		if (m->friend_getNextCallFrame() == 0) m->friend_setNextCallFrame(now);
		if (rm->friend_getNextCallFrame() == 0) rm->friend_setNextCallFrame(now);
	}
	/* the constructor set CAN_STEALTH on its object: the scene's status bits start from there */
	for (int b = 0; b < 8; ++b) { if (me->m_status.test(g_r26Status[b])) cfg[4] |= 1u << b; if (rider->m_status.test(g_r26Status[b])) cfg[18] |= 1u << (8 + b); }
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		if (o[0] == 6 && o[1] >= 3 && o[1] < 20) cfg[o[1]] = o[2];
		if (o[0] == 11) gl.m_frame = o[1];
		me->m_r26Stealth = m;
		rider->m_r26Stealth = (cfg[18] & 1) ? rm : 0;
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
		try {
			switch (o[0]) {
			case 0: g_r26Updating = m; res = (unsigned)m->update(); break;
			case 1: g_r26Updating = rm; res = (unsigned)rm->update(); break;
			case 2: m->markAsDetected(o[1]); break;
			case 3: m->receiveGrant((o[1] & 1) != 0, o[2]); break;
			case 4: m->disguiseAsObject(o[1] >= 1 && o[1] <= 7 ? g_r26Objs[o[1] - 1] : 0); break;
			case 5: rm->disguiseAsObject(o[1] >= 1 && o[1] <= 7 ? g_r26Objs[o[1] - 1] : 0); break;
			case 7: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r26Saved.assign(bytes, bytes + blen); break;
			case 8: {
				std::vector<unsigned char> b = g_r26Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 9: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 10: m->loadPostProcess(); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r26Updating = 0;
		/* the bits the module set or cleared are the scene's from now on */
		cfg[4] &= ~0xffu;
		cfg[18] &= ~0xff00u;
		for (int b = 0; b < 8; ++b) { if (me->m_status.test(g_r26Status[b])) cfg[4] |= 1u << b; if (rider->m_status.test(g_r26Status[b])) cfg[18] |= 1u << (8 + b); }
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			if (g_events[j].kind == 100) {
				const std::string &l = g_ulog[(size_t)g_events[j].p];
				if (l.compare(0, 7, "effpos ") == 0 || l.compare(0, 10, "efforient ") == 0) continue;   /* the new drawable's place: the client's */
				log.push_back(l);
			} else if (g_events[j].kind == 3) {
				log.push_back("markuidirty");
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
		R26StealthUpdate *mods[2] = { m, rm };
		for (int k = 0; k < 2; ++k) {
			R26StealthUpdate *s = mods[k];
			putU32(s->m_stealthAllowedFrame);
			putU32(s->m_detectionExpiresFrame);
			putU32(s->m_nextBlackMarketCheckFrame);
			putU32(s->m_enabled ? 1 : 0);
			putU32(realBits(s->m_pulsePhaseRate));
			putU32(realBits(s->m_pulsePhase));
			putU32((unsigned)s->m_disguiseAsPlayerIndex);
			putU32(s->m_disguiseTransitionFrames);
			putU32(s->m_disguiseHalfpointReached ? 1 : 0);
			putU32(s->m_transitioningToDisguise ? 1 : 0);
			putU32(s->m_disguised ? 1 : 0);
			putU32(s->m_framesGranted);
			putU32(s->m_xferRestoreDisguise ? 1 : 0);
			const char *name = s->m_disguiseAsTemplate ? s->m_disguiseAsTemplate->getName().str() : "";
			putStr(name, (int)strlen(name));
			putU32(s->friend_getNextCallFrame());
		}
		putU32(cfg[4] & 0xff);
		putU32(cfg[18] >> 8 & 0xff);
		for (int k = 0; k < 3; ++k) putU32((unsigned)g_r26Disguised[k]);
		putU32(me->m_drawable ? 1 : 0);
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_r26DrawPool = false;
	g_r26Freed = 0;
	g_teamRelHook = 0;
	g_findTemplateHook = 0;
	delete m;
	delete rm;
	delete d;
	delete d2;
	for (int k = 0; k < 7; ++k) {
		Object *ob = g_r26Objs[k];
		ob->m_ai = 0; ob->m_drawable = 0; ob->m_body = 0; ob->m_contain = 0; ob->m_spawn = 0; ob->m_team = 0; ob->m_player = 0; ob->m_containedBy = 0;
		ob->m_r26Stealth = 0; ob->m_r26Physics = 0;
		for (int s = 0; s < 3; ++s) ob->m_slotWeapon[s] = 0;
		delete ob;
		g_r26Objs[k] = 0;
	}
	TheGlobalData = 0;
	TheRadar = 0;
	TheInGameUI = 0;
	TheControlBar = 0;
	TheAudio = 0;
	ThePartitionManager = 0;
	ThePlayerList = 0;
	g_spLocal = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
