/* Differential-test wrapper for TheMouse's cursor (GitHub #117, upstream commit 0a05454d), cut verbatim by build.rs
** into OUT_DIR/mouse_gen:
**   mouse_cursor_info_class.inc  GameEngine/Include/GameClient/Mouse.h, `class CursorInfo` (to the `// Mouse ---` line);
**   mouse_enums.inc              Mouse.h, from `enum MouseCursor` to `static const char *RedrawModeName[RM_MAX];` (the
**                                cursors and the redraw modes, as the release build defines them: no ALLOW_DEMORALIZE,
**                                no ALLOW_SURRENDER);
**   mouse_cursor_info.inc        GameEngine/Source/GameClient/Input/Mouse.cpp, `CursorInfo::CursorInfo` (to
**                                `Mouse::Mouse( void )`);
**   mouse_set_mouse_text.inc     Mouse.cpp, `Mouse::setMouseText` (to `void Mouse::setPosition`);
**   mouse_cursor.inc             Mouse.cpp, `Mouse::getCursorIndex` and `Mouse::setCursor` (to
**                                `void INI::parseMouseCursorDefinition`);
**   win32_mouse.inc              GameEngineDevice/Source/Win32Device/GameClient/Win32Mouse.cpp, `setVisibility`,
**                                `initCursorResources`, `setCursor` (to `void Win32Mouse::capture`);
**   w3d_mouse.inc                GameEngineDevice/Source/W3DDevice/GameClient/W3DMouse.cpp, `setCursor` and `draw` (to
**                                `void W3DMouse::setRedrawMode`), every branch;
**   w3d_mouse_direction.inc      W3DMouse.cpp, `setCursorDirection` (to the end);
**   in_game_ui_mouse.inc         GameEngine/Source/GameClient/InGameUI.cpp, `InGameUI::setMouseCursor` (to `findSWInfo`);
**   in_game_ui_scrolling.inc     InGameUI.cpp, `InGameUI::setScrolling` (to `InGameUI::isScrolling`).
** No edit. They are compiled inside namespace mz on stand-in class declarations that hold the members the cut
** functions use, with the constructors' and `Mouse::init`'s cursor members written out by hand below (`setup`), and
** on stand-ins for the device: `LoadCursorFromFile` loads every path but those containing "missing" or ending in
** "3.ANI"; `SetCursor` records its handle; the D3D device is NULL, the W3D scene and models are stubs the RM_WINDOWS
** branches never reach (the mode is RM_WINDOWS, as `m_winCursors` makes it); `TheGameText->fetch(label)` is the label.
**
** Entry point (extern "C", tests/differential_mouse.rs):
**   ref_mouse_seq(in, nin, out, cap) runs one sequence on a new mouse and writes its words to out; returns the count.
**   `in` is the setup, 40 cursors of 8 words [text 0 none | 1 "CURSOR:A" | 2 "CURSOR:B", text colour, drop colour
**   (r | g << 8 | b << 16 | a << 24), texture 0 none | 1 "SCC<c>" | 2 "missing<c>", Directions, 0, 0, 0], then steps of
**   8 words [kind, a, b, c, d, e, f, g]:
**     kind 0  TheMouse->setCursor(a)            kind 1  TheMouse->setVisibility(a)
**     kind 2  TheMouse->draw()                  kind 3  TheWin32Mouse->lostFocus(a)
**     kind 4  InGameUI m_isScrolling = a, m_scrollAmt = (bits b, bits c)
**     kind 5  WM_SETCURSOR: TheWin32Mouse->setCursor(TheWin32Mouse->getMouseCursor())
**     kind 6  InGameUI m_mouseMode = b; TheInGameUI->setMouseCursor(a)
**     kind 7  InGameUI m_mouseMode = b; TheInGameUI->setScrolling(a)
**     kind 8  getCursorIndex of the name packed in b..g (4 bytes a word, NUL-terminated): record [103, index].
**   After the setup: [104, c, the mask of c's loaded directions] for each cursor; after the setup and after each step:
**   [100, m_currentCursor, m_visible, m_lostFocus, m_directionFrame, m_currentWin32Cursor, m_drawing, SetCursor's last
**   handle (-1 never called, -2 NULL, else cursor * 8 + direction of the slot it was loaded into)],
**   [101, m_currentD3DCursor, m_currentW3DCursor, m_currentPolygonCursor, the cursor text's FNV-1a hash (over its
**   16-bit characters; 0 without the text string), text colour, drop colour, the text string exists],
**   [102, InGameUI m_isScrolling, m_mouseModeCursor, 0, 0, 0, 0, 0]. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <new>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"

namespace mz {

std::vector<int> g_out;
void rec(Int tag, Int a = 0, Int b = 0, Int c = 0, Int d = 0, Int e = 0, Int f = 0, Int g = 0)
{
	Int w[8] = { tag, a, b, c, d, e, f, g };
	g_out.insert(g_out.end(), w, w + 8);
}

/* ---- Windows and the device (never reached in RM_WINDOWS but SetCursor and LoadCursorFromFile) ---- */
typedef int BOOL;
typedef long HRESULT;
typedef void *HWND;
typedef int *HCURSOR;
struct POINT { long x, y; };
enum { D3DCURSOR_IMMEDIATE_UPDATE = 1 };
int g_handles[64 * 8];
int g_nextHandle;
HCURSOR g_lastSet;
Bool g_setCalled;
HCURSOR LoadCursorFromFile(const char *path)
{
	size_t n = strlen(path);
	if (strstr(path, "missing") || (n >= 5 && strcmp(path + n - 5, "3.ANI") == 0))
		return NULL;
	return &g_handles[g_nextHandle++];
}
HCURSOR SetCursor(HCURSOR h) { g_lastSet = h; g_setCalled = TRUE; return NULL; }
BOOL GetCursorPos(POINT *) { return 0; }
BOOL ScreenToClient(HWND, POINT *) { return 0; }
UnsignedInt timeGetTime() { return 0; }
struct D3DSurf {};
struct SurfaceClass { D3DSurf *Peek_D3D_Surface() { return NULL; } };
struct Device8 {
	void ShowCursor(BOOL) {}
	HRESULT SetCursorProperties(Int, Int, D3DSurf *) { return 0; }
	void SetCursorPosition(Int, Int, Int) {}
};
typedef Device8 *LPDIRECT3DDEVICE8;
struct DX8Wrapper { static LPDIRECT3DDEVICE8 _Get_D3D_Device8() { return NULL; } };
struct CriticalSectionClass { struct LockClass { LockClass(CriticalSectionClass &) {} }; };
struct Vector2 { Real x, y; Vector2(Real a, Real b) : x(a), y(b) {} };
struct Vector3 {
	Real x, y, z;
	Vector3() : x(0), y(0), z(0) {}
	Vector3(Real a, Real b, Real c) : x(a), y(b), z(c) {}
	Vector3 &operator-=(const Vector3 &) { return *this; }
	Vector3 &operator+=(const Vector3 &) { return *this; }
	Vector3 &operator*=(Real) { return *this; }
	void Normalize() {}
	static Real Find_X_At_Z(Real, const Vector3 &, const Vector3 &) { return 0; }
	static Real Find_Y_At_Z(Real, const Vector3 &, const Vector3 &) { return 0; }
};
struct Matrix3D { Matrix3D(Int) {} void Set_Translation(const Vector3 &) {} void Rotate_Z(Real) {} };
struct CameraClass { Vector3 Get_Position() { return Vector3(); } void Un_Project(Vector3 &, const Vector2 &) {} Real Get_Depth() { return 1; } };
struct HAnimClass {};
struct RenderObjClass {
	enum { ANIM_MODE_MANUAL = 0, ANIM_MODE_LOOP, ANIM_MODE_ONCE };
	void Set_Animation(HAnimClass *, Int, Int) {}
	void Set_Transform(const Matrix3D &) {}
};
struct TextureClass {};
struct SceneStub { void Add_Render_Object(RenderObjClass *) {} void Remove_Render_Object(RenderObjClass *) {} };
struct W3DDisplay { static SceneStub *m_3DInterfaceScene; };
SceneStub *W3DDisplay::m_3DInterfaceScene = NULL;
struct WW3D { static void Render(SceneStub *, CameraClass *) {} };
void PixelScreenToW3DLogicalScreen(Int, Int, Real *x, Real *y, Int, Int) { *x = *y = 0; }
struct Image { Int getImageWidth() const { return 0; } Int getImageHeight() const { return 0; } };
struct DisplayStub {
	Bool getWindowed() { return TRUE; }
	Int getWidth() { return 800; }
	Int getHeight() { return 600; }
	void drawImage(const Image *, Int, Int, Int, Int) {}
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;

/* ---- the game's text and display string: the label itself, kept ---- */
struct GameTextStub {
	UnicodeString fetch(const char *label)
	{
		std::vector<WideChar> w;
		for (const char *c = label; *c; ++c)
			w.push_back((WideChar)(unsigned char)*c);
		w.push_back(0);
		return UnicodeString(&w[0]);
	}
};
GameTextStub g_text;
GameTextStub *TheGameText = &g_text;
struct DisplayString { UnicodeString m_text; void setText(UnicodeString t) { m_text = t; } };
DisplayString g_cursorText;

#define MAX_2D_CURSOR_ANIM_FRAMES 21
#define MAX_2D_CURSOR_DIRECTIONS 8
struct MouseIO { ICoord2D pos; };

#include "mouse_cursor_info_class.inc"

class Mouse
{
public:
#include "mouse_enums.inc"
	CursorInfo m_cursorInfo[NUM_MOUSE_CURSORS];
	virtual ~Mouse() {}
	virtual void setCursor( MouseCursor cursor );
	virtual void draw( void ) {}
	virtual void capture( void ) {}
	virtual void releaseCapture( void ) {}
	virtual void setVisibility(Bool visible) { m_visible = visible; }
	Bool getVisibility(void) { return m_visible; }
	MouseCursor getMouseCursor(void) { return m_currentCursor; }
	void setMouseText( UnicodeString text, const RGBAColorInt *color, const RGBAColorInt *dropColor );
	Int getCursorIndex( const AsciiString& name );
	void drawTooltip( void ) {}
	void drawCursorText( void ) {}
	Bool m_orthoCamera;
	Real m_orthoZoom;
	RedrawMode m_currentRedrawMode;
	MouseIO m_currMouse;
	Bool m_visible;
	MouseCursor m_currentCursor;
	DisplayString *m_cursorTextDisplayString;
	RGBAColorInt m_cursorTextColor;
	RGBAColorInt m_cursorTextDropColor;
};

class Win32Mouse : public Mouse
{
public:
	virtual void setCursor( MouseCursor cursor );
	virtual void setVisibility(Bool visible);
	void initCursorResources(void);
	void lostFocus (Bool state) { m_lostFocus = state;}
	MouseCursor m_currentWin32Cursor;
	Int m_directionFrame;
	Bool m_lostFocus;
};

class W3DMouse : public Win32Mouse
{
public:
	virtual void setCursor( MouseCursor cursor );
	virtual void draw( void );
	MouseCursor m_currentD3DCursor;
	SurfaceClass *m_currentD3DSurface[MAX_2D_CURSOR_ANIM_FRAMES];
	ICoord2D m_currentHotSpot;
	Int m_currentFrames;
	Real m_currentAnimFrame;
	Int m_currentD3DFrame;
	Int m_lastAnimTime;
	Real m_currentFMS;
	Bool m_drawing;
	void initD3DAssets(void) {}
	void freeD3DAssets(void) {}
	Bool loadD3DCursorTextures(MouseCursor) { return TRUE; }
	Bool releaseD3DCursorTextures(MouseCursor) { return TRUE; }
	CameraClass *m_camera;
	MouseCursor m_currentW3DCursor;
	void initW3DAssets(void) {}
	void freeW3DAssets(void) {}
	MouseCursor m_currentPolygonCursor;
	void initPolygonAssets(void) {}
	void freePolygonAssets(void) {}
	void setCursorDirection(MouseCursor cursor);
};

/* the file statics of Win32Mouse.cpp and W3DMouse.cpp */
HCURSOR cursorResources[Mouse::NUM_MOUSE_CURSORS][MAX_2D_CURSOR_DIRECTIONS];
static CriticalSectionClass mutex;
static Bool isThread;
static TextureClass *cursorTextures[Mouse::NUM_MOUSE_CURSORS][MAX_2D_CURSOR_ANIM_FRAMES];
static const Image *cursorImages[Mouse::NUM_MOUSE_CURSORS];
static RenderObjClass *cursorModels[Mouse::NUM_MOUSE_CURSORS];
static HAnimClass *cursorAnims[Mouse::NUM_MOUSE_CURSORS];

Mouse *TheMouse;
Win32Mouse *TheWin32Mouse;

/* ---- InGameUI and the tactical view, for setMouseCursor and setScrolling ---- */
typedef Int ObjectID;
#define INVALID_ID 0
struct ViewStub { void setCameraLock(ObjectID) {} void setCameraLockDrawable(void *) {} };
ViewStub g_view;
ViewStub *TheTacticalView = &g_view;
class InGameUI
{
public:
	enum MouseMode { MOUSEMODE_DEFAULT = 0, MOUSEMODE_BUILD_PLACE, MOUSEMODE_GUI_COMMAND, MOUSEMODE_MAX };
	void setMouseCursor(Mouse::MouseCursor c);
	void setScrolling( Bool isScrolling );
	Bool isScrolling( void ) { return m_isScrolling; }
	Coord2D getScrollAmount( void ) { return m_scrollAmt; }
	MouseMode m_mouseMode;
	Int m_mouseModeCursor;
	Bool m_isScrolling;
	Coord2D m_scrollAmt;
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

#include "mouse_cursor_info.inc"
#include "mouse_set_mouse_text.inc"
#include "mouse_cursor.inc"
#include "win32_mouse.inc"
HWND ApplicationHWnd;
#include "w3d_mouse.inc"
#include "w3d_mouse_direction.inc"
#include "in_game_ui_mouse.inc"
#include "in_game_ui_scrolling.inc"

alignas(W3DMouse) unsigned char g_buf[sizeof(W3DMouse)];

/* the slot a handle was loaded into: cursor * 8 + direction (-2 NULL) */
Int slotOf(HCURSOR h)
{
	if (!h)
		return -2;
	for (Int c = 0; c < Mouse::NUM_MOUSE_CURSORS; ++c)
		for (Int d = 0; d < MAX_2D_CURSOR_DIRECTIONS; ++d)
			if (cursorResources[c][d] == h)
				return c * 8 + d;
	return -3;
}

Int packColor(const RGBAColorInt &c) { return (Int)(c.red | (c.green << 8) | (c.blue << 16) | (c.alpha << 24)); }
RGBAColorInt unpackColor(Int w)
{
	RGBAColorInt c;
	c.red = w & 0xFF; c.green = (w >> 8) & 0xFF; c.blue = (w >> 16) & 0xFF; c.alpha = ((UnsignedInt)w >> 24) & 0xFF;
	return c;
}

void state(W3DMouse *m)
{
	rec(100, m->m_currentCursor, m->m_visible, m->m_lostFocus, m->m_directionFrame, m->m_currentWin32Cursor, m->m_drawing, g_setCalled ? slotOf(g_lastSet) : -1);
	Int hash = 0;
	if (m->m_cursorTextDisplayString) {
		UnsignedInt h = 2166136261u;
		for (const WideChar *c = m->m_cursorTextDisplayString->m_text.str(); *c; ++c) {
			h ^= (UnsignedInt)*c;
			h *= 16777619u;
		}
		hash = (Int)h;
	}
	rec(101, m->m_currentD3DCursor, m->m_currentW3DCursor, m->m_currentPolygonCursor, hash, packColor(m->m_cursorTextColor), packColor(m->m_cursorTextDropColor), m->m_cursorTextDisplayString != NULL);
	rec(102, g_ui.m_isScrolling, g_ui.m_mouseModeCursor);
}

}  // namespace mz

using namespace mz;

extern "C" int ref_mouse_seq(const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	g_nextHandle = 0;
	g_lastSet = NULL;
	g_setCalled = FALSE;
	memset(cursorResources, 0, sizeof cursorResources);
	g_cursorText.m_text = UnicodeString();
	g_ui.m_mouseMode = InGameUI::MOUSEMODE_DEFAULT;
	g_ui.m_mouseModeCursor = Mouse::NONE;
	g_ui.m_isScrolling = FALSE;
	g_ui.m_scrollAmt.x = g_ui.m_scrollAmt.y = 0;
	/* W3DMouse::W3DMouse (with Mouse::Mouse and Win32Mouse::Win32Mouse): their cursor members */
	memset(g_buf, 0, sizeof g_buf);
	W3DMouse *m = new (g_buf) W3DMouse();
	m->m_currentCursor = Mouse::ARROW;
	m->m_currentRedrawMode = Mouse::RM_WINDOWS; /* TheGlobalData->m_winCursors */
	m->m_visible = FALSE;
	m->m_cursorTextDisplayString = NULL;
	m->m_cursorTextColor = unpackColor(-1);
	m->m_cursorTextDropColor = unpackColor(-1);
	m->m_currentWin32Cursor = Mouse::NONE;
	m->m_directionFrame = 0;
	m->m_lostFocus = FALSE;
	m->m_currentD3DCursor = m->m_currentW3DCursor = m->m_currentPolygonCursor = Mouse::NONE;
	m->m_camera = NULL;
	m->m_drawing = FALSE;
	TheMouse = m;
	TheWin32Mouse = m;
	/* parseIni's definitions */
	char name[64];
	for (Int c = 0; c < Mouse::NUM_MOUSE_CURSORS && (c + 1) * 8 <= nin; ++c) {
		const int *w = in + c * 8;
		CursorInfo &ci = m->m_cursorInfo[c];
		ci.cursorText = w[0] == 1 ? "CURSOR:A" : w[0] == 2 ? "CURSOR:B" : "";
		ci.cursorTextColor = unpackColor(w[1]);
		ci.cursorTextDropColor = unpackColor(w[2]);
		sprintf(name, w[3] == 1 ? "SCC%d" : "missing%d", (int)c);
		ci.textureName = w[3] == 0 ? "" : name;
		ci.numDirections = w[4];
	}
	m->initCursorResources();
	/* W3DMouse::init: Mouse::init's cursor members, then setCursor(ARROW) */
	m->m_currentRedrawMode = Mouse::RM_WINDOWS;
	m->m_currentCursor = Mouse::ARROW;
	m->m_cursorTextDisplayString = &g_cursorText;
	m->setCursor(Mouse::ARROW);
	for (Int c = 0; c < Mouse::NUM_MOUSE_CURSORS; ++c) {
		Int mask = 0;
		for (Int d = 0; d < MAX_2D_CURSOR_DIRECTIONS; ++d)
			if (cursorResources[c][d])
				mask |= 1 << d;
		rec(104, c, mask);
	}
	state(m);
	for (int s = Mouse::NUM_MOUSE_CURSORS * 8; s + 8 <= nin; s += 8) {
		const int *w = in + s;
		switch (w[0]) {
		case 0: TheMouse->setCursor((Mouse::MouseCursor)w[1]); break;
		case 1: TheMouse->setVisibility(w[1] != 0); break;
		case 2: TheMouse->draw(); break;
		case 3: TheWin32Mouse->lostFocus(w[1] != 0); break;
		case 4: {
			g_ui.m_isScrolling = w[1] != 0;
			memcpy(&g_ui.m_scrollAmt.x, &w[2], 4);
			memcpy(&g_ui.m_scrollAmt.y, &w[3], 4);
			break;
		}
		case 5: TheWin32Mouse->setCursor(TheWin32Mouse->getMouseCursor()); break;
		case 6: g_ui.m_mouseMode = (InGameUI::MouseMode)w[2]; TheInGameUI->setMouseCursor((Mouse::MouseCursor)w[1]); break;
		case 7: g_ui.m_mouseMode = (InGameUI::MouseMode)w[2]; TheInGameUI->setScrolling(w[1] != 0); break;
		case 8: {
			char nm[25];
			memcpy(nm, &w[2], 24);
			nm[24] = 0;
			rec(103, TheMouse->getCursorIndex(AsciiString(nm)));
			break;
		}
		}
		state(m);
	}
	m->~W3DMouse();
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
