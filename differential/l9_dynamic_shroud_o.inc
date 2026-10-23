/* Differential-test driver for DynamicShroudClearingRangeUpdate (p1w13-sonnet, update batch O; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicShroudClearingRangeUpdate.cpp   (#included, unchanged)
** with its header unchanged, in wrap_module.cpp's unit on the stand-ins of differential/shim_module: an Object with a position the test sets, a shroud clearing
** range (the stand-in's m_shroudRange), a controlling player (the test's, or none) and the game logic whose frame the test sets, the REAL RadiusDecal.h with
** RadiusDecalTemplate's constructor and parser verbatim from RadiusDecal.cpp (build.rs, mod_gen/radius_decal_template.inc) and RadiusDecal's own members as
** stand-ins that log (the client's shadow decals), and the REAL UpdateModule, INI and Xfer. */
#define DEFINE_SHADOW_NAMES
#define REAL_TO_INT(x) ((Int)(x))   /* Shadow.h's inline colour helpers name it; nothing here calls them */
#include "GameClient/Shadow.h"
#include "GameClient/RadiusDecal.h"
#include "radius_decal_template.inc"   /* RadiusDecalTemplate::RadiusDecalTemplate and ::parseRadiusDecalTemplate, verbatim from RadiusDecal.cpp (build.rs, mod_gen) */
/* MSVC's <stdlib.h> max macro, which the file uses */
#define max(a,b) (((a) > (b)) ? (a) : (b))
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DynamicShroudClearingRangeUpdate.cpp"
#undef max

