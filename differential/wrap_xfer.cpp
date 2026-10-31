/* Differential-test wrapper for the Xfer family (upstream commit 0a05454d):
**   GameEngine/Source/Common/System/Xfer.cpp      (#included below, unchanged)
**   GameEngine/Source/Common/System/XferCRC.cpp   (compiled unchanged, own TU)
**   GameEngine/Source/Common/System/XferSave.cpp  (compiled unchanged, own TU)
**   GameEngine/Source/Common/System/XferLoad.cpp  (compiled unchanged, own TU)
**   GameEngine/Source/Common/System/Snapshot.cpp  (compiled unchanged, own TU)
** with the headers Xfer.h, XferCRC.h, XferDeepCRC.h, XferSave.h, XferLoad.h,
** Snapshot.h unchanged. The engine headers they pull in are replaced by the
** shims in differential/shim/Common (each says what it stands in for).
**
** Xfer.cpp is #included after matrix3d.h because xferMatrix3D needs the full
** Matrix3D/Vector4 (the real PreRTS.h brings WWMath in).
**
** ref_xfer_run interprets a script of Xfer operations (the same encoding as
** tests/differential_xfer.rs, which documents it) against a real XferCRC,
** XferSave, XferLoad or XferDeepCRC, and logs every operation's status and
** the values after it. Save/load/deep-CRC use real files at `path`. */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "matrix3d.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp"
#include "Common/XferCRC.h"
#include "Common/XferDeepCRC.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/Snapshot.h"

/* ---- stand-ins for the engine singletons the shims declare. The functions
** that use the lookups (xferMapName, xferScienceType/Vec, xferUpgradeMask)
** are NOT translated and NOT exercised; reaching them aborts. xferKindOf's
** two lookups forward to the REAL KindOfMaskType (BitFlags<KINDOF_COUNT>,
** KindOf.cpp's names) compiled in the BitFlags build (wrap_flags.cpp). */
extern "C" const char *ref_kindof_name_from_single_bit(int i);
extern "C" int ref_kindof_single_bit_from_name(const char *token);
/* p1w2-science: the three ScienceStore stand-ins are WEAK; Science.cpp (differential/wrap_science.cpp) defines the real ones */
__attribute__((weak)) ScienceStore *TheScienceStore = 0;
UpgradeCenter *TheUpgradeCenter = 0;
static GameState theShimGameState;
GameState *TheGameState = &theShimGameState;
__attribute__((weak)) AsciiString ScienceStore::getInternalNameForScience(ScienceType) const { abort(); }
__attribute__((weak)) ScienceType ScienceStore::getScienceFromInternalName(const AsciiString &) const { abort(); }
bool UpgradeMaskType::testForAll(const UpgradeMaskType &) const { abort(); }
void UpgradeMaskType::clear() { abort(); }
void UpgradeMaskType::set(const UpgradeMaskType &) { abort(); }
const UpgradeMaskType &UpgradeTemplate::getUpgradeMask() const { abort(); }
AsciiString UpgradeTemplate::getUpgradeName() const { abort(); }
UpgradeTemplate *UpgradeTemplate::friend_getNext() { abort(); }
UpgradeTemplate *UpgradeCenter::firstUpgradeTemplate() { abort(); }
const UpgradeTemplate *UpgradeCenter::findUpgrade(const AsciiString &) { abort(); }
AsciiString KindOfMaskType::getNameFromSingleBit(int i) { return AsciiString(ref_kindof_name_from_single_bit(i)); }
int KindOfMaskType::getSingleBitFromName(const char *token) { return ref_kindof_single_bit_from_name(token); }
/* layer 12 batch G1: a test that saves a map name through Xfer::xferMapName (virtual, so this build's) sets these hooks
** (differential/wrap_game_info.cpp); without them the two conversions abort, as before */
AsciiString (*g_xferRealToPortableHook)(const AsciiString &) = 0;
AsciiString (*g_xferPortableToRealHook)(const AsciiString &) = 0;
AsciiString GameState::realMapPathToPortableMapPath(const AsciiString &in) const { if (!g_xferRealToPortableHook) abort(); return g_xferRealToPortableHook(in); }
AsciiString GameState::portableMapPathToRealMapPath(const AsciiString &in) const { if (!g_xferPortableToRealHook) abort(); return g_xferPortableToRealHook(in); }
/* layer 12 batch S1: a test of GameState's post-process list (differential/wrap_game_state.cpp) sets this hook; without it the
** requests are counted, as before */
void (*g_xferAddPostProcessHook)(Snapshot *) = 0;
void GameState::addPostProcessSnapshot(Snapshot *s) { ++m_postProcessCount; if (g_xferAddPostProcessHook) g_xferAddPostProcessHook(s); }

