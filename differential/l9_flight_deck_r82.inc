/* Differential-test driver for FlightDeckBehavior (p1w13-sonnet, batch R82; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FlightDeckBehavior.cpp   (build.rs: mod_gen/FlightDeckBehavior_r82.cpp)
** and its header (mod_gen/FlightDeckBehavior_r82.h), in wrap_module.cpp's unit, at its end, with the mechanical edits build.rs lists: the class's
** AI base is R82AiBase below (an UpdateModule whose aiDoCommand does nothing; the class replaces it), its data's R82AiData (UpdateModuleData's
** fields); MSVC 6's vector iterators made portable. On the module build's stand-ins: the carrier (id 1: under construction, sold or not; a team
** of three with their players and default teams; maybe a production update whose count and unit id are the test's and whose queueCreateUnit
** is a line; maybe a drawable, whose clearAndSetModelConditionState is a line; the art's bones, the test's table through batch R74's hook; a
** position and an orientation), six planes (ids 2-7: a team; effectively dead or not; airborne or not; above the terrain or not; maybe an AI
** (the module build's stand-in: idle or not, its orders lines) whose JetAIUpdate questions (isTaxiingToParking, the pending command, isReloading)
** are the test's and friend_purgePendingCommand a line, R82JetAi below; maybe a body (batch M's MBody); a JetAIUpdate update module (its
** parking offset and takeoff the test's)), the payload's jets (ThingFactory::newObject: ids 8 and on, no modules), the particle systems (batch
** M's manager: setPosition and batch R70's setLocalTransform lines), Object::defect and setStatus (batch R57's lines), setProducer, the
** pathfinder (a line), the partition's distance (batch R57's real procedures), the REAL Object::kill, UpdateModule, INI and Xfer; the game
** logic (its frame). */

/* AI.h lines 336-399 (AICommandType, ALLOW_SURRENDER not defined) and 402-418 (AICommandParms), verbatim; AIStates.cpp lines 81-95 (its
** constructor), verbatim */
enum AICommandType	// Stored in save file, do not reorder/renumber.  jba.
{
	AICMD_NO_COMMAND = -1,
	AICMD_MOVE_TO_POSITION = 0,
	AICMD_MOVE_TO_OBJECT,
	AICMD_TIGHTEN_TO_POSITION,
	AICMD_MOVE_TO_POSITION_AND_EVACUATE,
	AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT,
	AICMD_IDLE,
	AICMD_FOLLOW_WAYPOINT_PATH,
	AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM,
	AICMD_FOLLOW_USER_PATH,	//Created by player in waypoint mode (this one will create little waypoint markers)
	AICMD_FOLLOW_PATH,
	AICMD_FOLLOW_EXITPRODUCTION_PATH,	// same as AICMD_FOLLOW_PATH, but only used when exiting your production facility.
	AICMD_ATTACK_OBJECT,
	AICMD_FORCE_ATTACK_OBJECT,
	AICMD_ATTACK_TEAM,
	AICMD_ATTACK_POSITION,
	AICMD_ATTACKMOVE_TO_POSITION,
	AICMD_ATTACKFOLLOW_WAYPOINT_PATH,
	AICMD_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM,
	AICMD_HUNT,
	AICMD_REPAIR,
	AICMD_RESUME_CONSTRUCTION,
	AICMD_GET_HEALED,
	AICMD_GET_REPAIRED,
	AICMD_ENTER,
	AICMD_DOCK,
	AICMD_EXIT,
	AICMD_EVACUATE,
	AICMD_EXECUTE_RAILED_TRANSPORT,
	AICMD_GO_PRONE,
	AICMD_GUARD_POSITION,
	AICMD_GUARD_OBJECT,
	AICMD_GUARD_AREA,
	AICMD_DEPLOY_ASSAULT_RETURN,
	AICMD_ATTACK_AREA,
	AICMD_HACK_INTERNET,
	AICMD_FACE_OBJECT,
	AICMD_FACE_POSITION,
	AICMD_RAPPEL_INTO,		// note that this applies to the rappeller.
	AICMD_COMBATDROP,			// note that this applies to the thing-being-rappelled-from.
	AICMD_COMMANDBUTTON_POS,
	AICMD_COMMANDBUTTON_OBJ,
	AICMD_COMMANDBUTTON,
	AICMD_WANDER,
	AICMD_WANDER_IN_PLACE,
	AICMD_PANIC,
	AICMD_BUSY,
	AICMD_FOLLOW_WAYPOINT_PATH_EXACT,
	AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT,
	AICMD_MOVE_AWAY_FROM_UNIT,
	AICMD_FOLLOW_PATH_APPEND,
	AICMD_MOVE_TO_POSITION_EVEN_IF_SLEEPING,	// same as AICMD_MOVE_TO_POSITION, but even AI_SLEEP units respond.
	AICMD_GUARD_TUNNEL_NETWORK,
	AICMD_EVACUATE_INSTANTLY,
	AICMD_EXIT_INSTANTLY,
	AICMD_GUARD_RETALIATE,

