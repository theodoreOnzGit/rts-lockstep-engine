/* Differential-test wrapper for the main menu's draw callbacks (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/w3d_main_menu.rs (advancePosition,
** W3DShellMenuSchemeDraw, W3DMainMenuDraw, W3DMainMenuFourDraw, W3DMainMenuMapBorder, W3DThinBorderDraw). Cut verbatim by build.rs (drafts_ui3b), no edit:
**   mm_statics.inc  GameEngineDevice/.../GUICallbacks/W3DMainMenu.cpp, from `static Color BrownishColor` to the banner of the public functions (BrownishColor, clipRegion);
**   mm_a.inc        from `static void advancePosition` to `void W3DMetalBarMenuDraw` (advancePosition, W3DShellMenuSchemeDraw, W3DMainMenuDraw, W3DMainMenuFourDraw);
**   mm_border.inc   from `void W3DMainMenuMapBorder` to the comment of the drop shadow button;
**   mm_thin.inc     from `void W3DThinBorderDraw` to `void W3DMainMenuInit`;
**   mm_screenpos.inc  GameWindow.cpp's GameWindow::winGetScreenPosition (the sum of the ancestors' region corners), and upstream's own GameClient/Color.h (GameMakeColor).
** Stand-ins: GameWindow (region, size, parent, the enabled image 0; winGetSize, winGetEnabledImage), WinInstanceData (the image offset); TheDisplay (getHeight; drawLine logs
**   [60, x0, y0, x1, y1, color] -- the width is NOT logged, the draft has no field for it; drawImage(image, x0, y0, x1, y1) logs [61, image token, x0, y0, x1, y1, color 0xFFFFFFFF] and draws
**   nothing for a NULL image, as W3DDisplay::drawImage; enableClipping nothing); TheWindowManager->winDrawImage(image, ...) the same as drawImage; TheMappedImageCollection->findImageByName(name)
**   (an Image with the case's size, or NULL); Image (getImageWidth, getImageHeight, a token); timeGetTime() the case's clock; TheShell (isShellActive, getShellMenuSchemeManager()->draw() logs [50]).
** advancePosition keeps function statics, so each case runs in a forked child process (a fresh copy of the statics, as the game's first call) and sends its words back through a pipe.
**
** Entry point ref_ui3b_w3dmm_run(in, n, out, cap): `in` = [display height, shell active, nwin, nwin x [parent + 1, x, y, w, h, enabled image token (0 none), image offset x, y], nimg, nimg x
** [exists, w, h] for the names MainMenuPulse, FrameCornerHorizontal, FrameCornerVertical, FrameCornerUL, UR, LL, LR, ncalls, ncalls x [kind (0 W3DMainMenuDraw, 1 W3DMainMenuFourDraw,
** 2 W3DMainMenuMapBorder, 3 W3DThinBorderDraw, 4 W3DShellMenuSchemeDraw), window, clock]]. Output per call: the draws logged, then [100]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#include "GameClient/Color.h"

namespace ui3bmm {

static std::vector<unsigned> g_log;
static void lg(unsigned a) { g_log.push_back(a); }
static unsigned g_now = 0;
static unsigned timeGetTime() { return g_now; }
typedef Int NameKeyType;
enum { WIN_ERR_OK = 0 };
#define INT_TO_REAL(x) ((Real) (x))   /* Libraries/Include/Lib/BaseType.h:240, copied (the shim's BaseType.h omits it) */

struct Image { unsigned token; Int w, h; Int getImageWidth() const { return w; } Int getImageHeight() const { return h; } };
static const char *g_names[7] = { "MainMenuPulse", "FrameCornerHorizontal", "FrameCornerVertical", "FrameCornerUL", "FrameCornerUR", "FrameCornerLL", "FrameCornerLR" };
static Image g_images[7];
static bool g_exists[7];
struct MappedImageCollection
{
	const Image *findImageByName(const char *name)
	{
		for (int i = 0; i < 7; ++i) if (!strcmp(name, g_names[i])) return g_exists[i] ? &g_images[i] : NULL;
		return NULL;
	}
};
static MappedImageCollection g_mic;
static MappedImageCollection *TheMappedImageCollection = &g_mic;
static Int g_height = 0;
struct Display
{
	Int getHeight() { return g_height; }
	void drawLine(Int x0, Int y0, Int x1, Int y1, Int, Color color) { lg(60); lg((unsigned)x0); lg((unsigned)y0); lg((unsigned)x1); lg((unsigned)y1); lg((unsigned)color); }
	void drawImage(const Image *image, Int x0, Int y0, Int x1, Int y1, Color color = 0xFFFFFFFF)
	{
		if (!image) return;
		lg(61); lg(image->token); lg((unsigned)x0); lg((unsigned)y0); lg((unsigned)x1); lg((unsigned)y1); lg((unsigned)color);
	}
	void enableClipping(Bool) {}
};
static Display g_display;
static Display *TheDisplay = &g_display;
struct WindowMgr { void winDrawImage(const Image *image, Int x0, Int y0, Int x1, Int y1) { g_display.drawImage(image, x0, y0, x1, y1); } };
static WindowMgr g_wmgr;
static WindowMgr *TheWindowManager = &g_wmgr;
struct ShellMenuSchemeManager { void draw() { lg(50); } };
struct Shell { bool active; ShellMenuSchemeManager mgr; Bool isShellActive() { return active; } ShellMenuSchemeManager *getShellMenuSchemeManager() { return &mgr; } };
static Shell g_shell;
static Shell *TheShell = &g_shell;

