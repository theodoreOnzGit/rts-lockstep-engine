/* Differential-test wrapper for the radar's drawing items of GitHub #57 (#162, upstream commit 0a05454d), cut verbatim
** by build.rs into OUT_DIR/radar_draw_gen:
**   rd_cells.inc        GameEngine/Include/Common/Radar.h, the RADAR_CELL_* enum and `enum RadarEventType` (to
**                       "// PROTOTYPES");
**   rd_frames.inc       GameEngine/Include/Common/GameCommon.h, the LOGICFRAMES_PER_SECOND enum to
**                       SECONDS_PER_LOGICFRAME_REAL (inclusive);
**   rd_psid.inc         GameEngine/Include/GameClient/ParticleSys.h, `enum ParticleSystemID` (to MAX_VOLUME_PARTICLE_DEPTH);
**   rd_win_status.inc   GameEngine/Include/GameClient/GameWindow.h, the WIN_STATUS_ENABLED line (inside an enum here);
**   rd_hero.inc         GameEngineDevice/Source/W3DDevice/Common/System/W3DRadar.cpp, `W3DRadar::radarToPixel` and
**                       `drawHeroIcon` (to drawViewBox's comment);
**   rd_glow.inc         GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, the RADAR_ATTACK_GLOW enum,
**                       `triggerRadarAttackGlow` and `updateRadarAttackGlow` (to initSpecialPowershortcutBar);
**   rd_glow_reset.inc   ControlBar.cpp, `ControlBar::reset`'s four glow lines (to m_displayedConstructPercent);
**   rd_win_enable.inc   GameEngine/Source/GameClient/GUI/GameWindow.cpp, `GameWindow::winEnable` (to winGetEnabled's
**                       comment);
**   rd_beacon_h.inc     GameEngine/Include/GameClient/Module/BeaconClientUpdate.h, both classes (to the #endif);
**   rd_beacon_data.inc  GameEngine/Source/GameClient/Drawable/Update/BeaconClientUpdate.cpp, the module data's
**                       constructor and destructor (to buildFieldParse);
**   rd_beacon.inc       BeaconClientUpdate.cpp, the module's constructor, destructor, createParticleSystem, hideBeacon
**                       and clientUpdate (to the CRC comment).
** No edit. They are compiled on stand-ins (namespace rd): a W3DRadar with m_mapExtent; a mapped image collection whose
** "HeroReticle" is one Image object whose size each case sets (drawHeroIcon keeps the pointer in a function static, so
** the lookup happens once a process: the null-image branch cannot be reached after the first call, and is not
** compared); a display recording drawImage; GameWindows with a status and children; a ControlBar with the glow's three
** members; the beacon's drawable, object, particle system manager, radar and logic, recording what is called.
**
** Entry points (extern "C", tests/differential_radar_draw.rs):
**   ref_radar_hero_icon(f, i, out): f = [extent width, extent height, pos x, pos y], i = [pixelX, pixelY, width,
**     height, icon width, icon height]; out = [drawImage called, startX, startY, endX, endY]. Only inputs the Rust
**     finds defined (float to Int conversions in range, no Int overflow) are given.
**   ref_radar_glow_seq(in, nin, out, cap): in = [window exists, its enabled bit, children (0..3), the children's
**     enabled bits], then steps of 2 words [op, arg]: 0 triggerRadarAttackGlow, 1 updateRadarAttackGlow, 2 reset's
**     glow lines, 3 the window's winEnable(arg) from elsewhere. After each step [100, on, remaining frames, the
**     window's enabled bit, the children's enabled bits]. Returns the word count.
**   ref_beacon_seq(in, nin, out, cap): in = [RadarPulseFrequency, RadarPulseDuration (frames), has object, indicator
**     colour, the colour's template exists, BeaconSmokeFFFFFF exists, createParticleSystem fails, logic frame], then
**     steps of 2 words [op, arg]: 0 clientUpdate, 1 hideBeacon, 2 the logic frame = arg, 3 hidden by stealth = arg,
**     4 the manager loses the current system, 5 has object = arg, 6 indicator colour = arg. Records of 8 words:
**     [200, default frequency, default duration] first; [1, length, 5 words of the name, found] findTemplate;
**     [2, id] a system made and attached; [3, id, colour] tintAllColors; [4, id] stop (a system the manager has);
**     [5] setDrawableHidden(TRUE); [6, enabled] setShadowsEnabled; [7, x, y, z, type, seconds] createEvent (floats as
**     bits); after each step [100, m_particleSystemID, m_lastRadarPulse]. Returns the word count. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"

namespace rd {

std::vector<int> g_out;
void rec(int tag, int a = 0, int b = 0, int c = 0, int d = 0, int e = 0, int f = 0, int g = 0)
{
	int w[8] = { tag, a, b, c, d, e, f, g };
	g_out.insert(g_out.end(), w, w + 8);
}
int bits(float f)
{
	int i;
	memcpy(&i, &f, 4);
	return i;
}

#include "rd_cells.inc"
#include "rd_frames.inc"
#include "rd_psid.inc"
enum WinStatusCut {
#include "rd_win_status.inc"
};

/* ---- drawHeroIcon ---- */
class Image
{
public:
	Int m_w, m_h;
	Int getImageWidth( void ) const { return m_w; }
	Int getImageHeight( void ) const { return m_h; }
};
Image g_heroReticle;
class ImageCollection
{
public:
	const Image *findImageByName( const AsciiString &name ) { return strcmp(name.str(), "HeroReticle") == 0 ? &g_heroReticle : NULL; }
};
ImageCollection g_images;
ImageCollection *TheMappedImageCollection = &g_images;
enum DrawImageMode { DRAW_IMAGE_SOLID, DRAW_IMAGE_GRAYSCALE, DRAW_IMAGE_ALPHA, DRAW_IMAGE_ADDITIVE };
typedef Int Color;
int g_drawn, g_drawRect[4];
class Display
{
public:
	void drawImage( const Image *image, Int startX, Int startY, Int endX, Int endY, Color color = 0xFFFFFFFF, DrawImageMode mode = DRAW_IMAGE_ALPHA )
	{
		if (image != &g_heroReticle || color != (Color)0xFFFFFFFF || mode != DRAW_IMAGE_ALPHA) abort();
		g_drawn = 1;
		g_drawRect[0] = startX; g_drawRect[1] = startY; g_drawRect[2] = endX; g_drawRect[3] = endY;
	}
};
Display g_display;
Display *TheDisplay = &g_display;
class W3DRadar
{
public:
	Region3D m_mapExtent;
	void radarToPixel( const ICoord2D *radar, ICoord2D *pixel, Int radarUpperLeftX, Int radarUpperLeftY, Int radarWidth, Int radarHeight );
	void drawHeroIcon( Int pixelX, Int pixelY, Int width, Int height, const Coord3D *pos );
};
#include "rd_hero.inc"

