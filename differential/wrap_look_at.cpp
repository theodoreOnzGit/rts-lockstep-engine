/* Differential-test wrapper for LookAtTranslator (GitHub #119, upstream commit 0a05454d), cut verbatim by build.rs into
** OUT_DIR/look_at_gen:
**   look_at_class.inc  GeneralsMD/Code/GameEngine/Include/GameClient/LookAtXlat.h, the class declaration (from
**                      `class LookAtTranslator` to the `extern` after it);
**   look_at_xlat.inc   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/LookAtXlat.cpp, from
**                      `LookAtTranslator *TheLookAtTranslator = NULL;` to the end of the file (the file statics, every
**                      member function), as the release build compiles it (no _DEBUG, _INTERNAL or
**                      _ALLOW_DEBUG_CHEATS_IN_RELEASE).
** One edit, in a copy: the file's `static enum { DIR_UP ... };` is an anonymous enum with a storage class, which MSVC
** accepts and g++ rejects; the copy drops the `static` (the enumerators are the same constants).
** They are compiled inside namespace lat on stand-ins for the singletons they name, with `private` read as `public` so
** the wrapper can dump the translator's members. The messages are the real GameMessage of the message stream build
** (wrap_message_stream.cpp), under its names. TheTacticalView is a recorder of calls.
**
** Entry point (extern "C", tests/differential_look_at.rs):
**   ref_look_at_seq(in, nin, out, cap)  runs one sequence on a new translator (the statics reset first) and writes its
**   words to out; returns the count. `in` is steps of 8 words [kind, a, b, c, d, e, f, g]:
**     kind 0  a message: type a, position (b, c) or key b, argument 1 e (the key's state, the wheel's count);
**     kind 1  flags a: bit 0 input enabled, 1 selecting, 2 shell active, 3 InGameUI scrolling (a script's stop when
**             cleared), 4 move the RMB anchor, 5 windowed, 6 save camera in replay, 7 single player, 8 skirmish,
**             9 a stats collector;
**     kind 2  logic frame a, client frame b;      kind 3  display width a, height b;
**     kind 4  the mouse cursor a;                  kind 5  resetModes();
**     kind 6  setCurrentPos((a, b));               kind 7  the scroll factors' bits: horizontal a, vertical b,
**                                                          keyboard c;
**     kind 8  the view's angle bits a, pitch bits b.
**   Every call that acts is a record of 8 words [tag, ...] (floats as their bits):
**     1 InGameUI setScrolling(b) | 2 setScrollAmount(x, y) | 3 message(bookmark slot, the label was GUI:BookmarkXSet)
**     | 4 setCursor(c) | 5 startScrollTime | 6 endScrollTime | 7 replay camera (pos x, y, z, angle, pitch, zoom, cursor)
**     | 8 its pixel (x, y) | 9 setMouseLock(b) | 10 setAngleAndPitchToDefault | 11 setZoomToDefault | 12 setAngle(a)
**     | 13 setPitch(a) | 14 zoomIn | 15 zoomOut | 16 scrollBy(x, y) | 17 getLocation | 18 setLocation(valid, x, y, z,
**     angle, pitch, zoom).
**   After each step two records of the translator's state: [100, disposition (-1 not a message), isScrolling,
**   isRotating, isPitching, isChangingFOV, scrollType, timestamp] and [101, anchor x, y, original anchor x, y, current
**   x, y, lastMouseMoveFrame], then [102, scrollDir 0..3, prevCursor, the RMB anchor's presence,
**   hasMouseMovedRecently()]. */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/MessageStream.h"

