/* Differential-test wrapper for ObjectTypes (upstream commit 0a05454d), p1w6-trackers:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectTypes.cpp  (#included below, unchanged)
** with the header GameLogic/ObjectTypes.h unchanged, and the stand-ins of
** differential/shim_object_types for Player::canBuild, TheThingFactory->findTemplate
** and ThingTemplate::getName (layers 5 and 7, not translated as classes; the
** Rust takes them as the arguments find and can_build).
**
** The stand-ins are renamed (ObjTypes...) so that no other differential build's
** Player or ThingTemplate can clash with them: one binary links all of them.
**
** Entry point (extern "C", tests/differential_object_types.rs):
**   ref_object_types_run  a program of ObjectTypes operations (see the table below),
**                         logged word by word, with the XferSave bytes and the
**                         XferLoad result of the input blob.
**
** The name table is shared with the Rust test (NAMES below, same order):
**   0..5 are known to the factory (findTemplate returns their template),
**   6..7 are templates the factory does not find, 8 is the empty name, 9 "alpha". */
#define Player ObjTypesPlayer
#define ThingFactory ObjTypesThingFactory
#define TheThingFactory ObjTypesTheThingFactory
#define ThingTemplate ObjTypesThingTemplate

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <vector>
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/GameState.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectTypes.cpp"

/* ---- the stand-ins ----------------------------------------------------- */
namespace {
const int NAME_COUNT = 10;
const int KNOWN_COUNT = 6;
const char *const NAMES[NAME_COUNT] = { "ALPHA", "BETA", "GAMMA", "DELTA", "EPSILON", "ZETA", "ETA", "THETA", "", "alpha" };
ObjTypesThingTemplate g_templates[NAME_COUNT];
unsigned g_buildMask = 0;
ObjTypesThingFactory g_factory;
ObjTypesPlayer g_player;
}

const ObjTypesThingTemplate *ObjTypesThingFactory::findTemplate(const AsciiString &name) const
{
	for (int i = 0; i < KNOWN_COUNT; ++i)
		if (name.compare(NAMES[i]) == 0) return &g_templates[i];
	return NULL;
}
Bool ObjTypesPlayer::canBuild(const ObjTypesThingTemplate *tmpl) const
{
	return (g_buildMask >> (tmpl - g_templates)) & 1u;
}
ObjTypesThingFactory *ObjTypesTheThingFactory = &g_factory;

namespace {
/* the index of a template in g_templates, -1 for NULL */
int indexOf(const ObjTypesThingTemplate *t)
{
	return t ? (int)(t - g_templates) : -1;
}

struct Log
{
	unsigned *w; int cap; int n;
	void put(unsigned v) { if (n < cap) w[n] = v; ++n; }
	void putAscii(const AsciiString &s)
	{
		put((unsigned)s.getLength());
		for (int k = 0; k < s.getLength(); ++k) put((unsigned char)s.str()[k]);
	}
};

std::vector<unsigned char> readFile(const char *path)
{
	std::vector<unsigned char> b;
	FILE *f = fopen(path, "rb");
	if (!f) return b;
	int c;
	while ((c = fgetc(f)) != EOF) b.push_back((unsigned char)c);
	fclose(f);
	return b;
}

/* the state of the list, in the log's format: list name, size, each name */
void dump(Log &lg, const ObjectTypes &ot)
{
	lg.putAscii(ot.getListName());
	lg.put((unsigned)ot.getListSize());
	for (unsigned k = 0; k < ot.getListSize(); ++k) lg.putAscii(ot.getNthInList((Int)k));
}
}

