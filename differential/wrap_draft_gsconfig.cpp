/* Differential-test wrapper for the GameSpy config drafts (GitHub #215, group netlogic; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/GSConfig.cpp      (#included whole, unchanged: the class GameSpyConfig is
**                                                                             defined in it)
**   .../Include/GameNetwork/GameSpy/GSConfig.h                              (unchanged)
** on the stand-ins of differential/shim_gsconfig (MapUtil.h: `TheMapCache->getMapDir()` is "Maps" and `findMap` finds a map when the
** sum of its name's bytes is not a multiple of 3) and differential/shim_netpkt (GameState.h, whose
** real <-> portable map path functions prefix "P:" and "R:"). Every class and global of the file is renamed Gc*, so that no symbol is
** shared with another wrapper's library.
**
** Entry point (extern "C", tests/differential_draft_gsconfig.rs): gc_run(in, nin, out, cap), in = [config length, the config bytes four
** to a word], out = the words of `dump`. */
#define GameSpyConfigInterface GcGameSpyConfigInterface
#define GameSpyConfig GcGameSpyConfig
#define TheGameSpyConfig GcTheGameSpyConfig
#define SectionChecker GcSectionChecker
#define MapCache GcMapCache
#define MapMetaData GcMapMetaData
#define TheMapCache GcTheMapCache
#define GameState NpGameState
#define TheGameState NpTheGameState

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <algorithm>
#include <string>
#include <vector>
#include <list>
#include <set>
using std::min;
using std::max;
/* (the real AsciiString.h and AsciiString.cpp, under names of their own, as the user preference files' build: see build.rs) */
DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theGsConfigAllocator;
static struct GsConfigAllocatorInit { GsConfigAllocatorInit() { TheDynamicMemoryAllocator = &theGsConfigAllocator; } } gsConfigAllocatorInit;
#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/GSConfig.cpp"
#undef protected
#undef private

AsciiString GcMapCache::getMapDir() const { AsciiString d; d.set("Maps"); return d; }
static GcMapMetaData g_md;
const GcMapMetaData *GcMapCache::findMap(AsciiString mapName)
{
	unsigned sum = 0;
	for (const char *p = mapName.str(); *p; ++p) sum += (unsigned char)*p;
	return (sum % 3) ? &g_md : NULL;
}
static GcMapCache g_cache;
GcMapCache *GcTheMapCache = &g_cache;
AsciiString NpGameState::realMapPathToPortableMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("P:") + in.str()).c_str()); return r; }
AsciiString NpGameState::portableMapPathToRealMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("R:") + in.str()).c_str()); return r; }
static NpGameState g_gameState;
NpGameState *NpTheGameState = &g_gameState;

namespace {
typedef std::vector<unsigned> Words;
void emit(Words &o, unsigned v) { o.push_back(v); }
void emitBytes(Words &o, const char *p, unsigned n)
{
	o.push_back(n);
	for (unsigned i = 0; i < n; i += 4) {
		unsigned v = 0;
		for (unsigned j = 0; j < 4 && i + j < n; ++j) v |= (unsigned)(unsigned char)p[i + j] << (8 * j);
		o.push_back(v);
	}
}
void emitStr(Words &o, AsciiString s) { emitBytes(o, s.str(), (unsigned)s.getLength()); }
}

extern "C" int gc_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	unsigned n = nin > 0 ? in[0] : 0;
	std::string text;
	for (unsigned i = 0; i < n; i += 4) {
		unsigned v = (int)(1 + i / 4) < nin ? in[1 + i / 4] : 0;
		for (unsigned j = 0; j < 4 && i + j < n; ++j) text += (char)(v >> (8 * j));
	}
	AsciiString cfg; cfg.set(text.c_str());
	GcGameSpyConfigInterface *c = GcGameSpyConfigInterface::create(cfg);
	Words o;
	std::list<AsciiString> ps = c->getPingServers();
	emit(o, (unsigned)ps.size());
	for (std::list<AsciiString>::iterator it = ps.begin(); it != ps.end(); ++it) emitStr(o, *it);
	emit(o, c->getNumPingRepetitions()); emit(o, c->getPingTimeoutInMs()); emit(o, c->getPingCutoffGood()); emit(o, c->getPingCutoffBad());
	std::list<AsciiString> qm = c->getQMMaps();
	emit(o, (unsigned)qm.size());
	for (std::list<AsciiString>::iterator it = qm.begin(); it != qm.end(); ++it) emitStr(o, *it);
	emit(o, c->getQMBotID()); emit(o, c->getQMChannel());
	c->setQMChannel(7); emit(o, c->getQMChannel());
	for (int r = -2; r <= 12; ++r) emit(o, c->getPointsForRank(r));
	for (int id = -1; id <= 40; ++id) emit(o, c->isPlayerVIP(id));
	for (int i = -1; i <= 12; ++i) {
		AsciiString h; UnsignedShort p = 0xABCD;
		Bool ok = c->getManglerLocation(i, h, p);
		emit(o, ok);
		if (ok) { emitStr(o, h); emit(o, p); }
	}
	emitStr(o, c->getLeftoverConfig());
	GcGameSpyConfig *g = (GcGameSpyConfig *)c;
	emit(o, c->restrictGamesToLobby());
	emit(o, g->getTimeBetweenRetries()); emit(o, g->getMaxManglerRetries());
	emit(o, (unsigned)g->getRetryInterval()); emit(o, (unsigned)g->getKeepaliveInterval()); emit(o, (unsigned)g->getPortTimeout()); emit(o, (unsigned)g->getRoundTimeout());
	delete c;
	int k = (int)o.size();
	if (k <= cap) memcpy(out, o.data(), k * sizeof(unsigned));
	return k;
}
