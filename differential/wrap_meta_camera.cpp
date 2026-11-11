/* Differential-test wrapper for the right button's bookkeeping and the keyboard camera (GitHub #131, upstream commit
** 0a05454d), cut verbatim by build.rs into OUT_DIR/meta_camera_gen:
**   command_gate_open.inc  CommandXlat.cpp, CommandTranslator::translateGameMessage from its first line to the debug
**                          build's locals after the input gate (not closed: the wrapper adds its switch);
**   system_message.inc     CommandXlat.cpp, isSystemMessage (to the end of the file);
**   right_cases.inc        CommandXlat.cpp, the cases MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN and _UP (3628-3657, to
**                          `case GameMessage::MSG_MOUSE_RIGHT_DOUBLE_CLICK:`);
**   camera_cases.inc       CommandXlat.cpp, the cases MSG_META_BEGIN_CAMERA_ROTATE_LEFT to MSG_META_CAMERA_RESET
**                          (3205-3239, to `case GameMessage::MSG_META_TOGGLE_CAMERA_TRACKING_DRAWABLE:`);
**   is_click.inc           Mouse.cpp, Mouse::isClick (to `CursorInfo::CursorInfo( void )`);
**   camera_flags.inc       InGameUI.h, the camera setters and questions (`void setCameraRotateLeft` to
**                          `void resetCamera();`), inside the stand-in InGameUI class;
**   keyboard_camera.inc    InGameUI.cpp, InGameUI::update's keyboard camera, from `//Handle keyboard camera rotations`
**                          to InGameUI::registerWindowLayout (the block and update's closing brace: the wrapper opens
**                          it as `void InGameUI::keyboardCameraUpdate() {`);
**   reset_camera_ui.inc    InGameUI.cpp, InGameUI::resetCamera (to canSelectedObjectsNonAttackInteractWithObject);
**   set_input_enabled.inc  InGameUI.cpp, InGameUI::setInputEnabled (to disregardDrawable's comment: it clears the four
**                          camera flags);
**   view_get_location.inc  View.cpp, View::getLocation (to View::setLocation's comment);
**   parabolic_ease.inc     ParabolicEase.h, `class ParabolicEase` (to the header's #endif);
**   parabolic_ease_cpp.inc ParabolicEase.cpp, from `namespace {` to the end (clamp, setEaseTimes, operator());
**   w3d_view_info.inc      W3DView.h, `enum {MAX_WAYPOINTS=25};` to `class W3DView` (the movement info structs);
**   w3d_view_statics.inc   W3DView.cpp, minf and maxf to W3DView::W3DView (normAngle, getHeightAroundPos);
**   w3d_update_moves.inc   W3DView.cpp, W3DView::updateCameraMovements (to updateView's comment);
**   w3d_move_to.inc        W3DView.cpp, W3DView::moveCameraTo (to rotateCamera);
**   w3d_zoom_pitch.inc     W3DView.cpp, W3DView::zoomCamera and pitchCamera (to cameraModFinalZoom);
**   w3d_reset.inc          W3DView.cpp, W3DView::resetCamera (to isCameraMovementFinished);
**   w3d_waypoints.inc      W3DView.cpp, W3DView::setupWaypointPath and makeQuadraticS (to rotateCameraOneFrame);
**   w3d_one_frame.inc      W3DView.cpp, zoomCameraOneFrame, pitchCameraOneFrame and moveAlongWaypointPath (to shake).
** No text is edited. Two namespaces: `mcx` (kinds 0 and 1) compiles the command translator's cases, isClick, the
** camera flags and the keyboard camera on stand-ins that record what they are asked (the view's setAngle, zoomIn,
** zoomOut; the InGameUI's placeBuildAvailable and resetCamera); `rcx` (kind 2) compiles InGameUI::resetCamera with
** View::getLocation and the W3DView pieces on a stand-in W3DView holding the members they use. rotateCameraOneFrame is
** a stand-in that aborts: setupWaypointPath clears m_doingRotateCamera, so the reset never reaches it.
** REAL_TO_INT_FLOOR is a stand-in (floorf): moveAlongWaypointPath's interpolation, the one user, is not reached by a
** reset at the view's own position (the path ends in moveCameraTo's own step). The messages are the real GameMessage
** of the message stream build (wrap_message_stream.cpp), under its names.
**
** Entry point (extern "C", tests/differential_meta_input.rs): ref_meta_camera_case(kind, in, nin, out, cap).
**   kind 0: right-button sequences on one CommandTranslator. in: m_dragTolerance, m_dragToleranceMS, then steps of 5
**     words: op, a, b, c, d. op 0: a raw message type a at (b, c), time d (argument 2) -> out the disposition and
**     every placeBuildAvailable as 4000; op 1: the input enabled a; op 2: the right click's test,
**     `TheMouse->isClick(&m_mouseRightDragAnchor, &m_mouseRightDragLift, m_mouseRightDown, m_mouseRightUp)` -> out
**     (5000, the answer). After each step (-1, anchor x, y, lift x, y, down, up).
**   kind 1: keyboard camera sequences on one InGameUI. in: the rotate speed's bits, the view's angle's bits, then steps
**     of 2 words: op, a. op 0: message type a through translateGameMessage (the gate, then the camera cases) -> out the
**     disposition and a resetCamera as 5004; op 1: setInputEnabled(a); op 2: InGameUI::update's keyboard camera ->
**     out every setAngle (5001, the angle's bits), zoomIn (5002), zoomOut (5003). After each step (-1, the four
**     flags).
**   kind 2: one InGameUI::resetCamera and the next frame's updateCameraMovements on a W3DView. in (float bits unless
**     said): pos x, y, z, angle, pitch, zoom, FX pitch, ground level, camera offset x, y, z, max height above ground,
**     height above ground, the constraint's lo x, y, hi x, y, the global camera pitch, yaw, m_disableCameraMovement
**     (int). The ground height at (x, y) is ((int)(x / 64) % 5) * 7.5 + y / 128. out (bits): pos x, y, z, angle, zoom,
**     FX pitch, ground level, offset x, y, z, height above ground, the constraint's lo x, y, hi x, y, then the four
**     m_doing* flags (zoom, pitch, rotate, move along the path) after the frame. */
#define GameMessage MsTestGameMessage
#define GameMessageArgument MsTestGameMessageArgument
#define GameMessageList MsTestGameMessageList
#define GameMessageTranslator MsTestGameMessageTranslator
#define MessageStream MsTestMessageStream
#define CommandList MsTestCommandList
#define TheMessageStream MsTestTheMessageStream
#define TheCommandList MsTestTheCommandList
#define buildRegion MsTestBuildRegion

