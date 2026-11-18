/* Differential-test wrapper for the command bar's drawing in the scheme's colours (GitHub #194, upstream commit
** 0a05454d), cut verbatim by build.rs into OUT_DIR/command_bar_draw_gen:
**   cbd_grid.inc         GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp,
**                        W3DCommandBarGridDraw (to W3DCommandBarGenExpDraw);
**   cbd_rect_clock.inc   GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp, W3DDisplay::drawRectClock (to the
**                        banner of drawRemainingRectClock);
**   cbd_remaining.inc    the same file, W3DDisplay::drawRemainingRectClock (to the banner of drawImage);
**   cbd_border.inc       GameEngine/Source/GameClient/GUI/GameWindow.cpp, GameWindow::winSetEnabledBorderColor (to the
**                        banner of winSetDisabledImage);
**   cbd_disabled.inc     GameEngine/Include/GameClient/GadgetPushButton.h, the GadgetButtonSetDisabledImage line;
**   cbd_bar_images.inc   GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp, ControlBar::updateBuildQueueDisabledImages
**                        and updateRightHUDImage (to updateBuildUpClockColor);
**   cbd_init_images.inc  GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp, init's two lines
**                        updateBuildQueueDisabledImages(m_buttonQueueImage) and updateRightHUDImage(m_rightHUDImage).
** GitHub #195 added:
**   cbd_cameo.inc        W3DControlBar.cpp, W3DCameoMovieDraw (to W3DLeftHUDDraw);
**   cbd_power.inc        W3DControlBar.cpp, W3DRightHUDDraw, logN and W3DPowerDraw (to W3DPowerDrawA, which no lexicon
**                        names);
**   cbd_gen_exp_top.inc  W3DControlBar.cpp, W3DCommandBarGenExpDraw and W3DCommandBarTopDraw (to
**                        W3DCommandBarBackgroundDraw);
**   cbd_help_popup.inc   W3DControlBar.cpp, W3DCommandBarHelpPopupDraw (to the end of the file);
**   cbd_win_draw_image.inc  GameEngine/Source/GameClient/GUI/GameWindowGlobal.cpp, GameWindowManager::winDrawImage (to
**                        the banner of winFillRect).
** No edit. The init lines go into a stand-in scheme's init(), inside `if(TheControlBar)` as upstream has them.
** WWMath's Vector2 and RectClass are the real headers.
** The stand-ins: GameWindow (screen position, size, status, the instance data's draw data: enabled 0..8 images and
** border colours, disabled image 0; winSetEnabledImage, winSetDisabledImage); W3DGameWinDefaultDraw (recorded with the
** window's IMAGE bit and enabled border colour 0 when called); TheDisplay->drawLine (recorded); W3DDisplay with a
** recording m_2DRender (Reset, Enable_Texturing, Add_Rect, Add_Tri, Render); TheControlBar (getBorderColor, m_queueData,
** m_contextParent, m_rightHUDWindow); TheWindowManager (winGetWindowFromId under CP_BUILD_QUEUE: the queue buttons,
** each present or not); TheNameKeyGenerator (a name's key is its index among the names asked for); Image (an index). AsciiString is the shim's.
** #195's stand-ins: Image's size (getImageWidth, getImageHeight); TheMappedImageCollection (findImageByName: the ten
** names the cuts ask for, each one fixed Image, indices 100.., whose size each case sets; upstream keeps them in
** function-level statics, so an image cannot be missing in one case and present in the next: every case has all ten);
** TheDisplay's drawImage (recorded with the clip state), setClipRegion and enableClipping (W3DDisplay's: the region,
** and the enabled flag), drawVideoBuffer (recorded); TheWindowManager's winGetWindowFromId(NULL, ButtonGeneral);
** GameWindow's winIsHidden; Player (isPlayerActive, the skill points, getEnergy), Energy (getProduction,
** getConsumption), ThePlayerList (getLocalPlayer), TheControlBar's observer bar (isObserverControlBarOn,
** getObserverLookAtPlayer), TheGlobalData (the three power bar values, or NULL), TheInGameUI (cameoVideoBuffer: one
** buffer, or NULL). <math.h> is the shim's: C's double log10, bound to petir's in the test binary.
**
** Entry point (extern "C", tests/differential_command_bar_draw.rs):
**   ref_command_bar_draw_case(in, nin, out, cap). `in`: [kind (0 W3DCommandBarGridDraw, 1 drawRectClock, 2
**   drawRemainingRectClock, 3 init's image lines), then
**     kind 0: parent x, y, window x, y, width, height, status, state, enabled border 0, enabled colour 0, the bar's border
**             colour;
**     kind 1, 2: x, y, width, height, percent, color;
**     kind 3: the queue image (-1 NULL), the right HUD image (-1 NULL), the right HUD window present, a mask of the nine
**             queue buttons present, then each queue button's enabled 5, enabled 6 and disabled 0 images (-1 NULL),
**             then the right HUD's enabled image 0].
**   Out: kind 0: [1, IMAGE bit, enabled border colour 0] at W3DGameWinDefaultDraw; [2, x0, y0, x1, y1, width bits, color]
**   per drawLine; then [3, enabled border colour 0]. Kind 1, 2: [10] Reset; [11, texturing]; [12, left, top, right,
**   bottom, border width (bits), border colour, fill colour] per Add_Rect; [13, six vertex and six uv floats (bits),
**   colour] per Add_Tri; [14] Render. Kind 3: [20, button, enabled 5, enabled 6, disabled 0] per queue button present;
**   [21, right HUD enabled 0] when the window is present.
** #195's kinds (`in`, after the kind: geometry g = parent x, y, window x, y, width, height):
**     kind 4 W3DPowerDraw: g, observer bar on, look-at player present, local player present, local production,
**             consumption, look-at production, consumption, TheGlobalData present, base, intervals (float bits),
**             yellow range, then width, height of PowerPointY, PowerPointR, PowerPointG, PowerBarSlider;
**     kind 5 W3DCommandBarGenExpDraw: g, active, skill points, level up, level down, then width, height of
**             GenExpBarTop1, GenExpBarBottom1, GenExpBar1;
**     kind 6 W3DCommandBarHelpPopupDraw: g, then width, height of Helpbox-top, Helpbox-bottom, Helpbox-middle;
**     kind 7 W3DRightHUDDraw: g, status, state, enabled border colour 0, enabled colour 0;
**     kind 8 W3DCameoMovieDraw: g, a cameo video buffer present;
**     kind 9 W3DCommandBarTopDraw: g, ButtonGeneral present, hidden, the local player active;
**     kind 10 logN: value bits, base bits.
**   Out: [30, image (100.. or -1), x0, y0, x1, y1, colour, clip on, lo.x, lo.y, hi.x, hi.y (0 when off)] per drawImage;
**   [31, x0, y0, x1, y1] per drawVideoBuffer; [1, ...] as kind 0's at W3DGameWinDefaultDraw; then for kinds 4 to 9
**   [32, clip on at the end]; kind 10 [33, logN's bits]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <vector>
#include <string>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"
#include "vector2.h"
#include "rect.h"
#include <math.h>
/* BaseType.h:240 */
#define INT_TO_REAL(x)						((Real) (x))

