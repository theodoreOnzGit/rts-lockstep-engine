/* Differential-test driver for BattlePlanUpdate (p1w13-sonnet, batch R71; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BattlePlanUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit, at its end (batch R50's destroyObject rename), on the module build's stand-ins: the
** strategy centre (id 1: a team of three with its player, maybe an AI whose turret is the test's and whose orders are lines, maybe a drawable,
** maybe a body whose maximum health is the test's and whose setMaxHealth is a line, maybe a REAL special power module of SP_0..SP_3 (batch
** R60's way), maybe a REAL StealthDetectorUpdate (batch R27's), its vision and shroud clearing ranges) and four troops (kinds; the player's
** objects the test lists), Object::getSpecialPowerModule (batch R53's, verbatim), the REAL special power store, the player's changeBattlePlan
** and academy record (lines), the radar, the message, the audio and the drawable (lines), Object::setDisabledUntil (batch R13's line),
** setWeaponSetFlag and setWeaponLock (lines), UpdateModule, INI, Xfer and logic random values; the game logic (its frame, batch R35's wake
** calls). */
#define destroyObject r50DestroyObject   /* (as in l9_physics_r50.inc: the inline one loses to other units' out-of-line ones) */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BattlePlanUpdate.cpp"
#undef destroyObject

namespace {

ModuleData *l9r71Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = BattlePlanUpdate::friend_newModuleData(&ini);
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

/* a REAL special power module (getSpecialPowerModule finds it by its template) */
class R71Power : public SpecialPowerModule {
public:
	R71Power(Thing *t, const ModuleData *d) : SpecialPowerModule(t, d) {}
	virtual ~R71Power() {}
};
/* the centre's body: batch Q's, its maximum health the test's, setMaxHealth a line */
class R71Body : public SdBody
{
public:
	R71Body(Thing *t, const ModuleData *md) : SdBody(t, md) {}
	virtual void setMaxHealth( Real maxHealth, MaxHealthChangeType healthChangeType )
	{
		ulog("setmaxhealth %d %08x %d", (int)getObject()->m_oid, realBits(maxHealth), (int)healthChangeType);
		m_max = maxHealth;
	}
};

std::vector<unsigned char> g_r71Saved;
Object *g_r71Objs[5];
Team g_r71Teams[3];
Player g_r71Players[3];
const KindOfType g_r71Kinds[3] = { KINDOF_INFANTRY, KINDOF_VEHICLE, KINDOF_STRUCTURE };
Real r71F(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }
const SpecialPowerTemplate *r71Template(unsigned k)
{
	if (k == 0 || k > 4) return 0;
	char name[8];
	sprintf(name, "SP_%u", k - 1);
	return (const SpecialPowerTemplate *)ref_sp_find_raw(name);
}
unsigned r71TemplateCode(const SpecialPowerTemplate *t) { for (unsigned k = 1; k <= 4; ++k) if (t && t == r71Template(k)) return k; return 0; }
Object *r71Find(ObjectID id) { return ((int)id >= 1 && (int)id <= 5) ? g_r71Objs[(int)id - 1] : 0; }
void r71Destroy(Object *obj) { ulog("destroy %d", (int)obj->m_oid); }
void r71UiMessage(const char *label) { ulog("uimessage %s", label); }
UpdateModule *g_r71Detector = 0;
/* findUpdateModule: the detector, whatever the key (the module's only call asks for "StealthDetectorUpdate"; its key is a function-local
** static made in the first case, and the name key generator is reset for every case, so it is not this case's key for that name) */
UpdateModule *r71FindUpdate(const Object *, NameKeyType)
{
	return g_r71Detector;
}

}  // namespace