#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <vector>
#define PI 3.14159265359f
#include "PreRTS.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"
#include "Common/MessageStream.h"
#include "vector2.h"

static std::vector<int> g_out;
static Int realBits(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
static Real bitsReal(Int i) { Real r; memcpy(&r, &i, sizeof r); return r; }

namespace mcx {

/* ---- the stand-ins ---- */
class Mouse
{
public:
	UnsignedInt m_dragTolerance;
	UnsignedInt m_dragToleranceMS;
	Bool isClick(const ICoord2D *anchor, const ICoord2D *dest, UnsignedInt previousMouseClick, UnsignedInt currentMouseClick);
};
#include "is_click.inc"
Mouse g_mouse;
Mouse *TheMouse = &g_mouse;

struct GlobalData { Real m_keyboardCameraRotateSpeed; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;

class View
{
public:
	Real m_angle;
	Real getAngle() { return m_angle; }
	void setAngle(Real a) { g_out.push_back(5001); g_out.push_back(realBits(a)); m_angle = a; }
	void zoomIn() { g_out.push_back(5002); }
	void zoomOut() { g_out.push_back(5003); }
};
View g_view;
View *TheTacticalView = &g_view;

class ThingTemplate;
class Drawable;
class InGameUI
{
public:
	Bool m_inputEnabled;
	Bool m_cameraRotatingLeft, m_cameraRotatingRight, m_cameraZoomingIn, m_cameraZoomingOut, m_cameraTrackingDrawable;
	Bool getInputEnabled() { return m_inputEnabled; }
	void placeBuildAvailable(const ThingTemplate *, Drawable *) { g_out.push_back(4000); }
	/* setInputEnabled's other modes: not under test here (tests/differential_input_gate.rs) */
	void setSelecting(Bool) {}
	void setForceAttackMode(Bool) {}
	void setForceMoveMode(Bool) {}
	void setWaypointMode(Bool) {}
	void setPreferSelectionMode(Bool) {}
	void setInputEnabled( Bool enable );
#include "camera_flags.inc"
	void resetCamera();
	void keyboardCameraUpdate();
};
void InGameUI::resetCamera() { g_out.push_back(5004); }
#include "set_input_enabled.inc"
void InGameUI::keyboardCameraUpdate()
{
#include "keyboard_camera.inc"
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

class CommandTranslator : public GameMessageTranslator
{
public:
	ICoord2D m_mouseRightDragAnchor;
	ICoord2D m_mouseRightDragLift;
	UnsignedInt m_mouseRightDown;
	UnsignedInt m_mouseRightUp;
	virtual GameMessageDisposition translateGameMessage(const GameMessage *msg);
};
static Bool isSystemMessage( const GameMessage *msg );
#include "command_gate_open.inc"
	switch (t)
	{
#include "camera_cases.inc"
			break;
#include "right_cases.inc"
			break;
		default:
			break;
	}
	return disp;
}
#include "system_message.inc"

} // namespace mcx

namespace rcx {

/* BaseType.h's max, `(((x)>(y)) ? (x) : (y))` (a header after it undefined the macro) */
#ifndef max
template <typename T> inline T max(T x, T y) { return ((x) > (y)) ? (x) : (y); }
#endif
#ifndef REAL_TO_INT_FLOOR
#define REAL_TO_INT_FLOOR(x) ((Int)floorf(x))
#endif
struct GlobalData { Bool m_disableCameraMovement; Real m_cameraPitch; Real m_cameraYaw; };
GlobalData g_global;
GlobalData *TheGlobalData = &g_global;
class TerrainLogic
{
public:
	Real getGroundHeight(Real x, Real y) { return (Real)((Int)(x / 64.0f) % 5) * 7.5f + y / 128.0f; }
};
TerrainLogic g_terrain;
TerrainLogic *TheTerrainLogic = &g_terrain;
Int TheW3DFrameLengthInMsec = 1000/30;
typedef Int ObjectID;
#include "parabolic_ease.inc"
#include "parabolic_ease_cpp.inc"
#include "w3d_view_info.inc"

/* View.h's ViewLocation, as the functions here use it */
class ViewLocation
{
public:
	Coord3D m_pos;
	Real m_angle, m_pitch, m_zoom;
	Bool m_valid;
	const Coord3D& getPosition() const { return m_pos; }
	void init(Real x, Real y, Real z, Real angle, Real pitch, Real zoom)
	{
		m_pos.x = x; m_pos.y = y; m_pos.z = z; m_angle = angle; m_pitch = pitch; m_zoom = zoom; m_valid = true;
	}
};
class View
{
public:
	Coord3D m_pos;
	Real m_angle, m_pitchAngle, m_zoom, m_heightAboveGround, m_maxHeightAboveGround;
	Real m_groundLevel;
	Coord3D m_cameraOffset;
	Region2D m_cameraConstraint;
	const Coord3D *getPosition() { return &m_pos; }
	void setPosition( const Coord3D *pos ) { m_pos = *pos; }
	Real getAngle() { return m_angle; }
	Real getPitch() { return m_pitchAngle; }
	Real getZoom() { return m_zoom; }
	void getLocation( ViewLocation *location );
};
#include "view_get_location.inc"
class W3DView : public View
{
public:
	Real m_FXPitch;
	Int m_timeMultiplier;
	Coord3D m_previousLookAtPosition;
	TMoveAlongWaypointPathInfo m_mcwpInfo;
	TRotateCameraInfo m_rcInfo;
	TPitchCameraInfo m_pcInfo;
	TZoomCameraInfo m_zcInfo;
	Bool m_doingMoveCameraOnWaypointPath, m_CameraArrivedAtWaypointOnPathFlag, m_doingRotateCamera, m_doingPitchCamera, m_doingZoomCamera;
	Bool m_doingScriptedCameraLock, m_freezeTimeForCameraMovement;
	Bool updateCameraMovements();
	void moveCameraTo(const Coord3D *o, Int milliseconds, Int shutter, Bool orient, Real easeIn, Real easeOut);
	void zoomCamera( Real finalZoom, Int milliseconds, Real easeIn, Real easeOut );
	void pitchCamera( Real finalPitch, Int milliseconds, Real easeIn, Real easeOut );
	void resetCamera(const Coord3D *location, Int milliseconds, Real easeIn, Real easeOut);
	void setupWaypointPath(Bool orient);
	void rotateCameraOneFrame(void) { abort(); }
	void zoomCameraOneFrame(void);
	void pitchCameraOneFrame(void);
	void moveAlongWaypointPath(Int milliseconds);
};
#include "w3d_view_statics.inc"
#include "w3d_update_moves.inc"
#include "w3d_move_to.inc"
#include "w3d_zoom_pitch.inc"
#include "w3d_reset.inc"
#include "w3d_waypoints.inc"
#include "w3d_one_frame.inc"
W3DView g_w3d;
W3DView *TheTacticalView = &g_w3d;
class InGameUI { public: void resetCamera(); };
#include "reset_camera_ui.inc"
InGameUI g_ui;
InGameUI *TheInGameUI = &g_ui;

} // namespace rcx

extern "C" int ref_meta_camera_case(int kind, const int *in, int nin, int *out, int cap)
{
	g_out.clear();
	if (kind == 0) {
		mcx::g_mouse.m_dragTolerance = (UnsignedInt)in[0];
		mcx::g_mouse.m_dragToleranceMS = (UnsignedInt)in[1];
		mcx::g_ui.m_inputEnabled = TRUE;
		mcx::CommandTranslator x;
		memset(&x.m_mouseRightDragAnchor, 0, sizeof x.m_mouseRightDragAnchor);
		memset(&x.m_mouseRightDragLift, 0, sizeof x.m_mouseRightDragLift);
		x.m_mouseRightDown = 0;
		x.m_mouseRightUp = 0;
		for (Int p = 2; p + 4 < nin; p += 5) {
			Int op = in[p];
			if (op == 0) {
				GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)in[p + 1]);
				ICoord2D px; px.x = in[p + 2]; px.y = in[p + 3];
				msg->appendPixelArgument(px);
				msg->appendIntegerArgument(0);
				msg->appendIntegerArgument(in[p + 4]);
				g_out.push_back((Int)x.translateGameMessage(msg));
				msg->deleteInstance();
			} else if (op == 1) {
				mcx::g_ui.m_inputEnabled = in[p + 1] != 0;
			} else {
				g_out.push_back(5000);
				g_out.push_back((Int)mcx::TheMouse->isClick(&x.m_mouseRightDragAnchor, &x.m_mouseRightDragLift, x.m_mouseRightDown, x.m_mouseRightUp));
			}
			Int w[7] = { -1, x.m_mouseRightDragAnchor.x, x.m_mouseRightDragAnchor.y, x.m_mouseRightDragLift.x, x.m_mouseRightDragLift.y, (Int)x.m_mouseRightDown, (Int)x.m_mouseRightUp };
			g_out.insert(g_out.end(), w, w + 7);
		}
	} else if (kind == 1) {
		mcx::g_global.m_keyboardCameraRotateSpeed = bitsReal(in[0]);
		mcx::g_view.m_angle = bitsReal(in[1]);
		mcx::InGameUI &ui = mcx::g_ui;
		ui.m_inputEnabled = TRUE;
		ui.m_cameraRotatingLeft = ui.m_cameraRotatingRight = ui.m_cameraZoomingIn = ui.m_cameraZoomingOut = ui.m_cameraTrackingDrawable = FALSE;
		mcx::CommandTranslator x;
		for (Int p = 2; p + 1 < nin; p += 2) {
			Int op = in[p];
			if (op == 0) {
				GameMessage *msg = newInstance(GameMessage)((GameMessage::Type)in[p + 1]);
				g_out.push_back((Int)x.translateGameMessage(msg));
				msg->deleteInstance();
			} else if (op == 1) {
				ui.setInputEnabled(in[p + 1] != 0);
			} else {
				ui.keyboardCameraUpdate();
			}
			Int w[5] = { -1, (Int)ui.m_cameraRotatingLeft, (Int)ui.m_cameraRotatingRight, (Int)ui.m_cameraZoomingIn, (Int)ui.m_cameraZoomingOut };
			g_out.insert(g_out.end(), w, w + 5);
		}
	} else if (kind == 2) {
		rcx::W3DView &v = rcx::g_w3d;
		memset(&v, 0, sizeof v);
		v.m_pos.x = bitsReal(in[0]); v.m_pos.y = bitsReal(in[1]); v.m_pos.z = bitsReal(in[2]);
		v.m_angle = bitsReal(in[3]); v.m_pitchAngle = bitsReal(in[4]); v.m_zoom = bitsReal(in[5]); v.m_FXPitch = bitsReal(in[6]);
		v.m_groundLevel = bitsReal(in[7]);
		v.m_cameraOffset.x = bitsReal(in[8]); v.m_cameraOffset.y = bitsReal(in[9]); v.m_cameraOffset.z = bitsReal(in[10]);
		v.m_maxHeightAboveGround = bitsReal(in[11]); v.m_heightAboveGround = bitsReal(in[12]);
		v.m_cameraConstraint.lo.x = bitsReal(in[13]); v.m_cameraConstraint.lo.y = bitsReal(in[14]);
		v.m_cameraConstraint.hi.x = bitsReal(in[15]); v.m_cameraConstraint.hi.y = bitsReal(in[16]);
		v.m_timeMultiplier = 1;
		rcx::g_global.m_cameraPitch = bitsReal(in[17]);
		rcx::g_global.m_cameraYaw = bitsReal(in[18]);
		rcx::g_global.m_disableCameraMovement = in[19] != 0;
		rcx::TheInGameUI->resetCamera();
		v.updateCameraMovements();
		Int w[20] = { realBits(v.m_pos.x), realBits(v.m_pos.y), realBits(v.m_pos.z), realBits(v.m_angle), realBits(v.m_zoom), realBits(v.m_FXPitch),
			realBits(v.m_groundLevel), realBits(v.m_cameraOffset.x), realBits(v.m_cameraOffset.y), realBits(v.m_cameraOffset.z), realBits(v.m_heightAboveGround),
			realBits(v.m_cameraConstraint.lo.x), realBits(v.m_cameraConstraint.lo.y), realBits(v.m_cameraConstraint.hi.x), realBits(v.m_cameraConstraint.hi.y),
			(Int)v.m_doingZoomCamera, (Int)v.m_doingPitchCamera, (Int)v.m_doingRotateCamera, (Int)v.m_doingMoveCameraOnWaypointPath, 0 };
		g_out.insert(g_out.end(), w, w + 19);
	}
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	return n;
}