namespace cbd {

std::vector<int> g_out;
static void rec(const Int *w, Int n) { g_out.insert(g_out.end(), w, w + n); }
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }

typedef Int Color;
typedef unsigned int uint32;
/* a name's key is its index among the names asked for (upstream's static ids outlive the AsciiString) */
typedef Int NameKeyType;
std::vector<std::string> g_names;
struct NameKeyGeneratorStub
{
	NameKeyType nameToKey( const char *n )
	{
		for (size_t i = 0; i < g_names.size(); ++i)
			if (g_names[i] == n)
				return (Int)i;
		g_names.push_back(n);
		return (Int)g_names.size() - 1;
	}
	NameKeyType nameToKey( const AsciiString &n ) { return nameToKey(n.str()); }
};
NameKeyGeneratorStub g_nkg;
NameKeyGeneratorStub *TheNameKeyGenerator = &g_nkg;

/* GameWindow.h's values */
enum { WIN_ERR_OK = 0, WIN_ERR_INVALID_PARAMETER = -3 };
enum { WIN_STATUS_IMAGE = 0x00000080 };
enum { MAX_DRAW_DATA = 9 };

class Image
{
public:
	Int m_index;
	Int m_w, m_h;
	Int getImageWidth( void ) const { return m_w; }
	Int getImageHeight( void ) const { return m_h; }
};
Image g_images[8] = { {0}, {1}, {2}, {3}, {4}, {5}, {6}, {7} };
static const Image *imageOf(Int i) { return (i >= 0 && i < 8) ? &g_images[i] : NULL; }
static Int indexOf(const Image *i) { return i ? i->m_index : -1; }