struct WinInstanceData { ICoord2D m_imageOffset; };
class GameWindow
{
public:
	IRegion2D m_region; ICoord2D m_size; GameWindow *m_parent; const Image *enabled0;
	Int winGetScreenPosition(Int *x, Int *y);
	void winGetSize(Int *x, Int *y) { *x = m_size.x; *y = m_size.y; }
	const Image *winGetEnabledImage(Int) { return enabled0; }
};

#include "mm_screenpos.inc"
#include "mm_statics.inc"
#include "mm_a.inc"
#include "mm_border.inc"
#include "mm_thin.inc"

static std::vector<unsigned> runCase(const unsigned *in)
{
	int p = 0;
	g_height = (Int)in[p++];
	g_shell.active = in[p++] != 0;
	unsigned nwin = in[p++];
	std::vector<GameWindow> wins(nwin);
	std::vector<WinInstanceData> insts(nwin);
	std::vector<Image> eimgs(nwin);
	for (unsigned i = 0; i < nwin; ++i) {
		unsigned par = in[p++];
		wins[i].m_parent = par ? &wins[par - 1] : NULL;
		wins[i].m_region.lo.x = (Int)in[p++]; wins[i].m_region.lo.y = (Int)in[p++];
		wins[i].m_size.x = (Int)in[p++]; wins[i].m_size.y = (Int)in[p++];
		wins[i].m_region.hi.x = wins[i].m_region.lo.x + wins[i].m_size.x; wins[i].m_region.hi.y = wins[i].m_region.lo.y + wins[i].m_size.y;
		unsigned tok = in[p++];
		eimgs[i].token = tok; eimgs[i].w = 1; eimgs[i].h = 1;
		wins[i].enabled0 = tok ? &eimgs[i] : NULL;
		insts[i].m_imageOffset.x = (Int)in[p++]; insts[i].m_imageOffset.y = (Int)in[p++];
	}
	unsigned nimg = in[p++];
	for (unsigned i = 0; i < nimg; ++i) { g_exists[i] = in[p++] != 0; g_images[i].token = 1000 + i; g_images[i].w = (Int)in[p++]; g_images[i].h = (Int)in[p++]; }
	unsigned ncalls = in[p++];
	std::vector<unsigned> all;
	for (unsigned c = 0; c < ncalls; ++c) {
		unsigned kind = in[p++], w = in[p++], clock = in[p++];
		g_now = clock;
		g_log.clear();
		switch (kind) {
		case 0: W3DMainMenuDraw(&wins[w], &insts[w]); break;
		case 1: W3DMainMenuFourDraw(&wins[w], &insts[w]); break;
		case 2: W3DMainMenuMapBorder(&wins[w], &insts[w]); break;
		case 3: W3DThinBorderDraw(&wins[w], &insts[w]); break;
		default: W3DShellMenuSchemeDraw(&wins[w], &insts[w]); break;
		}
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		all.push_back(100);
	}
	return all;
}

}

extern "C" int ref_ui3b_w3dmm_run(const unsigned *in, int n, unsigned *out, int cap)
{
	(void)n;
	int fd[2];
	if (pipe(fd) != 0) return -1;
	pid_t pid = fork();
	if (pid == 0) {
		close(fd[0]);
		std::vector<unsigned> all = ui3bmm::runCase(in);
		unsigned count = (unsigned)all.size();
		if (write(fd[1], &count, 4) != 4) _exit(1);
		size_t done = 0, total = all.size() * 4;
		const char *b = (const char *)(all.empty() ? NULL : &all[0]);
		while (done < total) { ssize_t k = write(fd[1], b + done, total - done); if (k <= 0) _exit(1); done += (size_t)k; }
		_exit(0);
	}
	close(fd[1]);
	unsigned count = 0;
	size_t got = 0;
	char *cb = (char *)&count;
	while (got < 4) { ssize_t k = read(fd[0], cb + got, 4 - got); if (k <= 0) break; got += (size_t)k; }
	int result = -1;
	if (got == 4 && (int)count <= cap) {
		size_t total = (size_t)count * 4; got = 0;
		char *ob = (char *)out;
		while (got < total) { ssize_t k = read(fd[0], ob + got, total - got); if (k <= 0) break; got += (size_t)k; }
		if (got == total) result = (int)count;
	}
	close(fd[0]);
	int status = 0;
	waitpid(pid, &status, 0);
	return result;
}
