/* Differential-test wrapper for the mouse input draft (#215, upstream commit 0a05454d), src/drafts/sweep_common_client/mouse.rs
** (MouseInput::move_mouse, update_mouse_data, check_for_drag, process_mouse_event), cut verbatim by build.rs into OUT_DIR/draft_guilogic_gen:
**   mouse_io.inc     GameClient/Mouse.h from `enum MouseButtonState` to `class CursorInfo` (MouseButtonState, MOUSE_MOVE_*, MOUSE_NONE,
**                    MOUSE_LOST, MOUSE_EVENT_NONE, MouseIO);
**   mouse_events.inc Mouse.h, `enum { NUM_MOUSE_EVENTS = 256 };` to `Bool m_visible;` (the event arrays, the region, the frames,
**                    m_inputMovesAbsolute) and mouse_count.inc, `Int m_eventsThisFrame;` (to the end of the class);
**   mouse_gwm.inc    GameClient/GameWindow.h, `enum GameWindowMessage` to the WinInputReturnCode comment (GWM_LEFT_DOWN ...);
**   mouse_fns.inc    GameClient/Input/Mouse.cpp from `void Mouse::moveMouse` to `/** Check for mouse click` (moveMouse, updateMouseData,
**                    processMouseEvent, checkForDrag; one mechanical edit, see build.rs).
** Stand-ins: class Mouse holds ONLY the members those cuts use, declared by the header's own lines above (the real class needs
** SubsystemInterface, the cursor tables and the INI parse table; its constructor is not in the cut), with the pure virtual
** getMouseEvent, here a scripted device: each call returns the next scripted result byte and fills `*result` with the scripted
** MouseIO (MOUSE_NONE and fills nothing once the script is used up). AsciiString/UnicodeString are not needed.
**
** Entry points (extern "C", tests/differential_draft_mouse.rs):
**   ref_gwm(k)  the GameWindowMessage enumerator k (0 LEFT_DOWN, 1 LEFT_UP, 2 LEFT_DOUBLE_CLICK, 3 LEFT_DRAG, then MIDDLE, then RIGHT).
**   ref_mouse_run(in, nin, out, cap)  runs a script on one Mouse; `in` = setup [minX, maxX, minY, maxY, inputMovesAbsolute, inputFrame,
**   deadInputFrame], then steps [kind, ...]: 0 x y relOrAbs: moveMouse; 1 index: processMouseEvent; 2: checkForDrag;
**   3 n, then n x (result byte, 15 MouseIO words): updateMouseData over that device script; 4 + 15 words: m_currMouse = ...;
**   5 + 15 words: m_prevMouse = ...; 6 index + 15 words: m_mouseEvents[index] = ...; 7 inputFrame: m_inputFrame = ...
**   (MouseIO words: pos.x pos.y time wheelPos delta.x delta.y, then state event frame for left, right, middle).
**   After each step: m_currMouse and m_prevMouse (15 words each), m_eventsThisFrame, m_deadInputFrame, m_inputFrame, then, after a
**   kind 3, m_mouseEvents[0 .. m_eventsThisFrame] (15 words each). Returns the word count (-1: cap too small). */
#include "PreRTS.h"
#include <vector>
#include <string.h>

#include "mouse_gwm.inc"
#include "mouse_io.inc"

class Mouse
{
public:
	Mouse()
	{
		memset(m_mouseEvents, 0, sizeof m_mouseEvents); memset(&m_currMouse, 0, sizeof m_currMouse); memset(&m_prevMouse, 0, sizeof m_prevMouse);
		m_minX = m_maxX = m_minY = m_maxY = 0; m_inputFrame = m_deadInputFrame = 0; m_inputMovesAbsolute = FALSE; m_eventsThisFrame = 0; m_scriptAt = 0;
	}
	virtual ~Mouse() {}
	void moveMouse( Int x, Int y, Int relOrAbs );
	void updateMouseData( );
	void processMouseEvent( Int index );
	void checkForDrag( void );
	virtual UnsignedByte getMouseEvent( MouseIO *result, Bool flush );

	std::vector<unsigned char> m_scriptResult;
	std::vector<MouseIO> m_scriptIO;
	size_t m_scriptAt;

#include "mouse_events.inc"
	Bool m_visible;
#include "mouse_count.inc"
};

