/* Differential-test wrapper for GeometryInfo (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/System/Geometry.cpp
**   (#included below, unchanged; its headers are the INI build's, see build.rs)
**
** Entry points (extern "C", called by tests/differential_geometry.rs):
**   ref_geometry_parse   the INI path: one block of text parsed by the upstream
**                        GeometryInfo::parse* functions from the upstream field
**                        table, through the INI reader's own tokeniser and
**                        exception messages.
**   ref_geometry_op      the non-random methods, one op code at a time.
**   ref_geometry_random  the two random methods, from a logic seed.
**   ref_geometry_save / _load / ref_geometry_deep_crc: XferSave, XferLoad and
**                        XferDeepCRC of the snapshot.
**
** Every entry takes the initial shape as the GeometryInfo constructor does
** (type, isSmall, height, majorRadius, minorRadius) and then acts on it.
** The state vector ("fields") written to out[] is, in order: type, isSmall,
** the stored height, majorRadius, minorRadius, boundingCircleRadius,
** boundingSphereRadius; the reals as their bit patterns. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "Common/File.h"
#include "Common/INIException.h"
/* Lib/BaseType.h lines 143-158, verbatim: the shim of that header omits
** these templates, which Geometry.cpp calls unqualified. */
template <typename NUM>
inline NUM sqr(NUM x)
{
	return x*x;
}

template <typename NUM>
inline NUM clamp(NUM lo, NUM val, NUM hi)
{
	if (val < lo) return lo;
	else if (val > hi) return hi;
	else return val;
}
#include "Common/Xfer.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/XferDeepCRC.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Geometry.cpp"

/* from wrap_ini.cpp (the in-memory file system of the INI build) and
** wrap_random_value.cpp (the global logic random state) */
extern "C" void ref_ini_reset(void);
extern "C" void ref_ini_add_file(const char *name, const char *data, int len);
extern "C" void ref_init_game_logic_random(unsigned int seed);

namespace {

/* INI::load's first steps for a file whose one block holds the fields: the
** first line is read and its first token (the block type, taken with
** strtok as load does) consumed, then the field lines until `End` go to
** initFromINI. */
class GeometryIniReader : public INI
{
public:
	void readBlock(const char *file, void *what, const FieldParse *table)
	{
		prepFile(AsciiString(file), INI_LOAD_OVERWRITE);
		try {
			readLine();
			/* INI::load takes the block type with strtok on the line */
			if (!::strtok(m_buffer, getSeps())) throw INI_INVALID_DATA;
			initFromINI(what, table);
		} catch (...) {
			if (m_file) { m_file->close(); m_file = NULL; }
			throw;
		}
		if (m_file) { m_file->close(); m_file = NULL; }
	}
};

/* the upstream field table of ThingTemplate.cpp's Geometry entries */
const FieldParse GeometryFields[] =
{
	{ "Geometry", GeometryInfo::parseGeometryType, NULL, 0 },
	{ "GeometryMajorRadius", GeometryInfo::parseGeometryMajorRadius, NULL, 0 },
	{ "GeometryMinorRadius", GeometryInfo::parseGeometryMinorRadius, NULL, 0 },
	{ "GeometryHeight", GeometryInfo::parseGeometryHeight, NULL, 0 },
	{ "GeometryIsSmall", GeometryInfo::parseGeometryIsSmall, NULL, 0 },
	{ NULL, NULL, NULL, 0 }
};

/* The private m_height is not visible through a getter (for a sphere,
** getMaxHeightAbovePosition is the radius), so it is read at its byte
** offset. The object is a Snapshot (a vtable pointer, 8 bytes on x86_64)
** followed by the members in declaration order: m_type @8, m_isSmall @12,
** m_height @16, m_majorRadius @20, m_minorRadius @24,
** m_boundingCircleRadius @28, m_boundingSphereRadius @32. checkLayout
** compares every one of them with the getters, and the wrapper refuses to
** run if they disagree. */
float rawFloat(const GeometryInfo &g, int offset)
{
	float f;
	memcpy(&f, (const char *)&g + offset, 4);
	return f;
}

int rawInt(const GeometryInfo &g, int offset)
{
	int i;
	memcpy(&i, (const char *)&g + offset, 4);
	return i;
}

/* m_isSmall is a Bool: one byte in the shim's BaseType.h (`typedef bool`) */
int rawByte(const GeometryInfo &g, int offset)
{
	return *((const unsigned char *)&g + offset);
}

bool sameBits(float a, float b)
{
	unsigned ua, ub;
	memcpy(&ua, &a, 4);
	memcpy(&ub, &b, 4);
	return ua == ub;
}

bool checkLayout(const GeometryInfo &g)
{
	return rawInt(g, 8) == (int)g.getGeomType() && rawByte(g, 12) == (g.getIsSmall() ? 1 : 0) &&
		sameBits(rawFloat(g, 20), g.getMajorRadius()) && sameBits(rawFloat(g, 24), g.getMinorRadius()) &&
		sameBits(rawFloat(g, 28), g.getBoundingCircleRadius()) &&
		sameBits(rawFloat(g, 32), g.getBoundingSphereRadius());
}

/* the state vector (see the file comment) */
void fieldsOf(const GeometryInfo &g, float *out)
{
	unsigned w[7];
	w[0] = (unsigned)rawInt(g, 8);
	w[1] = (unsigned)rawByte(g, 12);
	float h = rawFloat(g, 16);
	memcpy(&w[2], &h, 4);
	float v;
	v = g.getMajorRadius(); memcpy(&w[3], &v, 4);
	v = g.getMinorRadius(); memcpy(&w[4], &v, 4);
	v = g.getBoundingCircleRadius(); memcpy(&w[5], &v, 4);
	v = g.getBoundingSphereRadius(); memcpy(&w[6], &v, 4);
	memcpy(out, w, sizeof w);
}

}  // namespace

