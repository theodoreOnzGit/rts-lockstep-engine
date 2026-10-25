/* Differential-test driver for PhysicsBehavior (p1w13-sonnet, batch R50; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PhysicsUpdate.h         (#included as mod_gen/PhysicsUpdate_r50.h: its include guard renamed)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp (#included as mod_gen/PhysicsUpdate_r50.cpp: its include of the
**     header, and the type of the argument it hands the AI's processCollision, edited; build.rs)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp               (Object::canCrushOrSquish, verbatim: mod_gen/ObjectCrush.inc;
**     Object::onCollide and Object::findModule, verbatim below)
** in wrap_module.cpp's unit, at its end. The unit already has a PhysicsBehavior (the module build's stand-in, a block of memory other drivers
** define methods of), so the real class is compiled as R50PhysicsBehavior, and the accessor the file uses on an Object, getPhysics, is the shim
** Object's r50GetPhysics while it is included.
**
** The scene, on the module build's stand-ins: four objects (ids 1-4; each with the module build's MBody, whose attemptDamage is a line and whose
** crushed ends the test gives, kinds, crusher and crushable levels, a geometry, a position, disabled bits and status bits, maybe effectively dead,
** maybe off the map); object 1 the module's (its angle, maybe a drawable whose model condition calls are lines, maybe an AI: its locomotor set,
** ignored obstacle, its processCollision a line with the test's answer, its locomotor surfaces); objects 2-4 maybe with a PhysicsBehavior of
** their own (the same data); object 2 maybe with an AI (its ignored obstacle); containment (1 in 4, 2 in 1, 4 in 3); the gravity and the
** stiffnesses (GlobalData.cpp's), the structure rubble height; the terrain (its layer heights, cliff cells, water: the HeightDie, minefield and
** float drivers'); the object transform (kept: setTransformMatrix keeps the position and the angle, as Thing's does, and a position that is not
** a number marks the object destroyed, as Object::reactToTransformChange does; getTransformMatrix takes the position); the audio manager; the
** weapon store (WEAP_A, WEAP_B; createAndFireTempWeapon a line); the script engine's transferObjectName and Object::defect (lines); the game
** logic (its frame, destroyObject a line, its wake calls applied as GameLogic::friend_awakenUpdateModule applies them); and the REAL
** UpdateModule, INI, Xfer and random values. */
#include "ObjectCrush.inc"   /* Object::canCrushOrSquish, verbatim from Object.cpp (build.rs, mod_gen) */
/* Object.cpp 2393-2414, verbatim (the DEBUG_CRC lines are comments there) */
void Object::onCollide( Object *other, const Coord3D *loc, const Coord3D *normal )
{
	for (BehaviorModule** m = m_behaviors; *m; ++m)
	{
		CollideModuleInterface* collide = (*m)->getCollide();
		if (!collide)
			continue;

		// check each time thru the loop, in case a collide module sets it
		if( getStatusBits().test( OBJECT_STATUS_NO_COLLISIONS ) )
		{
			break;
		}
		collide->onCollide(other, loc, normal);
	}
}
/* Object.cpp 2871-2898, verbatim (INTENSE_DEBUG is not defined) */
Module* Object::findModule(NameKeyType key) const
{
	Module* m = NULL;

	for (BehaviorModule** b = m_behaviors; *b; ++b)
	{
		if ((*b)->getModuleNameKey() == key)
		{
			m = *b;
			break;
		}
	}

	return m;
}
Bool AIUpdateInterface::processCollision(PhysicsBehavior *, Object *other)
{
	ulog("aiprocess %d %d", (int)m_owner->m_oid, other ? (int)other->m_oid : 0);
	return m_r50Process;
}