extern "C" {

/* ops: triples (code, a, b); nOps triples. The program's log is the output.
**   0 addObjectType(name a)                       (no output)
**   1 removeObjectType(name a)                    1 word: 1 removed, 0 not in the list
**   2 setListName(name a)                         (no output)
**   3 isInSet(name a)                             1 word
**   4 isInSet(template a; -1 = NULL)              1 word
**   5 getListSize()                               1 word
**   6 getNthInList(index a)                       the name (length, bytes)
**   7 prepForPlayerCounting(a initial templates, b initial counts)
**                                                  the return, the template count, each template's index,
**                                                  the counts' size and values
**   8 canBuildAny(buildable mask a)               1 word
**   9 XferSave of the list                        the byte count, the bytes
**  10 XferLoad of the input blob into the list    the status (0 ok, 1 refused as SC_INVALID_DATA,
**                                                  else 2 + XferStatus), then the list state
**  11 the list state                              the list name, size and each name
** The log is the output (words). Returns the number of words, or -1 if they do not fit in cap. */
int ref_object_types_run(const int *ops, int nOps, const unsigned char *blob, int blobLen,
	const char *tmpPath, unsigned *out, int cap)
{
	ObjectTypes ot;
	Log lg = { out, cap, 0 };
	for (int k = 0; k < NAME_COUNT; ++k) {
		g_templates[k].m_name.set(NAMES[k]);
		g_templates[k].m_index = k;
	}
	g_buildMask = 0;
	for (int i = 0; i < nOps; ++i) {
		const int code = ops[3 * i], a = ops[3 * i + 1], b = ops[3 * i + 2];
		switch (code) {
		case 0: { AsciiString n(NAMES[a]); ot.addObjectType(n); break; }
		case 1: { AsciiString n(NAMES[a]); bool was = ot.isInSet(n); ot.removeObjectType(n); lg.put(was ? 1u : 0u); break; }
		case 2: { AsciiString n(NAMES[a]); ot.setListName(n); break; }
		case 3: { AsciiString n(NAMES[a]); lg.put(ot.isInSet(n) ? 1u : 0u); break; }
		case 4: { const ObjTypesThingTemplate *t = a < 0 ? NULL : &g_templates[a];
			lg.put(ot.isInSet(t) ? 1u : 0u); break; }
		case 5: lg.put((unsigned)ot.getListSize()); break;
		case 6: lg.putAscii(ot.getNthInList(a)); break;
		case 7: {
			/* the initial templates and counts the caller hands in */
			std::vector<const ObjTypesThingTemplate *> templates;
			std::vector<Int> counts;
			for (int k = 0; k < a; ++k) templates.push_back(&g_templates[k]);
			for (int k = 0; k < b; ++k) counts.push_back(5 + k);
			Int ret = ot.prepForPlayerCounting(templates, counts);
			lg.put((unsigned)ret);
			lg.put((unsigned)templates.size());
			for (size_t k = 0; k < templates.size(); ++k) lg.put((unsigned)indexOf(templates[k]));
			lg.put((unsigned)counts.size());
			for (size_t k = 0; k < counts.size(); ++k) lg.put((unsigned)counts[k]);
			break; }
		case 8: g_buildMask = (unsigned)a; lg.put(ot.canBuildAny(&g_player) ? 1u : 0u); break;
		case 9: {
			XferSave x;
			x.open(AsciiString(tmpPath));
			x.xferSnapshot(&ot);
			x.close();
			std::vector<unsigned char> bytes = readFile(tmpPath);
			lg.put((unsigned)bytes.size());
			for (size_t k = 0; k < bytes.size(); ++k) lg.put(bytes[k]);
			break; }
		case 10: {
			FILE *f = fopen(tmpPath, "wb");
			if (f) { fwrite(blob, 1, (size_t)blobLen, f); fclose(f); }
			unsigned status = 0;
			XferLoad x;
			try {
				x.open(AsciiString(tmpPath));
				x.xferSnapshot(&ot);
				x.close();
			} catch (SaveCode c) {
				status = (c == SC_INVALID_DATA) ? 1u : 0u;
				try { x.close(); } catch (...) {}
			} catch (XferStatus s) {
				status = 2u + (unsigned)s;
				try { x.close(); } catch (...) {}
			}
			lg.put(status);
			dump(lg, ot);
			break; }
		case 11: dump(lg, ot); break;
		default: return -2;
		}
	}
	return lg.n <= cap ? lg.n : -1;
}

} /* extern "C" */
