/* Differential-test driver for MissileLauncherBuildingUpdate (p1w13-sonnet, batch R53; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MissileLauncherBuildingUpdate.cpp   (#included, unchanged)
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpecialPowerUpdateModule.cpp        (#included, unchanged: its base)
** with their headers unchanged, in wrap_module.cpp's unit, on the module build's stand-ins: the silo (id 1: maybe a drawable, whose model
** condition changes are lines naming every flag (batch R43's hook) and whose setAnimationLoopDuration is a line; maybe under construction;
** maybe a special power module of the test's: its template one of the store's SP_0..SP_3 or none, isReady and getReadyFrame the test's),
** Object::getSpecialPowerModule (Object.cpp, verbatim below), the FX lists (FX_A, FX_B; doFXPos a line), the audio manager (addAudioEvent
** and removeAudioEvent lines, the handles 1, 2, ...; isCurrentlyPlaying the test's answer, a line: batch R34's), the special power store
** (the REAL one the test loaded, its templates found by name while the data is read), the game logic (its frame), and the REAL
** UpdateModule, INI and Xfer. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpecialPowerUpdateModule.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/MissileLauncherBuildingUpdate.cpp"

/* Object.cpp 5314-5334, verbatim */
SpecialPowerModuleInterface *Object::getSpecialPowerModule( const SpecialPowerTemplate *specialPowerTemplate ) const
{

	// sanity
	if( specialPowerTemplate == NULL )
		return NULL;

	// search the modules for the one with the matching template
	for( BehaviorModule** m = m_behaviors; *m; ++m )
	{
		SpecialPowerModuleInterface* sp = (*m)->getSpecialPower();
		if (!sp)
			continue;

		if( sp->isModuleForPower( specialPowerTemplate ) )
			return sp;
	}

	return NULL;

}
void Drawable::setAnimationLoopDuration(UnsignedInt numFrames) { ulog("animloop %d %u", m_ownerObject ? (int)m_ownerObject->m_oid : 0, numFrames); }