namespace {

ModuleData *l9dsRead(const char *filename)
{
	INI ini;
	ini.prepFile(AsciiString(filename), INI_LOAD_OVERWRITE);
	ModuleData *data = 0;
	try {
		ini.readLine();
		AsciiString currentLine = ini.m_buffer;
		try {
			data = DynamicShroudClearingRangeUpdate::friend_newModuleData(&ini);
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

std::vector<unsigned char> g_dsSaved;
const RadiusDecal *g_dsFirst = 0;   /* the first decal any call names is decal 0: every loop of the module starts there */

int dsIndex(const RadiusDecal *d)
{
	if (!g_dsFirst) g_dsFirst = d;
	return (int)(d - g_dsFirst);
}

/* FNV-1a over the bytes of every line, as the Rust side */
unsigned long long fnv(unsigned long long h, const std::string &s)
{
	for (size_t i = 0; i < s.size(); ++i) { h ^= (unsigned char)s[i]; h *= 1099511628211ULL; }
	h ^= 0xff;
	h *= 1099511628211ULL;
	return h;
}

}  // namespace

RadiusDecal::RadiusDecal() : m_template(0), m_decal(0), m_empty(true) {}
RadiusDecal::RadiusDecal(const RadiusDecal& that) : m_template(that.m_template), m_decal(that.m_decal), m_empty(that.m_empty) {}
RadiusDecal& RadiusDecal::operator=(const RadiusDecal& that) { m_template = that.m_template; m_decal = that.m_decal; m_empty = that.m_empty; return *this; }
RadiusDecal::~RadiusDecal() {}
/* batch R20 (RadiusDecalUpdate sleeps by isEmpty()): clear() and createRadiusDecal() also keep RadiusDecal.cpp's m_empty, as upstream (the stand-in
** never makes a client Shadow, so m_decal stays NULL); update() is a line; xferRadiusDecal() is RadiusDecal.cpp's body, verbatim */
void RadiusDecal::clear() { ulog("dclear %d", dsIndex(this)); m_template = NULL; m_decal = NULL; m_empty = true; }
void RadiusDecal::update() { ulog("dupdate %d", dsIndex(this)); }
void RadiusDecal::xferRadiusDecal( Xfer *xfer )
{
	/// @todo implement me
	if (xfer->getXferMode() == XFER_LOAD)
	{
		clear();
	}
}
void RadiusDecal::setPosition(const Coord3D& pos) { ulog("dpos %d %08x %08x %08x", dsIndex(this), realBits(pos.x), realBits(pos.y), realBits(pos.z)); }
void RadiusDecal::setOpacity( const Real o ) { ulog("dopacity %d %08x", dsIndex(this), realBits(o)); }
void RadiusDecalTemplate::createRadiusDecal(const Coord3D& pos, Real radius, const Player* owningPlayer, RadiusDecal& result) const
{
	ulog("dcreate %d %s %u %08x %08x %u %d %d %08x %08x,%08x,%08x %d", dsIndex(&result), m_name.str(), (unsigned)m_shadowType, realBits(m_minOpacity), realBits(m_maxOpacity),
		m_opacityThrobTime, (int)m_color, (int)m_onlyVisibleToOwningPlayer, realBits(radius), realBits(pos.x), realBits(pos.y), realBits(pos.z),
		owningPlayer ? (int)owningPlayer->m_idx : -1);
	/* RadiusDecal.cpp: result.clear() (its line is this call's), then "it is now considered nonEmpty, regardless of the state of m_decal" */
	result.m_template = NULL;
	result.m_decal = NULL;
	result.m_empty = true;
	if (owningPlayer == NULL)
		return;
	if (m_name.isEmpty() || radius <= 0.0f)
		return;
	result.m_empty = false;
}

extern "C" {

/* Read `filename` (block `D`) as the data of a DynamicShroudClearingRangeUpdate and make the module on an Object (id 1) at the frame cfg[0], with the controlling
** player cfg[1] (0 none, 1 to 3), the shroud clearing range cfg[2] (float bits) and the position cfg[3..5] (float bits, set after the module is made). The data
** dump goes first (the eight fields and the decal template's seven). Then `n` ops of 5 words [op, a, b, c, ...]: 0 update() a times  1 position = (a, b, c) float
** bits  2 the frame = a  3 the shroud clearing range = a (float bits)  4 the controlling player = a  5 xfer save  6 xfer load of the last save  7 xfer CRC. The
** constructor's line and each op append: the CATCH_ALL status and the sleep time of the last update() (0 for the others), the effect log as the number of lines, the
** FNV-1a hash of them all and the first 40 as strings ("dclear <i>", "dcreate <i> <name> <style> <min> <max> <throb> <color> <only> <radius> <pos> <player>", "dpos <i> <pos>",
** "dopacity <i> <o>"), the xfer status, bytes and CRC, the module's twelve state words (state, countdown, total frames, the three deadlines, the done-forever frame,
** the interval countdown, the decals flag, the three ranges), and the object's shroud clearing range and position (3 words). */
static int l9dsRunInner(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	g_dump.clear();
	g_dsSaved.clear();
	g_dsFirst = 0;
	*len = 0;
	int status = 0;
	ModuleData *d = 0;
	try { d = l9dsRead(filename); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) return status;
	{
		const DynamicShroudClearingRangeUpdateModuleData *dd = (const DynamicShroudClearingRangeUpdateModuleData *)d;
		putU32(dd->m_shrinkDelay);
		putU32(dd->m_shrinkTime);
		putU32(dd->m_growDelay);
		putU32(dd->m_growTime);
		putU32(realBits(dd->m_finalVision));
		putU32(dd->m_changeInterval);
		putU32(dd->m_growInterval);
		putU32(dd->m_doSpySatFX ? 1 : 0);
		const RadiusDecalTemplate &t = dd->m_gridDecalTemplate;
		putStr(t.m_name.str(), t.m_name.getLength());
		putU32((unsigned)t.m_shadowType);
		putU32(realBits(t.m_minOpacity));
		putU32(realBits(t.m_maxOpacity));
		putU32(t.m_opacityThrobTime);
		putU32((unsigned)t.m_color);
		putU32(t.m_onlyVisibleToOwningPlayer ? 1 : 0);
	}
	Player players[3];
	for (int i = 0; i < 3; ++i) players[i].m_idx = i;
	Object *obj = newObject();
	obj->m_oid = 1;
	obj->m_player = cfg[1] ? &players[cfg[1] - 1] : 0;
	memcpy(&obj->m_shroudRange, &cfg[2], 4);
	GameLogic gl;
	gl.m_frame = cfg[0];
	TheGameLogic = &gl;
	g_events.clear();
	g_ulog.clear();
	DynamicShroudClearingRangeUpdate *m = 0;
	status = 0;
	try { m = new DynamicShroudClearingRangeUpdate(obj, d); }
	CATCH_ALL(status, code, msg, msgCap)
	if (status) {
		TheGameLogic = 0;
		return 5;
	}
	memcpy(&obj->m_pos.x, &cfg[3], 4);
	memcpy(&obj->m_pos.y, &cfg[4], 4);
	memcpy(&obj->m_pos.z, &cfg[5], 4);
	for (int phase = 0; phase < 1 + n; ++phase) {
		const unsigned *o = phase ? ops + 5 * (phase - 1) : 0;
		int st = 0;
		unsigned c2 = 0;
		char m2[16];
		unsigned crc = 0;
		unsigned char bytes[512];
		int blen = 0;
		int xst = -2;
		int sleep = 0;
		std::vector<std::string> log;
		if (phase) {
			g_events.clear();
			g_ulog.clear();
			try {
				switch (o[0]) {
				case 0:
					for (unsigned k = 0; k < o[1]; ++k) {
						sleep = (int)m->update();
						for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
						g_events.clear();
						g_ulog.clear();
					}
					break;
				case 1: memcpy(&obj->m_pos.x, &o[1], 4); memcpy(&obj->m_pos.y, &o[2], 4); memcpy(&obj->m_pos.z, &o[3], 4); break;
				case 2: gl.m_frame = o[1]; break;
				case 3: memcpy(&obj->m_shroudRange, &o[1], 4); break;
				case 4: obj->m_player = o[1] ? &players[(o[1] - 1) % 3] : 0; break;
				case 5: xst = runXfer(m, 0, xfer_path, bytes, 512, &blen, &crc); g_dsSaved.assign(bytes, bytes + blen); break;
				case 6: {
					std::vector<unsigned char> b = g_dsSaved;
					if (b.size() > 512) b.resize(512);
					xst = runXfer(m, 1, xfer_path, b.empty() ? bytes : &b[0], (int)b.size(), &blen, &crc);
					break;
				}
				default: xst = runXfer(m, 2, "crc", bytes, 0, &blen, &crc); break;
				}
			}
			CATCH_ALL(st, &c2, m2, 16)
		}
		for (size_t j = 0; j < g_events.size(); ++j) if (g_events[j].kind == 100) log.push_back(g_ulog[(size_t)g_events[j].p]);
		putI32(st);
		putI32(sleep);
		unsigned long long h = 14695981039346656037ULL;
		for (size_t j = 0; j < log.size(); ++j) h = fnv(h, log[j]);
		putU32((unsigned)log.size());
		putU32((unsigned)(h >> 32));
		putU32((unsigned)h);
		for (size_t j = 0; j < log.size() && j < 40; ++j) putStr(log[j].c_str(), (int)log[j].size());
		putI32(xst);
		putU32((unsigned)blen);
		for (int j = 0; j < blen; ++j) putU32(bytes[j]);
		putU32(crc);
		putU32((unsigned)m->m_state);
		putU32((unsigned)m->m_stateCountDown);
		putU32((unsigned)m->m_totalFrames);
		putU32(m->m_growStartDeadline);
		putU32(m->m_sustainDeadline);
		putU32(m->m_shrinkStartDeadline);
		putU32(m->m_doneForeverFrame);
		putU32(m->m_changeIntervalCountdown);
		putU32(m->m_decalsCreated ? 1 : 0);
		putU32(realBits(m->m_visionChangePerInterval));
		putU32(realBits(m->m_nativeClearingRange));
		putU32(realBits(m->m_currentClearingRange));
		putU32(realBits(obj->m_shroudRange));
		putU32(realBits(obj->m_pos.x));
		putU32(realBits(obj->m_pos.y));
		putU32(realBits(obj->m_pos.z));
	}
	*len = finish(buf, cap);
	delete m;
	delete d;
	delete obj;
	TheGameLogic = 0;
	return 0;
}

int ref_l9ds_run(const char *filename, const char *xfer_path, const unsigned *cfg, int n, const unsigned *ops, unsigned char *buf, int cap, int *len, char *msg,
	int msgCap, unsigned *code)
{
	return l9TrappedRun(l9dsRunInner, filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msgCap, code);
}

}  /* extern "C" */