/* LocomotorSet.h lines 49-56, verbatim (the header needs the locomotor layer) */
enum LocomotorSurfaceType
{
	LOCOMOTORSURFACE_GROUND			= (1 << 0),									///< clear, unobstructed ground
	LOCOMOTORSURFACE_WATER			= (1 << 1),									///< water area
	LOCOMOTORSURFACE_CLIFF			= (1 << 2),									///< steep altitude change
	LOCOMOTORSURFACE_AIR				= (1 << 3),									///< airborne
	LOCOMOTORSURFACE_RUBBLE			= (1 << 4)									///< building rubble
};
/* BaseType.h lines 100-106 (while the file is included) and 149-155, verbatim: this build's BaseType leaves them out */
#define min(x,y) (((x)<(y)) ? (x) : (y))
#define max(x,y) (((x)>(y)) ? (x) : (y))
template <typename NUM>
inline NUM clamp(NUM lo, NUM val, NUM hi)
{
	if (val < lo) return lo;
	else if (val > hi) return hi;
	else return val;
}
typedef PhysicsBehavior R50OrigPhysics;
#define PhysicsBehavior R50PhysicsBehavior
#define PhysicsBehaviorModuleData R50PhysicsBehaviorModuleData
#define getPhysicsBehaviorModuleData getR50PhysicsBehaviorModuleData   /* the name MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA makes of the renamed class */
#define PhysicsTurningType R50PhysicsTurningType
#define TURN_NEGATIVE R50_TURN_NEGATIVE
#define TURN_NONE R50_TURN_NONE
#define TURN_POSITIVE R50_TURN_POSITIVE
#define getPhysics r50GetPhysics
#define calcDistSqr r50CalcDistSqr   /* (the unit has one already) */
#define destroyObject r50DestroyObject   /* (the shim's inline destroyObject loses to other units' out-of-line ones when it is not inlined) */
#include "PhysicsUpdate_r50.cpp"
#undef destroyObject
#undef calcDistSqr
#undef getPhysics
#undef max
#undef min
#undef TURN_POSITIVE
#undef TURN_NONE
#undef TURN_NEGATIVE
#undef PhysicsTurningType
#undef getPhysicsBehaviorModuleData
#undef PhysicsBehaviorModuleData
#undef PhysicsBehavior