/* ---- the script interpreter ------------------------------------------- */
namespace {

struct Script
{
	const unsigned *w; int n; int i; bool bad;
	unsigned next() { if (i < n) return w[i++]; bad = true; return 0; }
};

struct Log
{
	unsigned *w; int cap; int n;
	void put(unsigned v) { if (n < cap) w[n] = v; ++n; }
	void putReal(Real r) { unsigned u; memcpy(&u, &r, 4); put(u); }
};

static Real realFrom(unsigned u) { Real r; memcpy(&r, &u, 4); return r; }

/* The snapshot used by op 38: crc() xfers `a` and `b`; xfer() xfers a
** version, `a`, `b` and the string `s`. */
class TestSnapshot : public Snapshot
{
public:
	Int a; Real b; AsciiString s;
protected:
	virtual void crc(Xfer *x) { x->xferInt(&a); x->xferReal(&b); }
	virtual void xfer(Xfer *x)
	{
		XferVersion v = 1;
		x->xferVersion(&v, 1);
		x->xferInt(&a);
		x->xferReal(&b);
		x->xferAsciiString(&s);
	}
	virtual void loadPostProcess(void) {}
};

static void readAscii(Script &sc, AsciiString &s)
{
	int len = (int)sc.next();
	std::string tmp;
	for (int k = 0; k < len; ++k) tmp.push_back((char)sc.next());
	s.setBytes(tmp.data(), len);
}
static void logAscii(Log &lg, const AsciiString &s)
{
	lg.put((unsigned)s.getLength());
	for (int k = 0; k < s.getLength(); ++k) lg.put((unsigned char)s.str()[k]);
}
static void readUnicode(Script &sc, UnicodeString &s)
{
	int len = (int)sc.next();
	std::basic_string<WideChar> tmp;
	for (int k = 0; k < len; ++k) tmp.push_back((WideChar)sc.next());
	s.setUnits(tmp.data(), len);
}
static void logUnicode(Log &lg, const UnicodeString &s)
{
	lg.put((unsigned)s.getLength());
	for (int k = 0; k < s.getLength(); ++k) lg.put(s.str()[k]);
}

/* Runs one op; returns false at the end of the script. Throws XferStatus. */
static void runOp(int op, Script &sc, Log &lg, Xfer *x, XferCRC *crc, const char *path)
{
	switch (op) {
	case 0: { XferVersion v = (XferVersion)sc.next(); XferVersion cur = (XferVersion)sc.next();
		x->xferVersion(&v, cur); lg.put(v); break; }
	case 1: { Byte v = (Byte)sc.next(); x->xferByte(&v); lg.put((unsigned)(int)v); break; }
	case 2: { UnsignedByte v = (UnsignedByte)sc.next(); x->xferUnsignedByte(&v); lg.put(v); break; }
	case 3: { Bool v = sc.next() != 0; x->xferBool(&v); lg.put(v ? 1u : 0u); break; }
	case 4: { Int v = (Int)sc.next(); x->xferInt(&v); lg.put((unsigned)v); break; }
	case 5: { unsigned lo = sc.next(), hi = sc.next(); Int64 v = (Int64)(((unsigned long long)hi << 32) | lo);
		x->xferInt64(&v); lg.put((unsigned)(unsigned long long)v); lg.put((unsigned)((unsigned long long)v >> 32)); break; }
	case 6: { UnsignedInt v = sc.next(); x->xferUnsignedInt(&v); lg.put(v); break; }
	case 7: { Short v = (Short)sc.next(); x->xferShort(&v); lg.put((unsigned)(int)v); break; }
	case 8: { UnsignedShort v = (UnsignedShort)sc.next(); x->xferUnsignedShort(&v); lg.put(v); break; }
	case 9: { Real v = realFrom(sc.next()); x->xferReal(&v); lg.putReal(v); break; }
	case 10: { AsciiString s; readAscii(sc, s); x->xferMarkerLabel(s); break; }
	case 11: { AsciiString s; readAscii(sc, s); x->xferAsciiString(&s); logAscii(lg, s); break; }
	case 12: { UnicodeString s; readUnicode(sc, s); x->xferUnicodeString(&s); logUnicode(lg, s); break; }
	case 13: { Coord3D c; c.x = realFrom(sc.next()); c.y = realFrom(sc.next()); c.z = realFrom(sc.next());
		x->xferCoord3D(&c); lg.putReal(c.x); lg.putReal(c.y); lg.putReal(c.z); break; }
	case 14: { ICoord3D c; c.x = sc.next(); c.y = sc.next(); c.z = sc.next();
		x->xferICoord3D(&c); lg.put(c.x); lg.put(c.y); lg.put(c.z); break; }
	case 15: { Region3D r; Real *p = &r.lo.x; for (int k = 0; k < 6; ++k) p[k] = realFrom(sc.next());
		x->xferRegion3D(&r); for (int k = 0; k < 6; ++k) lg.putReal(p[k]); break; }
	case 16: { IRegion3D r; Int *p = &r.lo.x; for (int k = 0; k < 6; ++k) p[k] = sc.next();
		x->xferIRegion3D(&r); for (int k = 0; k < 6; ++k) lg.put(p[k]); break; }
	case 17: { Coord2D c; c.x = realFrom(sc.next()); c.y = realFrom(sc.next());
		x->xferCoord2D(&c); lg.putReal(c.x); lg.putReal(c.y); break; }
	case 18: { ICoord2D c; c.x = sc.next(); c.y = sc.next(); x->xferICoord2D(&c); lg.put(c.x); lg.put(c.y); break; }
	case 19: { Region2D r; Real *p = &r.lo.x; for (int k = 0; k < 4; ++k) p[k] = realFrom(sc.next());
		x->xferRegion2D(&r); for (int k = 0; k < 4; ++k) lg.putReal(p[k]); break; }
	case 20: { IRegion2D r; Int *p = &r.lo.x; for (int k = 0; k < 4; ++k) p[k] = sc.next();
		x->xferIRegion2D(&r); for (int k = 0; k < 4; ++k) lg.put(p[k]); break; }
	case 21: { RealRange r; r.lo = realFrom(sc.next()); r.hi = realFrom(sc.next());
		x->xferRealRange(&r); lg.putReal(r.lo); lg.putReal(r.hi); break; }
	case 22: { Color c = sc.next(); x->xferColor(&c); lg.put(c); break; }
	case 23: { RGBColor c; c.red = realFrom(sc.next()); c.green = realFrom(sc.next()); c.blue = realFrom(sc.next());
		x->xferRGBColor(&c); lg.putReal(c.red); lg.putReal(c.green); lg.putReal(c.blue); break; }
	case 24: { RGBAColorReal c; Real *p = &c.red; for (int k = 0; k < 4; ++k) p[k] = realFrom(sc.next());
		x->xferRGBAColorReal(&c); for (int k = 0; k < 4; ++k) lg.putReal(p[k]); break; }
	case 25: { RGBAColorInt c; UnsignedInt *p = &c.red; for (int k = 0; k < 4; ++k) p[k] = sc.next();
		x->xferRGBAColorInt(&c); for (int k = 0; k < 4; ++k) lg.put(p[k]); break; }
	case 26: { ObjectID id = (ObjectID)sc.next(); x->xferObjectID(&id); lg.put((unsigned)id); break; }
	case 27: { DrawableID id = (DrawableID)sc.next(); x->xferDrawableID(&id); lg.put((unsigned)id); break; }
	case 28: case 29: case 30: {
		/* operands: count, items; in Load mode the container starts empty
		** unless op 42 (below) asks otherwise */
		int cnt = (int)sc.next();
		std::vector<unsigned> items;
		for (int k = 0; k < cnt; ++k) items.push_back(sc.next());
		bool load = x->getXferMode() == XFER_LOAD;
		if (op == 28) {
			std::vector<ObjectID> v; if (!load) for (size_t k = 0; k < items.size(); ++k) v.push_back((ObjectID)items[k]);
			try { x->xferSTLObjectIDVector(&v); } catch (...) { lg.put(v.size()); for (size_t k = 0; k < v.size(); ++k) lg.put(v[k]); throw; }
			lg.put(v.size()); for (size_t k = 0; k < v.size(); ++k) lg.put(v[k]);
		} else if (op == 29) {
			std::list<ObjectID> v; if (!load) for (size_t k = 0; k < items.size(); ++k) v.push_back((ObjectID)items[k]);
			try { x->xferSTLObjectIDList(&v); } catch (...) { lg.put(v.size()); for (std::list<ObjectID>::iterator it = v.begin(); it != v.end(); ++it) lg.put(*it); throw; }
			lg.put(v.size()); for (std::list<ObjectID>::iterator it = v.begin(); it != v.end(); ++it) lg.put(*it);
		} else {
			std::list<Int> v; if (!load) for (size_t k = 0; k < items.size(); ++k) v.push_back((Int)items[k]);
			try { x->xferSTLIntList(&v); } catch (...) { lg.put(v.size()); for (std::list<Int>::iterator it = v.begin(); it != v.end(); ++it) lg.put(*it); throw; }
			lg.put(v.size()); for (std::list<Int>::iterator it = v.begin(); it != v.end(); ++it) lg.put(*it);
		}
		break; }
	case 31: { int cnt = (int)sc.next(); std::vector<unsigned char> b(cnt > 0 ? cnt : 1);
		for (int k = 0; k < cnt; ++k) b[k] = (unsigned char)sc.next();
		x->xferUser(&b[0], cnt);
		for (int k = 0; k < cnt; ++k) lg.put(b[k]); break; }
	case 32: { Matrix3D m; for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) m[r][c] = realFrom(sc.next());
		x->xferMatrix3D(&m); for (int r = 0; r < 3; ++r) for (int c = 0; c < 4; ++c) lg.putReal(m[r][c]); break; }
	case 33: { lg.put((unsigned)x->beginBlock()); break; }
	case 34: { x->endBlock(); break; }
	case 35: { x->skip((Int)sc.next()); break; }
	case 36: { x->setOptions(sc.next()); lg.put(x->getOptions()); break; }
	case 37: { x->clearOptions(sc.next()); lg.put(x->getOptions()); break; }
	case 38: { TestSnapshot s; s.a = (Int)sc.next(); s.b = realFrom(sc.next()); readAscii(sc, s.s);
		try { x->xferSnapshot(&s); } catch (...) { lg.put(s.a); lg.putReal(s.b); logAscii(lg, s.s); throw; }
		lg.put(s.a); lg.putReal(s.b); logAscii(lg, s.s); break; }
	case 39: { x->open(AsciiString(path)); break; }
	case 40: { x->close(); break; }
	case 41: { lg.put(crc ? crc->getCRC() : 0u); break; }
	case 42: { /* xferSTLIntList into a NON-empty list (load: XFER_LIST_NOT_EMPTY) */
		std::list<Int> v; v.push_back((Int)sc.next());
		try { x->xferSTLIntList(&v); } catch (...) { lg.put(v.size()); throw; }
		lg.put(v.size()); break; }
	default: sc.bad = true; break;
	}
}

} /* namespace */