	AICMD_NUM_COMMANDS	// keep last
};
class Waypoint;
class PolygonTrigger;
class CommandButton;
class Path;
struct AICommandParms
{
	AICommandType						m_cmd;
  CommandSourceType				m_cmdSource;
  Coord3D									m_pos;
  Object*									m_obj;
  Object*									m_otherObj;
  const Team*							m_team;
	std::vector<Coord3D>		m_coords;
  const Waypoint*         m_waypoint;
  const PolygonTrigger*   m_polygon;
  Int											m_intValue;       /// misc usage
  DamageInfo							m_damage;
	const CommandButton*		m_commandButton;
	Path*										m_path;

	AICommandParms(AICommandType cmd, CommandSourceType cmdSource);
};
AICommandParms::AICommandParms(AICommandType cmd, CommandSourceType cmdSource) :
	m_cmd(cmd),
	m_cmdSource(cmdSource),
	m_obj(NULL),
	m_otherObj(NULL),
	m_team(NULL),
	m_waypoint(NULL),
	m_polygon(NULL),
	m_intValue(0),
	m_commandButton(NULL),
	m_path(NULL)
{
	m_pos.zero();
	m_coords.clear();
}

/* the class's AI base: an update module (the class replaces its update and aiDoCommand); its data UpdateModuleData's fields */
class R82AiData : public UpdateModuleData
{
public:
	static void buildFieldParse(MultiIniFieldParse &p) { UpdateModuleData::buildFieldParse(p); }
};
class R82AiBase : public UpdateModule
{
public:
	R82AiBase( Thing *thing, const ModuleData *moduleData ) : UpdateModule( thing, moduleData ) {}
	virtual void aiDoCommand( const AICommandParms * ) {}
};

/* JetAIUpdate's questions, on whatever the class casts to it (a plane's AI, or its JetAIUpdate module): the test's answers, found by address */
class R82JetAi : public AIUpdateInterface
{
public:
	Bool isTaxiingToParking() const;
	AICommandType friend_getPendingCommandType() const;
	void friend_purgePendingCommand();
	Bool isReloading() const;
	Real friend_getParkingOffset() const;
	Bool friend_isTakeoffOrLandingInProgress() const;
};

#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc) */
#define JetAIUpdate R82JetAi
#define HEAL_RATE_FRAMES R82_HEAL_RATE_FRAMES   /* (ParkingPlaceBehavior.cpp, in the same unit, defines its own) */
#include "FlightDeckBehavior_r82.cpp"
#undef HEAL_RATE_FRAMES
#undef JetAIUpdate
#undef destroyObject

const ThingTemplate *r82TemplateOf(unsigned code);

namespace {

ModuleData *l9r82Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FlightDeckBehavior::friend_newModuleData(&ini);
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

unsigned g_r82Prod = 0;   /* the production's count (bits 0-7) and next unit id (bits 8-15) */
/* the carrier's production update: its count and unit id the test's, queueCreateUnit a line; the rest never called here */
class R82Production : public UpdateModule, public ProductionUpdateInterface
{
public:
	R82Production(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual ProductionUpdateInterface *getProductionUpdateInterface() { return this; }
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual CanMakeType canQueueCreateUnit( const ThingTemplate * ) const { return CANMAKE_OK; }
	virtual CanMakeType canQueueUpgrade( const UpgradeTemplate * ) const { return CANMAKE_OK; }
	virtual ProductionID requestUniqueUnitID( void ) { ulog("produniqueid"); return (ProductionID)(g_r82Prod >> 8 & 255); }
	virtual Bool queueUpgrade( const UpgradeTemplate * ) { return FALSE; }
	virtual void cancelUpgrade( const UpgradeTemplate * ) {}
	virtual Bool isUpgradeInQueue( const UpgradeTemplate * ) const { return FALSE; }
	virtual UnsignedInt countUnitTypeInQueue( const ThingTemplate * ) const { return 0; }
	virtual Bool queueCreateUnit( const ThingTemplate *t, ProductionID id )
	{
		ulog("prodqueue %d %d", t == r82TemplateOf(1) ? 1 : 0, (int)id);
		return TRUE;
	}
	virtual void cancelUnitCreate( ProductionID ) {}
	virtual void cancelAllUnitsOfType( const ThingTemplate * ) {}
	virtual void cancelAndRefundAllProduction( void ) {}
	virtual UnsignedInt getProductionCount( void ) const { return g_r82Prod & 255; }
	virtual const ProductionEntry *firstProduction( void ) const { return 0; }
	virtual const ProductionEntry *nextProduction( const ProductionEntry * ) const { return 0; }
	virtual void setHoldDoorOpen( ExitDoorType, Bool ) {}
	virtual const CommandButton* getSpecialPowerConstructionCommandButton() const { return 0; }
	virtual void setSpecialPowerConstructionCommandButton( const CommandButton * ) {}
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *) {}
	virtual void loadPostProcess() {}
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }
};

