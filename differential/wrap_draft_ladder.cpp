/* Differential-test wrapper for the ladder drafts (GitHub #215, group netlogic; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/LadderDefs.cpp   (#included whole, unchanged)
**   .../Include/GameNetwork/GameSpy/LadderDefs.h                           (unchanged)
** with the REAL AsciiString.h and AsciiString.cpp (differential/shim_ladder), on the stand-ins of differential/shim_ladder for what the
** file calls outside itself, all driven by the test's input: `ThePlayerTemplateStore` (a list of templates: side, playable, or a
** NULL one), `TheGameSpyConfig` (`getQMMaps`, `getLeftoverConfig`), `TheMapCache` (`getMapDir` is "Maps"; `findMap` finds a map when
** the byte sum of its path is not a multiple of 3 and gives it 2 + (sum % 7) players), `TheGameState` (differential/shim_netpkt:
** the real <-> portable path functions prefix "P:" and "R:"), `TheFileSystem` (a list of file names and a table of contents by
** lower-case name; `File` reads them), `TheGlobalData->getPath_UserData()`, `MultiByteToWideCharSingleLine` (each byte one 16-bit unit),
** and `TheGameText` (only inside a DEBUG_LOG). Every class and global of the file is renamed Ld*.
**
** Entry point (extern "C", tests/differential_draft_ladder.rs): ld_run(in, nin, out, cap); the input encoding is at `run`. */
#define LadderInfo LdLadderInfo
#define LadderList LdLadderList
#define TheLadderList LdTheLadderList
#define LadderInfoList LdLadderInfoList
#define GameSpyConfigInterface LdGameSpyConfigInterface
#define TheGameSpyConfig LdTheGameSpyConfig
#define MapCache LdMapCache
#define MapMetaData LdMapMetaData
#define TheMapCache LdTheMapCache
#define GameState LdGameState
#define TheGameState LdTheGameState
#define File LdFile
#define FileSystem LdFileSystem
#define TheFileSystem LdTheFileSystem
#define FilenameList LdFilenameList
#define PlayerTemplate LdPlayerTemplate
#define PlayerTemplateStore LdPlayerTemplateStore
#define ThePlayerTemplateStore LdThePlayerTemplateStore
#define GameText LdGameText
#define TheGameText LdTheGameText
#define GlobalData LdGlobalData
#define TheGlobalData LdTheGlobalData
#define MultiByteToWideCharSingleLine LdMultiByteToWideCharSingleLine

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <algorithm>
#include <string>
#include <vector>
#include <list>
#include <map>
using std::min;
using std::max;
#include "Common/GameMemory.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "../shim_netpkt/Common/GameState.h"

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theLadderAllocator;
static struct LadderAllocatorInit { LadderAllocatorInit() { TheDynamicMemoryAllocator = &theLadderAllocator; } } ladderAllocatorInit;

struct GlobalData { AsciiString m_userData; AsciiString getPath_UserData() const { return m_userData; } };
extern GlobalData *TheGlobalData;
std::basic_string<WideChar> MultiByteToWideCharSingleLine(const char *s);

#define protected public
#define private public
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/LadderDefs.cpp"
#undef protected
#undef private

/* ---- the stand-ins ---- */
static GlobalData g_global;
GlobalData *TheGlobalData = &g_global;
std::basic_string<WideChar> MultiByteToWideCharSingleLine(const char *s)
{
	std::basic_string<WideChar> r;
	for (; s && *s; ++s) r += (WideChar)(unsigned char)*s;
	return r;
}
AsciiString LdGameState::realMapPathToPortableMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("P:") + in.str()).c_str()); return r; }
AsciiString LdGameState::portableMapPathToRealMapPath(const AsciiString &in) const { AsciiString r; r.set((std::string("R:") + in.str()).c_str()); return r; }
static LdGameState g_gameState;
LdGameState *LdTheGameState = &g_gameState;