extern "C" {

/* kind: 0 XferCRC, 1 XferSave, 2 XferLoad, 3 XferDeepCRC. Each op logs its
** status first (0 = no exception, else the thrown XferStatus), then its
** values; the end of the log carries the mode, the options, the CRC (0 for
** save/load) and the post-process count. Returns the log length, or -1 for a
** malformed script. */
int ref_xfer_run(int kind, const char *path, const unsigned *script, int n, unsigned *log, int cap)
{
	XferCRC crcX; XferSave saveX; XferLoad loadX; XferDeepCRC deepX;
	Xfer *x; XferCRC *crc = 0;
	switch (kind) {
	case 0: x = &crcX; crc = &crcX; break;
	case 1: x = &saveX; break;
	case 2: x = &loadX; break;
	case 3: x = &deepX; crc = &deepX; break;
	default: return -1;
	}
	theShimGameState.m_postProcessCount = 0;
	Script sc = { script, n, 0, false };
	Log lg = { log, cap, 0 };
	while (sc.i < sc.n && !sc.bad) {
		int op = (int)sc.next();
		int statusAt = lg.n;
		lg.put(0);
		try {
			runOp(op, sc, lg, x, crc, path);
		} catch (XferStatus s) {
			if (statusAt < cap) log[statusAt] = (unsigned)s;
		}
	}
	if (sc.bad) return -1;
	lg.put((unsigned)x->getXferMode());
	lg.put(x->getOptions());
	lg.put(crc ? crc->getCRC() : 0u);
	lg.put((unsigned)theShimGameState.m_postProcessCount);
	/* close anything still open, without logging (destructors would otherwise
	** warn and close; the test reads the files afterwards) */
	try { if (kind == 1 || kind == 2 || kind == 3) x->close(); } catch (...) {}
	return lg.n;
}

/* Xfer::xferKindOf(&value) on a fresh xfer of `kind` (0 XferCRC, 1 XferSave
** to path, 2 XferLoad from path): returns 0 or the XferStatus thrown; *value
** after the call into out[0], the CRC (kind 0) into out[1]. */
int ref_xfer_kind_of(int kind, const char *path, int value, int *out)
{
	XferCRC crcX; XferSave saveX; XferLoad loadX;
	Xfer *x = kind == 0 ? (Xfer *)&crcX : kind == 1 ? (Xfer *)&saveX : (Xfer *)&loadX;
	KindOfType k = (KindOfType)value;
	int status = 0;
	x->open(AsciiString(path));
	try { x->xferKindOf(&k); } catch (XferStatus s) { status = (int)s; }
	x->close();
	out[0] = (int)k;
	out[1] = kind == 0 ? (int)crcX.getCRC() : 0;
	return status;
}

} /* extern "C" */