namespace lat {

std::vector<int> g_out;
Int g_flags;
UnsignedInt g_logicFrame, g_clientFrame, g_width, g_height;
Int g_locations;

Bool flag(Int bit) { return (g_flags >> bit) & 1; }
Int bits(Real r) { Int i; memcpy(&i, &r, 4); return i; }
Real real(Int i) { Real r; memcpy(&r, &i, 4); return r; }
void rec(Int tag, Int a = 0, Int b = 0, Int c = 0, Int d = 0, Int e = 0, Int f = 0, Int g = 0)
{
	Int w[8] = { tag, a, b, c, d, e, f, g };
	g_out.insert(g_out.end(), w, w + 8);
}

/* BaseType.h line 144: `template <typename NUM> inline NUM sqr(NUM x) { return x*x; }` (not in the shim) */
template <typename NUM> inline NUM sqr(NUM x) { return x*x; }

/* ---- KeyDefs.h's names (values as upstream: DirectInput scan codes), GameCommon.h, GameType.h ---- */
enum { KEY_UP = 0xC8, KEY_LEFT = 0xCB, KEY_RIGHT = 0xCD, KEY_DOWN = 0xD0 };
enum { KEY_STATE_UP = 0x0001 };
enum { LOGICFRAMES_PER_SECOND = 30 };
typedef UnsignedInt DrawableID;
enum { INVALID_DRAWABLE_ID = 0 };

/* ---- View.h's ViewLocation ---- */
class ViewLocation
{
public:
	Bool m_valid;
	Coord3D m_pos;
	Real m_angle, m_pitch, m_zoom;
	ViewLocation() { m_valid = FALSE; m_pos.x = m_pos.y = m_pos.z = 0; m_angle = m_pitch = m_zoom = 0.0; }
	void init(Real x, Real y, Real z, Real angle, Real pitch, Real zoom)
	{
		m_pos.x = x; m_pos.y = y; m_pos.z = z; m_angle = angle; m_pitch = pitch; m_zoom = zoom; m_valid = true;
	}
};

/* ---- the singletons ---- */
class View
{
public:
	Real m_angle, m_pitch;
	void setMouseLock(Bool b) { rec(9, b ? 1 : 0); }
	void setAngleAndPitchToDefault() { rec(10); }
	void setZoomToDefault() { rec(11); }
	Real getAngle() { return m_angle; }
	void setAngle(Real a) { m_angle = a; rec(12, bits(a)); }
	Real getPitch() { return m_pitch; }
	void setPitch(Real a) { m_pitch = a; rec(13, bits(a)); }
	void zoomIn() { rec(14); }
	void zoomOut() { rec(15); }
	void scrollBy(Coord2D *d) { rec(16, bits(d->x), bits(d->y)); }
	void getLocation(ViewLocation *l)
	{
		Int k = g_locations++;
		l->init(k * 1.5f, k * 2.25f, 0.5f, m_angle, m_pitch, 1.0f + k * 0.125f);
		rec(17);
	}
	void setLocation(const ViewLocation *l)
	{
		rec(18, l->m_valid ? 1 : 0, bits(l->m_pos.x), bits(l->m_pos.y), bits(l->m_pos.z), bits(l->m_angle), bits(l->m_pitch), bits(l->m_zoom));
	}
};
View g_view;
View *TheTacticalView = &g_view;

/* the bookmark's text: the slot the format was given, and whether the label asked for was the game's */
class LatText
{
public:
	Int m_slot, m_labelOk;
	void format(const LatText &fmt, Int slot) { m_slot = slot; m_labelOk = fmt.m_labelOk; }
};
#define UnicodeString LatText
class InGameUI
{
public:
	Bool m_scrolling;
	Bool getInputEnabled() { return flag(0); }
	Bool isSelecting() { return flag(1); }
	void setScrolling(Bool b) { m_scrolling = b; rec(1, b ? 1 : 0); }
	Bool isScrolling() { return m_scrolling; }
	void setScrollAmount(Coord2D amt) { rec(2, bits(amt.x), bits(amt.y)); }
	Bool shouldMoveRMBScrollAnchor() { return flag(4); }
	void message(UnicodeString);
};
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

void InGameUI::message(LatText t) { rec(3, t.m_slot, t.m_labelOk); }
class GameText { public: LatText fetch(const char *label) { LatText t; t.m_slot = -1; t.m_labelOk = strcmp(label, "GUI:BookmarkXSet") == 0; return t; } };
GameText g_text;
GameText *TheGameText = &g_text;

class Mouse
{
public:
	enum MouseCursor { INVALID_MOUSE_CURSOR = -1, NONE = 0, FIRST_CURSOR, NORMAL = FIRST_CURSOR, ARROW, SCROLL };
	Int m_cursor;
	MouseCursor getMouseCursor() { return (MouseCursor)m_cursor; }
	void setCursor(MouseCursor c) { rec(4, (Int)c); }
};
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;

class Shell { public: Bool isShellActive() { return flag(2); } };
Shell g_shell;
Shell *TheShell = &g_shell;

class StatsCollector { public: void startScrollTime() { rec(5); } void endScrollTime() { rec(6); } };
StatsCollector g_stats;
StatsCollector *TheStatsCollector = NULL;

class GameLogic
{
public:
	UnsignedInt getFrame() { return g_logicFrame; }
	Bool isInSinglePlayerGame() { return flag(7); }
	Bool isInSkirmishGame() { return flag(8); }
};
GameLogic g_logic;
GameLogic *TheGameLogic = &g_logic;

class GameClient { public: UnsignedInt getFrame() { return g_clientFrame; } };
GameClient g_client;
GameClient *TheGameClient = &g_client;

class Display { public: UnsignedInt getWidth() { return g_width; } UnsignedInt getHeight() { return g_height; } };
Display g_display;
Display *TheDisplay = &g_display;

struct GlobalData
{
	Bool m_windowed, m_saveCameraInReplay;
	Real m_horizontalScrollSpeedFactor, m_verticalScrollSpeedFactor, m_keyboardScrollFactor;
};
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;

/* MSG_SET_REPLAY_CAMERA: the real message, read back and logged after the step */
class LatStream
{
public:
	GameMessage *m_last;
	GameMessage *appendMessage(GameMessage::Type t) { m_last = newInstance(GameMessage)(t); return m_last; }
};
LatStream g_stream;
LatStream *TheMessageStream = &g_stream;

#define private public
#define protected public
#include "look_at_class.inc"
#undef private
#undef protected
#include "look_at_xlat.inc"
#undef UnicodeString

} // namespace lat