static unsigned mapSum(const char *p) { unsigned s = 0; for (; *p; ++p) s += (unsigned char)*p; return s; }
AsciiString LdMapCache::getMapDir() const { AsciiString d; d.set("Maps"); return d; }
static LdMapMetaData g_md;
const LdMapMetaData *LdMapCache::findMap(AsciiString mapName)
{
	unsigned s = mapSum(mapName.str());
	if (s % 3 == 0) return NULL;
	g_md.m_numPlayers = (Int)(2 + s % 7);
	return &g_md;
}
static LdMapCache g_cache;
LdMapCache *LdTheMapCache = &g_cache;

struct Template { std::string side; bool playable; bool isNull; };
static std::vector<Template> g_templates;
static std::vector<std::string> g_qmMaps;
static std::string g_leftover;
static std::vector<std::string> g_fileNames;
static std::map<std::string, std::string> g_fileData;
static std::vector<LdPlayerTemplate *> g_ptObjects;

void LdPlayerTemplateStore::getAllSideStrings(AsciiStringList *out)
{
	for (size_t i = 0; i < g_templates.size(); ++i) { AsciiString a; a.set(g_templates[i].side.c_str()); out->push_back(a); }
}
Int LdPlayerTemplateStore::getPlayerTemplateCount() const { return (Int)g_templates.size(); }
const LdPlayerTemplate *LdPlayerTemplateStore::getNthPlayerTemplate(Int i) const { return g_ptObjects[i]; }
static LdPlayerTemplateStore g_store;
LdPlayerTemplateStore *LdThePlayerTemplateStore = &g_store;

std::list<AsciiString> LdGameSpyConfigInterface::getQMMaps(void)
{
	std::list<AsciiString> l;
	for (size_t i = 0; i < g_qmMaps.size(); ++i) { AsciiString a; a.set(g_qmMaps[i].c_str()); l.push_back(a); }
	return l;
}
AsciiString LdGameSpyConfigInterface::getLeftoverConfig(void) { AsciiString a; a.set(g_leftover.c_str()); return a; }
static LdGameSpyConfigInterface g_gsConfig;
LdGameSpyConfigInterface *LdTheGameSpyConfig = &g_gsConfig;

void LdFileSystem::getFileListInDirectory(const AsciiString &, const AsciiString &, LdFilenameList &list, Bool) const
{
	for (size_t i = 0; i < g_fileNames.size(); ++i) { AsciiString a; a.set(g_fileNames[i].c_str()); list.push_back(a); }
}
LdFile *LdFileSystem::openFile(const Char *filename, Int)
{
	std::map<std::string, std::string>::iterator it = g_fileData.find(filename);
	if (it == g_fileData.end()) return NULL;
	LdFile *f = new LdFile; f->m_data = it->second; return f;
}
static LdFileSystem g_fs;
LdFileSystem *LdTheFileSystem = &g_fs;
static LdGameText g_text;
LdGameText *LdTheGameText = &g_text;

namespace {
typedef std::vector<unsigned> Words;
struct Rd { const unsigned *w; int n; int i; unsigned next() { return i < n ? w[i++] : 0; } };
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
std::string readStr(Rd &r)
{
	unsigned n = r.next();
	std::string s;
	for (unsigned i = 0; i < n; i += 4) {
		unsigned v = r.next();
		for (unsigned j = 0; j < 4 && i + j < n; ++j) s += (char)(v >> (8 * j));
	}
	return s;
}
void emitA(Words &o, const AsciiString &s) { emitBytes(o, s.str(), (unsigned)s.getLength()); }
void emitU(Words &o, const UnicodeString &s) { emit(o, (unsigned)s.getLength()); for (int i = 0; i < s.getLength(); ++i) emit(o, (unsigned)s.str()[i]); }
void dumpLadder(Words &o, const LdLadderInfo *l)
{
	if (!l) { emit(o, 0); return; }
	emit(o, 1);
	emitU(o, l->name); emitU(o, l->description); emitU(o, l->location);
	emit(o, (unsigned)l->playersPerTeam); emit(o, (unsigned)l->minWins); emit(o, (unsigned)l->maxWins);
	emit(o, l->randomMaps); emit(o, l->randomFactions); emit(o, l->validQM); emit(o, l->validCustom);
	emit(o, (unsigned)l->validMaps.size());
	for (std::list<AsciiString>::const_iterator it = l->validMaps.begin(); it != l->validMaps.end(); ++it) emitA(o, *it);
	emit(o, (unsigned)l->validFactions.size());
	for (std::list<AsciiString>::const_iterator it = l->validFactions.begin(); it != l->validFactions.end(); ++it) emitA(o, *it);
	emitA(o, l->cryptedPassword); emitA(o, l->address); emit(o, l->port); emitA(o, l->homepageURL);
	emit(o, l->submitReplay); emit(o, (unsigned)l->index);
}
void dumpList(Words &o, const LdLadderInfoList *l)
{
	emit(o, (unsigned)l->size());
	for (LdLadderInfoList::const_iterator it = l->begin(); it != l->end(); ++it) dumpLadder(o, *it);
}
}

