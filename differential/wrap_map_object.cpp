/* Differential-test wrapper for the map object chunk reader (upstream commit
** 0a05454d): GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp's
** ParseObjectDataChunk, cut VERBATIM out of that file by build.rs (it is
** `static` in a GUI file that needs the whole client to compile; the cut and
** its markers are checked there) and included below, unchanged.
**
** The names the cut uses are stand-ins, declared here as upstream declares
** them and renamed by macro so that they cannot clash with another fixture:
**   MapObject            -> ObjTestMapObject   (the constructor and destructor
**                          of WorldHeightMap.cpp's class, reduced to the
**                          non-NULL properties branch and the normalised
**                          angle; the destructor is where the object's final
**                          state is logged, since upstream deletes each object
**                          after classifying it)
**   ThingTemplate, ThingFactory, TheThingFactory -> ObjTest*  (findTemplate
**                          answers NULL: no template exists in this build, so
**                          the tech-building and supply branches are not
**                          reached, and they are the cut point on the Rust side)
**   TheKey_waypointID, TheKey_waypointName -> StaticNameKeys made per run (a
**                          static key caches its value, and each run resets
**                          TheNameKeyGenerator)
**   m_waypoints, m_techPositions, m_supplyPositions: MapUtil.cpp's statics
**
** Entry point (extern "C", tests/differential_map_object.rs): ref_map_object_run
** parses a chunk file with an `Object` chunk parser registered at the top
** level, and logs every object (at its deletion) and the waypoint map. */
#define MapObject ObjTestMapObject
#define ThingTemplate ObjTestThingTemplate
#define ThingFactory ObjTestThingFactory
#define TheThingFactory ObjTestTheThingFactory
#define TheKey_waypointID (*g_keyWaypointID)
#define TheKey_waypointName (*g_keyWaypointName)

#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include <algorithm>
#include "Common/DataChunk.h"
#include "Common/MapReaderWriterInfo.h"
#include "Common/NameKeyGenerator.h"
#include "Common/GameMemory.h"

Real normalizeAngle(Real angle); /* GameCommon.cpp, linked from the INI wrapper build */

/* MapUtil.h's types and MapUtil.cpp's statics the cut uses */
typedef std::map<AsciiString, Coord3D> WaypointMap;
typedef std::vector<Coord3D> Coord3DList;
static WaypointMap g_waypointMap;
static WaypointMap *m_waypoints = &g_waypointMap;
static Coord3DList m_techPositions;
static Coord3DList m_supplyPositions;

enum { KINDOF_TECH_BUILDING = 1, KINDOF_SUPPLY_SOURCE_ON_PREVIEW = 2 };

/* the keys, made per run (see the header) */
static StaticNameKey *g_keyWaypointID = NULL;
static StaticNameKey *g_keyWaypointName = NULL;

class ObjTestThingTemplate
{
public:
	Bool isKindOf(int) const { return false; }
};

/* findTemplate records the name it is asked for (the Rust side's
** thing_template name) and answers NULL */
static AsciiString g_lastLookup;
class ObjTestThingFactory
{
public:
	const ObjTestThingTemplate *findTemplate(const AsciiString &name, Bool) const
	{
		g_lastLookup = name;
		return NULL;
	}
};
static ObjTestThingFactory g_objFactory;
ObjTestThingFactory *ObjTestTheThingFactory = &g_objFactory;

