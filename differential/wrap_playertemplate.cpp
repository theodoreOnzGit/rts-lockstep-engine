/* Differential-test wrapper for PlayerTemplate (upstream commit 0a05454d):
**   GameEngine/Source/Common/RTS/PlayerTemplate.cpp  (compiled unchanged)
** with the headers PlayerTemplate.h, Money.h, Handicap.h, Science.h, INI.h,
** NameKeyGenerator.h unchanged, in the Science build's setup (clang++
** -fms-extensions, the real BitFlagsIO.h) plus differential/shim_playertemplate:
** the Science prelude and three typedefs of the real STLTypedefs.h.
**
** INI: the blocks are read by the REAL INI::load (upstream_ini): `PlayerTemplate`
** reaches PlayerTemplate.cpp (INI::parsePlayerTemplateDefinition is WEAK in
** wrap_ini.cpp, so the real one overrides it) and `Science` reaches the real
** Science.cpp (upstream_science), whose store validates IntrinsicSciences
** through the real INI::parseScienceVector. INI::parseAndTranslateLabel
** (DisplayName, and the sciences' labels) calls TheGameText->fetch, whose
** stand-in here translates a label to "T:" followed by its bytes, and to the
** empty string if it starts with "NOTEXT" (any case): the stand-in of
** wrap_science.cpp. INI::load is called with the load type the test chooses.
**
** Harness: this file defines no symbol another fixture defines. Money::deposit
** comes from upstream_money (its stand-ins of TheAudio and ThePlayerList),
** Handicap's constructor from upstream_handicap, ScienceStore and
** Player::hasScience (and so wrap_science.o) from upstream_science. The
** friend_lookupScience that INI::parseScience(Vector) reaches is the single
** dispatcher of wrap_ini.cpp; this fixture installs the real lookup (renamed
** real_friend_lookupScience in the Science build, and here, by build.rs)
** through ref_science_lookup_hook in its reset function.
**
** The store is dumped here (private members are made public for this file
** only). ThePlayerTemplateStore lives for the whole process, as upstream's:
** findPlayerTemplate's function statics (the fifteen faction name keys) are
** initialised by the first call and never again, so each test session calls
** init() on the one store instead of making a new one. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <strings.h>
#include <stdexcept>
#include <typeinfo>
#include <cxxabi.h>
#include <string>
#include <map>
#include <vector>
#include <list>
#include <set>
#define private public
#define protected public
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/NameKeyGenerator.h"
#include "Common/XferCRC.h"
#include "Common/Science.h"
#include "Common/Handicap.h"
#include "Common/Money.h"
#include "Common/PlayerTemplate.h"
#include "GameClient/GameText.h"
#undef private
#undef protected

namespace {

/* ---- the stand-in game text (as wrap_science.cpp's) */
class PtStandInText : public GameTextInterface
{
public:
	void init() {}
	void reset() {}
	void update() {}
	UnicodeString fetch(const Char *label, Bool *exists = NULL)
	{
		if (exists) *exists = TRUE;
		UnicodeString u;
		if (strncasecmp(label, "NOTEXT", 6) == 0) return u;
		std::basic_string<WideChar> w;
		w += (WideChar)'T';
		w += (WideChar)':';
		for (const unsigned char *p = (const unsigned char *)label; *p; ++p) w += (WideChar)*p;
		u.set(w.c_str());
		return u;
	}
	UnicodeString fetch(AsciiString label, Bool *exists = NULL) { return fetch(label.str(), exists); }
	AsciiStringVec &getStringsWithLabelPrefix(AsciiString) { static AsciiStringVec v; return v; }
	void initMapStringFile(const AsciiString &) {}
};
PtStandInText g_text;

std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_dump.push_back((unsigned char)v); }
void putU32(unsigned v) { put(&v, 4); }
void putI32(int v) { put(&v, 4); }
void putF32(float v) { put(&v, 4); }
void putUni(const UnicodeString &s)
{
	putU32((unsigned)s.getLength());
	for (int i = 0; i < s.getLength(); ++i) { unsigned short u = (unsigned short)s.getCharAt(i); put(&u, 2); }
}
void putAscii(const AsciiString &s) { putU32((unsigned)s.getLength()); put(s.str(), s.getLength()); }
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}

NameKeyGenerator *g_gen = 0;

