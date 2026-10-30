/* Differential-test driver for three layer-9 die modules (p1w11-sonnet, die batch D; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DamDie.cpp, SpecialPowerCompletionDie.cpp, EjectPilotDie.cpp   (#included, unchanged)
** with their headers unchanged, in wrap_module.cpp's unit (it is #included at that file's end, after l9_upgrades.inc, whose effect log and
** helpers it uses, and l9_helpers.inc, whose modhelp_find it answers `findObjectByID` with) on the stand-ins of differential/shim_module: a game
** logic list of objects (newest first) whose water waves can be cleared of disabled bits, a script engine that logs the completed special power,
** an object creation list store with OCL_A and OCL_B (l9_upgrades_b.inc), a template with the per-unit sounds the test gives it, an audio manager
** that logs the events, and the REAL DieModule, INI and (for the template field of SpecialPowerCompletionDie) the REAL SpecialPowerStore of
** wrap_upgrade.cpp behind the hook of wrap_ini.cpp. Every call a module makes is a line of the effect log (ulog). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DamDie.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/SpecialPowerCompletionDie.cpp"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/EjectPilotDie.cpp"

namespace {

Object *g_ddFirst = 0;                 /* the head of the game logic's object list (newest first) */
Object *ddFind(ObjectID id)
{
	for (Object *o = g_ddFirst; o; o = o->m_next) if ((ObjectID)o->m_oid == id) return o;
	return 0;
}
Real g_ddGravity = 0;

class DdAudio : public AudioManager {
public:
	virtual void stopAudio(AudioAffect which) {}
};
DdAudio g_ddAudio;
AudioEventRTS g_ddNoSound;
ScriptEngine g_ddScript;

}  // namespace

/* the object list and the disabled bits of an object */
Object *GameLogic::getFirstObject() { return g_ddFirst; }
void Object::clearDisabled(DisabledType t)
{
	ulog("cleardisabled %d %d", (int)m_oid, (int)t);
	m_disabledBits &= ~(1u << (unsigned)t);
}
Bool Object::isSignificantlyAboveTerrain() const { return m_pos.z > (Real)(-(3 * 3)) * g_ddGravity; }
const AudioEventRTS *(*g_perUnitSoundHook)(const AsciiString &) = 0;   /* batch R39: StickyBombUpdate's two sounds (null: none) */
const AudioEventRTS *ModTestThingTemplate::getPerUnitSound(const AsciiString &name) const
{
	if (name.isEmpty()) return &g_ddNoSound;
	if (g_perUnitSoundHook) { const AudioEventRTS *e = g_perUnitSoundHook(name); if (e) return e; }
	if (strcmp(name.str(), "VoiceEject") == 0) return &m_voiceEject;
	if (strcmp(name.str(), "SoundEject") == 0) return &m_soundEject;
	return &g_ddNoSound;
}
void ScriptEngine::notifyOfCompletedSpecialPower(Int playerIndex, const AsciiString &specialPowerName, ObjectID sourceObject)
{
	ulog("script %d %s %d", (int)playerIndex, specialPowerName.str(), (int)sourceObject);
}
ScriptEngine *TheScriptEngine = 0;
AudioManager *TheAudio = 0;
UnsignedInt g_audioHandleLast = 0;   /* p1w12-sonnet: the handles the audio manager gives out, 1, 2, 3, ... (FlammableUpdate keeps one); die batch D ignores it */
UnsignedInt AudioManager::addAudioEvent(const AudioEventRTS *eventToAdd)
{
	char pos[64];
	if (eventToAdd->m_hasPos) sprintf(pos, "%u,%u,%u", realBits(eventToAdd->m_pos.x), realBits(eventToAdd->m_pos.y), realBits(eventToAdd->m_pos.z));
	else strcpy(pos, "-");
	if (eventToAdd->m_drawableID)   /* batch R75: an event at a client drawable */
		ulog("audio %s %s %d %d draw %u", eventToAdd->getEventName().str(), pos, (int)eventToAdd->m_playerIndex, (int)eventToAdd->m_objectID, eventToAdd->m_drawableID);
	else if (eventToAdd->m_hasVolume)   /* batch R80: an event a volume was set on */
		ulog("audio %s %s %d %d vol %08x", eventToAdd->getEventName().str(), pos, (int)eventToAdd->m_playerIndex, (int)eventToAdd->m_objectID, realBits(eventToAdd->m_volume));
	else
		ulog("audio %s %s %d %d", eventToAdd->getEventName().str(), pos, (int)eventToAdd->m_playerIndex, (int)eventToAdd->m_objectID);
	return ++g_audioHandleLast;
}