namespace {

std::vector<unsigned char> g_objLog;

void objPut(const void *p, int n) { g_objLog.insert(g_objLog.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void objPut8(int v) { g_objLog.push_back((unsigned char)v); }
void objPutU32(unsigned v) { objPut(&v, 4); }
void objPutCStr(const char *s) { int n = (int)strlen(s); objPutU32((unsigned)n); objPut(s, n); }
void objPutWide(const WideChar *s)
{
	int n = 0;
	while (s[n]) ++n;
	objPutU32((unsigned)n);
	objPut(s, n * 2);
}

/* the property sheet, as the Rust side dumps it: sorted by key name */
void dumpDict(Dict *d)
{
	std::vector<std::pair<std::string, const Dict::Value *> > v;
	for (std::map<NameKeyType, Dict::Value>::const_iterator it = d->pairs().begin(); it != d->pairs().end(); ++it)
		v.push_back(std::make_pair(std::string(TheNameKeyGenerator->keyToName(it->first).str()), &it->second));
	std::sort(v.begin(), v.end(), [](const std::pair<std::string, const Dict::Value *> &a, const std::pair<std::string, const Dict::Value *> &b) { return a.first < b.first; });
	objPutU32((unsigned)v.size());
	for (size_t i = 0; i < v.size(); ++i) {
		objPutCStr(v[i].first.c_str());
		const Dict::Value &x = *v[i].second;
		objPut8(x.type);
		switch (x.type) {
		case Dict::DICT_BOOL: objPut8(x.b ? 1 : 0); break;
		case Dict::DICT_INT: objPut(&x.i, 4); break;
		case Dict::DICT_REAL: objPut(&x.r, 4); break;
		case Dict::DICT_ASCIISTRING: objPutCStr(x.a.str()); break;
		case Dict::DICT_UNICODESTRING: objPutWide(x.u.str()); break;
		default: abort();
		}
	}
}

} /* namespace */

class ObjTestMapObject : public MemoryPoolObject
{
public:
	ObjTestMapObject(Coord3D loc, AsciiString name, Real angle, Int flags, const Dict *props, const ObjTestThingTemplate *thingTemplate)
		: m_location(loc), m_objectName(name), m_thingTemplate(thingTemplate), m_angle(normalizeAngle(angle)), m_flags(flags),
		  m_runtimeFlags(0), m_lookup(g_lastLookup)
	{
		if (props) m_properties = *props;
	}
	virtual ~ObjTestMapObject()
	{
		/* the final state, when upstream deletes the object */
		objPutCStr(m_objectName.str());
		objPut(&m_location.x, 4);
		objPut(&m_location.y, 4);
		objPut(&m_location.z, 4);
		objPut(&m_angle, 4);
		objPut(&m_flags, 4);
		dumpDict(&m_properties);
		objPut8((m_runtimeFlags & 0x04) ? 1 : 0);
		objPutCStr(m_lookup.str());
	}
	Dict *getProperties() { return &m_properties; }
	void setIsWaypoint() { m_runtimeFlags |= 0x04; }
	AsciiString getWaypointName() { return m_properties.getAsciiString(TheKey_waypointName); }
	const ObjTestThingTemplate *getThingTemplate() const { return m_thingTemplate; }

private:
	Coord3D m_location;
	AsciiString m_objectName;
	const ObjTestThingTemplate *m_thingTemplate;
	Real m_angle;
	Int m_flags;
	Dict m_properties;
	Int m_runtimeFlags;
	AsciiString m_lookup;
};

/* ---- the cut itself ------------------------------------------------------- */
#include "map_object_parse.inc"

namespace {

class ObjTestStream : public CachedFileInputStream
{
public:
	void setData(const char *data, int len)
	{
		m_buffer = new char[len];
		memcpy(m_buffer, data, len);
		m_size = len;
		m_pos = 0;
	}
};

} /* namespace */

extern "C" {

/* Parse `data` with ParseObjectDataChunk registered for top-level `Object`
** chunks. The objects are logged at their deletion, in file order, then the
** waypoint map (count, then name and the three coordinate bits, in name
** order). Returns the parse's result (1 or 0), or 3 with the ErrorCode in
** *code when a read threw (readDict's corrupt-file and out-of-memory throws;
** the objects before it are logged, as upstream deletes them), or -1 if the
** log does not fit. */
int ref_map_object_run(const char *data, int len, unsigned char *log, int logCap, int *logLen, unsigned *code)
{
	g_objLog.clear();
	delete TheNameKeyGenerator;
	TheNameKeyGenerator = new NameKeyGenerator;
	TheNameKeyGenerator->init();
	delete g_keyWaypointID;
	delete g_keyWaypointName;
	g_keyWaypointID = new StaticNameKey("waypointID");
	g_keyWaypointName = new StaticNameKey("waypointName");
	g_waypointMap.clear();
	m_techPositions.clear();
	m_supplyPositions.clear();

	ObjTestStream stream;
	stream.setData(data, len);
	int status = 0;
	try {
		DataChunkInput in(&stream);
		in.registerParser("Object", "", ParseObjectDataChunk);
		status = in.parse(NULL) ? 1 : 0;
	} catch (ErrorCode c) {
		status = 3;
		*code = (unsigned)c;
	}
	objPutU32((unsigned)g_waypointMap.size());
	for (WaypointMap::const_iterator it = g_waypointMap.begin(); it != g_waypointMap.end(); ++it) {
		objPutCStr(it->first.str());
		objPut(&it->second.x, 4);
		objPut(&it->second.y, 4);
		objPut(&it->second.z, 4);
	}
	int n = (int)g_objLog.size();
	if (n > logCap) return -1;
	if (n > 0) memcpy(log, &g_objLog[0], n);
	*logLen = n;
	return status;
}

} /* extern "C" */
