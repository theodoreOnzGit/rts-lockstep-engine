/* Differential-test wrapper for the DebugDisplay draft (#215, upstream commit 0a05454d), src/drafts/logic_common/debug_display.rs:
**   GeneralsMD/Code/GameEngine/Include/GameClient/DebugDisplay.h (read unchanged, `protected` read as `public`) and
**   GameEngine/Source/GameClient/System/DebugDisplay.cpp from `DebugDisplay::DebugDisplay()` to the end of the file (the constructor,
**   reset, the cursor, size, colour and margin accessors, and printf), cut verbatim by build.rs into debug_display.inc.
**
** Stand-ins (outside the cut): DEBUG_ASSERTCRASH, empty (compiled out of the release build); a derived class TestDisplay whose drawText
** logs its call as words (the pure virtual the device implements) and whose constructor sets m_width and m_height (upstream's
** constructor leaves them 0; a real display sets them) and calls reset() again. printf is called as printf("%s", text), so a `%` in the
** text is just text; vsprintf is the C library's.
**
** Entry point (extern "C", tests/differential_draft_debug_display.rs): ref_dd_run(width, height, ops, nops, out, cap), op stream:
**   [0 setCursorPos x y] [1 setTextColor c] [2 setRightMargin v] [3 setLeftMargin v] [4 reset] [5 printf len bytes...] [6 the four getters];
**   after every op the state (m_textColor, m_xPos, m_yPos, m_width, m_height, m_rightMargin, m_leftMargin) is logged; drawText logs
**   [100 x y len bytes...] where it is called. Returns the count of words, or -1 when it exceeds cap. */
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <vector>

/* renamed: the classes are at global scope (the cut defines their members), and other differential builds define classes of these names */
#define DebugDisplay LcDebugDisplay
#define DebugDisplayInterface LcDebugDisplayInterface
#define DEBUG_ASSERTCRASH(c, m)
#define protected public
#include "GameClient/DebugDisplay.h"
#undef protected

static std::vector<int> theLog;

#include "debug_display.inc"

namespace {

class TestDisplay : public DebugDisplay
{
public:
	TestDisplay(int w, int h) { m_width = w; m_height = h; reset(); }
	virtual void drawText(Int x, Int y, Char *text)
	{
		theLog.push_back(100); theLog.push_back(x); theLog.push_back(y);
		int n = (int)strlen(text);
		theLog.push_back(n);
		for (int i = 0; i < n; ++i) theLog.push_back((unsigned char)text[i]);
	}
};

void putState(const TestDisplay &d)
{
	theLog.push_back(d.m_textColor); theLog.push_back(d.m_xPos); theLog.push_back(d.m_yPos); theLog.push_back(d.m_width);
	theLog.push_back(d.m_height); theLog.push_back(d.m_rightMargin); theLog.push_back(d.m_leftMargin);
}

} // namespace

extern "C" {

int ref_dd_run(int width, int height, const int *ops, int nops, int *out, int cap)
{
	theLog.clear();
	TestDisplay d(width, height);
	putState(d);
	int p = 0;
	while (p < nops)
	{
		int op = ops[p++];
		theLog.push_back(1000 + op);
		switch (op)
		{
		case 0: d.setCursorPos(ops[p], ops[p + 1]); p += 2; break;
		case 1: d.setTextColor((DebugDisplayInterface::Color)ops[p]); p += 1; break;
		case 2: d.setRightMargin(ops[p]); p += 1; break;
		case 3: d.setLeftMargin(ops[p]); p += 1; break;
		case 4: d.reset(); break;
		case 5: {
			int n = ops[p++];
			char text[700];
			for (int i = 0; i < n; ++i) text[i] = (char)ops[p + i];
			text[n] = 0;
			p += n;
			d.printf((Char *)"%s", text);
			break; }
		default:
			theLog.push_back(d.getCursorXPos()); theLog.push_back(d.getCursorYPos()); theLog.push_back(d.getWidth()); theLog.push_back(d.getHeight());
			break;
		}
		putState(d);
	}
	if ((int)theLog.size() > cap) return -1;
	for (size_t i = 0; i < theLog.size(); ++i) out[i] = theLog[i];
	return (int)theLog.size();
}

}
