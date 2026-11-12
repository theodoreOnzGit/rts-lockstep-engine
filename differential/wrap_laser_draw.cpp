/* Differential-test wrapper for W3DLaserDraw of GitHub #168 (upstream commit 0a05454d), cut verbatim by build.rs into
** OUT_DIR/laser_draw_gen:
**   ld_psid.inc         GameEngine/Include/GameClient/ParticleSys.h, `enum ParticleSystemID` (to MAX_VOLUME_PARTICLE_DEPTH);
**   ld_module.inc       GameEngine/Source/Common/Thing/Module.cpp, `Module::crc`, `xfer` and `loadPostProcess` (to
**                       ObjectModule's constructor);
**   ld_drawable_module.inc  Module.cpp, `DrawableModule::crc`, `xfer` and `loadPostProcess` (to UpgradeMuxData);
**   ld_draw_module.inc  GameEngine/Source/Common/Thing/DrawModule.cpp, `DrawModule::crc`, `xfer`, `loadPostProcess` (to
**                       the file's end);
**   ld_color.inc        GameEngine/Source/GameClient/Color.cpp, `GameGetColorComponentsReal` (to GameDarkenColor);
**   ld_laser_h.inc      GameEngine/Include/GameLogic/Module/LaserUpdate.h (both classes, to the #endif);
**   ld_laser_data.inc   GameEngine/Source/GameLogic/Object/Update/LaserUpdate.cpp, the data's constructor (to
**                       buildFieldParse);
**   ld_laser_ctor.inc   LaserUpdate.cpp, the module's constructor and destructor (to
**                       updateStartPos);
**   ld_laser_radius.inc LaserUpdate.cpp, `getCurrentLaserRadius` (to `LaserUpdate::crc`);
**   ld_w3d_h.inc        GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DLaserDraw.h (both classes, to the
**                       #endif);
**   ld_w3d_data.inc     GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DLaserDraw.cpp, the data's
**                       constructor and destructor (to buildFieldParse);
**   ld_w3d.inc          W3DLaserDraw.cpp, the module's constructor to the file's end (destructor,
**                       getLaserTemplateWidth, doDrawModule, crc, xfer, loadPostProcess).
** No edit. Compiled on stand-ins (namespace ld): the module bases' declarations (Module.h's, DrawModule.h's), one
** drawable with its draw module list and its client update module, `SegmentedLineClass` (records what it is given),
** `TextureClass` and the asset manager (a texture of the case's size for a non-empty name), the scene, the terrain
** (8 heights picked by the port's `terrain_cell` hash of the point) and the particle system manager; on the REAL
** XferSave / XferLoad (upstream_xfer), WWMath's Vector3 (its Length is WWMath::Sqrt) and Vector2. LaserUpdate's
** clientUpdate, crc, xfer and loadPostProcess are not cut (other tests compare them): empty here. `cos`, `sqrt`,
** `sqrtf` are the Rust's petir functions in the test binary.
**
** Entry points (extern "C", tests/differential_laser_draw.rs; floats travel as their bits):
**   ref_laser_draw_seq(in, nin, out, cap): in = [numBeams, segments, innerBeamWidth, outerBeamWidth, innerColor,
**     outerColor, scrollRate, tile, texture (0 none, 1 "Beam.tga"), its width, its height, arcHeight,
**     segmentOverlapRatio, tilingScalar, 8 ground heights, has a LaserUpdate, the draw module list (0 [laser], 1
**     [other, laser], 2 [other]), the update's start xyz, end xyz, dirty, width scalar] (32 words), then steps of 8 words
**     [op, a1..a7]: 0 doDrawModule; 1 start = (a1, a2, a3), end = (a4, a5, a6); 2 setDirty(a1); 3 width scalar = a1;
**     4 loadPostProcess; 5 getCurrentLaserRadius (records [9, radius]); 6 getLaserTemplateWidth (records [8, width]);
**     7 has a LaserUpdate = a1. Records: after the constructor [200, has texture, aspect ratio] and the state; after
**     each step [100] and the state. State: [update dirty, selfDirty, line count], then per line [texture, additive,
**     width, r, g, b, uv rate x, y, tiled, visible, in scene, tile factor set, tile factor, point count, then the
**     points' xyz]. A case whose numBeams * segments wraps is undefined upstream: [999] and no run.
**   ref_laser_draw_xfer(mode, path, out, cap): mode 0 saves a fresh module to `path` (XferSave), mode 1 loads `path`
**     (XferLoad) into one. out = [status (0, or the XferStatus + 1), selfDirty]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "matrix3d.h"
#include "vector3.h"
#include "vector2.h"

/* Libraries/Include/Lib/BaseType.h:86 (the shim's BaseType.h does not carry it) */
#ifndef PI
#define PI 3.14159265359f
#endif
#ifndef MAX
#define MAX(x, y) (((x) > (y)) ? (x) : (y))
#endif