const int R82_BONES = 19;
const char *const kR82Bones[R82_BONES] = {
	"Sp11", "Sp21", "Sp12", "Sp22", "Sp13", "TakeA1", "TakeB1", "TakeA2", "TakeB2", "LandA1", "LandB1", "LandA2", "LandB2",
	"Taxi1a", "Taxi1b", "Taxi2a", "Make1a", "Make1b", "Make2a" };
unsigned g_r82BoneBits = 0;
/* bone k: at (40 + 10 k, -25 + 7 k, 0.5 k + 0.5) (above the terrain), turned 0.3 k - 1 about z */
void r82BonePose(int k, Coord3D *pos, Real *angle)
{
	pos->x = 40.0f + 10.0f * (Real)k; pos->y = -25.0f + 7.0f * (Real)k; pos->z = 0.5f * (Real)k + 0.5f;
	*angle = 0.3f * (Real)k - 1.0f;
}
Object *g_r82Me = 0;
int r82Bone(const Object *obj, const char *name, Coord3D *position, Matrix3D *transform)
{
	if (obj != g_r82Me) return -1;
	for (int k = 0; k < R82_BONES; ++k) {
		if ((g_r82BoneBits >> k & 1) && strcmp(name, kR82Bones[k]) == 0) {
			Coord3D p; Real a;
			r82BonePose(k, &p, &a);
			if (position) *position = p;
			if (transform) { transform->Make_Identity(); transform->Rotate_Z(a); transform->Set_Translation(Vector3(p.x, p.y, p.z)); }
			return 1;
		}
	}
	/* (Object.cpp's fallback: the object's own position and transform) */
	if (position) *position = obj->m_pos;
	if (transform) *transform = *obj->getTransformMatrix();
	return 0;
}

struct R82Plane {
	Object *obj = 0;
	AIUpdateInterface ai;
	MBody body;
	JetAIUpdate *jet = 0;
	BehaviorModule *behaviors[2] = { 0, 0 };
	bool exists = true;
	unsigned word = 0;
};
R82Plane g_r82Planes[6];
std::vector<Object *> g_r82Made;   /* the payload's jets, ids 8 and on */
std::vector<bool> g_r82MadeExists;
BehaviorModule *g_r82NoModules[1] = { 0 };
ThingTemplate g_r82Tmpl;   /* R82Jet */
std::vector<unsigned char> g_r82Saved;
const int kR82Pending[8] = { AICMD_NO_COMMAND, AICMD_IDLE, AICMD_ATTACK_OBJECT, AICMD_FORCE_ATTACK_OBJECT, AICMD_ATTACK_POSITION, AICMD_ENTER,
	AICMD_GUARD_POSITION, AICMD_MOVE_TO_POSITION };
