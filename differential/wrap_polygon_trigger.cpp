/* Differential-test wrapper for PolygonTrigger (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp
** (#included below, unchanged). Its headers are the real ones (PolygonTrigger.h,
** DataChunk.h, MapReaderWriterInfo.h, Xfer.h, Snapshot.h), reached through the
** shims of differential/shim_polygon (PreRTS.h, GlobalData, MapObject's
** MAP_XY_FACTOR, TerrainLogic) and differential/shim_chunk (the Dict stand-in
** that DataChunk.cpp is compiled against), so the class layouts are those of
** the chunk build and the Xfer build.
**
** The engine singletons the .cpp names are renamed by macro, so that this
** build's stand-ins cannot clash with another fixture's (wrap_thing.cpp and
** wrap_chunk.cpp define their own TheGlobalData and so on):
**   GlobalData, TheGlobalData  -> PolyTestGlobalData, PolyTestTheGlobalData
**   TerrainLogic, TheTerrainLogic -> PolyTestTerrainLogic, PolyTestTheTerrainLogic
** DataChunkOutput is renamed too: its methods are defined here as a stand-in
** (the real one writes through a temporary file in the user data directory),
** while DataChunkInput is the real class, compiled in the chunk build
** (DataChunk.cpp), which this archive links against and does not define.
**
** Entry point (extern "C", tests/differential_polygon_trigger.rs):
** ref_polygon_run parses a chunk file with the real
** ParsePolygonTriggersDataChunk, then logs the chain, writes the chain back
** with the real WritePolygonTriggersDataChunk (into the stand-in output) and
** answers the queries of each trigger (pointInTrigger, getCenterPoint's x
** and y, getRadius). The point queries are sent as they are: an overflow is
** undefined upstream, and the Rust side reports it as an error and does not
** compare that answer. */
#define GlobalData PolyTestGlobalData
#define TheGlobalData PolyTestTheGlobalData
#define TerrainLogic PolyTestTerrainLogic
#define TheTerrainLogic PolyTestTheTerrainLogic
#define DataChunkOutput PolyTestDataChunkOutput

#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include <list>
#include <set>
#include <algorithm>
#include <bitset>
/* The engine's PolygonTrigger members are protected, and the dump reads the
** statics s_currentID and ThePolygonTriggerListPtr: the std headers are
** included above, so the access keywords can be rewritten for the .cpp alone. */
#define private public
#define protected public
/* PolygonTrigger.cpp, a copy from OUT_DIR/poly_gen (build.rs): the text is the
** upstream file's, except for one declaration moved before a loop (see build.rs) */
#include "PolygonTrigger.cpp"
#undef private
#undef protected

/* ---- stand-ins ----------------------------------------------------------- */

PolyTestGlobalData g_polyGlobalData;
PolyTestGlobalData *PolyTestTheGlobalData = &g_polyGlobalData;

/* getGroundHeight is the cut point of getCenterPoint's z: never compared here */
static PolyTestTerrainLogic g_polyTerrain;
PolyTestTerrainLogic *PolyTestTheTerrainLogic = &g_polyTerrain;
Real PolyTestTerrainLogic::getGroundHeight(Real, Real, Coord3D *) const { return 0.0f; }

PolyTestDataChunkOutput::PolyTestDataChunkOutput(OutputStream *pOut) : m_pOut(pOut), m_tmp_file(NULL), m_chunkStack(NULL) {}
PolyTestDataChunkOutput::~PolyTestDataChunkOutput() {}

namespace {

std::vector<unsigned char> g_log;
/* the bytes the stand-in output receives, and the size placeholders of the open chunks */
std::vector<unsigned char> g_out;
std::vector<size_t> g_openChunks;
unsigned g_writeId = 1; /* what allocateID answers */

void put(const void *p, int n) { g_log.insert(g_log.end(), (const unsigned char *)p, (const unsigned char *)p + n); }
void put8(int v) { g_log.push_back((unsigned char)v); }
void putI32(int v) { put(&v, 4); }
void putCStr(const char *s) { int n = (int)strlen(s); unsigned u = (unsigned)n; put(&u, 4); put(s, n); }

void outPut(const void *p, int n) { g_out.insert(g_out.end(), (const unsigned char *)p, (const unsigned char *)p + n); }

class TestStream : public CachedFileInputStream
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

class TestInput : public DataChunkInput
{
public:
	TestInput(ChunkInputStream *s) : DataChunkInput(s) {}
	UnsignedInt idOf(const char *name) { return m_contents.getID(AsciiString(name)); }
};

/* the dump of one trigger, as the Rust side writes it */
void dumpTrigger(const PolygonTrigger *t)
{
	putCStr(t->getTriggerName().str());
	putCStr(t->getLayerName().str());
	putI32(t->getID());
	put8(t->isWaterArea() ? 1 : 0);
	put8(t->isRiver() ? 1 : 0);
	putI32(t->getRiverStart());
	putI32(t->getNumPoints());
	for (Int i = 0; i < t->getNumPoints(); ++i) {
		const ICoord3D *p = t->getPoint(i);
		putI32(p->x);
		putI32(p->y);
		putI32(p->z);
	}
	put8(t->isValid() ? 1 : 0);
	put8(t->doExportWithScripts() ? 1 : 0);
	put8(t->getWaterHandle() != NULL ? 1 : 0);
}

} /* namespace */

