/* Differential-test wrapper for ExperienceTracker (upstream commit 0a05454d), p1w6-trackers:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ExperienceTracker.cpp  (#included below, unchanged)
** with the real headers ExperienceTracker.h, GameCommon.h, GameType.h and Snapshot.h, and the stand-ins
** of differential/shim_experience for Object (its template, its tracker, the relationship of one object
** toward another, onVeterancyLevelChanged), ThingTemplate (the three experience queries) and
** GameLogic (findObjectByID, the global TheGameLogic).
**
** The stand-ins are renamed (ExpTrack...) so that no other differential build's Object or ThingTemplate
** can clash with them: one binary links all of them.
**
** Objects are 1..n (n <= 4); id k is g_objects[k-1]. Ids outside 1..n are not objects (findObjectByID
** gives NULL), as are 0 (INVALID_ID) and any id above n.
**
** Entry point (extern "C", tests/differential_experience_tracker.rs):
**   ref_experience_run  a program of tracker operations on n objects, logged word by word. */
#define Object ExpTrackObject
#define ThingTemplate ExpTrackThingTemplate
#define GameLogic ExpTrackGameLogic
#define TheGameLogic ExpTrackTheGameLogic

#include "PreRTS.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <vector>
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "GameLogic/ExperienceTracker.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ExperienceTracker.cpp"

namespace {
const int MAX_OBJECTS = 4;
ExpTrackObject g_objects[MAX_OBJECTS];
ExpTrackThingTemplate g_templates[MAX_OBJECTS];
int g_count = 0;

/* one onVeterancyLevelChanged call: the object's id, old level, new level, feedback */
struct Event { int id; int oldLevel; int newLevel; int feedback; };
std::vector<Event> g_events;
}

ExpTrackGameLogic *ExpTrackTheGameLogic = 0;

ExpTrackObject *ExpTrackGameLogic::findObjectByID(ObjectID id)
{
	if (id < 1 || id > (ObjectID)g_count) return NULL;
	return &g_objects[id - 1];
}

void ExpTrackObject::onVeterancyLevelChanged(VeterancyLevel oldLevel, VeterancyLevel newLevel, Bool provideFeedback)
{
	Event e = { m_index + 1, (int)oldLevel, (int)newLevel, provideFeedback ? 1 : 0 };
	g_events.push_back(e);
}