Real r82F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
Object *r82Find(ObjectID id)
{
	if ((int)id == 1) return g_r82Me;
	if ((int)id >= 2 && (int)id <= 7 && g_r82Planes[id - 2].exists) return g_r82Planes[id - 2].obj;
	size_t k = (size_t)((int)id - 8);
	if ((int)id >= 8 && k < g_r82Made.size() && g_r82MadeExists[k]) return g_r82Made[k];
	return 0;
}
const R82Plane *r82PlaneOf(const void *p)
{
	for (int k = 0; k < 6; ++k) if (p == (const void *)&g_r82Planes[k].ai || p == (const void *)g_r82Planes[k].jet) return &g_r82Planes[k];
	return 0;
}
UpdateModule *r82FindUpdate(const Object *obj, NameKeyType)
{
	/* (the jet's key is a function-local static made in the first case; the key generator is reset for every case: any key finds it) */
	for (int k = 0; k < 6; ++k) if (g_r82Planes[k].obj == obj) return g_r82Planes[k].jet;
	return 0;
}
const ThingTemplate *r82FindTemplate(const AsciiString &name) { return name.compare("R82Jet") == 0 ? &g_r82Tmpl : 0; }
Team *g_r82Teams = 0;
Object *r82NewObject(const ThingTemplate *, Team *team)
{
	Object *o = newObject();
	o->m_oid = (ObjectID)(8 + g_r82Made.size());
	o->m_team = team;
	o->m_player = team ? team->m_controller : 0;
	o->m_behaviors = g_r82NoModules;
	o->m_tmplShared = &g_r82Tmpl;
	g_r82Made.push_back(o);
	g_r82MadeExists.push_back(true);
	return o;
}
void r82PutInfo(const ParkingPlaceBehaviorInterface::PPInfo &i)
{
	const Coord3D *cs[9] = { &i.parkingSpace, &i.runwayPrep, &i.runwayStart, &i.runwayEnd, &i.runwayExit, &i.runwayLandingStart, &i.runwayLandingEnd,
		&i.runwayApproach, &i.hangarInternal };
	for (int k = 0; k < 9; ++k) { putU32(realBits(cs[k]->x)); putU32(realBits(cs[k]->y)); putU32(realBits(cs[k]->z)); }
	putU32(realBits(i.parkingOrientation));
	putU32(realBits(i.runwayTakeoffDist));
	putU32(realBits(i.hangarInternalOrient));
}
unsigned r82Coords(const std::vector<Coord3D> *v)
{
	if (!v) return 0xffffffffu;
	unsigned h = (unsigned)v->size();
	for (size_t k = 0; k < v->size(); ++k) h = h * 31u ^ realBits((*v)[k].x) ^ realBits((*v)[k].y) * 7u ^ realBits((*v)[k].z) * 13u;
	return h;
}

}  // namespace

const ThingTemplate *r82TemplateOf(unsigned code) { return code == 1 ? &g_r82Tmpl : 0; }
Bool R82JetAi::isTaxiingToParking() const { const R82Plane *p = r82PlaneOf(this); return p && (p->word >> 9 & 1); }
AICommandType R82JetAi::friend_getPendingCommandType() const
{
	const R82Plane *p = r82PlaneOf(this);
	return (AICommandType)(p ? kR82Pending[p->word >> 11 & 7] : AICMD_NO_COMMAND);
}
void R82JetAi::friend_purgePendingCommand() { const R82Plane *p = r82PlaneOf(this); ulog("jetpurge %d", p ? (int)p->obj->m_oid : 0); }
Bool R82JetAi::isReloading() const { const R82Plane *p = r82PlaneOf(this); return p && (p->word >> 10 & 1); }
Real R82JetAi::friend_getParkingOffset() const { const R82Plane *p = r82PlaneOf(this); return p ? (Real)(signed char)(p->word >> 24 & 255) : 0.0f; }
Bool R82JetAi::friend_isTakeoffOrLandingInProgress() const { const R82Plane *p = r82PlaneOf(this); return p && (p->word >> 5 & 1); }
void AIUpdateInterface::aiGuardPosition( const Coord3D *pos, GuardMode guardMode, CommandSourceType cmdSource )
{
	ulog("aiguardpos %d %08x %08x %08x %d %d", (int)m_owner->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)guardMode, (int)cmdSource);
}
void AIUpdateInterface::aiAttackMoveToPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource )
{
	ulog("aiattackmove %d %08x %08x %08x %d %d", (int)m_owner->m_oid, realBits(pos->x), realBits(pos->y), realBits(pos->z), (int)maxShotsToFire, (int)cmdSource);
}

