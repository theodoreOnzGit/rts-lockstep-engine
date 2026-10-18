/* Differential-test wrapper for Science and RankInfo (upstream commit
** 0a05454d):
**   GameEngine/Source/Common/RTS/Science.cpp         (compiled unchanged)
**   GameEngine/Source/GameLogic/System/RankInfo.cpp  (compiled unchanged)
** with the headers Science.h, RankInfo.h, Overridable.h, INI.h,
** NameKeyGenerator.h unchanged, in the BitFlags build's setup (clang++
** -fms-extensions, the real BitFlagsIO.h) plus differential/shim_science:
** the real Science.h ahead of the stand-in, and Player (hasScience only).
**
** INI: the blocks are read by the REAL INI::load (upstream_ini): `Science`
** and `Rank` reach Science.cpp and RankInfo.cpp (the definitions in
** wrap_ini.cpp are weak, so these override them), and the block
** `WebpageURL` (an unused name of theTypeTable, weak in wrap_ini.cpp) is the
** vehicle for the REAL INI::parseScience and INI::parseScienceVector, whose
** results are kept in a list this file dumps. The real INI::parseAndTranslateLabel
** calls TheGameText->fetch, whose stand-in here translates a label to "T:"
** followed by its bytes, and to the empty string if it starts with
** "NOTEXT" (any case). INI::load is called with the load type the test
** chooses (ref_science_load).
**
** Xfer: Xfer::xferScienceType / xferScienceVec of the REAL Xfer.cpp
** (upstream_xfer), whose calls to ScienceStore reach the real store.
**
** The stores are dumped here (private members are made public for this
** file only; the prelude does the same for Science.h). */
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
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/Science.h"
#include "Common/Player.h"
#include "GameClient/GameText.h"
#include "GameLogic/RankInfo.h"
#undef private
#undef protected

/* ---- the three methods that take a Player are not translated (layer 7);
** the stand-in Player is never reached. */
Bool Player::hasScience(ScienceType) const { abort(); }