namespace {

ModuleData *l9r53Read(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = MissileLauncherBuildingUpdate::friend_newModuleData(&ini);
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

/* a special power module whose template, readiness and ready frame are the test's */
class R53Power : public SpecialPowerModuleInterface {
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
	virtual void markSpecialPowerTriggered(const Coord3D *) {}
	virtual void startPowerRecharge() {}
	virtual const AudioEventRTS &getInitiateSound() const { static AudioEventRTS a; return a; }
	virtual Bool isScriptOnly() const { return false; }
	virtual const ThingTemplate *getReferenceThingTemplate() const { return 0; }
	const SpecialPowerTemplate *m_template = 0;
	Bool m_ready = false;
	UnsignedInt m_readyFrame = 0;
};
class R53PowerBehavior : public TestBehavior {
public:
	R53PowerBehavior(Thing *t, const ModuleData *d) : TestBehavior(t, d) {}
	virtual SpecialPowerModuleInterface *getSpecialPower() { return &m_sp; }
	R53Power m_sp;
};

std::vector<unsigned char> g_r53Saved;
const SpecialPowerTemplate *r53Template(unsigned k)
{
	if (k > 3) return 0;
	char name[8];
	sprintf(name, "SP_%u", k);
	return (const SpecialPowerTemplate *)ref_sp_find_raw(name);
}

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of a MissileLauncherBuildingUpdate and make the module on the silo (id 1) at frame cfg[0]. `cfg`
** (4 words): 0 the frame, 1 flags (0 a drawable, 1 under construction, 2 the special power module exists (fixed), 3 it is ready, 4
** isCurrentlyPlaying answers yes), 2 the module's template (0-3: SP_k; else none; fixed), 3 its ready frame. The data dump goes first (the
** template's id, the three times, the five FX lists by first appearance, the audio's name). Then `n` ops of 5 words [op, a, b, c, d]: 0
** update()  1 the frame = a  2 the frame += 1  3 xfer save  4 xfer load of the last save  5 xfer CRC  6 the flags = a (bit 2 kept)
** 7 the ready frame = a  8 initiateIntentToDoSpecialPower(SP_a, else NULL)  9 isActive(). Each op appends: the CATCH_ALL status, the result,
** the effect log, the xfer status, bytes and CRC, and the state (the door state, the timeout state and frame, the idle sound's playing handle,
** isActive). */
static int l9r53RunInner(const char *filename, const char *xfer_path, const unsigned *cfg_in, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	g_dump.clear();
	g_fxIds.clear();
	g_r53Saved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	ref_ini_sp_bind(ref_sp_find_raw);
	try { d = l9r53Read(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	ref_ini_sp_bind(0);
	if (status) return status;
	unsigned cfg[4];
	memcpy(cfg, cfg_in, sizeof(cfg));
	{
		const MissileLauncherBuildingUpdateModuleData *md = (const MissileLauncherBuildingUpdateModuleData *)d;
		putU32(ref_sp_raw_id(md->m_specialPowerTemplate));
		putU32(md->m_doorOpenTime);
		putU32(md->m_doorWaitOpenTime);
		putU32(md->m_doorClosingTime);
		const FXList *fxs[5] = { md->m_openingFX, md->m_openFX, md->m_waitingToCloseFX, md->m_closingFX, md->m_closedFX };
		for (int k = 0; k < 5; ++k) putI32(fxId(fxs[k]));
		putStr(md->m_openIdleAudio.getEventName().str(), (int)md->m_openIdleAudio.getEventName().getLength());
	}
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	TheAudio = &g_ddAudio;
	g_audioHandleLast = 0;
	g_mcHook = r43Mc;
	Object *me = newObject();
	me->m_oid = 1;
	Drawable draw;
	draw.m_ownerObject = me;
	UpdateModuleData behaviorData;
	R53PowerBehavior *power = 0;
	BehaviorModule *list[3] = { 0, 0, 0 };
	auto scene = [&]() {
		unsigned f = cfg[1];
		me->m_drawable = (f & 1) ? &draw : 0;
		me->m_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (f >> 1 & 1) ? 1 : 0);
		if (power) { power->m_sp.m_ready = (f >> 3 & 1) != 0; power->m_sp.m_readyFrame = cfg[3]; }
		g_r34Playing = (f >> 4 & 1) != 0;
	};
	MissileLauncherBuildingUpdate *m = 0;
	status = 0;
	try { m = newInstance(MissileLauncherBuildingUpdate)(me, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) { TheGameLogic = 0; TheAudio = 0; g_mcHook = 0; delete me; delete d; return 5; }
	list[0] = m;
	if (cfg[1] >> 2 & 1) {
		power = new R53PowerBehavior(me, &behaviorData);
		power->m_sp.m_template = r53Template(cfg[2]);
		list[1] = power;
	}
	me->m_behaviors = list;
	scene();
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 5 * i;
		if (o[0] == 1) gl.m_frame = o[1];
		if (o[0] == 2) gl.m_frame += 1;
		if (o[0] == 6) cfg[1] = (o[1] & ~4u) | (cfg[1] & 4u);
		if (o[0] == 7) cfg[3] = o[1];
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
			case 3: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_r53Saved.assign(bytes, bytes + blen); break;
			case 4: {
				std::vector<unsigned char> b = g_r53Saved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 5: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 8: res = m->initiateIntentToDoSpecialPower(r53Template(o[1]), 0, 0, 0, 0) ? 1 : 0; break;
			case 9: res = m->isActive() ? 1 : 0; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
		putU32(res);
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)m->m_doorState);
		putU32((unsigned)m->m_timeoutState);
		putU32(m->m_timeoutFrame);
		putU32(m->m_openIdleAudio.getPlayingHandle());
		putU32(m->isActive() ? 1 : 0);
	}
	*len = finish(buf, cap);
	me->m_behaviors = 0;
	me->m_drawable = 0;
	delete m;
	delete power;
	delete me;
	delete d;
	g_mcHook = 0;
	g_r34Playing = false;
	TheAudio = 0;
	TheGameLogic = 0;
	return 0;
}

int ref_l9r53_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9r53RunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
