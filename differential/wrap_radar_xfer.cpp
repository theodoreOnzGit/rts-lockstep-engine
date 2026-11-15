/* Differential-test wrapper for the radar's save (GitHub #57: CHUNK_Radar, upstream commit 0a05454d), cut verbatim by
** build.rs into OUT_DIR/radar_xfer_gen:
**   rx_save_code.inc        GameEngine/Include/Common/GameState.h, `enum SaveCode` (to `enum SnapshotType`);
**   rx_cells.inc            GameEngine/Include/Common/Radar.h, the RADAR_CELL_* enum and `enum RadarEventType` (to
**                           "// PROTOTYPES");
**   rx_radar_object.inc     GameEngine/Include/Common/Radar.h, `class RadarObject` (to the "Radar priorities" comment);
**   rx_radar_event.inc      Radar.h, class Radar's `MAX_RADAR_EVENTS`, `struct RadarEvent`, m_event, m_nextFreeRadarEvent
**                           and m_lastRadarEvent (to m_radarWindow);
**   rx_radar_object_ctor.inc  GameEngine/Source/Common/System/Radar.cpp, RadarObject's constructor and destructor (to
**                           isTemporarilyHidden);
**   rx_radar_object_xfer.inc  Radar.cpp, `RadarObject::crc`, `xfer` and `loadPostProcess` (to Radar's constructor);
**   rx_radar_xfer.inc       Radar.cpp, `xferRadarObjectList` and `Radar::xfer` (to Radar::loadPostProcess).
** No edit. GameMakeColor is upstream's GameClient/Color.h. They are compiled on stand-ins: MemoryPoolObject (deleteInstance is `delete this`), `newInstance`, an Object
** with an id and a destroyed flag whose friend_setRadarData records the id, TheGameLogic->findObjectByID over the
** case's objects, and a Radar holding the members Radar::xfer reaches (the cut's own declarations). On the REAL Snapshot,
** XferSave and XferLoad (upstream_xfer).
**
** Entry point (extern "C", tests/differential_radar_xfer.rs):
**   ref_radar_xfer(mode, in, nin, path, out, cap): mode 0 saves the radar `in` gives to `path` (XferSave), mode 1
**   loads `path` (XferLoad) into the radar `in` gives (its lists are the "list should be empty" check's).
**   in = [n objects, then n x (id, destroyed)], [hidden, forced], then two lists (local, regular) each [count, count x
**   (object index, -1 for NULL; colour)], then 64 events x 19 words [type, active, createFrame, dieFrame, fadeFrame,
**   color1 rgba, color2 rgba, worldLoc xyz (bits), radarLoc xy, soundPlayed], then [next free, last].
**   out = [status (0; an XferStatus + 1; 100 + a SaveCode), hidden, forced, then each list [count, count x (the
**   object's id or 0 for NULL, colour)], the 64 events (19 words each), next free, last, then [n, n ids] the
**   friend_setRadarData calls in order]. Returns the word count. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "GameClient/Color.h"
#include "Common/Snapshot.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"

namespace rx {

#include "rx_save_code.inc"
#include "rx_cells.inc"

#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( cls, name ) public: virtual ~cls();
#define DEBUG_ASSERTCRASH( c, m )
#define DEBUG_CRASH( m )
#define newInstance( cls ) new cls
class MemoryPoolObject
{
public:
	virtual ~MemoryPoolObject() {}
	void deleteInstance() { delete this; }
};

std::vector<int> g_radarData;
class RadarObject;
class Object
{
public:
	ObjectID m_id;
	Bool m_destroyed;
	ObjectID getID() const { return m_id; }
	Bool isDestroyed() const { return m_destroyed; }
	void friend_setRadarData( RadarObject * ) { g_radarData.push_back((int)m_id); }
};
std::vector<Object> g_objects;
class GameLogic
{
public:
	Object *findObjectByID( ObjectID id )
	{
		for (size_t i = 0; i < g_objects.size(); ++i)
			if (g_objects[i].m_id == id) return &g_objects[i];
		return NULL;
	}
};
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;

#include "rx_radar_object.inc"

class Radar
{
public:
	void xfer( Xfer *xfer );
	Bool m_radarHidden;
	Bool m_radarForceOn;
	RadarObject *m_objectList;
	RadarObject *m_localObjectList;
#include "rx_radar_event.inc"
};

#include "rx_radar_object_ctor.inc"
#include "rx_radar_object_xfer.inc"
#include "rx_radar_xfer.inc"

/* a peek at the protected members */
class RadarObjectPeek : public RadarObject
{
public:
	static Object *object( RadarObject *o ) { return ((RadarObjectPeek *)o)->m_object; }
	static Color color( RadarObject *o ) { return ((RadarObjectPeek *)o)->m_color; }
	static void set( RadarObject *o, Object *obj, Color c ) { ((RadarObjectPeek *)o)->m_object = obj; ((RadarObjectPeek *)o)->m_color = c; }
};