namespace {

/* ---- the stand-in game text */
class StandInText : public GameTextInterface
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
StandInText g_text;

std::vector<unsigned char> g_dump;
void put(const void *p, int n) { g_dump.insert(g_dump.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_dump.push_back((unsigned char)v); }
void putU32(unsigned v) { put(&v, 4); }
void putI32(int v) { put(&v, 4); }
void putUni(const UnicodeString &s)
{
	putU32((unsigned)s.getLength());
	for (int i = 0; i < s.getLength(); ++i) { unsigned short u = (unsigned short)s.getCharAt(i); put(&u, 2); }
}
void putStr(const char *s, int n) { putU32((unsigned)n); put(s, n); }
void putVec(const ScienceVec &v)
{
	putU32((unsigned)v.size());
	for (size_t i = 0; i < v.size(); ++i) putI32((int)v[i]);
}
int finish(unsigned char *buf, int cap)
{
	int n = (int)g_dump.size();
	if (n <= cap && n > 0) memcpy(buf, &g_dump[0], n);
	return n;
}

void dumpScience(const ScienceInfo *si)
{
	put8(si->m_isOverride ? 1 : 0);
	putI32((int)si->m_science);
	putUni(si->m_name);
	putUni(si->m_description);
	putVec(si->m_rootSciences);
	putVec(si->m_prereqSciences);
	putI32(si->m_sciencePurchasePointCost);
	put8(si->m_grantable ? 1 : 0);
}

void dumpRank(const RankInfo *ri)
{
	put8(ri->m_isOverride ? 1 : 0);
	putUni(ri->m_rankName);
	putI32(ri->m_skillPointsNeeded);
	putI32(ri->m_sciencePurchasePointsGranted);
	putVec(ri->m_sciencesGranted);
}

/* ---- the WebpageURL vehicle: INI::parseScience and INI::parseScienceVector */
struct WebRec
{
	ScienceType one;
	ScienceVec vec;
	WebRec() : one(SCIENCE_INVALID) {}
};
std::list<WebRec> g_web;

NameKeyGenerator *g_gen = 0;

/* the INI's throw as a status: 0 none, 1 INIException (message into msg),
** 2 an INI_* status code (anonymous enum), 3 an ErrorCode, 4
** std::out_of_range, 5 other; as wrap_ini.cpp */
int unnamedEnumStatus(unsigned *code)
{
	const std::type_info *t = abi::__cxa_current_exception_type();
	if (t && dynamic_cast<const abi::__enum_type_info *>(t)) { *code = (unsigned)ERROR_BAD_INI; return 2; }
	return 5;
}

} /* namespace */

void INI::parseWebpageURLDefinition(INI *ini)
{
	g_web.push_back(WebRec());
	WebRec &r = g_web.back();
	static const FieldParse fp[] =
	{
		{ "Science", INI::parseScience, NULL, offsetof(WebRec, one) },
		{ "Sciences", INI::parseScienceVector, NULL, offsetof(WebRec, vec) },
		{ 0, 0, 0, 0 }
	};
	ini->initFromINI(&r, fp);
}

extern "C" {

/* a NameKeyGenerator (installed as TheNameKeyGenerator, reset: keys start at
** 1 again), empty stores, the stand-in text, and an empty WebpageURL list */
void ref_science_reset(void)
{
	if (!g_gen) g_gen = new NameKeyGenerator;
	g_gen->reset();
	TheNameKeyGenerator = g_gen;
	TheGameText = &g_text;
	delete TheScienceStore;
	delete TheRankInfoStore;
	TheScienceStore = new ScienceStore;
	TheRankInfoStore = new RankInfoStore;
	g_web.clear();
}

unsigned ref_science_name_key(const char *name) { return (unsigned)TheNameKeyGenerator->nameToKey(name); }

/* INI::load(filename, loadType, xfer ? &crc : NULL). Returns the throw
** status (0 none, 1 INIException with the message in msg and its length in
** *code, 2 an INI_* code, 3 an ErrorCode, 4 std::out_of_range, 5 other). */
int ref_science_load(const char *filename, int loadType, int withXfer, unsigned *crc, char *msg, int msgCap, unsigned *code)
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

/* op 0 ScienceStore::reset(), RankInfoStore::reset();
** op 1 ScienceStore::init(), RankInfoStore::init() */
void ref_science_op(int op)
{
	if (op == 0) { TheScienceStore->reset(); TheRankInfoStore->reset(); }
	else { TheScienceStore->init(); TheRankInfoStore->init(); }
}

/* ScienceStore: the count of sciences, then per science its chain length and
** every node of the chain (first definition first): override flag, science,
** name, description (each as units), roots, prerequisites, purchase cost,
** grantable. Returns the length (may exceed cap). */
int ref_science_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)TheScienceStore->m_sciences.size());
	for (size_t i = 0; i < TheScienceStore->m_sciences.size(); ++i) {
		unsigned len = 0;
		for (const Overridable *o = TheScienceStore->m_sciences[i]; o; o = o->m_nextOverride) ++len;
		putU32(len);
		for (const Overridable *o = TheScienceStore->m_sciences[i]; o; o = o->m_nextOverride)
			dumpScience((const ScienceInfo *)o);
	}
	return finish(buf, cap);
}

/* RankInfoStore: the count of ranks, then per rank its chain length and
** every node: override flag, name, skill points, purchase points, sciences. */
int ref_rank_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)TheRankInfoStore->m_rankInfos.size());
	for (size_t i = 0; i < TheRankInfoStore->m_rankInfos.size(); ++i) {
		unsigned len = 0;
		for (const Overridable *o = TheRankInfoStore->m_rankInfos[i]; o; o = o->m_nextOverride) ++len;
		putU32(len);
		for (const Overridable *o = TheRankInfoStore->m_rankInfos[i]; o; o = o->m_nextOverride)
			dumpRank((const RankInfo *)o);
	}
	return finish(buf, cap);
}

/* the WebpageURL records: count, then per record the science of `Science =`
** and the vector of `Sciences =` */
int ref_science_web_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	putU32((unsigned)g_web.size());
	for (std::list<WebRec>::const_iterator it = g_web.begin(); it != g_web.end(); ++it) {
		putI32((int)it->one);
		putVec(it->vec);
	}
	return finish(buf, cap);
}

/* the lookups of one science id: isValidScience, isScienceGrantable,
** getNameAndDescription (found, name, description), getSciencePurchaseCost,
** getInternalNameForScience */
int ref_science_probe(int st, unsigned char *buf, int cap)
{
	g_dump.clear();
	ScienceType s = (ScienceType)st;
	put8(TheScienceStore->isValidScience(s) ? 1 : 0);
	put8(TheScienceStore->isScienceGrantable(s) ? 1 : 0);
	UnicodeString n, d;
	bool found = TheScienceStore->getNameAndDescription(s, n, d);
	put8(found ? 1 : 0);
	putUni(n);
	putUni(d);
	putI32(TheScienceStore->getSciencePurchaseCost(s));
	AsciiString in = TheScienceStore->getInternalNameForScience(s);
	putStr(in.str(), in.getLength());
	return finish(buf, cap);
}