/* in: [ntemplates, per template: kind (0 a template, 1 a NULL one), side (bytes), playable; nqm, per map: name (bytes); the leftover config
** (bytes); nfiles, per file: name (bytes), content (bytes) (the file is listed under that name and readable under its lower-case one);
** nstandalone, per text: the text (bytes) for parseLadder alone; nprobes, per probe: address (bytes), port] (bytes: a count, then four to a
** word). out: the standard, special and local ladders (every field), then findLadder and findLadderByIndex (-4 to 8) on the probes, then
** parseLadder of each standalone text. */
extern "C" int ld_run(const unsigned *in, int nin, unsigned *out, int cap)
{
	Rd r = {in, nin, 0};
	g_templates.clear(); g_qmMaps.clear(); g_fileNames.clear(); g_fileData.clear(); g_ptObjects.clear();
	unsigned nt = r.next();
	for (unsigned i = 0; i < nt; ++i) {
		Template t; t.isNull = r.next() != 0; t.side = readStr(r); t.playable = r.next() != 0;
		g_templates.push_back(t);
		LdPlayerTemplate *p = NULL;
		if (!t.isNull) { p = new LdPlayerTemplate; p->m_side.set(t.side.c_str()); p->m_playable = t.playable; }
		g_ptObjects.push_back(p);
	}
	unsigned nq = r.next();
	for (unsigned i = 0; i < nq; ++i) g_qmMaps.push_back(readStr(r));
	g_leftover = readStr(r);
	unsigned nf = r.next();
	for (unsigned i = 0; i < nf; ++i) {
		std::string name = readStr(r), content = readStr(r);
		g_fileNames.push_back(name);
		std::string low = name;
		for (size_t k = 0; k < low.size(); ++k) low[k] = (char)tolower((unsigned char)low[k]);
		g_fileData[low] = content;
	}
	unsigned ns = r.next();
	std::vector<std::string> standalone;
	for (unsigned i = 0; i < ns; ++i) standalone.push_back(readStr(r));
	unsigned np = r.next();
	std::vector<std::pair<std::string, unsigned> > probes;
	for (unsigned i = 0; i < np; ++i) { std::string a = readStr(r); unsigned p = r.next(); probes.push_back(std::make_pair(a, p)); }

	Words o;
	LdLadderList *list = new LdLadderList;
	dumpList(o, list->getStandardLadders()); dumpList(o, list->getSpecialLadders()); dumpList(o, list->getLocalLadders());
	for (size_t i = 0; i < probes.size(); ++i) {
		AsciiString a; a.set(probes[i].first.c_str());
		dumpLadder(o, list->findLadder(a, (UnsignedShort)probes[i].second));
	}
	for (int idx = -4; idx <= 8; ++idx) dumpLadder(o, list->findLadderByIndex(idx));
	delete list;
	for (size_t i = 0; i < standalone.size(); ++i) {
		AsciiString raw; raw.set(standalone[i].c_str());
		LdLadderInfo *l = parseLadder(raw);
		dumpLadder(o, l);
		delete l;
	}
	int k = (int)o.size();
	if (k <= cap) memcpy(out, o.data(), k * sizeof(unsigned));
	return k;
}
