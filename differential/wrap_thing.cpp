/* Differential-test wrapper for Thing (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp
** (#included below, unchanged; its headers are the Thing build's, see build.rs
** and differential/shim_thing).
**
** The stand-ins of differential/shim_thing: a ThingTemplate (kind mask and
** build cost), a GlobalData with m_gravity, TheTerrainLogic (ground height
** and water as a plane, and alignOnTerrain as a rotation about Z over the
** ground height), and an Object. Thing is abstract (reactToTransformChange is
** pure virtual), so TestThing records each reaction.
**
** The stand-ins are named apart from the same names in other differential
** builds (wrap_big.cpp defines a TheGlobalData, others define their own
** ThingTemplate): one binary links them all, and identical global or class
** symbols would clash. The upstream .cpp sees these names only through the
** macros, so its own text is unchanged.
**
** Entry point (extern "C", tests/differential_thing.rs): ref_thing_run runs a
** program of operations on one Thing and writes what each one observes; the
** Rust interpreter in the test writes the same words for the same program.
** The op codes are listed at ref_thing_run. */
#define Coord3D ThingTestCoord3D
#define ThingTemplate ThingTestTemplate
#define Object ThingTestObject
#define TerrainLogic ThingTestTerrainLogic
#define TheTerrainLogic ThingTestTheTerrainLogic
#define GlobalData ThingTestGlobalData
#define TheGlobalData ThingTestTheGlobalData

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp"

/* ---- stand-ins ----------------------------------------------------------- */

/* the terrain of one program: ground(x, y) = ga*x + gb*y + gc, water under
** every point with x < wx at height wz, and the gravity */
namespace {
struct TerrainParams {
	float ga, gb, gc, wx, wz, gravity;
} g_terrain = { 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f };

ThingTestGlobalData g_globalData;
}

ThingTestGlobalData *ThingTestTheGlobalData = &g_globalData;

Real ThingTestTerrainLogic::getGroundHeight(Real x, Real y, Coord3D *) const
{
	return g_terrain.ga * x + g_terrain.gb * y + g_terrain.gc;
}

Bool ThingTestTerrainLogic::isUnderwater(Real x, Real y, Real *waterZ, Real *terrainZ)
{
	(void)y;
	if (x < g_terrain.wx) {
		if (waterZ) *waterZ = g_terrain.wz;
		if (terrainZ) *terrainZ = getGroundHeight(x, y);
		return TRUE;
	}
	return FALSE;
}

/* the matrix upstream's real alignOnTerrain would write: about Z by the
** orientation, translated to the position, at the ground height when the
** thing sticks to the ground */
Int ThingTestTerrainLogic::alignOnTerrain(Real angle, const Coord3D &pos, Bool stickToGround, Matrix3D &mtx)
{
	Real z = stickToGround ? getGroundHeight(pos.x, pos.y) : pos.z;
	Real c = Cos(angle);
	Real s = Sin(angle);
	mtx.Set(c, -s, 0.0f, pos.x,
	        s, c, 0.0f, pos.y,
	        0.0f, 0.0f, 1.0f, z);
	return 0;
}

static ThingTestTerrainLogic g_terrainLogic;
ThingTestTerrainLogic *ThingTestTheTerrainLogic = &g_terrainLogic;

namespace {

/* Records every reaction: how many, and the old state of the last one. */
class TestThing : public Thing
{
public:
	explicit TestThing(const ThingTemplate *t) : Thing(t), reactions(0)
	{
		memset(last, 0, sizeof last);
	}
	int reactions;
	float last[16];	/* the old Row[3][4], the old position, the old angle */

protected:
	void reactToTransformChange(const Matrix3D *oldMtx, const Coord3D *oldPos, Real oldAngle)
	{
		++reactions;
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c)
				last[4 * r + c] = (*oldMtx)[r][c];
		last[12] = oldPos->x;
		last[13] = oldPos->y;
		last[14] = oldPos->z;
		last[15] = oldAngle;
	}
};

unsigned bitsOf(float f)
{
	unsigned u;
	memcpy(&u, &f, 4);
	return u;
}

void putFloat(unsigned *out, int &n, float f)
{
	out[n++] = bitsOf(f);
}

}  // namespace

