/* Differential-test wrapper for the control bar's position printer (#215, group ui3b, upstream commit 0a05454d):
** src/drafts/ui/control_bar_print_positions.rs. Cut verbatim by build.rs (drafts_ui3b), no edit:
**   print_positions.inc  GameClient/GUI/ControlBar/ControlBarPrintPositions.cpp, from `void PrintInfoRecursive` to the end
**                        (PrintInfoRecursive, PrintOffsetsFromControlBarParent).
** GameWindowManager::winGetWindowFromId, cut verbatim by build.rs (ui3b_winget.inc, GameWindowManager.cpp).
** Stand-ins (the shared ones in differential/shim_ui3b/ui3b_stubs.h: windows with a position, size, decorated name, first child, next sibling;
** winCreateLayout; WindowLayout):
**   fopen / fclose (renamed ui3b_fopen / ui3b_fclose by #define after <stdio.h>, so no C library function is replaced): fopen logs [920], checks the
**     name "ControlBarEasier.txt" (logs [999] when different) and the mode "w" ([998]) and answers an in-memory stream (open_memstream); fclose logs
**     [921] and closes it. The upstream `fprintf` is the C library's, writing to that stream.
**   TheNameKeyGenerator->nameToKey: the shared FNV-1a name keys.
** Not covered: a failing fopen (the draft has the caller write the file: it cannot fail in the draft).
** Entry point ref_ui3b_print_run(in, n, out, cap): `in` = [kind (0 PrintInfoRecursive(window index + 1 or 0 for NULL), 1 PrintOffsetsFromControlBarParent),
** window index (kind 0), control bar parent window present, layout answered (kind 1), layout's first window index + 1 (0 NULL), nnodes,
** nnodes x [x, y, w, h, first child index + 1, next index + 1, name length, name bytes]]. Output: [fopen called, text length, text bytes]; for kind 1 the
** the calls on the layout ([912] winCreateLayout, [910] destroyWindows, [911] deleteInstance) and the file ([920], [921]) are logged but not compared: the draft
** leaves them to its caller. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace ui3bprint {

#include "ui3b_stubs.h"
#include "ui3b_winget.inc"

static char *g_buf = NULL;
static size_t g_len = 0;
static FILE *ui3b_fopen(const char *name, const char *mode)
{
	lg(920);
	if (strcmp(name, "ControlBarEasier.txt") != 0) lg(999);
	if (strcmp(mode, "w") != 0) lg(998);
	g_buf = NULL; g_len = 0;
	return open_memstream(&g_buf, &g_len);
}
static int ui3b_fclose(FILE *f) { lg(921); return fclose(f); }
#define fopen ui3b_fopen
#define fclose ui3b_fclose

#include "print_positions.inc"

#undef fopen
#undef fclose

}

extern "C" int ref_ui3b_print_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bprint;
	(void)n;
	int p = 0;
	unsigned kind = in[p++], win = in[p++], parentPresent = in[p++], layoutAnswered = in[p++], layoutFirst = in[p++], nnodes = in[p++];
	std::vector<GameWindow> nodes(nnodes);
	std::vector<unsigned> childOf(nnodes), nextOf(nnodes);
	for (unsigned i = 0; i < nnodes; ++i) {
		nodes[i].x = (Int)in[p++]; nodes[i].y = (Int)in[p++]; nodes[i].w = (Int)in[p++]; nodes[i].h = (Int)in[p++];
		childOf[i] = in[p++]; nextOf[i] = in[p++];
		unsigned len = in[p++];
		for (unsigned k = 0; k < len; ++k) nodes[i].inst.m_decoratedNameString.s.push_back((char)in[p++]);
	}
	for (unsigned i = 0; i < nnodes; ++i) {
		nodes[i].m_child = childOf[i] ? &nodes[childOf[i] - 1] : NULL;
		nodes[i].m_next = nextOf[i] ? &nodes[nextOf[i] - 1] : NULL;
	}
	g_log.clear();
	g_buf = NULL; g_len = 0;
	std::vector<unsigned> all;
	GameWindow parentWin;
	parentWin.id = NAMEKEY("ControlBar.wnd:ControlBarParent");
	g_wm.table.clear();
	if (parentPresent) g_wm.table.push_back(&parentWin);
	g_wm.m_windowList = parentPresent ? &parentWin : NULL;
	WindowLayout layout;
	layout.first = layoutFirst ? &nodes[layoutFirst - 1] : NULL;
	g_wm.layoutToReturn = layoutAnswered ? &layout : NULL;
	unsigned fopened = 0;
	if (kind == 0) {
		FILE *fp = open_memstream(&g_buf, &g_len);
		PrintInfoRecursive(win ? &nodes[win - 1] : NULL, fp);
		fclose(fp);
		fopened = 1;
	} else {
		PrintOffsetsFromControlBarParent();
		for (size_t i = 0; i < g_log.size(); ++i) if (g_log[i] == 920) fopened = 1;
	}
	all.push_back(fopened);
	all.push_back((unsigned)g_len);
	for (size_t i = 0; i < g_len; ++i) all.push_back((unsigned char)g_buf[i]);
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