/* the INI's throw as a status (as wrap_science.cpp): 2 an INI_* code, 5 other */
int unnamedEnumStatus(unsigned *code)
{
	const std::type_info *t = abi::__cxa_current_exception_type();
	if (t && dynamic_cast<const abi::__enum_type_info *>(t)) { *code = (unsigned)ERROR_BAD_INI; return 2; }
	return 5;
}

/* One template, through the getters where there is one: the name key and
** name, the display name, side, base side, the four handicap multipliers,
** the money (amount and player), the colour, the starting building, the
** starting units 0..9 (and -1, 10 beyond), the three maps, the sciences, the
** command sets, the shortcut data, the music, the tooltip, the flags, the
** purchase points, the image names (members: the getters resolve them
** through the client), the beacon, the features and the medallions. */
void dumpTemplate(const PlayerTemplate *p)
{
	putU32((unsigned)p->getNameKey());
	putAscii(p->getName());
	putUni(p->getDisplayName());
	putAscii(p->getSide());
	putAscii(p->getBaseSide());
	for (int i = 0; i < Handicap::HANDICAP_TYPE_COUNT; ++i)
		for (int j = 0; j < Handicap::THING_TYPE_COUNT; ++j)
			putF32(p->getHandicap()->m_handicaps[i][j]);
	putU32(p->getMoney()->countMoney());
	putI32(p->getMoney()->m_playerIndex);
	putF32(p->getPreferredColor()->red);
	putF32(p->getPreferredColor()->green);
	putF32(p->getPreferredColor()->blue);
	putAscii(p->getStartingBuilding());
	for (int i = -1; i <= MAX_MP_STARTING_UNITS; ++i) putAscii(p->getStartingUnit(i));
	const ProductionChangeMap *maps[2] = { &p->getProductionCostChanges(), &p->getProductionTimeChanges() };
	for (int m = 0; m < 2; ++m) {
		putU32((unsigned)maps[m]->size());
		for (ProductionChangeMap::const_iterator it = maps[m]->begin(); it != maps[m]->end(); ++it) {
			putU32((unsigned)it->first);
			putF32(it->second);
		}
	}
	const ProductionVeterancyMap &vet = p->getProductionVeterancyLevels();
	putU32((unsigned)vet.size());
	for (ProductionVeterancyMap::const_iterator it = vet.begin(); it != vet.end(); ++it) {
		putU32((unsigned)it->first);
		putI32((int)it->second);
	}
	const ScienceVec &sc = p->getIntrinsicSciences();
	putU32((unsigned)sc.size());
	for (size_t i = 0; i < sc.size(); ++i) putI32((int)sc[i]);
	putAscii(p->getPurchaseScienceCommandSetRank1());
	putAscii(p->getPurchaseScienceCommandSetRank3());
	putAscii(p->getPurchaseScienceCommandSetRank8());
	putAscii(p->getSpecialPowerShortcutCommandSet());
	putAscii(p->getSpecialPowerShortcutWinName());
	putI32(p->getSpecialPowerShortcutButtonCount());
	putAscii(p->getLoadScreenMusic());
	putAscii(p->getScoreScreenMusic());
	putAscii(p->getTooltip());
	put8(p->isObserver() ? 1 : 0);
	put8(p->isPlayableSide() ? 1 : 0);
	put8(p->isOldFaction() ? 1 : 0);
	putI32(p->getIntrinsicSciencePurchasePoints());
	putAscii(p->getScoreScreen());
	putAscii(p->getLoadScreen());
	putAscii(p->m_headWaterMark);
	putAscii(p->m_flagWaterMark);
	putAscii(p->m_enabledImage);
	putAscii(p->m_sideIconImage);
	putAscii(p->m_generalImage);
	putAscii(p->getBeaconTemplate());
	putAscii(p->getGeneralFeatures());
	putAscii(p->getMedallionNormal());
	putAscii(p->getMedallionHilite());
	putAscii(p->getMedallionSelected());
}

/* a pointer into the store's vector as an index, -1 for NULL */
int indexOf(const PlayerTemplate *p)
{
	if (!p) return -1;
	return (int)(p - &ThePlayerTemplateStore->m_playerTemplates[0]);
}

/* the real lookup (renamed real_friend_lookupScience by build.rs); see the
** header comment */
ScienceType real_lookup(const ScienceStore *s, const char *n) { return s->friend_lookupScience(n); }

} /* namespace */

