/* Differential-test driver for ParkingPlaceBehavior (p1w13-sonnet, batch R74; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end (batch R50's destroyObject rename), on the module build's stand-ins: the
** airfield (id 1: under construction, sold or not; a team of three with their players; maybe a production update whose setHoldDoorOpen is a
** line (Object::getProductionUpdateInterface, Object.cpp's, verbatim below); the art's bones (the test's table, a hook on the module
** build's getSingleLogicalBonePosition: a bone's position and a transform turned about z; no bone: the object's position and transform, as
** upstream); a position and an orientation, Thing's pose, batch R28's), six planes (ids 2-7: a template, R74Jet or R74Heli, the latter
** KINDOF_PRODUCED_AT_HELIPAD; a team; effectively dead or not; a height (above the terrain or not); an AI whose orders are lines; a body
** (batch M's MBody: attemptDamage and attemptHealing lines); a JetAIUpdate (the module build's stand-in: its parking offset and whether a
** takeoff or a landing is in progress the test's; found whatever the key, as batch R71's detector)), Object::defect and setStatus (batch
** R57's lines), setProducer (batch R15's), the pathfinder (batch R13's line), the REAL Object::kill, UpdateModule, INI and Xfer; the game
** logic (its frame, batch R35's wake calls). */
#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc: the inline one loses to other units' out-of-line ones) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/ParkingPlaceBehavior.cpp"
#undef destroyObject

/* Object.cpp lines 5835-5850, verbatim */
ProductionUpdateInterface* Object::getProductionUpdateInterface( void )
{
	ProductionUpdateInterface *pui;

	// tell our update modules that we intend to do this special power.
	for( BehaviorModule** u = m_behaviors; *u; ++u )
	{

		pui = (*u)->getProductionUpdateInterface();
		if( pui )
			return pui;

	}  // end for

	return NULL;
}
extern int (*g_r74BoneHook)(const Object *, const char *, Coord3D *, Matrix3D *);
extern UpdateModule *(*g_findUpdateHook)(const Object *, NameKeyType);