using namespace lat;

static Bool isMouseType(Int t)
{
	return t >= (Int)GameMessage::MSG_RAW_MOUSE_BEGIN && t <= (Int)GameMessage::MSG_RAW_MOUSE_END;
}

extern "C" int ref_look_at_seq(const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	g_flags = 0;
	g_logicFrame = g_clientFrame = 0;
	g_width = 800;
	g_height = 600;
	g_locations = 0;
	g_view.m_angle = g_view.m_pitch = 0.0f;
	g_ui.m_scrolling = FALSE;
	g_mouse.m_cursor = 0;
	memset(&g_global, 0, sizeof g_global);
	TheStatsCollector = NULL;
	memset(scrollDir, 0, sizeof scrollDir);
	prevCursor = Mouse::ARROW;
	{
		LookAtTranslator x;
		for (int s = 0; s + 8 <= nin; s += 8) {
			const int *w = in + s;
			Int disp = -1;
			switch (w[0]) {
			case 0: {
				Int t = w[1];
				GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)t);
				if (isMouseType(t)) {
					ICoord2D p; p.x = w[2]; p.y = w[3];
					msg->appendPixelArgument(p);
				} else {
					msg->appendIntegerArgument(w[2]);
				}
				msg->appendIntegerArgument(w[5]);
				g_stream.m_last = NULL;
				disp = (Int)x.translateGameMessage(msg);
				msg->deleteInstance();
				if (g_stream.m_last) {
					GameMessage *r = g_stream.m_last;
					Coord3D pos = r->getArgument(0)->location;
					rec(7, bits(pos.x), bits(pos.y), bits(pos.z), bits(r->getArgument(1)->real), bits(r->getArgument(2)->real), bits(r->getArgument(3)->real), r->getArgument(4)->integer);
					rec(8, r->getArgument(5)->pixel.x, r->getArgument(5)->pixel.y);
					r->deleteInstance();
				}
				break;
			}
			case 1:
				g_flags = w[1];
				g_ui.m_scrolling = g_ui.m_scrolling && flag(3);
				g_global.m_windowed = flag(5);
				g_global.m_saveCameraInReplay = flag(6);
				TheStatsCollector = flag(9) ? &g_stats : NULL;
				break;
			case 2: g_logicFrame = (UnsignedInt)w[1]; g_clientFrame = (UnsignedInt)w[2]; break;
			case 3: g_width = (UnsignedInt)w[1]; g_height = (UnsignedInt)w[2]; break;
			case 4: g_mouse.m_cursor = w[1]; break;
			case 5: x.resetModes(); break;
			case 6: { ICoord2D p; p.x = w[1]; p.y = w[2]; x.setCurrentPos(p); break; }
			case 7:
				g_global.m_horizontalScrollSpeedFactor = real(w[1]);
				g_global.m_verticalScrollSpeedFactor = real(w[2]);
				g_global.m_keyboardScrollFactor = real(w[3]);
				break;
			case 8: g_view.m_angle = real(w[1]); g_view.m_pitch = real(w[2]); break;
			}
			rec(100, disp, x.m_isScrolling, x.m_isRotating, x.m_isPitching, x.m_isChangingFOV, x.m_scrollType, (Int)x.m_timestamp);
			rec(101, x.m_anchor.x, x.m_anchor.y, x.m_originalAnchor.x, x.m_originalAnchor.y, x.m_currentPos.x, x.m_currentPos.y, (Int)x.m_lastMouseMoveFrame);
			Int anchor = x.getRMBScrollAnchor() != NULL;
			Int moved = x.hasMouseMovedRecently();
			rec(102, scrollDir[0], scrollDir[1], scrollDir[2], scrollDir[3], (Int)prevCursor, anchor, moved);
		}
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