namespace ld {

std::vector<int> g_out;
void rec(const std::vector<int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }
int bits(float f)
{
	int i;
	memcpy(&i, &f, 4);
	return i;
}
float real(int i)
{
	float f;
	memcpy(&f, &i, 4);
	return f;
}

#include "ld_psid.inc"

typedef Int Color;
typedef Int NameKeyType;
#define NAMEKEY( s ) ((NameKeyType)1)
#ifndef NEW
#define NEW new
#endif
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( cls, name ) public: virtual ~cls();
#define MAKE_STANDARD_MODULE_MACRO( cls ) \
protected: \
	virtual void crc( Xfer *xfer ); \
	virtual void xfer( Xfer *xfer ); \
	virtual void loadPostProcess( void );
#define MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( cls, clsmd ) \
	MAKE_STANDARD_MODULE_MACRO(cls) \
private: \
	const clsmd* get##clsmd() const { return (clsmd*)getModuleData(); }
#define DEBUG_ASSERTCRASH( c, m )
#define DEBUG_CRASH( m )
#define REF_PTR_RELEASE( x ) { delete x; x = NULL; }
class MultiIniFieldParse;
class Thing {};
class Object;
class ModuleData { public: virtual ~ModuleData() {} };
class ClientUpdateModuleData : public ModuleData {};

/* ---- the world the module sees ---- */
struct LdWorld
{
	Bool hasUpdate;
	Int list;
	Real ground[8];
	Int texW, texH;
	Int scene;   /* render objects in the scene */
};
LdWorld g_w;

class ParticleSystemManager { public: void destroyParticleSystemByID( ParticleSystemID ) { abort(); } };
ParticleSystemManager g_psm;
ParticleSystemManager *TheParticleSystemManager = &g_psm;

class TerrainLogic
{
public:
	/* the port's `terrain_cell` (object_cuts.rs) */
	Real getGroundHeight( Real x, Real y ) const
	{
		UnsignedInt xb, yb;
		memcpy(&xb, &x, 4);
		memcpy(&yb, &y, 4);
		UnsignedInt r = (yb << 13) | (yb >> 19);
		return g_w.ground[(((xb ^ r) * 2654435761u) >> 28) & 7];
	}
};
TerrainLogic g_terrain;
TerrainLogic *TheTerrainLogic = &g_terrain;

/* ---- WW3D stand-ins ---- */
class SurfaceClass { public: struct SurfaceDescription { UnsignedInt Width, Height; }; };
class TextureClass
{
public:
	Bool m_init;
	TextureClass() : m_init(FALSE) {}
	Bool Is_Initialized() const { return m_init; }
	void Init() { m_init = TRUE; }
	void Get_Level_Description( SurfaceClass::SurfaceDescription &d ) { d.Width = (UnsignedInt)g_w.texW; d.Height = (UnsignedInt)g_w.texH; }
};
TextureClass g_texture;
class WW3DAssetManager
{
public:
	static WW3DAssetManager *Get_Instance() { static WW3DAssetManager m; return &m; }
	TextureClass *Get_Texture( const char *name ) { return (name == NULL || strlen(name) == 0) ? NULL : &g_texture; }
};
class ShaderClass { public: Int m_n; static ShaderClass _PresetAdditiveShader; };
ShaderClass ShaderClass::_PresetAdditiveShader = { 1 };
class SegLineRendererClass { public: enum TextureMapMode { UNIFORM_WIDTH_TEXTURE_MAP = 0, UNIFORM_LENGTH_TEXTURE_MAP, TILED_TEXTURE_MAP }; };
class SegmentedLineClass
{
public:
	Bool texture, additive, tiled, visible, inScene, tileSet;
	Real width, color[3], rate[2], tile;
	std::vector<Vector3> points;
	SegmentedLineClass() : texture(FALSE), additive(FALSE), tiled(FALSE), visible(TRUE), inScene(FALSE), tileSet(FALSE), width(0.0f), tile(0.0f)
	{
		color[0] = color[1] = color[2] = 1.0f;
		rate[0] = rate[1] = 0.0f;
	}
	void Set_Texture( TextureClass *t ) { texture = t != NULL; }
	void Set_Shader( ShaderClass s ) { additive = s.m_n == 1; }
	void Set_Width( float w ) { width = w; }
	void Set_Color( const Vector3 &c ) { color[0] = c.X; color[1] = c.Y; color[2] = c.Z; }
	void Set_UV_Offset_Rate( const Vector2 &r ) { rate[0] = r.X; rate[1] = r.Y; }
	void Set_Texture_Mapping_Mode( SegLineRendererClass::TextureMapMode m ) { tiled = m == SegLineRendererClass::TILED_TEXTURE_MAP; }
	void Set_Visible( int onoff ) { visible = onoff != 0; }
	void Set_Texture_Tile_Factor( float f ) { tileSet = TRUE; tile = f; }
	void Set_Points( unsigned int n, Vector3 *locs ) { points.assign(locs, locs + n); }
	void state()
	{
		rec({texture ? 1 : 0, additive ? 1 : 0, bits(width), bits(color[0]), bits(color[1]), bits(color[2]), bits(rate[0]), bits(rate[1]),
			tiled ? 1 : 0, visible ? 1 : 0, inScene ? 1 : 0, tileSet ? 1 : 0, bits(tile), (int)points.size()});
		for (size_t i = 0; i < points.size(); ++i) rec({bits(points[i].X), bits(points[i].Y), bits(points[i].Z)});
	}
};
class Scene
{
public:
	void Add_Render_Object( SegmentedLineClass *l ) { l->inScene = TRUE; ++g_w.scene; }
	void Remove_Render_Object( SegmentedLineClass *l ) { l->inScene = FALSE; --g_w.scene; }
};
Scene g_scene;
class W3DDisplay { public: static Scene *m_3DScene; };
Scene *W3DDisplay::m_3DScene = &g_scene;

/* ---- the module bases (Module.h's and DrawModule.h's declarations; the definitions are the cuts') ---- */
class ClientUpdateModule;
class DrawModule;
class Template { public: AsciiString getName() const { return AsciiString("T"); } };
class Drawable
{
public:
	ClientUpdateModule *m_update;
	DrawModule *m_modules[3];
	ClientUpdateModule *findClientUpdateModule( NameKeyType ) { return g_w.hasUpdate ? m_update : NULL; }
	const DrawModule **getDrawModules() const { return (const DrawModule **)m_modules; }
	const Template *getTemplate() const { static Template t; return &t; }
};
Drawable g_drawable;

class Module
{
public:
	Module( const ModuleData *moduleData ) : m_moduleData(moduleData) {}
	virtual ~Module() {}
	const ModuleData *getModuleData() const { return m_moduleData; }
protected:
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
private:
	const ModuleData *m_moduleData;
};
class DrawableModule : public Module
{
public:
	DrawableModule( Thing *, const ModuleData *moduleData ) : Module(moduleData) {}
	Drawable *getDrawable() const { return &g_drawable; }
protected:
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
};
class ClientUpdateModule : public DrawableModule
{
public:
	ClientUpdateModule( Thing *thing, const ModuleData *moduleData ) : DrawableModule(thing, moduleData) {}
	virtual void clientUpdate() = 0;
};
class LaserDrawInterface
{
public:
	virtual Real getLaserTemplateWidth() const = 0;
};
class DrawModule : public DrawableModule
{
public:
	DrawModule( Thing *thing, const ModuleData *moduleData ) : DrawableModule(thing, moduleData) {}
	virtual void doDrawModule( const Matrix3D *transformMtx ) = 0;
	virtual void setShadowsEnabled( Bool enable ) = 0;
	virtual void releaseShadows( void ) = 0;
	virtual void allocateShadows( void ) = 0;
	virtual void setFullyObscuredByShroud( Bool fullyObscured ) = 0;
	virtual void reactToTransformChange( const Matrix3D *oldMtx, const Coord3D *oldPos, Real oldAngle ) = 0;
	virtual void reactToGeometryChange() = 0;
	virtual Bool isLaser() const { return false; }
	virtual LaserDrawInterface *getLaserDrawInterface() { return NULL; }
	virtual const LaserDrawInterface *getLaserDrawInterface() const { return NULL; }
protected:
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
};
#include "ld_module.inc"
#include "ld_drawable_module.inc"
#include "ld_draw_module.inc"
#include "ld_color.inc"

/* a draw module without the laser interface (W3DModelDraw's place in the list) */
class OtherDraw : public DrawModule
{
public:
	OtherDraw() : DrawModule(NULL, NULL) {}
	virtual void doDrawModule( const Matrix3D * ) {}
	virtual void setShadowsEnabled( Bool ) {}
	virtual void releaseShadows( void ) {}
	virtual void allocateShadows( void ) {}
	virtual void setFullyObscuredByShroud( Bool ) {}
	virtual void reactToTransformChange( const Matrix3D *, const Coord3D *, Real ) {}
	virtual void reactToGeometryChange() {}
};

#include "ld_laser_h.inc"
#include "ld_laser_data.inc"
#include "ld_laser_ctor.inc"
#include "ld_laser_radius.inc"
void LaserUpdate::clientUpdate( void ) {}
void LaserUpdate::crc( Xfer * ) {}
void LaserUpdate::xfer( Xfer * ) {}
void LaserUpdate::loadPostProcess( void ) {}

#include "ld_w3d_h.inc"
#include "ld_w3d_data.inc"
#include "ld_w3d.inc"

/* the members, through derived classes (they are protected) */
struct LaserPeek : public LaserUpdate
{
	static LaserPeek *p(LaserUpdate *s) { return (LaserPeek *)s; }
	void set(const int *a) { m_startPos.x = real(a[0]); m_startPos.y = real(a[1]); m_startPos.z = real(a[2]); m_endPos.x = real(a[3]); m_endPos.y = real(a[4]); m_endPos.z = real(a[5]); }
	void width(int w) { m_currentWidthScalar = real(w); }
	Bool dirty() const { return m_dirty; }
};
struct DrawPeek : public W3DLaserDraw
{
	static DrawPeek *p(W3DLaserDraw *s) { return (DrawPeek *)s; }
	void state(int n)
	{
		rec({LaserPeek::p((LaserUpdate *)g_drawable.m_update)->dirty() ? 1 : 0, m_selfDirty ? 1 : 0, n});
		for (int i = 0; i < n; ++i) m_line3D[i]->state();
	}
	void head() { rec({m_texture != NULL ? 1 : 0, bits(m_textureAspectRatio)}); }
	void doXfer(Xfer *x) { xfer(x); }
	void doLoadPostProcess() { loadPostProcess(); }
	Bool selfDirty() const { return m_selfDirty; }
};

int copy_out(int *out, int cap)
{
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	if (n > 0) memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

} /* namespace ld */

extern "C" {

int ref_laser_draw_seq(const int *in, int nin, int *out, int cap)
{
	using namespace ld;
	g_out.clear();
	memset(&g_w, 0, sizeof g_w);
	g_texture.m_init = FALSE;
	W3DLaserDrawModuleData data;
	data.m_numBeams = (UnsignedInt)in[0];
	data.m_segments = (UnsignedInt)in[1];
	data.m_innerBeamWidth = real(in[2]);
	data.m_outerBeamWidth = real(in[3]);
	data.m_innerColor = (Color)in[4];
	data.m_outerColor = (Color)in[5];
	data.m_scrollRate = real(in[6]);
	data.m_tile = in[7] != 0;
	data.m_textureName = AsciiString(in[8] ? "Beam.tga" : "");
	g_w.texW = in[9];
	g_w.texH = in[10];
	data.m_arcHeight = real(in[11]);
	data.m_segmentOverlapRatio = real(in[12]);
	data.m_tilingScalar = real(in[13]);
	for (int k = 0; k < 8; ++k) g_w.ground[k] = real(in[14 + k]);
	g_w.hasUpdate = in[22] != 0;
	g_w.list = in[23];
	UnsignedInt n = data.m_numBeams * data.m_segments;
	if ((unsigned long long)data.m_numBeams * data.m_segments != (unsigned long long)n) {
		rec({999});
		return copy_out(out, cap);
	}
	LaserUpdateModuleData laserData;
	Thing thing;
	LaserUpdate *update = new LaserUpdate(&thing, &laserData);
	LaserPeek::p(update)->set(in + 24);
	update->setDirty(in[30] != 0);
	LaserPeek::p(update)->width(in[31]);
	g_drawable.m_update = update;
	W3DLaserDraw *draw = new W3DLaserDraw(&thing, &data);
	OtherDraw other;
	switch (g_w.list) {
	case 0: g_drawable.m_modules[0] = draw; g_drawable.m_modules[1] = NULL; break;
	case 1: g_drawable.m_modules[0] = &other; g_drawable.m_modules[1] = draw; g_drawable.m_modules[2] = NULL; break;
	default: g_drawable.m_modules[0] = &other; g_drawable.m_modules[1] = NULL; break;
	}
	rec({200});
	DrawPeek::p(draw)->head();
	DrawPeek::p(draw)->state((int)n);
	for (int st = 32; st + 7 < nin; st += 8) {
		const int *a = in + st;
		switch (a[0]) {
		case 0: draw->doDrawModule(NULL); break;
		case 1: LaserPeek::p(update)->set(a + 1); break;
		case 2: update->setDirty(a[1] != 0); break;
		case 3: LaserPeek::p(update)->width(a[1]); break;
		case 4: DrawPeek::p(draw)->doLoadPostProcess(); break;
		case 5: rec({9, bits(update->getCurrentLaserRadius())}); break;
		case 6: rec({8, bits(draw->getLaserTemplateWidth())}); break;
		default: g_w.hasUpdate = a[1] != 0; break;
		}
		rec({100});
		DrawPeek::p(draw)->state((int)n);
	}
	delete draw;
	if (g_w.scene != 0) abort();
	delete update;
	return copy_out(out, cap);
}

int ref_laser_draw_xfer(int mode, const char *path, int *out, int cap)
{
	using namespace ld;
	g_out.clear();
	memset(&g_w, 0, sizeof g_w);
	W3DLaserDrawModuleData data;
	Thing thing;
	W3DLaserDraw *draw = new W3DLaserDraw(&thing, &data);
	int status = 0;
	XferSave xs;
	XferLoad xl;
	Xfer *x = mode == 0 ? (Xfer *)&xs : (Xfer *)&xl;
	try {
		x->open(AsciiString(path));
		DrawPeek::p(draw)->doXfer(x);
		x->close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x->close(); } catch (...) {}
	}
	rec({status, DrawPeek::p(draw)->selfDirty() ? 1 : 0});
	delete draw;
	return copy_out(out, cap);
}

} /* extern "C" */