/* #195: the mapped images the draw callbacks name, each one fixed Image (index 100 + its place here) */
enum { NUM_NAMED = 10 };
const char *g_namedNames[ NUM_NAMED ] = { "PowerPointY", "PowerPointR", "PowerPointG", "PowerBarSlider",
	"GenExpBarTop1", "GenExpBarBottom1", "GenExpBar1", "Helpbox-top", "Helpbox-bottom", "Helpbox-middle" };
Image g_named[ NUM_NAMED ];
struct MappedImageCollectionStub
{
	const Image *findImageByName( const char *name )
	{
		for (Int i = 0; i < NUM_NAMED; ++i)
			if (strcmp(g_namedNames[i], name) == 0)
				return &g_named[i];
		return NULL;
	}
	const Image *findImageByName( const AsciiString &name ) { return findImageByName(name.str()); }
};
MappedImageCollectionStub g_mic;
MappedImageCollectionStub *TheMappedImageCollection = &g_mic;

struct WinDrawData { const Image *image; Color color; Color borderColor; };
struct WinInstanceData
{
	WinDrawData m_enabledDrawData[ MAX_DRAW_DATA ];
	WinDrawData m_disabledDrawData[ MAX_DRAW_DATA ];
	UnsignedInt m_state;
};

class GameWindow
{
public:
	Int m_x, m_y, m_w, m_h;
	UnsignedInt m_status;
	WinInstanceData m_instData;
	Bool m_hidden;
	Bool winIsHidden( void ) { return m_hidden; }
	UnsignedInt winGetStatus( void ) { return m_status; }
	Int winGetScreenPosition( Int *x, Int *y ) { *x = m_x; *y = m_y; return 0; }
	Int winGetSize( Int *width, Int *height ) { *width = m_w; *height = m_h; return 0; }
	Int winSetEnabledBorderColor( Int index, Color color );
	Int winSetEnabledImage( Int index, const Image *image ) { m_instData.m_enabledDrawData[ index ].image = image; return WIN_ERR_OK; }
	Int winSetDisabledImage( Int index, const Image *image ) { m_instData.m_disabledDrawData[ index ].image = image; return WIN_ERR_OK; }
};
#include "cbd_border.inc"
#include "cbd_disabled.inc"

static void W3DGameWinDefaultDraw( GameWindow *window, WinInstanceData * )
{
	Int w[3] = { 1, (window->m_status & WIN_STATUS_IMAGE) ? 1 : 0, window->m_instData.m_enabledDrawData[0].borderColor };
	rec(w, 3);
}

class VideoBuffer { public: Int m_unused; };