/* ---- the radar attack glow ---- */
class GameWindow
{
public:
	UnsignedInt m_status;
	GameWindow *m_child, *m_next;
	Int winEnable( Bool enable );
	UnsignedInt winGetStatus( void ) { return m_status; }
};
enum { WIN_ERR_OK = 0 };
#include "rd_win_enable.inc"
class ControlBar
{
public:
	Bool m_radarAttackGlowOn;
	Int m_remainingRadarAttackGlowFrames;
	GameWindow *m_radarAttackGlowWindow;
	/* ControlBar::ControlBar's lines (ControlBar.cpp:960-962) */
	ControlBar() : m_radarAttackGlowOn(FALSE), m_remainingRadarAttackGlowFrames(0), m_radarAttackGlowWindow(NULL) {}
	void triggerRadarAttackGlow( void );
	void updateRadarAttackGlow ( void );
	void resetGlow( void )
	{
#include "rd_glow_reset.inc"
	}
};
#include "rd_glow.inc"

/* ---- BeaconClientUpdate ---- */
#define MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( cls, name ) public: virtual ~cls();
#define MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( cls, data )
#define DEBUG_ASSERTCRASH( c, m )
class MultiIniFieldParse;
class Thing {};
class ModuleData { public: virtual ~ModuleData() {} };
class ClientUpdateModuleData : public ModuleData {};
struct BeaconWorld
{
	Bool hasObject, colorTemplate, whiteTemplate, createFails, hiddenByStealth, hidden, lost;
	Int color;
	UnsignedInt frame;
	Int nextId;
};
BeaconWorld g_bw;
class Object
{
public:
	Int getIndicatorColor() const { return g_bw.color; }
};
Object g_object;
class Drawable
{
public:
	Coord3D m_pos;
	Object *getObject() { return g_bw.hasObject ? &g_object : NULL; }
	void setDrawableHidden( Bool hidden ) { if (!hidden) abort(); rec(5); g_bw.hidden = hidden; }
	void setShadowsEnabled( Bool enable ) { rec(6, enable ? 1 : 0); }
	Bool isDrawableEffectivelyHidden() const { return g_bw.hidden || g_bw.hiddenByStealth; }
	const Coord3D *getPosition() const { return &m_pos; }
};
Drawable g_drawable;
class ClientUpdateModule
{
public:
	const ModuleData *m_data;
	ClientUpdateModule( Thing *, const ModuleData *data ) : m_data(data) {}
	virtual ~ClientUpdateModule() {}
	virtual void clientUpdate( void ) = 0;
	Drawable *getDrawable() const { return &g_drawable; }
	const ModuleData *getModuleData() const { return m_data; }
};
class ParticleSystemTemplate { public: Int m_white; };
ParticleSystemTemplate g_colorTemplate = { 0 }, g_whiteTemplate = { 1 };
class ParticleSystem
{
public:
	ParticleSystemID m_id;
	ParticleSystemID getSystemID() const { return m_id; }
	void attachToDrawable( const Drawable *d ) { if (d != &g_drawable) abort(); rec(2, m_id); }
	void tintAllColors( Color c ) { rec(3, m_id, c); }
	void stop() { rec(4, m_id); }
};
ParticleSystem g_system;
class ParticleSystemManager
{
public:
	const ParticleSystemTemplate *findTemplate( const AsciiString &name ) const
	{
		const char *s = name.str();
		int w[5] = { 0, 0, 0, 0, 0 };
		int n = (int)strlen(s);
		memcpy(w, s, n < 20 ? n : 20);
		const ParticleSystemTemplate *t = NULL;
		char expect[32];
		snprintf(expect, sizeof expect, "BeaconSmoke%6.6X", 0xffffff & g_bw.color);
		if (strcmp(s, "BeaconSmokeFFFFFF") == 0 && g_bw.whiteTemplate) t = &g_whiteTemplate;
		else if (strcmp(s, expect) == 0 && g_bw.colorTemplate) t = &g_colorTemplate;
		rec(1, n, w[0], w[1], w[2], w[3], w[4], t ? 1 : 0);
		return t;
	}
	ParticleSystem *createParticleSystem( const ParticleSystemTemplate *t, Bool createSlaves = TRUE )
	{
		if (!createSlaves || t == NULL) abort();
		if (g_bw.createFails) return NULL;
		g_system.m_id = (ParticleSystemID)(++g_bw.nextId);
		g_bw.lost = FALSE;
		return &g_system;
	}
	ParticleSystem *findParticleSystem( ParticleSystemID id )
	{
		if (id == INVALID_PARTICLE_SYSTEM_ID || g_bw.lost || id != g_system.m_id) return NULL;
		return &g_system;
	}
};
ParticleSystemManager g_psm;
ParticleSystemManager *TheParticleSystemManager = &g_psm;
class Radar
{
public:
	void createEvent( const Coord3D *pos, RadarEventType type, Real secondsToLive = 4.0f )
	{
		rec(7, bits(pos->x), bits(pos->y), bits(pos->z), (int)type, bits(secondsToLive));
	}
};
Radar g_radar;
Radar *TheRadar = &g_radar;
class GameLogic { public: UnsignedInt getFrame() { return g_bw.frame; } };
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;
class Xfer;
#include "rd_beacon_h.inc"
#include "rd_beacon_data.inc"
#include "rd_beacon.inc"

} /* namespace rd */