extern "C" {

/* Parses `data` (one block, see GeometryIniReader) into a GeometryInfo that
** starts as GeometryInfo(type0, small0, height0, major0, minor0). out[0] is
** the status: 0 parsed, 1 an INIException (its message copied into msg, and
** *msgLen its length), 2 an INI_* status code (the enum has no name here).
** out[1..7] the state, as fieldsOf. Returns the status, or -1 if the layout
** check fails. */
int ref_geometry_parse(const char *file, const char *data, int len,
	int type0, int small0, float height0, float major0, float minor0,
	unsigned *out, char *msg, int msgCap, int *msgLen)
{
	ref_ini_reset();
	ref_ini_add_file(file, data, len);
	GeometryInfo g((GeometryType)type0, small0 != 0, height0, major0, minor0);
	if (!checkLayout(g)) return -1;
	int status = 0;
	*msgLen = 0;
	try {
		GeometryIniReader ini;
		ini.readBlock(file, &g, GeometryFields);
	} catch (INIException &e) {
		status = 1;
		const char *m = e.mFailureMessage ? e.mFailureMessage : "";
		int n = (int)strlen(m);
		*msgLen = n;
		if (n + 1 <= msgCap) memcpy(msg, m, n + 1);
	} catch (...) {
		status = 2;
	}
	out[0] = (unsigned)status;
	fieldsOf(g, (float *)(out + 1));
	return status;
}

/* Non-random methods. op codes, and the meaning of in[] and out[]:
**   0  getFootprintArea()                        out[0]
**   1  getMaxHeightAbovePosition()               out[0]
**   2  getMaxHeightBelowPosition()               out[0]
**   3  getZDeltaToCenterPosition()               out[0]
**   4  getCenterPosition(pos)                    in[0..3] pos; out[0..3]
**   5  calcPitches(thisPos, that, thatPos)       in[0..3] thisPos,
**        in[3..8] that = (type, isSmall, height, major, minor), in[8..11]
**        thatPos; out[0] minPitch, out[1] maxPitch
**   6  expandFootprint(r)                        in[0] r; out[0..7] fields
**   7  get2DBounds(center, angle)                in[0..3] center, in[3] angle;
**        out[0..4] lo.x, lo.y, hi.x, hi.y
**   8  clipPointToFootprint(center, pt)          in[0..3] center, in[3..6] pt;
**        out[0..3]
**   9  isPointInFootprint(center, pt)            in as 8; out[0] (1 or 0)
**  10  isIntersectedByLineSegment(loc, a, b)     in[0..3] loc, in[3..6] a,
**        in[6..9] b; out[0] (1 or 0)
**  11  setMaxHeightAbovePosition(z)              in[0] z; out[0..7] fields
**  12  setMajorRadius(r)                         in[0] r; out[0..7] fields
**  13  setMinorRadius(r)                         in[0] r; out[0..7] fields
**  14  set(type, isSmall, height, major, minor)  in[0..5]; out[0..7] fields
** Returns 0, -1 for an unknown op, or -2 if the layout check fails. */
int ref_geometry_op(int op, int type0, int small0, float height0, float major0, float minor0,
	const float *in, float *out, int *outCount)
{
	GeometryInfo g((GeometryType)type0, small0 != 0, height0, major0, minor0);
	if (!checkLayout(g)) return -2;
	int n = 1;
	switch (op) {
	case 0:
		out[0] = g.getFootprintArea();
		break;
	case 1:
		out[0] = g.getMaxHeightAbovePosition();
		break;
	case 2:
		out[0] = g.getMaxHeightBelowPosition();
		break;
	case 3:
		out[0] = g.getZDeltaToCenterPosition();
		break;
	case 4: {
		Coord3D pos = { in[0], in[1], in[2] }, c;
		g.getCenterPosition(pos, c);
		out[0] = c.x; out[1] = c.y; out[2] = c.z;
		n = 3;
		break;
	}
	case 5: {
		Coord3D thisPos = { in[0], in[1], in[2] };
		GeometryInfo that((GeometryType)(int)in[3], in[4] != 0.0f, in[5], in[6], in[7]);
		Coord3D thatPos = { in[8], in[9], in[10] };
		Real minP, maxP;
		g.calcPitches(thisPos, that, thatPos, minP, maxP);
		out[0] = minP; out[1] = maxP;
		n = 2;
		break;
	}
	case 6:
		g.expandFootprint(in[0]);
		fieldsOf(g, out);
		n = 7;
		break;
	case 7: {
		Coord3D center = { in[0], in[1], in[2] };
		Region2D b;
		g.get2DBounds(center, in[3], b);
		out[0] = b.lo.x; out[1] = b.lo.y; out[2] = b.hi.x; out[3] = b.hi.y;
		n = 4;
		break;
	}
	case 8: {
		Coord3D center = { in[0], in[1], in[2] };
		Coord3D pt = { in[3], in[4], in[5] };
		g.clipPointToFootprint(center, pt);
		out[0] = pt.x; out[1] = pt.y; out[2] = pt.z;
		n = 3;
		break;
	}
	case 9: {
		Coord3D center = { in[0], in[1], in[2] };
		Coord3D pt = { in[3], in[4], in[5] };
		out[0] = g.isPointInFootprint(center, pt) ? 1.0f : 0.0f;
		break;
	}
	case 10: {
		Coord3D loc = { in[0], in[1], in[2] };
		Coord3D from = { in[3], in[4], in[5] };
		Coord3D to = { in[6], in[7], in[8] };
		out[0] = g.isIntersectedByLineSegment(loc, from, to) ? 1.0f : 0.0f;
		break;
	}
	case 11:
		g.setMaxHeightAbovePosition(in[0]);
		fieldsOf(g, out);
		n = 7;
		break;
	case 12:
		g.setMajorRadius(in[0]);
		fieldsOf(g, out);
		n = 7;
		break;
	case 13:
		g.setMinorRadius(in[0]);
		fieldsOf(g, out);
		n = 7;
		break;
	case 14:
		g.set((GeometryType)(int)in[0], in[1] != 0.0f, in[2], in[3], in[4]);
		fieldsOf(g, out);
		n = 7;
		break;
	default:
		return -1;
	}
	*outCount = n;
	return 0;
}

/* The random methods from logic seed `seed`, `count` calls in a row (the
** generator state carries over): op 0 makeRandomOffsetWithinFootprint, op 1
** makeRandomOffsetOnPerimeter. out[3i..3i+3] is the point of call i. A
** perimeter call starts from z = 7, which upstream leaves alone for a sphere
** or cylinder. */
int ref_geometry_random(int op, unsigned seed, int type0, int small0, float height0, float major0, float minor0,
	int count, float *out)
{
	GeometryInfo g((GeometryType)type0, small0 != 0, height0, major0, minor0);
	ref_init_game_logic_random(seed);
	for (int i = 0; i < count; ++i) {
		Coord3D pt = { 0.0f, 0.0f, op == 1 ? 7.0f : 0.0f };
		if (op == 0) g.makeRandomOffsetWithinFootprint(pt);
		else g.makeRandomOffsetOnPerimeter(pt);
		out[3 * i] = pt.x;
		out[3 * i + 1] = pt.y;
		out[3 * i + 2] = pt.z;
	}
	return 0;
}

/* XferSave of GeometryInfo(type0, ...) to `path`; returns 0, or the
** XferStatus thrown plus one. */
int ref_geometry_save(const char *path, int type0, int small0, float height0, float major0, float minor0)
{
	GeometryInfo g((GeometryType)type0, small0 != 0, height0, major0, minor0);
	XferSave x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(&g);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
	}
	return status;
}

/* XferLoad of `path` into GeometryInfo(type0, ...). out[0] is the status
** (0 none, else the XferStatus plus one), out[1..7] the state. */
int ref_geometry_load(const char *path, int type0, int small0, float height0, float major0, float minor0, unsigned *out)
{
	GeometryInfo g((GeometryType)type0, small0 != 0, height0, major0, minor0);
	XferLoad x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(&g);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x.close(); } catch (...) {}
	}
	out[0] = (unsigned)status;
	fieldsOf(g, (float *)(out + 1));
	return status;
}

/* XferDeepCRC of GeometryInfo(type0, ...) written to `path`: the CRC of its xfer bytes. */
unsigned ref_geometry_deep_crc(const char *path, int type0, int small0, float height0, float major0, float minor0)
{
	GeometryInfo g((GeometryType)type0, small0 != 0, height0, major0, minor0);
	XferDeepCRC x;
	x.open(AsciiString(path));  /* XferDeepCRC writes the file it is given */
	x.xferSnapshot(&g);
	x.close();
	return x.getCRC();
}

} /* extern "C" */