/* Display.h's drawLine( Int startX, Int startY, Int endX, Int endY, Real lineWidth, UnsignedInt lineColor ); #195: its
** drawImage, drawVideoBuffer, setClipRegion and enableClipping (W3DDisplay's state: the region and the flag) */
struct DisplayStub
{
	IRegion2D m_clipRegion;
	Bool m_isClippedEnabled;
	void setClipRegion( IRegion2D *region ) { m_clipRegion = *region; m_isClippedEnabled = TRUE; }
	void enableClipping( Bool onoff ) { m_isClippedEnabled = onoff; }
	void drawImage( const Image *image, Int startX, Int startY, Int endX, Int endY, Color color = 0xFFFFFFFF )
	{
		Bool on = m_isClippedEnabled;
		Int w[12] = { 30, image ? image->m_index : -1, startX, startY, endX, endY, color, on ? 1 : 0,
			on ? m_clipRegion.lo.x : 0, on ? m_clipRegion.lo.y : 0, on ? m_clipRegion.hi.x : 0, on ? m_clipRegion.hi.y : 0 };
		rec(w, 12);
	}
	void drawVideoBuffer( VideoBuffer *buffer, Int startX, Int startY, Int endX, Int endY )
	{
		Int w[5] = { 31, startX, startY, endX, endY };
		rec(w, 5);
	}
	void drawLine( Int startX, Int startY, Int endX, Int endY, Real lineWidth, UnsignedInt lineColor )
	{
		Int w[7] = { 2, startX, startY, endX, endY, bitsOf(lineWidth), (Int)lineColor };
		rec(w, 7);
	}
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;

/* Render2DClass's calls (render2d.h's signatures) */
struct Render2DStub
{
	void Reset( void ) { Int w[1] = { 10 }; rec(w, 1); }
	void Enable_Texturing( bool b ) { Int w[2] = { 11, b ? 1 : 0 }; rec(w, 2); }
	void Add_Rect( const RectClass & rect, float border_width = 1.0F, uint32 border_color = 0xFF000000, uint32 fill_color = 0xFFFFFFFF )
	{
		Int w[8] = { 12, bitsOf(rect.Left), bitsOf(rect.Top), bitsOf(rect.Right), bitsOf(rect.Bottom), bitsOf(border_width), (Int)border_color, (Int)fill_color };
		rec(w, 8);
	}
	void Add_Tri( const Vector2 & v0, const Vector2 & v1, const Vector2 & v2, const Vector2 & uv0, const Vector2 & uv1, const Vector2 & uv2, unsigned long color = 0xFFFFFFFF )
	{
		Int w[14] = { 13, bitsOf(v0.X), bitsOf(v0.Y), bitsOf(v1.X), bitsOf(v1.Y), bitsOf(v2.X), bitsOf(v2.Y),
			bitsOf(uv0.X), bitsOf(uv0.Y), bitsOf(uv1.X), bitsOf(uv1.Y), bitsOf(uv2.X), bitsOf(uv2.Y), (Int)color };
		rec(w, 14);
	}
	void Render( void ) { Int w[1] = { 14 }; rec(w, 1); }
};

class W3DDisplay
{
public:
	Render2DStub *m_2DRender;
	void drawRectClock(Int startX, Int startY, Int width, Int height, Int percent, UnsignedInt color);
	void drawRemainingRectClock(Int startX, Int startY, Int width, Int height, Int percent, UnsignedInt color);
};
#include "cbd_rect_clock.inc"
#include "cbd_remaining.inc"

/* the control bar: the border colour, the build queue and the right HUD */
enum { MAX_BUILD_QUEUE_BUTTONS = 9 };
enum ControlBarContextParent { CP_MASTER, CP_BUILD_QUEUE, NUM_CONTEXT_PARENTS };
struct QueueEntry { GameWindow *control; };
GameWindow g_queue[ MAX_BUILD_QUEUE_BUTTONS ];
GameWindow g_queueParent;
GameWindow g_rightHUD;
GameWindow g_buttonGeneral;
/* (the class is GameWindowManager so that winDrawImage's definition is cut verbatim) */
class GameWindowManager
{
public:
	Bool m_has[ MAX_BUILD_QUEUE_BUTTONS ];
	Bool m_hasGeneral;
	void winDrawImage( const Image *image, Int startX, Int startY, Int endX, Int endY, Color color = 0xFFFFFFFF );
	GameWindow *winGetWindowFromId( GameWindow *parent, NameKeyType name )
	{
		if (parent == NULL && g_names[name] == "ControlBar.wnd:ButtonGeneral")
			return m_hasGeneral ? &g_buttonGeneral : NULL;
		if (parent != &g_queueParent)
			return NULL;
		for (Int i = 0; i < MAX_BUILD_QUEUE_BUTTONS; ++i) {
			char n[64];
			sprintf(n, "ControlBar.wnd:ButtonQueue%02d", i + 1);
			if (g_names[name] == n)
				return m_has[i] ? &g_queue[i] : NULL;
		}
		return NULL;
	}
};
GameWindowManager g_wm;
GameWindowManager *TheWindowManager = &g_wm;
#include "cbd_win_draw_image.inc"

/* #195: the players, their energy, the global data and the in-game UI */
class Energy
{
public:
	Int m_production, m_consumption;
	Int getProduction() const { return m_production; }
	Int getConsumption() const { return m_consumption; }
};
class Player
{
public:
	Bool m_active;
	Int m_skillPoints, m_levelUp, m_levelDown;
	Energy m_energy;
	Bool isPlayerActive(void) const { return m_active; }
	Int getSkillPoints() const { return m_skillPoints; }
	Int getSkillPointsLevelUp() const { return m_levelUp; }
	Int getSkillPointsLevelDown() const { return m_levelDown; }
	inline Energy *getEnergy() { return &m_energy; }
};
Player g_localPlayer, g_lookAtPlayer;
struct PlayerListStub
{
	Player *m_local;
	Player *getLocalPlayer( void ) { return m_local; }
};
PlayerListStub g_playerList;
PlayerListStub *ThePlayerList = &g_playerList;
struct GlobalDataStub
{
	Int m_powerBarBase;
	Real m_powerBarIntervals;
	Int m_powerBarYellowRange;
};
GlobalDataStub g_globalData;
GlobalDataStub *TheGlobalData = NULL;
VideoBuffer g_video;
struct InGameUIStub
{
	VideoBuffer *m_cameoVideoBuffer;
	VideoBuffer* cameoVideoBuffer( void ) { return m_cameoVideoBuffer; }
};
InGameUIStub g_inGameUI;
InGameUIStub *TheInGameUI = &g_inGameUI;

class ControlBar
{
public:
	Color m_commandBarBorderColor;
	QueueEntry m_queueData[ MAX_BUILD_QUEUE_BUTTONS ];
	GameWindow *m_contextParent[ NUM_CONTEXT_PARENTS ];
	GameWindow *m_rightHUDWindow;
	Bool m_isObserverCommandBar;
	Player *m_observerLookAtPlayer;
	Player *getObserverLookAtPlayer (void ) { return m_observerLookAtPlayer;}
	Bool isObserverControlBarOn( void ) { return m_isObserverCommandBar;}
	Color getBorderColor( void ) { return m_commandBarBorderColor; }
	void updateBuildQueueDisabledImages( const Image *image );
	void updateRightHUDImage( const Image *image );
};
ControlBar *TheControlBar = NULL;
#include "cbd_bar_images.inc"

#include "cbd_grid.inc"
#include "cbd_cameo.inc"
#include "cbd_power.inc"
#include "cbd_gen_exp_top.inc"
#include "cbd_help_popup.inc"

class ControlBarScheme
{
public:
	const Image *m_buttonQueueImage;
	const Image *m_rightHUDImage;
	void init( void )
	{
		if(TheControlBar)
		{
#include "cbd_init_images.inc"
		}
	}
};

static void setWindow( GameWindow &w )
{
	memset(&w, 0, sizeof w);
}

static Real realOf( Int bits ) { Real r; memcpy(&r, &bits, sizeof r); return r; }

/* #195: a window at parent (in[1], in[2]) + (in[3], in[4]), size (in[5], in[6]) */
static void placeWindow( GameWindow &w, const int *in )
{
	setWindow(w);
	w.m_x = in[1] + in[3];
	w.m_y = in[2] + in[4];
	w.m_w = in[5];
	w.m_h = in[6];
}

static void setNamed( Int first, const int *sizes, Int n )
{
	for (Int i = 0; i < NUM_NAMED; ++i) {
		g_named[i].m_index = 100 + i;
		g_named[i].m_w = 0;
		g_named[i].m_h = 0;
	}
	for (Int i = 0; i < n; ++i) {
		g_named[first + i].m_w = sizes[i * 2];
		g_named[first + i].m_h = sizes[i * 2 + 1];
	}
}

} // namespace cbd

