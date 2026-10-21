/* Differential-test driver for FlammableUpdate (p1w12-sonnet, update batch N; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FlammableUpdate.cpp   (#included, unchanged)
** (with one mechanical edit, see below) with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with status bits, a Drawable (it logs the
** model-condition calls), a body (MBody, l9_body_m.inc: it logs the damage and setAflame it is given), an audio manager that hands out handles 1, 2, 3, ...
** and logs, the REAL FireSpreadUpdate (l9_fire_spread_o.inc; found through the findUpdateModule hook), the game logic whose frame the test sets and
** whose wake recorder counts the calls setWakeFrame makes, and the REAL UpdateModule, INI and Xfer. */
#include "FlammableUpdate_nokey.cpp"   /* FlammableUpdate.cpp with the `static` of its name key removed by build.rs (mod_gen) */

namespace {

ModuleData *l9flRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = FlammableUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_flSaved;
Drawable g_flDrawable;
FireSpreadUpdate *g_flSpread = 0;   /* the REAL FireSpreadUpdate (l9_fire_spread_o.inc), made when the case has one */
AudioManager *g_flAudio = 0;

/* findUpdateModule("FireSpreadUpdate"): the case's FireSpreadUpdate, if it has one */
UpdateModule *flFindHook(const Object *, NameKeyType key)
{
	if (g_flSpread && key == TheNameKeyGenerator->nameToKey("FireSpreadUpdate")) return g_flSpread;
	return 0;
}

}  // namespace

void Drawable::clearModelConditionState(ModelConditionFlagType a) { ulog("mcclear %d", (int)a); }
void Object::clearModelConditionState(ModelConditionFlagType a) { if (m_drawable) m_drawable->clearModelConditionState(a); }
/* Object.cpp:1818-1823; the shock wave that follows is for damage that has one, which the test's does not */
void Object::attemptDamage( DamageInfo *damageInfo )
{
	BodyModuleInterface* body = getBodyModule();
	if (body)
		body->attemptDamage( damageInfo );
}
void AudioManager::removeAudioEvent( UnsignedInt audioEvent ) { ulog("audioremove %u", audioEvent); }