/* the lookups of one name: getScienceFromInternalName (a new name gets a
** key), then friend_lookupScience (found, or the throw) */
int ref_science_name_probe(const char *name, unsigned char *buf, int cap)
{
	g_dump.clear();
	putI32((int)TheScienceStore->getScienceFromInternalName(AsciiString(name)));
	try {
		putI32((int)TheScienceStore->friend_lookupScience(name));
		put8(1);
	} catch (...) {
		put8(0);
	}
	return finish(buf, cap);
}

/* friend_getScienceNames() */
int ref_science_names(unsigned char *buf, int cap)
{
	g_dump.clear();
	std::vector<AsciiString> v = TheScienceStore->friend_getScienceNames();
	putU32((unsigned)v.size());
	for (size_t i = 0; i < v.size(); ++i) putStr(v[i].str(), v[i].getLength());
	return finish(buf, cap);
}

/* RankInfoStore::getRankLevelCount() and getRankInfo(level): found, then the
** fields */
int ref_rank_probe(int level, unsigned char *buf, int cap)
{
	g_dump.clear();
	putI32(TheRankInfoStore->getRankLevelCount());
	const RankInfo *ri = TheRankInfoStore->getRankInfo(level);
	put8(ri ? 1 : 0);
	if (ri) {
		putUni(ri->m_rankName);
		putI32(ri->m_skillPointsNeeded);
		putI32(ri->m_sciencePurchasePointsGranted);
		putVec(ri->m_sciencesGranted);
	}
	return finish(buf, cap);
}

namespace {
/* the vector xfers: kind 0 xferScienceType (one value), 1 xferScienceVec */
int xferRun(Xfer &x, int kind, ScienceVec &v, int *status)
{
	if (kind == 0) {
		ScienceType s = v.empty() ? SCIENCE_INVALID : v[0];
		try { x.xferScienceType(&s); } catch (XferStatus st) { *status = (int)st; } catch (...) { *status = -1; }
		v.assign(1, s);
	} else {
		try { x.xferScienceVec(&v); } catch (XferStatus st) { *status = (int)st; } catch (...) { *status = -1; }
	}
	return *status;
}
}

/* Save: `vals` through XferSave to the file at `path` (kind 0: xferScienceType
** of vals[0], 1: xferScienceVec), then the file's bytes. Returns the
** XferStatus thrown (0 none; -1 anything else); *len is the file's length. */
int ref_science_save(int kind, const int *vals, int n, const char *path, unsigned char *out, int cap, int *len)
{
	ScienceVec v;
	for (int i = 0; i < n; ++i) v.push_back((ScienceType)vals[i]);
	int status = 0;
	{
		XferSave x;
		try { x.open(AsciiString(path)); } catch (...) { status = -1; }
		if (status == 0) xferRun(x, kind, v, &status);
		try { x.close(); } catch (...) {}
	}
	*len = 0;
	if (FILE *f = fopen(path, "rb")) {
		*len = (int)fread(out, 1, cap, f);
		fclose(f);
	}
	return status;
}

/* Load: `bytes` written to `path`, read through XferLoad into a vector that
** starts as `vals`; the resulting vector goes to `vals` (capacity `cap`) and
** its length to *n. Returns the XferStatus thrown. */
int ref_science_load_xfer(int kind, int *vals, int cap, int *n, const unsigned char *bytes, int nbytes, const char *path)
{
	if (FILE *f = fopen(path, "wb")) { if (nbytes > 0) fwrite(bytes, 1, nbytes, f); fclose(f); }
	ScienceVec v;
	for (int i = 0; i < *n; ++i) v.push_back((ScienceType)vals[i]);
	int status = 0;
	{
		XferLoad x;
		try { x.open(AsciiString(path)); } catch (...) { status = -1; }
		if (status == 0) xferRun(x, kind, v, &status);
		try { x.close(); } catch (...) {}
	}
	*n = (int)v.size();
	for (int i = 0; i < *n && i < cap; ++i) vals[i] = (int)v[i];
	return status;
}

/* CRC: XferCRC of `vals`. Returns the status; *crc is getCRC(). */
int ref_science_crc(int kind, const int *vals, int n, unsigned *crc)
{
	ScienceVec v;
	for (int i = 0; i < n; ++i) v.push_back((ScienceType)vals[i]);
	int status = 0;
	XferCRC x;
	try { x.open(AsciiString("crc")); } catch (...) { status = -1; }
	if (status == 0) xferRun(x, kind, v, &status);
	try { x.close(); } catch (...) {}
	*crc = x.getCRC();
	return status;
}

} /* extern "C" */
