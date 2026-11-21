/* Differential-test wrapper for the window video manager (#215, group ui3b, upstream commit 0a05454d): src/drafts/ui/window_video_manager.rs.
** Cut verbatim by build.rs (drafts_ui3b), no edit:
**   wvm_enums.inc     GameClient/WindowVideoManager.h: the enums WindowVideoPlayType and WindowVideoStates;
**   wvm_class.inc     the same header's `class WindowVideo`, and wvm_inline.inc its inline accessors;
**   wvm_cpp.inc       GameClient/GUI/WindowVideoManager.cpp, from `WindowVideo::WindowVideo( void )` to the end (the WindowVideo and WindowVideoManager members).
** Stand-ins (the mini window manager of differential/shim_ui3b/ui3b_wm.h: windows in a tree, `winIsHidden` the window's OWN bit, and WinInstanceData::setVideoBuffer, logged
**   [30, window id, buffer token or 0]):
**   class WindowVideoManager: the header's class, but m_playingVideos is a std::map keyed by the window pointer, in ascending pointer order (upstream's is a std::hash_map
**     on the pointer, whose iteration order is unspecified; the Rust draft iterates in ascending WinId, which is the window's index in the scene's array here) and the accessors the test reads;
**   AsciiString (a movie index, with clear()), VideoStreamInterface (a token; a length; a ready-mask consumed one bit per isFrameReady call [1, stream]: frameDecompress,
**     frameRender(buffer) and frameNext are logged [2, stream], [3, stream, buffer], [4, stream]; frameIndex() is the frame, 0 after the last; close() logs [21, stream]),
**   VideoBuffer (a token; allocate(w, h) answers the movie's script and logs [12, buffer]; its destructor, `delete`, logs [20, buffer]), TheVideoPlayer->open(name)
**   (logs [10, movie]; NULL when the movie's script says so), TheDisplay->createVideoBuffer() (logs [11]; NULL when the script says so).
** Entry point ref_ui3b_wvm_run(in, n, out, cap): `in` = [tree scene block, nmovies, nmovies x [open ok, create buffer ok, allocate ok, length, ready mask], nsteps,
** nsteps x [kind, a, b, c]]: kinds 0 playMovie(window a, movie b, play type c), 1 pauseMovie(a), 2 hideMovie(a), 3 resumeMovie(a), 4 stopMovie(a), 5 stopAndRemoveMovie(a),
** 6 stopAllMovies, 7 pauseAllMovies, 8 resumeAllMovies, 9 update, 10 reset, 11 init, 12 the window a hidden (b = 1) or shown, 13 getWinState(a).
** Output per step: the calls logged, then [100, the scene's dump, every window's getWinState, the number of playing videos, m_stopAllMovies, m_pauseAllMovies]. */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>
#include <functional>
#include "Lib/BaseType.h"