extern "C" {

/* Read `filename` (block `D`) as the data of a FlammableUpdate and make the module on an Object with a drawable, a body (health cfg[1] float bits, maximum
** cfg[2]; none if cfg[3] is 0) and a FireSpreadUpdate if cfg[4] is not 0, at the frame cfg[0]. The data dump goes first (the fields). Then the constructor's
** wake calls. Then `n` ops of 5 words [op, a, b, ...]: 0 update()  1 onDamage (damage type a, actual damage dealt b as float bits)  2 tryToIgnite  3 wouldIgnite
** 4 the frame = a  5 the object's status bits AFLAME (a & 1) and BURNED (a & 2) = b  6 xfer save  7 xfer load of the last save  8 xfer CRC. Each op appends: the CATCH_ALL status, the result (the sleep time update() returned, or the Bool of wouldIgnite), the effect log (count,
** then each as a string: "mc <n>", "mcclear <n>", "damage <object> <amount> <type> <death> <source>", "aflame <0|1>", "audio <name> <pos> <player> <object>",
** "audioremove <handle>"), the number of wake calls and the frame of the last, the xfer status, bytes and CRC, the module's seven state words, the
** object's AFLAME and BURNED bits and its body's health bits. */
static int l9flRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_flSaved.clear();
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9flRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const FlammableUpdateModuleData *fd = (const FlammableUpdateModuleData *)d;
		putU32(fd->m_burnedDelay);
		putU32(fd->m_aflameDuration);
		putU32(fd->m_aflameDamageDelay);
		putI32(fd->m_aflameDamageAmount);
		putStr(fd->m_burningSoundName.str(), fd->m_burningSoundName.getLength());
		putU32(realBits(fd->m_flameDamageLimitData));
		putU32(fd->m_flameDamageExpirationDelay);
	}
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_drawable = &g_flDrawable;
	MBody body;
	body.m_owner = obj;
	memcpy(&body.m_health, &cfg[1], 4);
	memcpy(&body.m_max, &cfg[2], 4);
	obj->m_body = cfg[3] ? &body : 0;
	InitGameLogicRandom(0);
	g_findUpdateHook = flFindHook;
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	AudioManager *saveAudio = TheAudio;
	struct FlAudio : public AudioManager { void stopAudio(AudioAffect) {} } audio;
	TheAudio = &audio;
	g_audioHandleLast = 0;
	g_events.clear();
	g_ulog.clear();
	FlammableUpdate *m = 0;
	FireSpreadUpdateModuleData *spreadData = 0;
	g_flSpread = 0;
	status = 0;
	/* the module order of the Rust side: the spread module (if any), then this one */
	try {
		if (cfg[4]) {
			spreadData = (FireSpreadUpdateModuleData *)FireSpreadUpdate::friend_newModuleData(0);
			g_flSpread = new FireSpreadUpdate(obj, spreadData);
		}
		m = new FlammableUpdate(obj, d);
	}
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0; TheAudio = saveAudio; g_findUpdateHook = 0; g_flSpread = 0;
		return 5;
	}
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		unsigned res = 0;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			gl.m_awakenCalls = 0;
			gl.m_awakenFrame = 0;
			try {
				switch (o[0]) {
				case 0: res = (unsigned)m->update(); break;
				case 1: {
					DamageInfo info;
					info.in.m_damageType = (DamageType)o[1];
					memcpy(&info.out.m_actualDamageDealt, &o[2], 4);
					m->onDamage(&info);
					break;
				}
				case 2: m->tryToIgnite(); break;
				case 3: res = m->wouldIgnite() ? 1 : 0; break;
				case 4: gl.m_frame = o[1]; break;
				case 5: {
					ObjectStatusMaskType a, b;
					a.set(OBJECT_STATUS_AFLAME);
					b.set(OBJECT_STATUS_BURNED);
					if (o[2] & 1) obj->m_status.set(a); else obj->m_status.clear(a);
					if (o[2] & 2) obj->m_status.set(b); else obj->m_status.clear(b);
					break;
				}
				case 6: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_flSaved.assign(bytes, bytes + blen); break;
				case 7: {
					std::vector<unsigned char> b = g_flSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				case 8: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				default: break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
			putI32(st);
			putU32(res);
		}
		std::vector<std::string> log;
		for (size_t j = 0; j < g_events.size(); ++j) {
			const Event &e = g_events[j];
			if (e.kind == 100) log.push_back(g_ulog[(size_t)e.p]);
		}
		putU32((unsigned)log.size());
		for (size_t j = 0; j < log.size(); ++j) putStr(log[j].c_str(), (int)log[j].size());
		putU32((unsigned)gl.m_awakenCalls);
		putU32(gl.m_awakenCalls ? gl.m_awakenFrame : 0);
		if (phase) {
			putI32(xst);
			putU32((unsigned)blen);
			for (int j = 0; j < blen; ++j) putU32(bytes[j]);
			putU32(crc);
		}
		putU32((unsigned)m->m_status);
		putU32(m->m_aflameEndFrame);
		putU32(m->m_burnedEndFrame);
		putU32(m->m_damageEndFrame);
		putU32(m->m_audioHandle);
		putU32(realBits(m->m_flameDamageLimit));
		putU32(m->m_lastFlameDamageDealt);
		putU32(obj->getStatusBits().test(OBJECT_STATUS_AFLAME) ? 1 : 0);
		putU32(obj->getStatusBits().test(OBJECT_STATUS_BURNED) ? 1 : 0);
		putU32(obj->m_body ? realBits(body.m_health) : 0);
	}
	*len = finish(buf, cap);
	delete m;
	delete g_flSpread;
	delete spreadData;
	g_flSpread = 0;
	delete d;
	delete obj;
	TheGameLogic = 0;
	TheAudio = saveAudio;
	g_findUpdateHook = 0;
	return 0;
}

int ref_l9fl_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9flRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