extern "C" {

/* Read `filename` (block `D`) as the data of a FlightDeckBehavior and make the module on the carrier (id 1) at frame cfg[0]. `cfg` (32 words;
** 2-13 also settable by op 2): 1 the carrier's parts (read once): bit 0 a production update, 1 a drawable; 2 the carrier: bits 0-1 its team (0
** none, else 1-3), 2 under construction, 3 sold; 3 its bones (bit k: kR82Bones[k]); 4-6 its x, y, orientation (float bits); 7-12 planes 2-7:
** bit 0 airborne, 1-2 the team, 3 effectively dead, 4 above the terrain, 5 a takeoff or landing in progress, 6 a body, 7 an AI, 8 the AI idle,
** 9 taxiing to parking, 10 reloading, 11-13 the pending command (kR82Pending), 16 the carrier its producer, 24-31 the parking offset (whole
** units, signed); 13 the production's count (bits 0-7) and next unit id (8-15). The data dump goes first, then the constructor's state. Then
** `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 onDie  4 xfer save  5 xfer load of the last save
** 6 xfer CRC  7 the interface's call a with b, c, d (see the switch)  8 object a destroyed (no longer found)  9 object a put on
** space b's place. Each op appends: the CATCH_ALL
** status, the result, a PPInfo (or none), the effect log, the xfer status, bytes and CRC, the state (as the Rust state_words, with the
** runways' uninitialised start orientation and transform marked) and each object's. */
static int l9r82RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r82Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r82Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	const FlightDeckBehaviorModuleData *fd = (const FlightDeckBehaviorModuleData *)d;
	{
		putU32(realBits(fd->m_healAmount));
		putU32(realBits(fd->m_approachHeight));
		putU32(realBits(fd->m_landingDeckHeightOffset));
		putI32(fd->m_numRows);
		putI32(fd->m_numCols);
		const UnsignedInt fr[9] = { fd->m_cleanupFrames, fd->m_humanFollowFrames, fd->m_replacementFrames, fd->m_dockAnimationFrames,
			fd->m_launchWaveFrames, fd->m_launchRampFrames, fd->m_lowerRampFrames, fd->m_catapultFireFrames, 0 };
		for (int k = 0; k < 8; ++k) putU32(fr[k]);
		putStr(fd->m_thingTemplateName.str(), (int)strlen(fd->m_thingTemplateName.str()));
		for (int r = 0; r < MAX_RUNWAYS; ++r) {
			const RunwayDefinition &rd = fd->m_runwayInfo[r];
			const std::vector<AsciiString> *vs[3] = { &rd.m_spacesBoneNames, &rd.m_taxiBoneNames, &rd.m_creationBoneNames };
			for (int v = 0; v < 3; ++v) {
				putU32((unsigned)vs[v]->size());
				for (size_t j = 0; j < vs[v]->size(); ++j) putStr((*vs[v])[j].str(), (int)strlen((*vs[v])[j].str()));
			}
			for (int b = 0; b < 2; ++b) putStr(rd.m_takeoffBoneNames[b].str(), (int)strlen(rd.m_takeoffBoneNames[b].str()));
			for (int b = 0; b < 2; ++b) putStr(rd.m_landingBoneNames[b].str(), (int)strlen(rd.m_landingBoneNames[b].str()));
			std::string ps = rd.m_catapultParticleSystem ? g_ahPsNames[(size_t)rd.m_catapultParticleSystem->m_idx] : std::string("(null)");
			putStr(ps.c_str(), (int)ps.size());
		}
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;
	AI ai;
	TheAI = &ai;
	ThingFactory factory;
	TheThingFactory = &factory;
	TheParticleSystemManager = &g_ahPsm;
	g_ahPsLastId = 0;
	g_r28ThingPose = true;   /* (setOrientation keeps the angle, batch R28's way) */
	g_objXformSyncPos = true;
	g_r82Tmpl.m_name = AsciiString("R82Jet");
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
		players[p].m_defaultTeam = &teams[p];
	}
	g_r82Teams = teams;
	g_r82Made.clear();
	g_r82MadeExists.clear();
	BehaviorModule *none[1] = { 0 };
	g_r82Me = newObject();
	Object *me = g_r82Me;
	me->m_oid = 1;
	me->m_behaviors = none;
	Drawable draw;
	UpdateModuleData *ud = new UpdateModuleData;
	for (int k = 0; k < 6; ++k) {
		R82Plane &pl = g_r82Planes[k];
		pl.obj = newObject();
		pl.obj->m_oid = (ObjectID)(k + 2);
		pl.exists = true;
		pl.ai.m_owner = pl.obj;
		pl.body.m_owner = pl.obj;
		pl.body.m_health = pl.body.m_max = 100.0f;
		pl.jet = new JetAIUpdate(pl.obj, ud);
		pl.behaviors[0] = pl.jet;
		pl.behaviors[1] = 0;
		pl.obj->m_behaviors = pl.behaviors;
	}
	R82Production *prod = (cfg[1] & 1) ? new R82Production(me, ud) : 0;
	me->m_drawable = (cfg[1] >> 1 & 1) ? &draw : 0;
	auto scene = [&]() {
		unsigned f = cfg[2];
		unsigned t = f & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		me->m_status.clear();
		if (f >> 2 & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		if (f >> 3 & 1) me->m_status.set(OBJECT_STATUS_SOLD);
		g_r82BoneBits = cfg[3];
		me->m_pos.x = r82F(cfg[4]); me->m_pos.y = r82F(cfg[5]); me->m_pos.z = 0.0f;
		me->setOrientation(r82F(cfg[6]));
		me->poseMatrix();
		g_r82Prod = cfg[13];
		for (int k = 0; k < 6; ++k) {
			R82Plane &pl = g_r82Planes[k];
			Object *o = pl.obj;
			unsigned b = cfg[7 + k];
			pl.word = b;
			o->m_tmplShared = &g_r82Tmpl;
			o->m_status.set(OBJECT_STATUS_AIRBORNE_TARGET, (b & 1) ? 1 : 0);
			unsigned pt = b >> 1 & 3;
			o->m_team = pt ? &teams[pt - 1] : 0;
			o->m_player = pt ? &players[pt - 1] : 0;
			o->m_effDead = (b >> 3 & 1) != 0;
			o->m_pos.z = (b >> 4 & 1) ? 30.0f : 0.0f;
			pl.jet->m_takeoffOrLanding = (b >> 5 & 1) != 0;
			pl.jet->m_parkingOffset = (Real)(signed char)(b >> 24 & 255);
			o->m_body = (b >> 6 & 1) ? &pl.body : 0;
			o->m_ai = (b >> 7 & 1) ? &pl.ai : 0;
			pl.ai.m_idle = (b >> 8 & 1) != 0;
			o->m_producerID = (b >> 16 & 1) ? me->getID() : INVALID_ID;
		}
	};
	scene();
	g_modFindHook = r82Find;
	g_r74BoneHook = r82Bone;
	g_findUpdateHook = r82FindUpdate;
	g_findTemplateHook = r82FindTemplate;
	g_newObjectHook = r82NewObject;
	g_setStatusHook = r57SetStatus;
	g_defectHook = r57Defect;
	g_distPosDcHook = r57DistPos;
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
	g_events.clear();
	g_ulog.clear();
	FlightDeckBehavior *m = 0;
	try { m = newInstance(FlightDeckBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	((unsigned char *)m->m_rampUp)[MAX_RUNWAYS] = 0;   /* (the byte xfer saves as m_rampUp[MAX_RUNWAYS]: padding, uninitialised) */
	BehaviorModule *behaviors[3] = { m, prod, 0 };
	me->m_behaviors = behaviors;
	/* whether runway r's start orientation and transform were ever set: some runway up to it has a creation bone (one RunwayInfo for all) */
	auto runwaySet = [&](size_t r) {
		for (size_t c = 0; c <= r && c < (size_t)MAX_RUNWAYS; ++c) if (!fd->m_runwayInfo[c].m_creationBoneNames.empty()) return true;
		return false;
	};
	auto putState = [&]() {
		putU32(m->m_thingTemplate == &g_r82Tmpl ? 1u : (m->m_thingTemplate ? 2u : 0xffffffffu));
		putU32(m->m_nextHealFrame);
		putU32(m->m_nextCleanupFrame);
		putU32(m->m_startedProductionFrame);
		putU32(m->m_nextAllowedProductionFrame);
		for (int i = 0; i < MAX_RUNWAYS; ++i) {
			putU32(m->m_nextLaunchWaveFrame[i]);
			putU32(m->m_rampUpFrame[i]);
			putU32(m->m_catapultSystemFrame[i]);
			putU32(m->m_lowerRampFrame[i]);
			putU32(m->m_rampUp[i] ? 1 : 0);
		}
		putU32((unsigned)m->m_designatedTarget);
		putU32((unsigned)(Int)m->m_designatedCommand);
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_designatedPosition.x)[k]));
		putU32(m->m_gotInfo ? 1 : 0);
		putU32(((const unsigned char *)m->m_rampUp)[MAX_RUNWAYS]);
		putU32((unsigned)m->m_spaces.size());
		for (size_t i = 0; i < m->m_spaces.size(); ++i) {
			const FlightDeckBehavior::FlightDeckInfo &s = m->m_spaces[i];
			for (int k = 0; k < 3; ++k) putU32(realBits((&s.m_prep.x)[k]));
			putU32(realBits(s.m_orientation));
			putU32((unsigned)s.m_runway);
			putU32((unsigned)s.m_objectInSpace);
		}
		putU32((unsigned)m->m_runways.size());
		for (size_t i = 0; i < m->m_runways.size(); ++i) {
			const FlightDeckBehavior::RunwayInfo &r = m->m_runways[i];
			const Coord3D *cs[4] = { &r.m_start, &r.m_end, &r.m_landingStart, &r.m_landingEnd };
			for (int c = 0; c < 4; ++c) for (int k = 0; k < 3; ++k) putU32(realBits((&cs[c]->x)[k]));
			bool set = runwaySet(i);
			putU32(set ? realBits(r.m_startOrient) : 0xffffffffu);
			if (set) {
				for (int row = 0; row < 3; ++row) for (int col = 0; col < 4; ++col) putU32(realBits(r.m_startTransform[row][col]));
			} else {
				putU32(0xffffffffu);
			}
			putU32((unsigned)r.m_taxi.size());
			for (size_t j = 0; j < r.m_taxi.size(); ++j) for (int k = 0; k < 3; ++k) putU32(realBits((&r.m_taxi[j].x)[k]));
			putU32((unsigned)r.m_creation.size());
			for (size_t j = 0; j < r.m_creation.size(); ++j) for (int k = 0; k < 3; ++k) putU32(realBits((&r.m_creation[j].x)[k]));
			putU32((unsigned)r.m_inUseByForTakeoff);
			putU32((unsigned)r.m_inUseByForLanding);
		}
		putU32((unsigned)m->m_healing.size());
		for (std::list<FlightDeckBehavior::HealingInfo>::const_iterator it = m->m_healing.begin(); it != m->m_healing.end(); ++it) {
			putU32((unsigned)it->m_gettingHealedID);
			putU32(it->m_healStartFrame);
		}
		/* the objects: planes 2-7, then the jets made (a destroyed one only its id) */
		putU32((unsigned)(6 + g_r82Made.size()));
		for (size_t k = 0; k < 6 + g_r82Made.size(); ++k) {
			Object *o = k < 6 ? g_r82Planes[k].obj : g_r82Made[k - 6];
			bool exists = k < 6 ? g_r82Planes[k].exists : (bool)g_r82MadeExists[k - 6];
			putU32((unsigned)o->m_oid);
			if (!exists) { putU32(0xffffffffu); continue; }
			putU32((unsigned)o->m_producerID);
			putU32(o->m_team ? (unsigned)o->m_team->m_dummy : 0u);
			for (int j = 0; j < 3; ++j) putU32(realBits((&o->m_pos.x)[j]));
			putU32(realBits(o->getOrientation()));
		}
	};
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 13) cfg[o[1]] = o[2];
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 8) {
			int id = (int)o[1];
			if (id >= 2 && id <= 7) g_r82Planes[id - 2].exists = false;
			if (id >= 8 && (size_t)(id - 8) < g_r82Made.size()) g_r82MadeExists[(size_t)(id - 8)] = false;
		}
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
		bool hasInfo = false;
		ParkingPlaceBehaviorInterface::PPInfo info;
		memset(&info, 0, sizeof(info));
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: { DamageInfo di; m->onDie(&di); break; }
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r82Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r82Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 9: {   /* the plane onto space b's place (none: not moved) */
				Object *p = r82Find((ObjectID)o[1]);
				if (p && o[2] < m->m_spaces.size()) p->setPosition(&m->m_spaces[o[2]].m_prep);
				break;
			}
			case 7: {
				ObjectID id = (ObjectID)o[2];
				switch (o[1]) {
				case 0: res = m->shouldReserveDoorWhenQueued(&g_r82Tmpl) ? 1 : 0; break;
				case 1: res = m->hasAvailableSpaceFor(&g_r82Tmpl) ? 1 : 0; break;
				case 2: res = m->hasReservedSpace(id) ? 1 : 0; break;
				case 3: res = (unsigned)m->getSpaceIndex(id); break;
				case 4: hasInfo = (o[4] & 1) != 0; res = m->reserveSpace(id, r82F(o[3]), hasInfo ? &info : 0) ? 1 : 0; break;
				case 5: m->releaseSpace(id); break;
				case 6: res = m->reserveRunway(id, o[3] != 0) ? 1 : 0; break;
				case 7: hasInfo = true; m->calcPPInfo(id, &info); break;
				case 8: m->releaseRunway(id); break;
				case 9: res = (unsigned)m->getRunwayCount(); break;
				case 10: res = (unsigned)m->getRunwayReservation((Int)o[2], (RunwayReservationType)o[3]); break;
				case 11: m->transferRunwayReservationToNextInLineForTakeoff(id); break;
				case 12: res = realBits(m->getApproachHeight()) ^ realBits(m->getLandingDeckHeightOffset()); break;
				case 13: { Object *h = r82Find(id); if (h) m->setHealee(h, o[3] != 0); break; }
				case 14: m->killAllParkedUnits(); break;
				case 15: m->defectAllParkedUnits(&teams[o[2] % 3], o[3]); break;
				case 16: res = (unsigned)m->reserveDoorForExit(&g_r82Tmpl, 0); break;
				case 17: { Object *p = r82Find(id); if (p) m->exitObjectViaDoor(p, (ExitDoorType)(Int)o[3]); break; }
				case 18: m->unreserveDoorForExit((ExitDoorType)(Int)o[2]); break;
				case 19: {
					/* the position only when asked, a sentinel where it is not written; the new index -7 where it is not written */
					Coord3D pos; pos.x = 1.5f; pos.y = -2.5f; pos.z = 3.5f;
					Int oldIndex = -7, newIndex = -7;
					Bool moved = m->calcBestParkingAssignment(id, (o[3] & 1) ? &pos : NULL, &oldIndex, &newIndex);
					hasInfo = true;
					info.parkingSpace = pos;
					info.parkingOrientation = r82F((unsigned)oldIndex);
					info.runwayTakeoffDist = r82F((unsigned)newIndex);
					res = moved ? 1 : 0;
					break;
				}
				case 20: {
					static const AICommandType kCmds[8] = { AICMD_GUARD_POSITION, AICMD_ATTACK_POSITION, AICMD_FORCE_ATTACK_OBJECT, AICMD_ATTACK_OBJECT,
						AICMD_ATTACKMOVE_TO_POSITION, AICMD_IDLE, AICMD_MOVE_TO_POSITION, AICMD_ENTER };
					AICommandParms parms(kCmds[o[2] & 7], (CommandSourceType)(Int)o[3]);
					parms.m_pos.x = 12.5f; parms.m_pos.y = -7.0f; parms.m_pos.z = 0.0f;
					parms.m_obj = o[4] ? r82Find((ObjectID)o[4]) : 0;
					m->aiDoCommand(&parms);
					break;
				}
				case 21: res = r82Coords(m->getTaxiLocations(id)); break;
				case 22: res = r82Coords(m->getCreationLocations(id)); break;
				case 23: {
					Coord3D p; p.zero();
					res = (m->isExitBusy() ? 1u : 0u) | (m->getExitPosition(p) ? 2u : 0u) | (m->getNaturalRallyPoint(p, TRUE) ? 4u : 0u) |
						(m->getRallyPoint() ? 8u : 0u);
					m->setRallyPoint(&p);
					break;
				}
				}
				break;
			}
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putU32(hasInfo ? 1 : 0);
		if (hasInfo) r82PutInfo(info);
		putLog();
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putState();
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_r74BoneHook = 0;
	g_findUpdateHook = 0;
	g_findTemplateHook = 0;
	g_newObjectHook = 0;
	g_setStatusHook = 0;
	g_defectHook = 0;
	g_distPosDcHook = 0;
	g_r28ThingPose = false;
	g_objXformSyncPos = false;
	me->m_behaviors = 0;
	me->m_team = 0; me->m_player = 0; me->m_drawable = 0;
	delete m;
	delete prod;
	for (int k = 0; k < 6; ++k) {
		R82Plane &pl = g_r82Planes[k];
		Object *x = pl.obj;
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_ai = 0; x->m_body = 0; x->m_tmplShared = 0;
		delete x;
		delete pl.jet;
		pl.jet = 0;
		pl.obj = 0;
	}
	for (size_t k = 0; k < g_r82Made.size(); ++k) {
		Object *x = g_r82Made[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_tmplShared = 0;
		delete x;
	}
	g_r82Made.clear();
	g_r82MadeExists.clear();
	delete ud;
	delete d;
	delete me;
	g_r82Me = 0;
	TheAI = 0;
	TheThingFactory = 0;
	TheParticleSystemManager = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r82_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r82RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