void Player::changeBattlePlan( Int plan, Int delta, BattlePlanBonuses *bonus )
{
	ulog("battleplan %d %d %d %08x %d %d %d %08x", (int)m_idx, (int)plan, (int)delta, realBits(bonus->m_armorScalar), (int)bonus->m_bombardment,
		(int)bonus->m_searchAndDestroy, (int)bonus->m_holdTheLine, realBits(bonus->m_sightRangeScalar));
}
void PowerAcademyStats::recordBattlePlanSelected() { ulog("academyplan %d", (int)((Player *)(void *)this)->m_idx); }
void Radar::createPlayerEvent( Player *player, const Coord3D *world, RadarEventType type, Real secondsToLive )
{
	ulog("radarplayer %d %08x %08x %08x %d", player ? (int)player->m_idx : -1, realBits(world->x), realBits(world->y), realBits(world->z), (int)type);
}
void AudioManager::getInfoForAudioEvent( const AudioEventRTS *eventToFindAndFill ) const {}   /* (the client's: nothing the logic reads) */
void AIUpdateInterface::setTurretEnabled( WhichTurretType tur, Bool enabled ) { ulog("turretenable %d %d %d", (int)m_owner->m_oid, (int)tur, (int)enabled); }
void AIUpdateInterface::recenterTurret( WhichTurretType tur ) { ulog("turretrecenter %d %d", (int)m_owner->m_oid, (int)tur); }
Bool AIUpdateInterface::isTurretInNaturalPosition( WhichTurretType tur ) const
{
	ulog("turretnatural %d %d", (int)m_owner->m_oid, (int)tur);
	return m_turretNatural;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a BattlePlanUpdate and make the module on object 1 of five at frame cfg[0], then
** onObjectCreated. `cfg` (32 words): 1 the logic random seed; read once, 2 the centre: bits 0-1 its team (0 none, else 1-3), bit 2 an AI, bit 3 a
** drawable, bit 4 a body, bits 8-10 its special power module (0 none, else SP_(k-1)), bit 11 a StealthDetectorUpdate, bit 12 initially
** disabled; 8 its vision range, 9 its shroud clearing range, 10 its body's maximum health (float bits); also settable by op 2, 3 the AI:
** bits 0-1 the turret for the current weapon (0 and 3: TURRET_INVALID, 1 the main, 2 the alternate), bit 2 the turret in its natural
** position; 4-7 objects 2-5: bits 0-2 their kinds (of g_r71Kinds), bit 3 one of the player's objects (in order). The data dump goes first.
** Then onObjectCreated's effects, then `n` ops of 5 words [op, a, b, c, d]: 0 update()  1 the frame = a  2 cfg[a] = b  3
** initiateIntentToDoSpecialPower(SP_(a-1), else NULL; options b)  4 xfer save  5 xfer load of the last save  6 xfer CRC  7 onDelete  8 the
** queries. Each op appends: the CATCH_ALL status, the result, the effect log, the xfer status, bytes and CRC, the state (as the Rust
** state_words), the centre's vision and shroud ranges and its body's maximum health, and the detector's
** enabled and next call frame. */
static int l9r71RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_r71Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	ref_ini_sp_bind(ref_sp_find_raw);   /* (INI::parseSpecialPowerTemplate reads the real store: upgrade batch C's binding) */
	try { d = l9r71Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_ini_sp_bind(0);
	if (status) return status;
	unsigned cfg[32];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const BattlePlanUpdateModuleData *bd = (const BattlePlanUpdateModuleData *)d;
		putU32(r71TemplateCode(bd->m_specialPowerTemplate));
		const UnsignedInt frames[5] = { bd->m_bombardmentPlanAnimationFrames, bd->m_holdTheLinePlanAnimationFrames, bd->m_searchAndDestroyPlanAnimationFrames,
			bd->m_transitionIdleFrames, bd->m_battlePlanParalyzeFrames };
		for (int i = 0; i < 5; ++i) putU32(frames[i]);
		const AsciiString *names[14] = { &bd->m_bombardmentUnpackName, &bd->m_bombardmentPackName, &bd->m_bombardmentMessageLabel, &bd->m_bombardmentAnnouncementName,
			&bd->m_searchAndDestroyUnpackName, &bd->m_searchAndDestroyIdleName, &bd->m_searchAndDestroyPackName, &bd->m_searchAndDestroyMessageLabel,
			&bd->m_searchAndDestroyAnnouncementName, &bd->m_holdTheLineUnpackName, &bd->m_holdTheLinePackName, &bd->m_holdTheLineMessageLabel,
			&bd->m_holdTheLineAnnouncementName, &bd->m_visionObjectName };
		for (int i = 0; i < 14; ++i) putStr(names[i]->str(), names[i]->getLength());
		for (int i = 0; i < KINDOF_COUNT; ++i) if (bd->m_validMemberKindOf.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		for (int i = 0; i < KINDOF_COUNT; ++i) if (bd->m_invalidMemberKindOf.test((KindOfType)i)) putU32((unsigned)i);
		putU32(0xFFFFFFFFu);
		const Real reals[4] = { bd->m_holdTheLineArmorDamageScalar, bd->m_searchAndDestroySightRangeScalar, bd->m_strategyCenterSearchAndDestroySightRangeScalar,
			bd->m_strategyCenterHoldTheLineMaxHealthScalar };
		for (int i = 0; i < 4; ++i) putU32(realBits(reals[i]));
		putU32(bd->m_strategyCenterSearchAndDestroyDetectsStealth ? 1 : 0);
		putU32((unsigned)bd->m_strategyCenterHoldTheLineMaxHealthChangeType);
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	gl.m_awakenHook = r35Awaken;
	InitGameLogicRandom(cfg[1]);
	Radar radar;
	TheRadar = &radar;
	AbAudio audio;
	TheAudio = &audio;
	g_audioHandleLast = 0;
	AbUi ui;
	TheInGameUI = &ui;
	GameTextInterface text;
	TheGameText = &text;
	BehaviorModule *none[1] = { 0 };
	for (int k = 0; k < 5; ++k) {
		g_r71Objs[k] = newObject();
		g_r71Objs[k]->m_oid = (ObjectID)(k + 1);
		g_r71Objs[k]->m_behaviors = none;
	}
	for (int t = 0; t < 3; ++t) {
		g_r71Players[t].m_idx = t;
		g_r71Teams[t].m_dummy = t + 1;
		g_r71Teams[t].m_controller = &g_r71Players[t];
	}
	Object *me = g_r71Objs[0];
	AIUpdateInterface ai;
	ai.m_owner = me;
	Drawable draw;
	draw.m_ownerObject = me;
	R71Body *body = new R71Body(me, d);
	body->m_max = r71F(cfg[10]);
	me->m_vision = r71F(cfg[8]);
	me->m_shroudRange = r71F(cfg[9]);
	unsigned t = cfg[2] & 3;
	me->m_team = t ? &g_r71Teams[t - 1] : 0;
	me->m_player = t ? &g_r71Players[t - 1] : 0;
	/* the detector (made first: its constructor draws a random phase, as the Rust side's does at the object's creation) */
	StealthDetectorUpdateModuleData *sdd = 0;
	StealthDetectorUpdate *detector = 0;
	if (cfg[2] >> 11 & 1) {
		sdd = new StealthDetectorUpdateModuleData;
		sdd->m_initiallyDisabled = (cfg[2] >> 12 & 1) != 0;
		detector = newInstance(StealthDetectorUpdate)(me, sdd);
	}
	g_r71Detector = detector;
	auto scene = [&]() {
		me->m_ai = (cfg[2] >> 2 & 1) ? &ai : 0;
		me->m_drawable = (cfg[2] >> 3 & 1) ? &draw : 0;
		me->m_body = (cfg[2] >> 4 & 1) ? body : 0;
		unsigned tu = cfg[3] & 3;
		ai.m_curTurret = (tu == 1) ? 0 : ((tu == 2) ? 1 : -1);
		ai.m_turretNatural = (cfg[3] >> 2 & 1) != 0;
		Player *pl = me->m_player;
		if (pl) pl->m_objects.clear();
		for (int k = 1; k < 5; ++k) {
			Object *o = g_r71Objs[k];
			unsigned f = cfg[3 + k];
			o->m_kindOf = KindOfMaskType();
			for (int b = 0; b < 3; ++b) if (f >> b & 1) o->m_kindOf.set(g_r71Kinds[b]);
			o->m_tmpl.m_kindOf = o->m_kindOf;
			if ((f >> 3 & 1) && pl) pl->m_objects.push_back(o);
		}
	};
	scene();
	g_modFindHook = r71Find;
	g_modDestroyHook = r71Destroy;
	g_uiMessageHook = r71UiMessage;
	g_findUpdateHook = r71FindUpdate;
	auto putLog = [&]() {
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
	};
	BattlePlanUpdate *m = 0;
	try { m = newInstance(BattlePlanUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return 5;   /* (the data is never NULL here: not reached) */
	SpecialPowerModuleData *pd = 0;
	R71Power *power = 0;
	if (r71Template(cfg[2] >> 8 & 7)) {
		pd = new SpecialPowerModuleData;
		pd->m_specialPowerTemplate = r71Template(cfg[2] >> 8 & 7);
		power = new R71Power(me, pd);
	}
	BehaviorModule *behaviors[4] = { m, 0, 0, 0 };
	int nb = 1;
	if (power) behaviors[nb++] = power;
	if (detector) behaviors[nb++] = detector;
	me->m_behaviors = behaviors;
	g_events.clear();
	g_ulog.clear();
	{
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		try { m->onObjectCreated(); }
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
	}
	putLog();
	auto putState = [&]() {
		putU32((unsigned)m->m_status);
		putU32((unsigned)m->m_currentPlan);
		putU32((unsigned)m->m_desiredPlan);
		putU32((unsigned)m->m_planAffectingArmy);
		putU32(m->m_nextReadyFrame);
		putU32(m->m_invalidSettings ? 1 : 0);
		putU32(m->m_centeringTurret ? 1 : 0);
		putU32(realBits(m->m_bonuses->m_armorScalar));
		putU32((unsigned)m->m_bonuses->m_bombardment);
		putU32((unsigned)m->m_bonuses->m_searchAndDestroy);
		putU32((unsigned)m->m_bonuses->m_holdTheLine);
		putU32(realBits(m->m_bonuses->m_sightRangeScalar));
		putU32((unsigned)m->m_visionObjectID);
		const AudioEventRTS *sounds[7] = { &m->m_bombardmentUnpack, &m->m_bombardmentPack, &m->m_searchAndDestroyUnpack, &m->m_searchAndDestroyIdle,
			&m->m_searchAndDestroyPack, &m->m_holdTheLineUnpack, &m->m_holdTheLinePack };
		for (int i = 0; i < 7; ++i) putU32(sounds[i]->getPlayingHandle());
		/* (no module call frame: the module sets no wake, so it is the game logic's registration, which this driver does not do) */
		putU32(realBits(me->getVisionRange()));
		putU32(realBits(me->getShroudClearingRange()));
		putU32(realBits(body->m_max));
		putU32(detector ? (detector->isSDEnabled() ? 1 : 0) : 2);
		putU32(detector ? detector->friend_getNextCallFrame() : 0);
	};
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 2 && o[1] >= 3 && o[1] <= 7) cfg[o[1]] = o[2];
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
			case 3: res = m->initiateIntentToDoSpecialPower(r71Template(o[1]), 0, 0, 0, o[2]) ? 1 : 0; break;
			case 4: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r71Saved.assign(bytes, bytes + blen); break;
			case 5: {
				std::vector<unsigned char> b = g_r71Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				m->loadPostProcess();
				break;
			}
			case 6: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 7: m->onDelete(); break;
			case 8: res = (unsigned)m->getCommandOption() | (m->isActive() ? 0x100u : 0u) | ((unsigned)m->getActiveBattlePlan() << 12); break;
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
	gl.m_awakenHook = 0;
	g_modFindHook = 0;
	g_modDestroyHook = 0;
	g_uiMessageHook = 0;
	g_findUpdateHook = 0;
	g_r71Detector = 0;
	me->m_behaviors = 0;
	me->m_ai = 0;
	me->m_drawable = 0;
	me->m_body = 0;
	if (me->m_player) me->m_player->m_objects.clear();
	delete m;
	delete power;
	delete pd;
	delete detector;
	delete sdd;
	delete body;
	delete d;
	for (int k = 0; k < 5; ++k) {
		Object *x = g_r71Objs[k];
		x->m_team = 0; x->m_player = 0; x->m_behaviors = 0;
		delete x;
		g_r71Objs[k] = 0;
	}
	TheGameText = 0;
	TheInGameUI = 0;
	TheAudio = 0;
	TheRadar = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r71_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r71RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