extern "C" {

/* ref_thing_run: a program of operations on one Thing (a template with the
** kind mask given as two 64-bit words, bit k = kind k), on the terrain
** `terrain` = {ga, gb, gc, wx, wz, gravity}.
**
** ops[3*i..3*i+3] = (code, arg1, arg2) and params[16*i..16*i+16] are op i's
** operands. The codes, and what each writes to out[] after its own words:
**   0 setPosition(params 0..3)                   (none)
**   1 setPositionZ(params 0)                     (none)
**   2 setOrientation(params 0)                   (none)
**   3 setTransformMatrix(params 0..12)           (none)
**   4 transformPoint(in = params 0..3, out)      out 3 words (the output)
**   5 transformPoint(NULL, out)                  out 3 words
**   6 transformPoint(in, NULL)                   (none)
**   7 convertBonePosToWorldPos(bonePos = params 0..3, boneTransform = params 3..15)
**                                                worldPos 3 words, worldTransform 12
**   8 getUnitDirectionVector2D()                 3 words
**   9 getUnitDirectionVector3D()                 3 words
**  10 getHeightAboveTerrain()                    1 word
**  11 getHeightAboveTerrainOrWater()             1 word
**  12 isSignificantlyAboveTerrain()              1 word (0 or 1)
**  13 isAboveTerrain()                           1 word
**  14 isAboveTerrainOrWater()                    1 word
**  15 isKindOf(arg1)                             1 word
**  16 isKindOfMulti(set = {arg1}, clear = {arg2}) 1 word (arg < 0: the empty mask)
**  17 isAnyKindOf({arg1})                        1 word (arg1 < 0: the empty mask)
** After every op: the base state (the 12 transform entries, the cached
** position and the cached orientation, 16 words); then the number of
** reactions the op caused (1 word), and if it caused one, that reaction's
** old state (16 words, TestThing::last).
** Returns the number of words written, or -1 if they would not fit in cap. */
int ref_thing_run(const unsigned long long *kindWords, const float *terrain, int nOps, const int *ops,
	const float *params, unsigned *out, int cap)
{
	g_terrain.ga = terrain[0];
	g_terrain.gb = terrain[1];
	g_terrain.gc = terrain[2];
	g_terrain.wx = terrain[3];
	g_terrain.wz = terrain[4];
	g_terrain.gravity = terrain[5];
	g_globalData.m_gravity = terrain[5];

	KindOfMaskType kinds;
	for (int k = 0; k < KINDOF_COUNT; ++k)
		kinds.set(k, (kindWords[k / 64] >> (k % 64)) & 1ull);
	ThingTestTemplate tmpl(kinds, 0);
	TestThing thing(&tmpl);

	int n = 0;
	for (int i = 0; i < nOps; ++i) {
		const int code = ops[3 * i];
		const int a1 = ops[3 * i + 1];
		const int a2 = ops[3 * i + 2];
		const float *p = params + 16 * i;
		const int reactionsBefore = thing.reactions;
		if (n + 64 > cap) return -1;
		switch (code) {
		case 0: {
			Coord3D pos = { p[0], p[1], p[2] };
			thing.setPosition(&pos);
			break;
		}
		case 1:
			thing.setPositionZ(p[0]);
			break;
		case 2:
			thing.setOrientation(p[0]);
			break;
		case 3: {
			Matrix3D m;
			m.Set(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[11]);
			thing.setTransformMatrix(&m);
			break;
		}
		case 4: {
			Coord3D in = { p[0], p[1], p[2] }, o = { 0.0f, 0.0f, 0.0f };
			thing.transformPoint(&in, &o);
			putFloat(out, n, o.x);
			putFloat(out, n, o.y);
			putFloat(out, n, o.z);
			break;
		}
		case 5: {
			Coord3D o = { 0.0f, 0.0f, 0.0f };
			thing.transformPoint(NULL, &o);
			putFloat(out, n, o.x);
			putFloat(out, n, o.y);
			putFloat(out, n, o.z);
			break;
		}
		case 6: {
			Coord3D in = { p[0], p[1], p[2] };
			thing.transformPoint(&in, NULL);
			break;
		}
		case 7: {
			Coord3D bonePos = { p[0], p[1], p[2] }, wp = { 0.0f, 0.0f, 0.0f };
			Matrix3D boneTm, wt;
			boneTm.Set(p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[11], p[12], p[13], p[14]);
			thing.convertBonePosToWorldPos(&bonePos, &boneTm, &wp, &wt);
			putFloat(out, n, wp.x);
			putFloat(out, n, wp.y);
			putFloat(out, n, wp.z);
			for (int r = 0; r < 3; ++r)
				for (int c = 0; c < 4; ++c)
					putFloat(out, n, wt[r][c]);
			break;
		}
		case 8: {
			const Coord3D *d = thing.getUnitDirectionVector2D();
			putFloat(out, n, d->x);
			putFloat(out, n, d->y);
			putFloat(out, n, d->z);
			break;
		}
		case 9: {
			Coord3D d = { 0.0f, 0.0f, 0.0f };
			thing.getUnitDirectionVector3D(d);
			putFloat(out, n, d.x);
			putFloat(out, n, d.y);
			putFloat(out, n, d.z);
			break;
		}
		case 10:
			putFloat(out, n, thing.getHeightAboveTerrain());
			break;
		case 11:
			putFloat(out, n, thing.getHeightAboveTerrainOrWater());
			break;
		case 12:
			out[n++] = thing.isSignificantlyAboveTerrain() ? 1u : 0u;
			break;
		case 13:
			out[n++] = thing.isAboveTerrain() ? 1u : 0u;
			break;
		case 14:
			out[n++] = thing.isAboveTerrainOrWater() ? 1u : 0u;
			break;
		case 15:
			out[n++] = thing.isKindOf((KindOfType)a1) ? 1u : 0u;
			break;
		case 16: {
			KindOfMaskType must, clr;
			if (a1 >= 0) must.set(a1, 1);
			if (a2 >= 0) clr.set(a2, 1);
			out[n++] = thing.isKindOfMulti(must, clr) ? 1u : 0u;
			break;
		}
		case 17: {
			KindOfMaskType any;
			if (a1 >= 0) any.set(a1, 1);
			out[n++] = thing.isAnyKindOf(any) ? 1u : 0u;
			break;
		}
		default:
			return -2;
		}
		/* the base state */
		const Matrix3D *tm = thing.getTransformMatrix();
		for (int r = 0; r < 3; ++r)
			for (int c = 0; c < 4; ++c)
				putFloat(out, n, (*tm)[r][c]);
		putFloat(out, n, thing.getPosition()->x);
		putFloat(out, n, thing.getPosition()->y);
		putFloat(out, n, thing.getPosition()->z);
		putFloat(out, n, thing.getOrientation());
		/* the reactions */
		int delta = thing.reactions - reactionsBefore;
		out[n++] = (unsigned)delta;
		if (delta > 0)
			for (int k = 0; k < 16; ++k)
				putFloat(out, n, thing.last[k]);
	}
	return n;
}

} /* extern "C" */