extern "C" ScienceType (*ref_science_lookup_hook)(const ScienceStore *, const char *);

extern "C" {

/* a NameKeyGenerator (installed as TheNameKeyGenerator, reset: keys start at
** 1 again), an empty ScienceStore, the stand-in text, the lookup hook, and
** the one PlayerTemplateStore (made on the first call, emptied by init()
** after that: see the header comment) */
void ref_pt_reset(void)
{
	ref_science_lookup_hook = real_lookup;
	if (!g_gen) g_gen = new NameKeyGenerator;
	g_gen->reset();
	TheNameKeyGenerator = g_gen;
	TheGameText = &g_text;
	delete TheScienceStore;
	TheScienceStore = new ScienceStore;
	if (!ThePlayerTemplateStore) ThePlayerTemplateStore = new PlayerTemplateStore;
	ThePlayerTemplateStore->init();
}

unsigned ref_pt_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* INI::load(filename, loadType, xfer ? &crc : NULL). Returns the throw
** status (0 none, 1 INIException with the message in msg and its length in
** *code, 2 an INI_* code, 3 an ErrorCode, 4 std::out_of_range, 5 other). */
int ref_pt_load(const char *filename, int loadType, int withXfer, unsigned *crc, char *msg, int msgCap, unsigned *code)
{
	XferCRC x;
	int status = 0;
	try {
		INI ini;
		ini.load(AsciiString(filename), (INILoadType)loadType, withXfer ? &x : NULL);
	}
	catch (INIException &e) {
		status = 1;
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";
		int n = (int)strlen(m);
		*code = (unsigned)n;
		if (n + 1 <= msgCap) memcpy(msg, m, n + 1);
	}
	catch (ErrorCode c) { status = 3; *code = (unsigned)c; }
	catch (std::out_of_range &) { status = 4; }
	catch (...) { status = unnamedEnumStatus(code); }
	*crc = x.getCRC();
	return status;
}

/* op 0 PlayerTemplateStore::reset(); op 1 PlayerTemplateStore::init();
** op 2 update() */
void ref_pt_op(int op)
{
	if (op == 0) ThePlayerTemplateStore->reset();
	else if (op == 1) ThePlayerTemplateStore->init();
	else ThePlayerTemplateStore->update();
}

/* the store: the count, then every template (getNthPlayerTemplate(i)) */
int ref_pt_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putI32(ThePlayerTemplateStore->getPlayerTemplateCount());
	for (int i = 0; i < ThePlayerTemplateStore->getPlayerTemplateCount(); ++i)
		dumpTemplate(ThePlayerTemplateStore->getNthPlayerTemplate(i));
	return finish(buf, cap);
}

/* getNthPlayerTemplate(i): the index it found (-1 for NULL) */
int ref_pt_nth(int i) { return indexOf(ThePlayerTemplateStore->getNthPlayerTemplate(i)); }

/* findPlayerTemplate(key): the index it found (-1 for NULL) */
int ref_pt_find(unsigned key) { return indexOf(ThePlayerTemplateStore->findPlayerTemplate((NameKeyType)key)); }

/* getTemplateNumByName(name) */
int ref_pt_num_by_name(const char *name) { return ThePlayerTemplateStore->getTemplateNumByName(AsciiString(name)); }

/* getAllSideStrings(&list) on a list that starts with the n names in
** `initial` (NUL-separated): the list afterwards */
int ref_pt_sides(const char *initial, int n, unsigned char *buf, int cap)
{
	AsciiStringList list;
	for (int i = 0; i < n; ++i) { list.push_back(AsciiString(initial)); initial += strlen(initial) + 1; }
	ThePlayerTemplateStore->getAllSideStrings(&list);
	g_dump.clear();
	putU32((unsigned)list.size());
	for (AsciiStringList::const_iterator it = list.begin(); it != list.end(); ++it) putAscii(*it);
	return finish(buf, cap);
}

/* getAllSideStrings(NULL): returns normally */
void ref_pt_sides_null(void) { ThePlayerTemplateStore->getAllSideStrings(NULL); }

/* the fifteen statics of findPlayerTemplate cannot be read; their effect is
** compared through findPlayerTemplate */

} /* extern "C" */