extern "C" {

void ref_radar_hero_icon(const float *f, const int *i, int *out)
{
	using namespace rd;
	W3DRadar r;
	r.m_mapExtent.lo.x = 0.0f; r.m_mapExtent.lo.y = 0.0f; r.m_mapExtent.lo.z = 0.0f;
	r.m_mapExtent.hi.x = f[0]; r.m_mapExtent.hi.y = f[1]; r.m_mapExtent.hi.z = 0.0f;
	Coord3D pos;
	pos.x = f[2]; pos.y = f[3]; pos.z = 0.0f;
	g_heroReticle.m_w = i[4];
	g_heroReticle.m_h = i[5];
	g_drawn = 0;
	r.drawHeroIcon(i[0], i[1], i[2], i[3], &pos);
	out[0] = g_drawn;
	for (int k = 0; k < 4; ++k) out[1 + k] = g_drawn ? g_drawRect[k] : 0;
}

int ref_radar_glow_seq(const int *in, int nin, int *out, int cap)
{
	using namespace rd;
	g_out.clear();
	GameWindow win, kids[3];
	memset(&win, 0, sizeof win);
	memset(kids, 0, sizeof kids);
	int nk = in[2];
	win.m_status = in[1] ? WIN_STATUS_ENABLED : 0;
	for (int k = 0; k < nk; ++k) {
		kids[k].m_status = (in[3] >> k) & 1 ? WIN_STATUS_ENABLED : 0;
		if (k > 0) kids[k - 1].m_next = &kids[k];
	}
	if (nk > 0) win.m_child = &kids[0];
	ControlBar bar;
	if (in[0]) bar.m_radarAttackGlowWindow = &win;
	for (int s = 4; s + 1 < nin; s += 2) {
		switch (in[s]) {
		case 0: bar.triggerRadarAttackGlow(); break;
		case 1: bar.updateRadarAttackGlow(); break;
		case 2: bar.resetGlow(); break;
		default: win.winEnable(in[s + 1] != 0); break;
		}
		int kb = 0;
		for (int k = 0; k < nk; ++k) kb |= (BitTest(kids[k].m_status, WIN_STATUS_ENABLED) ? 1 : 0) << k;
		rec(100, bar.m_radarAttackGlowOn ? 1 : 0, bar.m_remainingRadarAttackGlowFrames, BitTest(win.m_status, WIN_STATUS_ENABLED) ? 1 : 0, kb);
	}
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

int ref_beacon_seq(const int *in, int nin, int *out, int cap)
{
	using namespace rd;
	g_out.clear();
	BeaconClientUpdateModuleData defaults;
	rec(200, (int)defaults.m_framesBetweenRadarPulses, (int)defaults.m_radarPulseDuration);
	BeaconClientUpdateModuleData data;
	data.m_framesBetweenRadarPulses = (UnsignedInt)in[0];
	data.m_radarPulseDuration = (UnsignedInt)in[1];
	memset(&g_bw, 0, sizeof g_bw);
	g_bw.hasObject = in[2] != 0;
	g_bw.color = in[3];
	g_bw.colorTemplate = in[4] != 0;
	g_bw.whiteTemplate = in[5] != 0;
	g_bw.createFails = in[6] != 0;
	g_bw.frame = (UnsignedInt)in[7];
	g_system.m_id = INVALID_PARTICLE_SYSTEM_ID;
	g_drawable.m_pos.x = 12.5f; g_drawable.m_pos.y = -3.25f; g_drawable.m_pos.z = 7.0f;
	Thing thing;
	BeaconClientUpdate *b = new BeaconClientUpdate(&thing, &data);
	struct Peek : public BeaconClientUpdate {
		static ParticleSystemID id(BeaconClientUpdate *b) { return ((Peek *)b)->m_particleSystemID; }
		static UnsignedInt last(BeaconClientUpdate *b) { return ((Peek *)b)->m_lastRadarPulse; }
	};
	for (int s = 8; s + 1 < nin; s += 2) {
		int arg = in[s + 1];
		switch (in[s]) {
		case 0: b->clientUpdate(); break;
		case 1: b->hideBeacon(); break;
		case 2: g_bw.frame = (UnsignedInt)arg; break;
		case 3: g_bw.hiddenByStealth = arg != 0; break;
		case 4: g_bw.lost = TRUE; break;
		case 5: g_bw.hasObject = arg != 0; break;
		default: g_bw.color = arg; break;
		}
		rec(100, (int)Peek::id(b), (int)Peek::last(b));
	}
	delete b;
	int n = (int)g_out.size();
	if (n > cap) n = cap;
	memcpy(out, &g_out[0], n * sizeof(int));
	return (int)g_out.size();
}

} /* extern "C" */