using namespace cbd;

extern "C" int ref_command_bar_draw_case(const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	Int kind = in[0];
	if (kind == 0) {
		GameWindow win;
		setWindow(win);
		win.m_x = in[1] + in[3];
		win.m_y = in[2] + in[4];
		win.m_w = in[5];
		win.m_h = in[6];
		win.m_status = (UnsignedInt)in[7];
		win.m_instData.m_state = (UnsignedInt)in[8];
		win.m_instData.m_enabledDrawData[0].borderColor = in[9];
		win.m_instData.m_enabledDrawData[0].color = in[10];
		ControlBar bar;
		bar.m_commandBarBorderColor = in[11];
		TheControlBar = &bar;
		W3DCommandBarGridDraw(&win, &win.m_instData);
		TheControlBar = NULL;
		Int w[2] = { 3, win.m_instData.m_enabledDrawData[0].borderColor };
		rec(w, 2);
	} else if (kind >= 4 && kind <= 9) {
		GameWindow win;
		placeWindow(win, in);
		g_display.m_isClippedEnabled = FALSE;
		memset(&g_display.m_clipRegion, 0, sizeof g_display.m_clipRegion);
		memset(&g_localPlayer, 0, sizeof g_localPlayer);
		memset(&g_lookAtPlayer, 0, sizeof g_lookAtPlayer);
		g_playerList.m_local = &g_localPlayer;
		ControlBar bar;
		memset(&bar, 0, sizeof bar);
		TheControlBar = &bar;
		if (kind == 4) {
			bar.m_isObserverCommandBar = in[7] != 0;
			bar.m_observerLookAtPlayer = in[8] ? &g_lookAtPlayer : NULL;
			g_playerList.m_local = in[9] ? &g_localPlayer : NULL;
			g_localPlayer.m_energy.m_production = in[10];
			g_localPlayer.m_energy.m_consumption = in[11];
			g_lookAtPlayer.m_energy.m_production = in[12];
			g_lookAtPlayer.m_energy.m_consumption = in[13];
			g_globalData.m_powerBarBase = in[15];
			g_globalData.m_powerBarIntervals = realOf(in[16]);
			g_globalData.m_powerBarYellowRange = in[17];
			TheGlobalData = in[14] ? &g_globalData : NULL;
			setNamed(0, &in[18], 4);
			W3DPowerDraw(&win, &win.m_instData);
			TheGlobalData = NULL;
		} else if (kind == 5) {
			g_localPlayer.m_active = in[7] != 0;
			g_localPlayer.m_skillPoints = in[8];
			g_localPlayer.m_levelUp = in[9];
			g_localPlayer.m_levelDown = in[10];
			setNamed(4, &in[11], 3);
			W3DCommandBarGenExpDraw(&win, &win.m_instData);
		} else if (kind == 6) {
			setNamed(7, &in[7], 3);
			W3DCommandBarHelpPopupDraw(&win, &win.m_instData);
		} else if (kind == 7) {
			win.m_status = (UnsignedInt)in[7];
			win.m_instData.m_state = (UnsignedInt)in[8];
			win.m_instData.m_enabledDrawData[0].borderColor = in[9];
			win.m_instData.m_enabledDrawData[0].color = in[10];
			W3DRightHUDDraw(&win, &win.m_instData);
		} else if (kind == 8) {
			g_inGameUI.m_cameoVideoBuffer = in[7] ? &g_video : NULL;
			W3DCameoMovieDraw(&win, &win.m_instData);
		} else {
			g_wm.m_hasGeneral = in[7] != 0;
			setWindow(g_buttonGeneral);
			g_buttonGeneral.m_hidden = in[8] != 0;
			g_localPlayer.m_active = in[9] != 0;
			W3DCommandBarTopDraw(&win, &win.m_instData);
		}
		TheControlBar = NULL;
		Int w[2] = { 32, g_display.m_isClippedEnabled ? 1 : 0 };
		rec(w, 2);
	} else if (kind == 10) {
		Int w[2] = { 33, bitsOf(logN(realOf(in[1]), realOf(in[2]))) };
		rec(w, 2);
	} else if (kind == 1 || kind == 2) {
		Render2DStub r;
		W3DDisplay d;
		d.m_2DRender = &r;
		if (kind == 1)
			d.drawRectClock(in[1], in[2], in[3], in[4], in[5], (UnsignedInt)in[6]);
		else
			d.drawRemainingRectClock(in[1], in[2], in[3], in[4], in[5], (UnsignedInt)in[6]);
	} else {
		ControlBar bar;
		memset(&bar, 0, sizeof bar);
		bar.m_contextParent[CP_BUILD_QUEUE] = &g_queueParent;
		bar.m_rightHUDWindow = in[3] ? &g_rightHUD : NULL;
		setWindow(g_rightHUD);
		for (Int i = 0; i < MAX_BUILD_QUEUE_BUTTONS; ++i) {
			setWindow(g_queue[i]);
			g_wm.m_has[i] = (in[4] >> i) & 1;
			g_queue[i].m_instData.m_enabledDrawData[5].image = imageOf(in[5 + i * 3]);
			g_queue[i].m_instData.m_enabledDrawData[6].image = imageOf(in[6 + i * 3]);
			g_queue[i].m_instData.m_disabledDrawData[0].image = imageOf(in[7 + i * 3]);
		}
		g_rightHUD.m_instData.m_enabledDrawData[0].image = imageOf(in[5 + MAX_BUILD_QUEUE_BUTTONS * 3]);
		ControlBarScheme s;
		s.m_buttonQueueImage = imageOf(in[1]);
		s.m_rightHUDImage = imageOf(in[2]);
		TheControlBar = &bar;
		s.init();
		TheControlBar = NULL;
		for (Int i = 0; i < MAX_BUILD_QUEUE_BUTTONS; ++i) {
			if (!g_wm.m_has[i])
				continue;
			WinInstanceData &d = g_queue[i].m_instData;
			Int w[5] = { 20, i, indexOf(d.m_enabledDrawData[5].image), indexOf(d.m_enabledDrawData[6].image), indexOf(d.m_disabledDrawData[0].image) };
			rec(w, 5);
		}
		if (in[3]) {
			Int w[2] = { 21, indexOf(g_rightHUD.m_instData.m_enabledDrawData[0].image) };
			rec(w, 2);
		}
	}
	(void)nin;
	int nout = (int)g_out.size();
	if (nout > cap)
		nout = cap;
	if (nout > 0)
		memcpy(out, &g_out[0], nout * sizeof(int));
	return nout;
}