extern "C" {

/* Parse `data` (a chunk file with a PolygonTriggers chunk) with the water
** extents given, then:
**   log:      the parse's result (1 or 0), every trigger of the chain in order
**             (dumpTrigger), the end marker 'E' and s_currentID, then the
**             bytes WritePolygonTriggersDataChunk writes, with u32 length;
**   queries:  for each trigger k (chain order) and query j (x, y pairs in
**             `queries`): inside[k*nq + j] = pointInTrigger; centers[2k],
**             centers[2k+1] = getCenterPoint's x and y bits; radii[k] = getRadius's bits.
** `*nt` is the number of triggers; only the first trigCap are stored.
** Returns the log's length, or -1 if the log does not fit in logCap. */
int ref_polygon_run(const char *data, int len, float waterX, float waterY, unsigned writeId,
                    const int *queries, int nq,
                    unsigned char *log, int logCap, int *logLen,
                    unsigned char *inside, unsigned *centers, unsigned *radii, int trigCap, int *nt)
{
	g_log.clear();
	g_out.clear();
	g_openChunks.clear();
	g_writeId = writeId;
	PolyTestTheGlobalData->m_waterExtentX = waterX;
	PolyTestTheGlobalData->m_waterExtentY = waterY;
	/* the chain of an earlier run is emptied first: a file whose table is not
	** read never reaches the parser, which is the only other caller of it */
	PolygonTrigger::deleteTriggers();

	TestStream stream;
	stream.setData(data, len);
	TestInput in(&stream);
	in.registerParser("PolygonTriggers", "", PolygonTrigger::ParsePolygonTriggersDataChunk);
	Bool r = in.parse(NULL);
	put8(r ? 1 : 0);
	int k = 0;
	for (PolygonTrigger *t = PolygonTrigger::getFirstPolygonTrigger(); t; t = t->getNext(), ++k) {
		dumpTrigger(t);
	}
	put8('E');
	putI32(PolygonTrigger::s_currentID);
	PolyTestDataChunkOutput writer(NULL);
	PolygonTrigger::WritePolygonTriggersDataChunk(writer);
	unsigned outLen = (unsigned)g_out.size();
	put(&outLen, 4);
	put(&g_out[0], (int)g_out.size());

	*nt = k;
	int j = 0;
	for (PolygonTrigger *t = PolygonTrigger::getFirstPolygonTrigger(); t; t = t->getNext(), ++j) {
		if (j >= trigCap) break;
		Coord3D c;
		t->getCenterPoint(&c);
		unsigned cx, cy, rr;
		float rv = t->getRadius();
		memcpy(&cx, &c.x, 4);
		memcpy(&cy, &c.y, 4);
		memcpy(&rr, &rv, 4);
		centers[2 * j] = cx;
		centers[2 * j + 1] = cy;
		radii[j] = rr;
		for (int q = 0; q < nq; ++q) {
			ICoord3D p;
			p.x = queries[2 * q];
			p.y = queries[2 * q + 1];
			p.z = 0;
			inside[j * nq + q] = t->pointInTrigger(p) ? 1 : 0;
		}
	}

	int n = (int)g_log.size();
	if (n > logCap) return -1;
	if (n > 0) memcpy(log, &g_log[0], n);
	*logLen = n;
	return n;
}

} /* extern "C" */

/* ---- the stand-in output (see the header comment) ----------------------- */

void PolyTestDataChunkOutput::openDataChunk(char *name, DataChunkVersionType ver)
{
	(void)name; /* allocateID is the caller's: g_writeId */
	unsigned id = g_writeId;
	outPut(&id, 4);
	outPut(&ver, 2);
	g_openChunks.push_back(g_out.size());
	Int dummy = 0xffff;
	outPut(&dummy, 4);
}

void PolyTestDataChunkOutput::closeDataChunk(void)
{
	if (g_openChunks.empty()) return;
	size_t filepos = g_openChunks.back();
	g_openChunks.pop_back();
	Int size = (Int)(g_out.size() - filepos - sizeof(Int));
	memcpy(&g_out[filepos], &size, sizeof(Int));
}

void PolyTestDataChunkOutput::writeInt(Int i) { outPut(&i, 4); }

void PolyTestDataChunkOutput::writeByte(Byte b) { outPut(&b, 1); }

void PolyTestDataChunkOutput::writeAsciiString(const AsciiString &theString)
{
	/* upstream's writeAsciiString takes getLength(), which is strlen (AsciiString.h
	** line 412): a name read with an embedded NUL is written up to the NUL. The
	** shim's own getLength is the std::string size, so strlen is taken here. */
	UnsignedShort len = (UnsignedShort)strlen(theString.str());
	outPut(&len, 2);
	outPut(theString.str(), len);
}