namespace {

ModuleData *l9ddRead(int kind, const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			switch (kind) {
			case 0: data = DamDie::friend_newModuleData(&ini); break;
			case 1: data = SpecialPowerCompletionDie::friend_newModuleData(&ini); break;
			default: data = EjectPilotDie::friend_newModuleData(&ini); break;
			}
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

std::vector<unsigned char> g_ddSaved;

}  // namespace

extern "C" {

/* Read `filename` (block `D`) as the data of die module `kind` (0 DamDie, 1 SpecialPowerCompletionDie, 2 EjectPilotDie) and make the module on an
** Object (id cfg[0]) whose controlling player is player cfg[1] of three (-1 none). `cfg`: [the object's id, its player, the number of other objects
** (0..4), which of the objects have KINDOF_WAVEGUIDE (bit 0 the object under test, 1..4 the others, 5 the damage dealer), their initial disabled bits
** (4 bits each, objects 1..4), has a damage dealer (the last object), gravity (float bits), the names of the per-unit sounds VoiceEject and SoundEject
** (0 none, else "V<n>" / "S<n>")]. The others are created after the object under test, the damage dealer last; the list is newest first.
** The data dump goes first (the die mux, then the module's own fields). Then `n` ops of 6 words [op, a, b, c, d, e]: 0 onDie (death type a,
** veterancy b, status words c d, the damage source's id e)  1 setCreator(a)  2 xfer save  3 xfer load of the last save  4 xfer CRC  5 the object's
** position = (a, b, c) float bits  6 the controlling player = a - 1 (0 none)  7 object a's disabled bits = b. Each op appends: the CATCH_ALL
** status, the effect log (count, then each as a string), the xfer status, its bytes (for a save) and CRC. */
int ref_l9dd_run(int kind, const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap,
	int *len, char *msg, int msgCap, unsigned *code)
{
	initDisabledMasks();
	g_dump.clear();
	g_fxIds.clear();
	g_ddSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	if (kind == 1) ref_ini_sp_bind(ref_sp_find_raw);
	try { d = l9ddRead(kind, filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (kind == 1) ref_ini_sp_bind(0);
	if (status) return status;
	putDieMux(((DieModuleData *)d)->m_dieMuxData);
	if (kind == 1) {
		const SpecialPowerTemplate *t = ((SpecialPowerCompletionDieModuleData *)d)->m_specialPowerTemplate;
		putU32(ref_sp_raw_id(t));
	}
	if (kind == 2) {
		const EjectPilotDieModuleData *e = (EjectPilotDieModuleData *)d;
		putI32(e->m_oclInAir == 0 ? 0 : (e->m_oclInAir == &g_upOcls[0] ? 1 : 2));
		putI32(e->m_oclOnGround == 0 ? 0 : (e->m_oclOnGround == &g_upOcls[0] ? 1 : 2));
		putU32(e->m_invulnerableTime);
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) players[i].m_idx = i;
	memcpy(&g_ddGravity, &cfg[6], 4);
	unsigned nOthers = cfg[2] > 4 ? 4 : cfg[2];
	unsigned total = 1 + nOthers + (cfg[5] ? 1 : 0);
	std::vector<Object *> objs;
	for (unsigned i = 0; i < total; ++i) {
		Object *o = newObject();
		o->m_oid = (Int)(cfg[0] + i);
		if (cfg[3] >> i & 1) o->m_kindOf.set(KINDOF_WAVEGUIDE);
		o->m_disabledBits = (i >= 1 && i <= 4) ? (cfg[4] >> (4 * (i - 1))) & 15 : 0;
		o->m_next = i == 0 ? 0 : objs[i - 1];     /* newest first: each later object points at the one before it */
		objs.push_back(o);
	}
	g_ddFirst = objs[total - 1];
	Object *obj = objs[0];
	obj->m_player = (int)cfg[1] >= 0 ? &players[cfg[1]] : 0;
	{
		AsciiString v = cfg[7] ? AsciiString("V") : AsciiString();
		char nm[16];
		sprintf(nm, "V%u", cfg[7]);
		obj->m_tmpl.m_voiceEject.setEventName(cfg[7] ? AsciiString(nm) : AsciiString());
		sprintf(nm, "S%u", cfg[8]);
		obj->m_tmpl.m_soundEject.setEventName(cfg[8] ? AsciiString(nm) : AsciiString());
	}
	g_modFindHook = ddFind;                          /* findObjectByID finds the objects of the list by id */
	GameLogic gl;
	TheGameLogic = &gl;
	TheAudio = &g_ddAudio;
	TheScriptEngine = &g_ddScript;
	Module *m = 0;
	switch (kind) {
	case 0: m = new DamDie(obj, d); break;
	case 1: m = new SpecialPowerCompletionDie(obj, d); break;
	default: m = new EjectPilotDie(obj, d); break;
	}
	for (int i = 0; i < n; ++i) {
		const unsigned *o = ops + 6 * i;
		g_events.clear();
		g_ulog.clear();
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		try {
			switch (o[0]) {
			case 0: {
				DamageInfo info;
				info.in.m_deathType = (DeathType)o[1];
				info.in.m_sourceID = (ObjectID)o[5];
				obj->m_level = (VeterancyLevel)o[2];
				obj->m_status = maskFromU32<ObjectStatusMaskType>(OBJECT_STATUS_COUNT, o + 3);
				dynamic_cast<DieModuleInterface *>(m)->onDie(&info);
				break;
			}
			case 1: if (kind == 1) ((SpecialPowerCompletionDie *)m)->setCreator((ObjectID)o[1]); break;
			case 2: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_ddSaved.assign(bytes, bytes + blen); break;
			case 3: {
				std::vector<unsigned char> b = g_ddSaved;
				if (b.size() > 512) b.resize(512);
				xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
				break;
			}
			case 4: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
			case 5: { memcpy(&obj->m_pos.x, &o[1], 4); memcpy(&obj->m_pos.y, &o[2], 4); memcpy(&obj->m_pos.z, &o[3], 4); break; }
			case 6: obj->m_player = o[1] ? &players[o[1] - 1] : 0; break;
			case 7: if (o[1] >= 1 && o[1] <= nOthers) objs[o[1]]->m_disabledBits = o[2] & 15; break;
			}
		}
		CATCH_ALL(st, &c2, m2, 16)
		putI32(st);
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
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	for (unsigned i = 0; i < total; ++i) delete objs[i];
	g_ddFirst = 0;
	g_modFindHook = 0;
	TheGameLogic = 0;
	TheAudio = 0;
	TheScriptEngine = 0;
	return 0;
}

}  /* extern "C" */