namespace {

ModuleData *l9r50Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = R50PhysicsBehavior::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_r50Saved;
UpdateModule *g_r50Updating = 0;
UpdateModule *g_r50Me = 0;
bool g_r50Constructing = false;   /* the module's own constructor is running */
int g_r50Wakes = 0;
UnsignedInt g_r50WakeFrame = 0;
/* GameLogic::friend_awakenUpdateModule as R32's driver applies it; the module's own calls are counted */
void r50Awaken(Object *, UpdateModule *u, UnsignedInt whenToWakeUp)
{
	if (u == g_r50Me || g_r50Constructing) { ++g_r50Wakes; g_r50WakeFrame = whenToWakeUp; }
	UnsignedInt now = TheGameLogic->getFrame();
	if (u == g_r50Updating) return;
	if (whenToWakeUp == u->friend_getNextCallFrame()) return;
	if ((now > 0) && (u->friend_getNextCallFrame() == now) && (whenToWakeUp == now + 1)) return;
	u->friend_setNextCallFrame(whenToWakeUp);
}
/* Thing::setTransformMatrix keeps the position and the angle of the transform */
void r50Xform(Object *o, const Matrix3D *mx)
{
	o->m_pos.x = mx->Get_X_Translation();
	o->m_pos.y = mx->Get_Y_Translation();
	o->m_pos.z = mx->Get_Z_Translation();
	o->m_orientation = mx->Get_Z_Rotation();
	/* Object::reactToTransformChange (Object.cpp 1767-1770): a position that is not a number destroys the object; GameLogic::destroyObject's
	** mark (the test's objects have no modules whose onDestroy matters) */
	if (_isnan(o->m_pos.x) || _isnan(o->m_pos.y) || _isnan(o->m_pos.z)) o->m_status.set(OBJECT_STATUS_DESTROYED);
}
void r50Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
void r50Defect(Object *obj, Team *team, UnsignedInt detectionTime) { ulog("defect %d %d %u", (int)obj->m_oid, team ? 1 : 0, detectionTime); }
Real r50F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
const Real kR50Radii[8] = { 1.0f, 2.5f, 5.0f, 10.0f, 20.0f, 0.5f, 3.0f, 7.5f };
const Real kR50Heights[4] = { 1.0f, 3.0f, 8.0f, 0.0f };

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a PhysicsBehavior and make the module on object 1 of four. `cfg` (32 words):
** fixed: 0 the frame, 1 the random seed, 2 flags (0 object 1's drawable, 1 its AI, 2-4 objects 2-4 have a PhysicsBehavior, 5 object 2's AI,
** 6 object 1 is in 4, 7 object 2 is in 1, 8 object 4 is in 3), 3 the kinds (a byte per object: 0 infantry, 1 vehicle, 2 immobile, 3 structure,
** 4 drone, 5 projectile), 4 the crusher (low nibble) and crushable (high nibble) levels (a byte per object), 13 the crushed ends (bits 2k front,
** 2k+1 back of object k+1), 14 the geometries (a byte per object: bits 0-1 the type, 2-4 the radius, 5-6 the height; tables above), 15-26 the
** positions (x, y, z per object, float bits), 27 object 1's angle (float bits);
** settable by op 1: 5 the disabled bits (a byte per object: 0 UNMANNED, 1 HELD, 2 FREEFALL), 6 the status (a byte per object: 0 PARACHUTING,
** 1 BRAKING, 2 DECK_HEIGHT_OFFSET, 3 NO_COLLISIONS, 4 effectively dead, 5 off the map), 7 object 1's AI (low byte the locomotor set, 255 none;
** byte 1 the ignored obstacle; bit 16 processCollision's answer; bits 20-23 the locomotor surfaces), 8 object 2's AI's ignored obstacle,
** 9 the layer heights (eight nibbles, halves), 10 the cliff cells (low byte) and the water cells (byte 1), 11 the water height, 12 the
** structure rubble height (float bits), 18-26 objects 2-4's positions (setting one places that object).
** The data dump goes first, then the constructor's wake calls. Then `n` ops of 5 words [op, a, b, c, unused]: 0 update()  1 cfg[a] = b  2 the frame
** = a  3 xfer save  4 xfer load of the last save  5 xfer CRC  6 applyForce(a, b, c)  7 applyMotiveForce(a, b, c)  8 applyShock(a, b, c)
** 9 applyRandomRotation()  10 onCollide(object a or NULL, NULL, NULL)  11 setter a with b (see below)  12 resetDynamicPhysics()  13 setAngles(a, b,
** c)  14 onObjectCreated()  15 getter a (b an object), the result  16 object a's physics' setIgnoreCollisionsWith(object b or NULL)
** 17 addVelocityTo(a, b, c) (a, b, c float bits throughout). Each op appends: the CATCH_ALL status, the result, the effect log, the wake calls
** (count, last frame), the xfer status, bytes and CRC, the module's 23 state words, object 1's transform and angle, objects 2-4's positions and
** physics velocities, the four captured flags and the module's next call frame. */
int ref_l9r50_run(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops,
	unsigned char *buf, int cap, int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r50Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	GlobalData globals;
	TheGlobalData = &globals;
	g_ddGravity = globals.m_gravity;
	ref_weapon_lookup_hook = kWeaponLookup;
	TheWeaponStore = &g_kStore;
	try { d = l9r50Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { ref_weapon_lookup_hook = 0; TheWeaponStore = 0; TheGlobalData = 0; g_ddGravity = 0; return status; }
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	auto weaponCode = [](const WeaponTemplate *t) -> int { return t == 0 ? 0 : (t == (const WeaponTemplate *)&g_kWeapons[0] ? 1 : 2); };
	{
		const R50PhysicsBehaviorModuleData *pd = (const R50PhysicsBehaviorModuleData *)d;
		const Real reals[13] = { pd->m_mass, pd->m_shockResistance, pd->m_shockMaxYaw, pd->m_shockMaxPitch, pd->m_shockMaxRoll, pd->m_forwardFriction,
			pd->m_lateralFriction, pd->m_ZFriction, pd->m_aerodynamicFriction, pd->m_centerOfMassOffset, pd->m_minFallSpeedForDamage,
			pd->m_fallHeightDamageFactor, pd->m_pitchRollYawFactor };
		for (int k = 0; k < 13; ++k) putU32(realBits(reals[k]));
		putU32(pd->m_killWhenRestingOnGround ? 1 : 0);
		putU32(pd->m_allowBouncing ? 1 : 0);
		putU32(pd->m_allowCollideForce ? 1 : 0);
		putI32(weaponCode(pd->m_vehicleCrashesIntoBuildingWeaponTemplate));
		putI32(weaponCode(pd->m_vehicleCrashesIntoNonBuildingWeaponTemplate));
	}
	initKindOfMasks();
	initDisabledMasks();
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	InitGameLogicRandom(cfg[1]);
	TheAudio = &g_ddAudio;
	g_audioHandleLast = 0;
	TheTerrainLogic = &g_hdTerrain;
	ScriptEngine script;
	TheScriptEngine = &script;
	g_modDestroyHook = r50Destroy;
	g_defectHook = r50Defect;
	g_xformHook = r50Xform;
	g_objXformSyncPos = true;
	Object *objs[4];
	MBody bodies[4];
	AIUpdateInterface ais[2];
	BehaviorModule *behaviors[4][2] = { { 0, 0 }, { 0, 0 }, { 0, 0 }, { 0, 0 } };
	R50PhysicsBehavior *phys[4] = { 0, 0, 0, 0 };
	const unsigned flags = cfg[2];
	for (int k = 0; k < 4; ++k) {
		Object *o = objs[k] = newObject();
		o->m_oid = (ObjectID)(k + 1);
		bodies[k].m_owner = o;
		bodies[k].m_front = (cfg[13] >> (2 * k) & 1) != 0;
		bodies[k].m_back = (cfg[13] >> (2 * k + 1) & 1) != 0;
		o->m_body = &bodies[k];
		o->m_behaviors = behaviors[k];
		unsigned kinds = cfg[3] >> (8 * k) & 255;
		o->m_kindOf.clear();
		const KindOfType kk[6] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_IMMOBILE, KINDOF_STRUCTURE, KINDOF_DRONE, KINDOF_PROJECTILE };
		for (int b = 0; b < 6; ++b) if (kinds >> b & 1) o->m_kindOf.set(kk[b]);
		o->m_crusher = (UnsignedByte)(cfg[4] >> (8 * k) & 15);
		o->m_crushable = (UnsignedByte)(cfg[4] >> (8 * k + 4) & 15);
		unsigned g = cfg[14] >> (8 * k) & 255;
		o->m_geometry = GeometryInfo((GeometryType)((g & 3) % 3), false, kR50Heights[g >> 5 & 3], kR50Radii[g >> 2 & 7], kR50Radii[g >> 2 & 7]);
		o->m_pos.x = r50F(cfg[15 + 3 * k]);
		o->m_pos.y = r50F(cfg[16 + 3 * k]);
		o->m_pos.z = r50F(cfg[17 + 3 * k]);
		o->m_orientation = k == 0 ? r50F(cfg[27]) : 0.0f;
		o->poseMatrix();
		o->m_name = AsciiString(k == 0 ? "UnitA" : "");
	}
	Object *me = objs[0];
	g_cpDrawable.m_ownerObject = me;
	me->m_drawable = (flags & 1) ? &g_cpDrawable : 0;
	ais[0].m_owner = me;
	ais[1].m_owner = objs[1];
	me->m_ai = (flags >> 1 & 1) ? &ais[0] : 0;
	objs[1]->m_ai = (flags >> 5 & 1) ? &ais[1] : 0;
	if (flags >> 6 & 1) me->m_containedBy = objs[3];
	if (flags >> 7 & 1) objs[1]->m_containedBy = me;
	if (flags >> 8 & 1) objs[3]->m_containedBy = objs[2];
	auto scene = [&]() {
		for (int k = 0; k < 4; ++k) {
			Object *o = objs[k];
			unsigned dis = cfg[5] >> (8 * k) & 255;
			o->m_disabledBits = 0;
			if (dis & 1) o->m_disabledBits |= 1u << DISABLED_UNMANNED;
			if (dis & 2) o->m_disabledBits |= 1u << DISABLED_HELD;
			if (dis & 4) o->m_disabledBits |= 1u << DISABLED_FREEFALL;
			unsigned st = cfg[6] >> (8 * k) & 255;
			const ObjectStatusTypes bits[4] = { OBJECT_STATUS_PARACHUTING, OBJECT_STATUS_BRAKING, OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_NO_COLLISIONS };
			for (int b = 0; b < 4; ++b) o->m_status.set(bits[b], (st >> b & 1) ? 1 : 0);
			o->m_effDead = (st >> 4 & 1) != 0;
			o->m_offMap = (st >> 5 & 1) != 0;
		}
		unsigned a = cfg[7];
		ais[0].m_r50LocoSet = (a & 255) == 255 ? -1 : (Int)(a & 255);
		ais[0].m_r50Ignored = (ObjectID)(a >> 8 & 255);
		ais[0].m_r50Process = (a >> 16 & 1) != 0;
		ais[0].m_r50Surfaces = a >> 20 & 15;
		ais[1].m_r50Ignored = (ObjectID)(cfg[8] & 255);
		for (int c = 0; c < 8; ++c) {
			g_hdLayer[c] = LAYER_GROUND;
			g_hdLayerHeight[c] = (Real)(cfg[9] >> (4 * c) & 15) * 0.5f;
			g_r41Cliff[c] = (cfg[10] >> c & 1) != 0;
			g_flrHasWater[c] = (cfg[10] >> (8 + c) & 1) != 0;
			g_flrWater[c] = r50F(cfg[11]);
		}
		globals.m_defaultStructureRubbleHeight = r50F(cfg[12]);
	};
	scene();
	gl.m_awakenHook = r50Awaken;
	g_r50Wakes = 0;
	g_r50WakeFrame = 0;
	R50PhysicsBehavior *m = 0;
	status = 0;
	try {
		g_r50Constructing = true;
		m = newInstance(R50PhysicsBehavior)(me, d);   /* (the memory pool's zero-filled block, as Thing's modules are made: ThingFactory/Object.cpp) */
		g_r50Constructing = false;
		g_r50Me = m;
		phys[0] = m;
		for (int k = 1; k < 4; ++k) if (flags >> (1 + k) & 1) phys[k] = newInstance(R50PhysicsBehavior)(objs[k], d);
	}
	CATCH_ALL(status, code, msg, msgCap)
	g_r50Constructing = false;
	if (status) return 5;
	for (int k = 0; k < 4; ++k) { behaviors[k][0] = phys[k]; objs[k]->m_r50Physics = phys[k]; }
	putU32((unsigned)g_r50Wakes);
	putU32(g_r50Wakes ? g_r50WakeFrame : 0);
	/* the Rust world schedules an update module when it makes the object (frame 1 at frame 0) for a constructor that sets no wake frame */
	for (int k = 0; k < 4; ++k) if (phys[k] && phys[k]->friend_getNextCallFrame() == 0) phys[k]->friend_setNextCallFrame(gl.m_frame ? gl.m_frame : 1);
	AudioEventRTS bounce(AsciiString("SND_A"), INVALID_ID);
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1 && ((o[1] >= 5 && o[1] <= 12) || (o[1] >= 18 && o[1] <= 26))) {
			cfg[o[1]] = o[2];
			if (o[1] >= 18) {
				int k = (int)(o[1] - 15) / 3;
				objs[k]->m_pos.x = r50F(cfg[15 + 3 * k]);
				objs[k]->m_pos.y = r50F(cfg[16 + 3 * k]);
				objs[k]->m_pos.z = r50F(cfg[17 + 3 * k]);
			}
		}
		if (o[0] == 2) gl.m_frame = o[1];
		scene();
		g_events.clear();
		g_ulog.clear();
		g_r50Wakes = 0;
		g_r50WakeFrame = 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[1024];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		Coord3D f3;
		f3.x = r50F(o[1]); f3.y = r50F(o[2]); f3.z = r50F(o[3]);
		Object *objB = (o[2] >= 1 && o[2] <= 4) ? objs[o[2] - 1] : 0;
		try {
			switch (o[0]) {
			case 0: g_r50Updating = m; res = (unsigned)m->update(); break;
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 1024, &blen, &crc); g_r50Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r50Saved;
				if (b.size() > 1024) b.resize(1024);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 6: m->applyForce(&f3); break;
			case 7: m->applyMotiveForce(&f3); break;
			case 8: m->applyShock(&f3); break;
			case 9: m->applyRandomRotation(); break;
			case 10: m->onCollide((o[1] >= 1 && o[1] <= 4) ? objs[o[1] - 1] : 0, 0, 0); break;
			case 11: {
				Real fb = r50F(o[2]);
				Bool bb = o[2] != 0;
				switch (o[1]) {
				case 0: m->setYawRate(fb); break;
				case 1: m->setPitchRate(fb); break;
				case 2: m->setRollRate(fb); break;
				case 3: m->setAllowBouncing(bb); break;
				case 4: m->setAllowCollideForce(bb); break;
				case 5: m->setStickToGround(bb); break;
				case 6: m->setAllowToFall(bb); break;
				case 7: m->setImmuneToFallingDamage(bb); break;
				case 8: m->setAllowAirborneFriction(bb); break;
				case 9: m->setStunned(bb); break;
				case 10: m->setIsInFreeFall(bb); break;
				case 11: m->setIgnoreCollisionsWith(objB); break;
				case 12: m->setTurning((R50PhysicsTurningType)(Int)o[2]); break;
				case 13: m->scrubVelocity2D(fb); break;
				case 14: m->scrubVelocityZ(fb); break;
				case 15: m->setMass(fb); break;
				case 16: m->setExtraBounciness(fb); break;
				case 17: m->setExtraFriction(fb); break;
				case 18: m->clearAcceleration(); break;
				case 19: m->setBounceSound(o[2] ? &bounce : 0); break;
				case 20: if (objB) m->addOverlap(objB); break;
				}
				break;
			}
			case 12: m->resetDynamicPhysics(); break;
			case 13: m->setAngles(f3.x, f3.y, f3.z); break;
			case 14: m->onObjectCreated(); break;
			case 15:
				switch (o[1]) {
				case 0: res = realBits(m->getMass()); break;
				case 1: res = realBits(m->getVelocityMagnitude()); break;
				case 2: res = realBits(m->getForwardSpeed2D()); break;
				case 3: res = realBits(m->getForwardSpeed3D()); break;
				case 4: res = m->isMotive() ? 1 : 0; break;
				case 5: res = (unsigned)m->getCurrentOverlap(); break;
				case 6: res = (unsigned)m->getPreviousOverlap(); break;
				case 7: res = (unsigned)m->getLastCollidee(); break;
				case 8: res = m->isIgnoringCollisionsWith((ObjectID)o[2]) ? 1 : 0; break;
				case 9: res = m->getAllowToFall() ? 1 : 0; break;
				case 10: res = m->getIsStunned() ? 1 : 0; break;
				case 11: res = m->getIsInFreeFall() ? 1 : 0; break;
				case 12: res = realBits(m->getCenterOfMassOffset()); break;
				case 13: res = m->wouldLikeToCollideWith(objB) ? 1 : 0; break;
				case 14: res = (unsigned)(Int)m->getTurning(); break;
				}
				break;
			case 16: {
				int k = (int)o[1] - 1;
				if (k >= 1 && k <= 3 && phys[k]) phys[k]->setIgnoreCollisionsWith(objB);
				break;
			}
			case 17: m->addVelocityTo(&f3); break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		g_r50Updating = 0;
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			/* (object 1's transform is dumped: its line is not) */
			if (e.kind == 100 && g_ulog[(size_t)e.p] != "settransform 1") log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)g_r50Wakes);
		putU32(g_r50Wakes ? g_r50WakeFrame : 0);
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		{
			const unsigned w[23] = { realBits(m->m_yawRate), realBits(m->m_rollRate), realBits(m->m_pitchRate), realBits(m->m_accel.x),
				realBits(m->m_accel.y), realBits(m->m_accel.z), realBits(m->m_prevAccel.x), realBits(m->m_prevAccel.y), realBits(m->m_prevAccel.z),
				realBits(m->m_vel.x), realBits(m->m_vel.y), realBits(m->m_vel.z), (unsigned)(Int)m->m_turning, (unsigned)m->m_ignoreCollisionsWith,
				(unsigned)m->m_flags, realBits(m->m_mass), (unsigned)m->m_currentOverlap, (unsigned)m->m_previousOverlap, (unsigned)m->m_lastCollidee,
				m->m_motiveForceExpires, realBits(m->m_extraBounciness), realBits(m->m_extraFriction), realBits(m->m_velMag) };
			for (int j = 0; j < 23; ++j) putU32(w[j]);
		}
		const Matrix3D &mx = *me->getTransformMatrix();
		for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) putU32(realBits(mx[r][c]));
		putU32(realBits(me->m_orientation));
		for (int k = 1; k < 4; ++k) {
			putU32(realBits(objs[k]->m_pos.x));
			putU32(realBits(objs[k]->m_pos.y));
			putU32(realBits(objs[k]->m_pos.z));
			putU32(phys[k] ? realBits(phys[k]->m_vel.x) : 0);
			putU32(phys[k] ? realBits(phys[k]->m_vel.y) : 0);
			putU32(phys[k] ? realBits(phys[k]->m_vel.z) : 0);
		}
		for (int k = 0; k < 4; ++k) putU32(objs[k]->m_r50Captured ? 1 : 0);
		putU32(m->friend_getNextCallFrame());
	}
	*len = finish(buf, cap);
	gl.m_awakenHook = 0;
	g_r50Me = 0;
	for (int k = 0; k < 4; ++k) {
		objs[k]->m_behaviors = 0;
		objs[k]->m_r50Physics = 0;
		objs[k]->m_body = 0;
		objs[k]->m_ai = 0;
		objs[k]->m_drawable = 0;
		objs[k]->m_containedBy = 0;
		delete phys[k];
		delete objs[k];
	}
	delete d;
	g_cpDrawable.m_ownerObject = 0;
	g_modDestroyHook = 0;
	g_defectHook = 0;
	g_xformHook = 0;
	g_objXformSyncPos = false;
	ref_weapon_lookup_hook = 0;
	TheWeaponStore = 0;
	TheGlobalData = 0;
	g_ddGravity = 0;
	TheAudio = 0;
	TheTerrainLogic = 0;
	TheScriptEngine = 0;
	TheGameLogic = 0;
	return 0;
}

}  /* extern "C" */