float real(int i)
{
	float f;
	memcpy(&f, &i, 4);
	return f;
}
int bits(float f)
{
	int i;
	memcpy(&i, &f, 4);
	return i;
}

const int *readList( const int *p, RadarObject **head )
{
	int n = *p++;
	RadarObject *last = NULL;
	for (int i = 0; i < n; ++i) {
		int k = *p++;
		int c = *p++;
		RadarObject *o = new RadarObject;
		RadarObjectPeek::set(o, k < 0 ? NULL : &g_objects[k], c);
		if (last) last->friend_setNext(o); else *head = o;
		last = o;
	}
	return p;
}

void putList( std::vector<int> &out, RadarObject *head )
{
	int n = 0;
	for (RadarObject *o = head; o; o = o->friend_getNext()) ++n;
	out.push_back(n);
	for (RadarObject *o = head; o; o = o->friend_getNext()) {
		Object *obj = RadarObjectPeek::object(o);
		out.push_back(obj ? (int)obj->getID() : 0);
		out.push_back(RadarObjectPeek::color(o));
	}
}

void freeList( RadarObject *head )
{
	while (head) {
		RadarObject *n = head->friend_getNext();
		delete head;
		head = n;
	}
}

} /* namespace rx */

extern "C" {

int ref_radar_xfer(int mode, const int *in, int nin, const char *path, int *out, int cap)
{
	using namespace rx;
	(void)nin;
	g_radarData.clear();
	g_objects.clear();
	const int *p = in;
	int nObj = *p++;
	for (int i = 0; i < nObj; ++i) {
		Object o;
		o.m_id = (ObjectID)*p++;
		o.m_destroyed = *p++ != 0;
		g_objects.push_back(o);
	}
	Radar *r = new Radar;
	r->m_radarHidden = *p++ != 0;
	r->m_radarForceOn = *p++ != 0;
	r->m_localObjectList = NULL;
	r->m_objectList = NULL;
	p = readList(p, &r->m_localObjectList);
	p = readList(p, &r->m_objectList);
	for (int i = 0; i < Radar::MAX_RADAR_EVENTS; ++i) {
		Radar::RadarEvent &e = r->m_event[i];
		e.type = (RadarEventType)*p++;
		e.active = *p++ != 0;
		e.createFrame = (UnsignedInt)*p++;
		e.dieFrame = (UnsignedInt)*p++;
		e.fadeFrame = (UnsignedInt)*p++;
		e.color1.red = *p++; e.color1.green = *p++; e.color1.blue = *p++; e.color1.alpha = *p++;
		e.color2.red = *p++; e.color2.green = *p++; e.color2.blue = *p++; e.color2.alpha = *p++;
		e.worldLoc.x = real(*p++); e.worldLoc.y = real(*p++); e.worldLoc.z = real(*p++);
		e.radarLoc.x = *p++; e.radarLoc.y = *p++;
		e.soundPlayed = *p++ != 0;
	}
	r->m_nextFreeRadarEvent = *p++;
	r->m_lastRadarEvent = *p++;

	int status = 0;
	XferSave xs;
	XferLoad xl;
	Xfer *x = mode == 0 ? (Xfer *)&xs : (Xfer *)&xl;
	try {
		x->open(AsciiString(path));
		r->xfer(x);
		x->close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x->close(); } catch (...) {}
	} catch (SaveCode c) {
		status = 100 + (int)c;
		try { x->close(); } catch (...) {}
	}

	std::vector<int> o;
	o.push_back(status);
	o.push_back(r->m_radarHidden ? 1 : 0);
	o.push_back(r->m_radarForceOn ? 1 : 0);
	putList(o, r->m_localObjectList);
	putList(o, r->m_objectList);
	for (int i = 0; i < Radar::MAX_RADAR_EVENTS; ++i) {
		const Radar::RadarEvent &e = r->m_event[i];
		int w[19] = { (int)e.type, e.active ? 1 : 0, (int)e.createFrame, (int)e.dieFrame, (int)e.fadeFrame,
			e.color1.red, e.color1.green, e.color1.blue, e.color1.alpha, e.color2.red, e.color2.green, e.color2.blue,
			e.color2.alpha, bits(e.worldLoc.x), bits(e.worldLoc.y), bits(e.worldLoc.z), e.radarLoc.x, e.radarLoc.y,
			e.soundPlayed ? 1 : 0 };
		o.insert(o.end(), w, w + 19);
	}
	o.push_back(r->m_nextFreeRadarEvent);
	o.push_back(r->m_lastRadarEvent);
	o.push_back((int)g_radarData.size());
	o.insert(o.end(), g_radarData.begin(), g_radarData.end());
	freeList(r->m_localObjectList);
	freeList(r->m_objectList);
	delete r;
	int n = (int)o.size();
	if (n <= cap) memcpy(out, o.data(), (size_t)n * 4);
	return n;
}

} /* extern "C" */