UnsignedByte Mouse::getMouseEvent( MouseIO *result, Bool flush )
{
	if (m_scriptAt >= m_scriptResult.size()) return MOUSE_NONE;
	*result = m_scriptIO[m_scriptAt];
	return m_scriptResult[m_scriptAt++];
}

#include "mouse_fns.inc"

namespace {

void readIO(const int *p, MouseIO &m)
{
	m.pos.x = p[0]; m.pos.y = p[1]; m.time = (UnsignedInt)p[2]; m.wheelPos = p[3]; m.deltaPos.x = p[4]; m.deltaPos.y = p[5];
	m.leftState = (MouseButtonState)p[6]; m.leftEvent = p[7]; m.leftFrame = p[8];
	m.rightState = (MouseButtonState)p[9]; m.rightEvent = p[10]; m.rightFrame = p[11];
	m.middleState = (MouseButtonState)p[12]; m.middleEvent = p[13]; m.middleFrame = p[14];
}

void putIO(std::vector<int> &o, const MouseIO &m)
{
	int w[15] = { m.pos.x, m.pos.y, (int)m.time, m.wheelPos, m.deltaPos.x, m.deltaPos.y, (int)m.leftState, m.leftEvent, m.leftFrame,
		(int)m.rightState, m.rightEvent, m.rightFrame, (int)m.middleState, m.middleEvent, m.middleFrame };
	for (int i = 0; i < 15; ++i) o.push_back(w[i]);
}

}

extern "C" {

int ref_gwm(int k)
{
	static const int v[12] = { GWM_LEFT_DOWN, GWM_LEFT_UP, GWM_LEFT_DOUBLE_CLICK, GWM_LEFT_DRAG, GWM_MIDDLE_DOWN, GWM_MIDDLE_UP,
		GWM_MIDDLE_DOUBLE_CLICK, GWM_MIDDLE_DRAG, GWM_RIGHT_DOWN, GWM_RIGHT_UP, GWM_RIGHT_DOUBLE_CLICK, GWM_RIGHT_DRAG };
	return v[k];
}

int ref_mouse_run(const int *in, int nin, int *out, int cap)
{
	Mouse m;
	m.m_minX = in[0]; m.m_maxX = in[1]; m.m_minY = in[2]; m.m_maxY = in[3];
	m.m_inputMovesAbsolute = in[4] != 0;
	m.m_inputFrame = (UnsignedInt)in[5];
	m.m_deadInputFrame = (UnsignedInt)in[6];
	std::vector<int> o;
	int at = 7;
	while (at < nin)
	{
		int kind = in[at++];
		bool events = false;
		switch (kind)
		{
			case 0: m.moveMouse(in[at], in[at + 1], in[at + 2]); at += 3; break;
			case 1: m.processMouseEvent(in[at]); at += 1; break;
			case 2: m.checkForDrag(); break;
			case 3:
			{
				int n = in[at++];
				m.m_scriptResult.clear(); m.m_scriptIO.clear(); m.m_scriptAt = 0;
				for (int i = 0; i < n; ++i)
				{
					m.m_scriptResult.push_back((unsigned char)in[at++]);
					MouseIO io; memset(&io, 0, sizeof io); readIO(in + at, io); at += 15;
					m.m_scriptIO.push_back(io);
				}
				m.updateMouseData();
				events = true;
				break;
			}
			case 4: readIO(in + at, m.m_currMouse); at += 15; break;
			case 5: readIO(in + at, m.m_prevMouse); at += 15; break;
			case 6: { int idx = in[at++]; readIO(in + at, m.m_mouseEvents[idx]); at += 15; break; }
			default: m.m_inputFrame = (UnsignedInt)in[at++]; break;
		}
		putIO(o, m.m_currMouse); putIO(o, m.m_prevMouse);
		o.push_back(m.m_eventsThisFrame); o.push_back((int)m.m_deadInputFrame); o.push_back((int)m.m_inputFrame);
		if (events) for (int i = 0; i <= m.m_eventsThisFrame; ++i) putIO(o, m.m_mouseEvents[i]);
	}
	if ((int)o.size() > cap) return -1;
	for (size_t i = 0; i < o.size(); ++i) out[i] = o[i];
	return (int)o.size();
}

}