namespace {
ExpTrackGameLogic g_logic;

/* the log: a word, then a real's bits */
struct Log
{
	unsigned *w; int cap; int n;
	void put(unsigned v) { if (n < cap) w[n] = v; ++n; }
	void putBool(bool b) { put(b ? 1u : 0u); }
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

/* the events of the last operation, then cleared: the count, and each event's four words */
void putEvents(Log &lg)
{
	lg.put((unsigned)g_events.size());
	for (size_t k = 0; k < g_events.size(); ++k) {
		lg.put((unsigned)g_events[k].id);
		lg.put((unsigned)g_events[k].oldLevel);
		lg.put((unsigned)g_events[k].newLevel);
		lg.put((unsigned)g_events[k].feedback);
	}
	g_events.clear();
}

unsigned bitsOf(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
}

extern "C" {

/* spec: n objects, object i (0-based) with its values[4], required[4], trainable (9 words), then the
** relationship matrix: rel[killer][target] (n*n words; 0 ENEMIES, 1 NEUTRAL, 2 ALLIES).
** ops: quadruples (code, id, x, y), id 1-based:
**   0 addExperiencePoints(x, y != 0)                   the events
**   1 setExperienceAndLevel(x, y != 0)                 the events
**   2 setVeterancyLevel(level x, y != 0)               the events
**   3 setMinVeterancyLevel(level x)                    the events
**   4 gainExpForLevel(x, y != 0)                       the result, the events
**   5 canGainExpForLevel(x)                            the result
**   6 setExperienceSink(x)                             (no output)
**   7 setExperienceScalar(x / 4)                       (no output)
**   8 getExperienceValue(killer id x)                  the value
**   9 isTrainable()                                    the result
**  10 isAcceptingExperiencePoints()                    the result
**  11 the state: level, points, scalar bits
**  12 XferSave: the byte count and the bytes
**  13 XferLoad of the blob: the status (0 ok, else 1 + XferStatus), then the state
**  14 crc: the byte count and the bytes
** Every operation then logs its level-change events (count, then id, old, new, feedback each).
** Returns the number of words, or -1 if they do not fit in cap, -2 for a bad program. */
int ref_experience_run(int n, const int *spec, const int *ops, int nOps, const unsigned char *blob, int blobLen,
	const char *tmpPath, unsigned *out, int cap)
{
	if (n < 1 || n > MAX_OBJECTS) return -2;
	g_count = n;
	ExpTrackTheGameLogic = &g_logic;
	const int *p = spec;
	for (int i = 0; i < n; ++i) {
		ExpTrackThingTemplate &t = g_templates[i];
		for (int k = 0; k < LEVEL_COUNT; ++k) t.m_values[k] = *p++;
		for (int k = 0; k < LEVEL_COUNT; ++k) t.m_required[k] = *p++;
		t.m_trainable = *p++ != 0;
	}
	for (int i = 0; i < n; ++i) {
		ExpTrackObject &o = g_objects[i];
		o.m_index = i;
		o.m_template = &g_templates[i];
		for (int j = 0; j < n; ++j) o.m_relations[j] = (Relationship)*p++;
	}
	/* each object's tracker, made as ThingFactory::newObject would: a tracker owned by its object */
	ExperienceTracker *trackers[MAX_OBJECTS];
	for (int i = 0; i < n; ++i) {
		trackers[i] = new ExperienceTracker(&g_objects[i]);
		g_objects[i].m_tracker = trackers[i];
	}
	g_events.clear();

	Log lg = { out, cap, 0 };
	for (int i = 0; i < nOps; ++i) {
		const int code = ops[4 * i], id = ops[4 * i + 1], x = ops[4 * i + 2], y = ops[4 * i + 3];
		if (id < 1 || id > n) { for (int k = 0; k < n; ++k) delete trackers[k]; return -2; }
		ExperienceTracker *t = trackers[id - 1];
		switch (code) {
		case 0: t->addExperiencePoints(x, y != 0); putEvents(lg); break;
		case 1: t->setExperienceAndLevel(x, y != 0); putEvents(lg); break;
		case 2: t->setVeterancyLevel((VeterancyLevel)x, y != 0); putEvents(lg); break;
		case 3: t->setMinVeterancyLevel((VeterancyLevel)x); putEvents(lg); break;
		case 4: lg.putBool(t->gainExpForLevel(x, y != 0)); putEvents(lg); break;
		case 5: lg.putBool(t->canGainExpForLevel(x)); putEvents(lg); break;
		case 6: t->setExperienceSink((ObjectID)x); putEvents(lg); break;
		case 7: t->setExperienceScalar((Real)x / 4.0f); putEvents(lg); break;
		case 8:
			if (x < 1 || x > n) { for (int k = 0; k < n; ++k) delete trackers[k]; return -2; }
			lg.put((unsigned)t->getExperienceValue(&g_objects[x - 1]));
			putEvents(lg);
			break;
		case 9: lg.putBool(t->isTrainable()); putEvents(lg); break;
		case 10: lg.putBool(t->isAcceptingExperiencePoints()); putEvents(lg); break;
		case 11:
			lg.put((unsigned)(int)t->getVeterancyLevel());
			lg.put((unsigned)t->getCurrentExperience());
			lg.put(bitsOf(t->getExperienceScalar()));
			putEvents(lg);
			break;
		case 12: {
			XferSave xs;
			xs.open(AsciiString(tmpPath));
			xs.xferSnapshot(t);
			xs.close();
			std::vector<unsigned char> bytes = readFile(tmpPath);
			lg.put((unsigned)bytes.size());
			for (size_t k = 0; k < bytes.size(); ++k) lg.put(bytes[k]);
			putEvents(lg);
			break; }
		case 13: {
			FILE *f = fopen(tmpPath, "wb");
			if (f) { fwrite(blob, 1, (size_t)blobLen, f); fclose(f); }
			unsigned status = 0;
			XferLoad xl;
			try {
				xl.open(AsciiString(tmpPath));
				xl.xferSnapshot(t);
				xl.close();
			} catch (XferStatus s) {
				status = 1u + (unsigned)s;
				try { xl.close(); } catch (...) {}
			}
			lg.put(status);
			lg.put((unsigned)(int)t->getVeterancyLevel());
			lg.put((unsigned)t->getCurrentExperience());
			lg.put(bitsOf(t->getExperienceScalar()));
			putEvents(lg);
			break; }
		case 14: {
			XferSave xs;
			xs.open(AsciiString(tmpPath));
			t->crc(&xs);
			xs.close();
			std::vector<unsigned char> bytes = readFile(tmpPath);
			lg.put((unsigned)bytes.size());
			for (size_t k = 0; k < bytes.size(); ++k) lg.put(bytes[k]);
			putEvents(lg);
			break; }
		default:
			for (int k = 0; k < n; ++k) delete trackers[k];
			return -2;
		}
	}
	for (int k = 0; k < n; ++k) delete trackers[k];
	return lg.n <= cap ? lg.n : -1;
}

} /* extern "C" */