namespace {

ModuleData *l9r74Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = ParkingPlaceBehavior::friend_newModuleData(&ini);
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

/* the airfield's production update: setHoldDoorOpen a line; the rest never called here */
class R74Production : public UpdateModule, public ProductionUpdateInterface
{
public:
	R74Production(Thing *thing, const ModuleData *d) : UpdateModule(thing, d) {}
	virtual ProductionUpdateInterface *getProductionUpdateInterface() { return this; }
	virtual UpdateSleepTime update() { return UPDATE_SLEEP_NONE; }
	virtual CanMakeType canQueueCreateUnit( const ThingTemplate * ) const { return CANMAKE_OK; }
	virtual CanMakeType canQueueUpgrade( const UpgradeTemplate * ) const { return CANMAKE_OK; }
	virtual ProductionID requestUniqueUnitID( void ) { return (ProductionID)0; }
	virtual Bool queueUpgrade( const UpgradeTemplate * ) { return FALSE; }
	virtual void cancelUpgrade( const UpgradeTemplate * ) {}
	virtual Bool isUpgradeInQueue( const UpgradeTemplate * ) const { return FALSE; }
	virtual UnsignedInt countUnitTypeInQueue( const ThingTemplate * ) const { return 0; }
	virtual Bool queueCreateUnit( const ThingTemplate *, ProductionID ) { return FALSE; }
	virtual void cancelUnitCreate( ProductionID ) {}
	virtual void cancelAllUnitsOfType( const ThingTemplate * ) {}
	virtual void cancelAndRefundAllProduction( void ) {}
	virtual UnsignedInt getProductionCount( void ) const { return 0; }
	virtual const ProductionEntry *firstProduction( void ) const { return 0; }
	virtual const ProductionEntry *nextProduction( const ProductionEntry * ) const { return 0; }
	virtual void setHoldDoorOpen( ExitDoorType exitDoor, Bool holdIt ) { ulog("holddoor %d %d %d", (int)getObject()->getID(), (int)exitDoor, holdIt ? 1 : 0); }
	virtual const CommandButton* getSpecialPowerConstructionCommandButton() const { return 0; }
	virtual void setSpecialPowerConstructionCommandButton( const CommandButton * ) {}
	virtual void crc(Xfer *) {}
	virtual void xfer(Xfer *) {}
	virtual void loadPostProcess() {}
	virtual NameKeyType getModuleNameKey() const { return (NameKeyType)0; }
};

const int R74_BONES = 17;
const char *const kR74Bones[R74_BONES] = {
	"Runway1Park1Han", "Runway1Parking1", "Runway1Prep1", "Runway2Park1Han", "Runway2Parking1", "Runway2Prep1",
	"Runway1Park2Han", "Runway1Parking2", "Runway1Prep2", "Runway2Park2Han", "Runway2Parking2", "Runway2Prep2",
	"RunwayStart1", "RunwayStart2", "RunwayEnd1", "RunwayEnd2", "HeliPark01" };
unsigned g_r74BoneBits = 0;
/* bone k: at (40 + 10 k, -25 + 7 k, 0.5 k), turned 0.3 k - 1 about z */
void r74BonePose(int k, Coord3D *pos, Real *angle)
{
	pos->x = 40.0f + 10.0f * (Real)k; pos->y = -25.0f + 7.0f * (Real)k; pos->z = 0.5f * (Real)k;
	*angle = 0.3f * (Real)k - 1.0f;
}
Object *g_r74Me = 0;
int r74Bone(const Object *obj, const char *name, Coord3D *position, Matrix3D *transform)
{
	if (obj != g_r74Me) return -1;
	for (int k = 0; k < R74_BONES; ++k) {
		if ((g_r74BoneBits >> k & 1) && strcmp(name, kR74Bones[k]) == 0) {
			Coord3D p; Real a;
			r74BonePose(k, &p, &a);
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

struct R74Plane {
	Object *obj = 0;
	AIUpdateInterface ai;
	MBody body;
	JetAIUpdate *jet = 0;
	BehaviorModule *behaviors[2] = { 0, 0 };
	bool exists = true;
};
R74Plane g_r74Planes[6];
ThingTemplate g_r74Tmpl[2];   /* R74Jet, R74Heli (KINDOF_PRODUCED_AT_HELIPAD) */
std::vector<unsigned char> g_r74Saved;
Real r74F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
Object *r74Find(ObjectID id)
{
	if ((int)id == 1) return g_r74Me;
	if ((int)id >= 2 && (int)id <= 7 && g_r74Planes[id - 2].exists) return g_r74Planes[id - 2].obj;
	return 0;
}
UpdateModule *r74FindUpdate(const Object *obj, NameKeyType)
{
	/* (the jet's key is a function-local static made in the first case; the key generator is reset for every case: any key finds it) */
	for (int k = 0; k < 6; ++k) if (g_r74Planes[k].obj == obj) return g_r74Planes[k].jet;
	return 0;
}
const ThingTemplate *r74Template(unsigned code) { return code == 1 ? &g_r74Tmpl[0] : (code == 2 ? &g_r74Tmpl[1] : 0); }
void r74PutInfo(const ParkingPlaceBehaviorInterface::PPInfo &i)
{
	const Coord3D *cs[9] = { &i.parkingSpace, &i.runwayPrep, &i.runwayStart, &i.runwayEnd, &i.runwayExit, &i.runwayLandingStart, &i.runwayLandingEnd,
		&i.runwayApproach, &i.hangarInternal };
	for (int k = 0; k < 9; ++k) { putU32(realBits(cs[k]->x)); putU32(realBits(cs[k]->y)); putU32(realBits(cs[k]->z)); }
	putU32(realBits(i.parkingOrientation));
	putU32(realBits(i.runwayTakeoffDist));
	putU32(realBits(i.hangarInternalOrient));
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a ParkingPlaceBehavior and make the module on the airfield (id 1) at frame cfg[0]. `cfg` (32 words;
** 2-20 also settable by op 2): 1 the airfield's parts (read once): bit 0 a production update; 2 the airfield: bits 0-1 its team (0 none, else
** 1-3), 2 under construction, 3 sold; 3 its bones (bit k: kR74Bones[k]); 4-6 its x, y, orientation (float bits); 7-12 planes 2-7 (a byte
** each... see r74Plane below): bit 0 the heli template, 1-2 the team, 3 effectively dead, 4 in the air, 5 a takeoff or landing in progress,
** 6 a body, 7 an AI, bits 8-15 the parking offset (whole units, signed); 13 (unused). The data dump goes first. Then the constructor's wake
** state (the wake calls are not compared: the module never sleeps, and the constructor's wake is the game logic's registration). Then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3 onDie  4 xfer save  5 xfer load
** of the last save  6 xfer CRC  7 the interface's call a with b, c, d (see the switch). Each op appends: the CATCH_ALL status, the result,
** a PPInfo (or none), the effect log, the xfer status, bytes and CRC, the state (as the Rust state_words), and each plane's
** producer, position and orientation. */
static int l9r74RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r74Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9r74Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const ParkingPlaceBehaviorModuleData *pd = (const ParkingPlaceBehaviorModuleData *)d;
		putU32(realBits(pd->m_healAmount));
		putI32(pd->m_numRows);
		putI32(pd->m_numCols);
		putU32(realBits(pd->m_approachHeight));
		putU32(realBits(pd->m_landingDeckHeightOffset));
		putU32(pd->m_hasRunways ? 1 : 0);
		putU32(pd->m_parkInHangars ? 1 : 0);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;
	AI ai;
	TheAI = &ai;
	g_r28ThingPose = true;   /* (setOrientation keeps the angle, batch R28's way) */
	g_objXformSyncPos = true;
	g_r74Tmpl[0].m_name = AsciiString("R74Jet");
	g_r74Tmpl[1].m_name = AsciiString("R74Heli");
	g_r74Tmpl[1].m_kindOf.set(KINDOF_PRODUCED_AT_HELIPAD);
	Player players[3];
	Team teams[3];
	for (int p = 0; p < 3; ++p) {
		players[p].m_idx = p;
		teams[p].m_dummy = p + 1;
		teams[p].m_controller = &players[p];
	}
	BehaviorModule *none[1] = { 0 };
	g_r74Me = newObject();
	Object *me = g_r74Me;
	me->m_oid = 1;
	me->m_behaviors = none;
	UpdateModuleData *ud = new UpdateModuleData;
	for (int k = 0; k < 6; ++k) {
		R74Plane &pl = g_r74Planes[k];
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
	R74Production *prod = (cfg[1] & 1) ? new R74Production(me, ud) : 0;
	auto scene = [&]() {
		unsigned f = cfg[2];
		unsigned t = f & 3;
		me->m_team = t ? &teams[t - 1] : 0;
		me->m_player = t ? &players[t - 1] : 0;
		me->m_status.clear();
		if (f >> 2 & 1) me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION);
		if (f >> 3 & 1) me->m_status.set(OBJECT_STATUS_SOLD);
		g_r74BoneBits = cfg[3];
		me->m_pos.x = r74F(cfg[4]); me->m_pos.y = r74F(cfg[5]); me->m_pos.z = 0.0f;
		me->setOrientation(r74F(cfg[6]));
		me->poseMatrix();
		for (int k = 0; k < 6; ++k) {
			R74Plane &pl = g_r74Planes[k];
			Object *o = pl.obj;
			unsigned b = cfg[7 + k];
			o->m_tmplShared = r74Template((b & 1) ? 2 : 1);
			o->m_kindOf = o->m_tmplShared->m_kindOf;
			unsigned pt = b >> 1 & 3;
			o->m_team = pt ? &teams[pt - 1] : 0;
			o->m_player = pt ? &players[pt - 1] : 0;
			o->m_effDead = (b >> 3 & 1) != 0;
			o->m_pos.z = (b >> 4 & 1) ? 30.0f : 0.0f;
			pl.jet->m_takeoffOrLanding = (b >> 5 & 1) != 0;
			o->m_body = (b >> 6 & 1) ? &pl.body : 0;
			o->m_ai = (b >> 7 & 1) ? &pl.ai : 0;
			pl.jet->m_parkingOffset = (Real)(signed char)(b >> 8 & 255);
			o->m_producerID = (b >> 16 & 1) ? me->getID() : INVALID_ID;
		}
	};
	scene();
	g_modFindHook = r74Find;
	g_r74BoneHook = r74Bone;
	g_findUpdateHook = r74FindUpdate;
	g_setStatusHook = r57SetStatus;
	g_defectHook = r57Defect;
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
	ParkingPlaceBehavior *m = 0;
	try { m = newInstance(ParkingPlaceBehavior)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	BehaviorModule *behaviors[3] = { m, prod, 0 };
	me->m_behaviors = behaviors;
	auto putState = [&]() {
		putU32(m->m_gotInfo ? 1 : 0);
		putU32(m->m_nextHealFrame);
		putU32(m->m_heliRallyPointExists ? 1 : 0);
		for (int k = 0; k < 3; ++k) putU32(realBits((&m->m_heliRallyPoint.x)[k]));
		putU32((unsigned)m->m_spaces.size());
		for (size_t i = 0; i < m->m_spaces.size(); ++i) {
			const ParkingPlaceBehavior::ParkingPlaceInfo &s = m->m_spaces[i];
			for (int k = 0; k < 3; ++k) putU32(realBits((&s.m_hangarStart.x)[k]));
			putU32(realBits(s.m_hangarStartOrient));
			for (int k = 0; k < 3; ++k) putU32(realBits((&s.m_location.x)[k]));
			for (int k = 0; k < 3; ++k) putU32(realBits((&s.m_prep.x)[k]));
			putU32(realBits(s.m_orientation));
			putU32((unsigned)s.m_runway);
			putU32((unsigned)s.m_door);
			putU32((unsigned)s.m_objectInSpace);
			putU32(s.m_reservedForExit ? 1 : 0);
		}
		putU32((unsigned)m->m_runways.size());
		for (size_t i = 0; i < m->m_runways.size(); ++i) {
			const ParkingPlaceBehavior::RunwayInfo &r = m->m_runways[i];
			for (int k = 0; k < 3; ++k) putU32(realBits((&r.m_start.x)[k]));
			for (int k = 0; k < 3; ++k) putU32(realBits((&r.m_end.x)[k]));
			putU32((unsigned)r.m_inUseBy);
			putU32((unsigned)r.m_nextInLineForTakeoff);
			putU32(r.m_wasInLine ? 1 : 0);
		}
		putU32((unsigned)m->m_healing.size());
		for (std::list<ParkingPlaceBehavior::HealingInfo>::const_iterator it = m->m_healing.begin(); it != m->m_healing.end(); ++it) {
			putU32((unsigned)it->m_gettingHealedID);
			putU32(it->m_healStartFrame);
		}
		for (int k = 0; k < 6; ++k) {
			Object *o = g_r74Planes[k].obj;
			putU32((unsigned)o->m_producerID);
			for (int j = 0; j < 3; ++j) putU32(realBits((&o->m_pos.x)[j]));
			putU32(realBits(o->getOrientation()));
		}
	};
	putState();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 2 && o[1] <= 12) cfg[o[1]] = o[2];
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
		bool hasInfo = false;
		ParkingPlaceBehaviorInterface::PPInfo info;
		memset(&info, 0, sizeof(info));
		try {
			switch (o[0]) {
			case 0: res = (unsigned)m->update(); break;
			case 3: { DamageInfo di; m->onDie(&di); break; }
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r74Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r74Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 7: {
				ObjectID id = (ObjectID)o[2];
				switch (o[1]) {
				case 0: res = m->shouldReserveDoorWhenQueued(r74Template(o[2] ? o[2] : 1)) ? 1 : 0; break;
				case 1: res = m->hasAvailableSpaceFor(r74Template(o[2] ? o[2] : 1)) ? 1 : 0; break;
				case 2: res = m->hasReservedSpace(id) ? 1 : 0; break;
				case 3: res = (unsigned)m->getSpaceIndex(id); break;
				case 4: hasInfo = (o[4] & 1) != 0; res = m->reserveSpace(id, r74F(o[3]), hasInfo ? &info : 0) ? 1 : 0; break;
				case 5: m->releaseSpace(id); break;
				case 6: res = m->reserveRunway(id, o[3] != 0) ? 1 : 0; break;
				case 7: hasInfo = true; m->calcPPInfo(id, &info); break;
				case 8: m->releaseRunway(id); break;
				case 9: res = (unsigned)m->getRunwayCount(); break;
				case 10: res = (unsigned)m->getRunwayReservation((Int)o[2], (RunwayReservationType)o[3]); break;
				case 11: m->transferRunwayReservationToNextInLineForTakeoff(id); break;
				case 12: res = realBits(m->getApproachHeight()) ^ realBits(m->getLandingDeckHeightOffset()); break;
				case 13: { Object *h = r74Find(id); if (h) m->setHealee(h, o[3] != 0); break; }
				case 14: m->killAllParkedUnits(); break;
				case 15: m->defectAllParkedUnits(&teams[o[2] % 3], o[3]); break;
				case 16: res = (unsigned)m->reserveDoorForExit(r74Template(o[2]), 0); break;
				case 17: { Object *p = r74Find(id); if (p) m->exitObjectViaDoor(p, (ExitDoorType)(Int)o[3]); break; }
				case 18: m->unreserveDoorForExit((ExitDoorType)(Int)o[2]); break;
				case 19: { Coord3D p; p.x = r74F(o[2]); p.y = r74F(o[3]); p.z = 0.0f; m->setRallyPoint(&p); break; }
				case 20: { const Coord3D *p = m->getRallyPoint(); res = p ? realBits(p->x) ^ realBits(p->y) ^ 1u : 0u; break; }
				/* (the position only when found: the Rust interface answers no position for FALSE) */
				case 21: { Coord3D p; p.zero(); Bool b = m->getExitPosition(p); res = b ? 1u ^ realBits(p.x) ^ realBits(p.y) ^ realBits(p.z) : 0u; break; }
				case 22: { Coord3D p; p.zero(); Bool b = m->getNaturalRallyPoint(p, TRUE); res = b ? 1u ^ realBits(p.x) ^ realBits(p.y) ^ realBits(p.z) : 0u; break; }
				}
				break;
			}
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		putU32(hasInfo ? 1 : 0);
		if (hasInfo) r74PutInfo(info);
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
	g_setStatusHook = 0;
	g_defectHook = 0;
	g_r28ThingPose = false;
	g_objXformSyncPos = false;
	me->m_behaviors = 0;
	me->m_team = 0; me->m_player = 0;
	delete m;
	delete prod;
	for (int k = 0; k < 6; ++k) {
		R74Plane &pl = g_r74Planes[k];
		Object *x = pl.obj;
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0; x->m_ai = 0; x->m_body = 0; x->m_tmplShared = 0;
		delete x;
		delete pl.jet;
		pl.jet = 0;
		pl.obj = 0;
	}
	delete ud;
	delete d;
	delete me;
	g_r74Me = 0;
	TheAI = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r74_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r74RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