namespace ui3bwvm {

#include "ui3b_msgs.inc"
#include "ui3b_gwm.inc"
#include "ui3b_status.inc"
#include "ui3b_winerr.inc"
#include "ui3b_wm.h"
#include "wvm_enums.inc"

#define NEW new
#define DEBUG_CRASH(x)

struct AsciiString { Int idx; void clear() { idx = -1; } };
struct MovieScript { bool openOk, bufOk, allocOk; unsigned length, readyMask; };
static std::vector<MovieScript> g_movies;
static unsigned g_nextStream = 1, g_nextBuffer = 1;
static Int g_openingMovie = 0;

struct VideoBuffer
{
	unsigned token;
	VideoBuffer() : token(g_nextBuffer++) {}
	~VideoBuffer() { lg(20); lg(token); }
	bool allocate(Int, Int) { lg(12); lg(token); return g_movies[g_openingMovie].allocOk; }
};
struct VideoStreamInterface
{
	unsigned token, length, frame, readyMask, calls;
	Int width() { return 4; }
	Int height() { return 3; }
	Bool isFrameReady() { lg(1); lg(token); bool r = (readyMask >> (calls % 32)) & 1; ++calls; return r; }
	void frameDecompress() { lg(2); lg(token); }
	void frameRender(VideoBuffer *b) { lg(3); lg(token); lg(b->token); }
	void frameNext() { lg(4); lg(token); frame = (frame + 1) % length; }
	Int frameIndex() { return (Int)frame; }
	void close() { lg(21); lg(token); }
};
struct VideoPlayer
{
	VideoStreamInterface *open(AsciiString name)
	{
		lg(10); lg((unsigned)name.idx);
		g_openingMovie = name.idx;
		const MovieScript &m = g_movies[name.idx];
		if (!m.openOk) return NULL;
		VideoStreamInterface *s = new VideoStreamInterface;
		s->token = g_nextStream++; s->length = m.length; s->frame = 0; s->readyMask = m.readyMask; s->calls = 0;
		return s;
	}
};
static VideoPlayer g_vp;
static VideoPlayer *TheVideoPlayer = &g_vp;
struct Display { VideoBuffer *createVideoBuffer() { lg(11); return g_movies[g_openingMovie].bufOk ? new VideoBuffer : NULL; } };
static Display g_disp;
static Display *TheDisplay = &g_disp;

#include "wvm_class.inc"

class WindowVideoManager
{
public:
	WindowVideoManager(void);
	~WindowVideoManager(void);
	void init(void);
	void reset(void);
	void update(void);
	void playMovie(GameWindow *win, AsciiString movieName, WindowVideoPlayType playType);
	void hideMovie(GameWindow *win);
	void pauseMovie(GameWindow *win);
	void resumeMovie(GameWindow *win);
	void stopMovie(GameWindow *win);
	void stopAndRemoveMovie(GameWindow *win);
	void stopAllMovies(void);
	void pauseAllMovies(void);
	void resumeAllMovies(void);
	Int getWinState(GameWindow *win);
	size_t count() const { return m_playingVideos.size(); }
	bool stopAll() const { return m_stopAllMovies; }
	bool pauseAll() const { return m_pauseAllMovies; }
private:
	typedef std::map<const GameWindow *, WindowVideo *> WindowVideoMap;
	WindowVideoMap m_playingVideos;
	Bool m_stopAllMovies;
	Bool m_pauseAllMovies;
};

#include "wvm_inline.inc"
#include "wvm_cpp.inc"

}

extern "C" int ref_ui3b_wvm_run(const unsigned *in, int n, unsigned *out, int cap)
{
	using namespace ui3bwvm;
	(void)n;
	int p = readScene(in, 0);
	unsigned nm = in[p++];
	g_movies.assign(nm, MovieScript());
	for (unsigned i = 0; i < nm; ++i) {
		g_movies[i].openOk = in[p++] != 0; g_movies[i].bufOk = in[p++] != 0; g_movies[i].allocOk = in[p++] != 0;
		g_movies[i].length = in[p++]; g_movies[i].readyMask = in[p++];
	}
	g_nextStream = 1; g_nextBuffer = 1;
	WindowVideoManager *mgr = new WindowVideoManager;
	unsigned nsteps = in[p++];
	std::vector<unsigned> all;
	for (unsigned s = 0; s < nsteps; ++s) {
		unsigned kind = in[p++], a = in[p++], b = in[p++], c = in[p++];
		g_log.clear();
		GameWindow *w = &g_wins[a % g_wins.size()];
		switch (kind) {
		case 0: { AsciiString m; m.idx = (Int)b; mgr->playMovie(w, m, (WindowVideoPlayType)c); break; }
		case 1: mgr->pauseMovie(w); break;
		case 2: mgr->hideMovie(w); break;
		case 3: mgr->resumeMovie(w); break;
		case 4: mgr->stopMovie(w); break;
		case 5: mgr->stopAndRemoveMovie(w); break;
		case 6: mgr->stopAllMovies(); break;
		case 7: mgr->pauseAllMovies(); break;
		case 8: mgr->resumeAllMovies(); break;
		case 9: mgr->update(); break;
		case 10: mgr->reset(); break;
		case 11: mgr->init(); break;
		case 12: if (b) w->m_status |= WIN_STATUS_HIDDEN; else w->m_status &= ~(unsigned)WIN_STATUS_HIDDEN; break;
		default: break;
		}
		unsigned st13 = (unsigned)mgr->getWinState(w);
		for (size_t i = 0; i < g_log.size(); ++i) all.push_back(g_log[i]);
		if (kind == 13) all.push_back(st13);
		all.push_back(100);
		dumpScene(all);
		for (size_t i = 0; i < g_wins.size(); ++i) all.push_back((unsigned)mgr->getWinState(&g_wins[i]));
		all.push_back((unsigned)mgr->count()); all.push_back(mgr->stopAll() ? 1 : 0); all.push_back(mgr->pauseAll() ? 1 : 0);
	}
	delete mgr;
	if ((int)all.size() > cap) return -1;
	memcpy(out, &all[0], all.size() * sizeof(unsigned));
	return (int)all.size();
}
