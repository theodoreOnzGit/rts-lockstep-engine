/* Differential-test wrapper for the display draft (#215, group lc3, upstream commit 0a05454d), src/drafts/sweep_common_client/display.rs. Cut verbatim by build.rs:
**   display_views.inc  Display.cpp  deleteViews, attachView, drawViews, updateViews (to `/// Redraw the entire display`);
**   display_stop.inc   Display.cpp  stopMovie (the draft has no stopMovie: the test's closure mirrors this cut);
**   display_logo.inc   Display.cpp  playLogoMovie (to the playMovie banner);
**   display_dbg.inc    Display.cpp  setDebugDisplayCallback and getDebugDisplayCallback (to the end of the file);
**   view_prepend.inc   View.cpp     View::prependViewToList.
**
** Stand-ins (outside the cuts): AsciiString (std::string with isEmpty, assignment, TheEmptyString); class View (m_next, getNextView, a draw/update/delete that log
** to a list, virtual destructor); class Display with only the members the cuts use; TheVideoPlayer->open (a stream or NULL by the case), the stream's
** close/width/height; createVideoBuffer (a buffer whose allocate always succeeds: the draft leaves the buffer out and a failed allocation, which
** upstream answers with stopMovie, is excluded from the test); TheDisplayStringManager->freeDisplayString (nothing); timeGetTime (the case's time).
** Display's constructor is not cut: the members start at 0 / NULL / empty, as DisplayViews::default() does.
**
** Entry point ref_lc3_display_run(prog, nprog, out). Ops (4 words [kind, a, b, c]): 0 attachView(new View(a)); 1 deleteViews; 2 drawViews; 3 updateViews;
** 4 playLogoMovie("m<a>", b, c) with words 4 and 5 = [openOk, time]; 5 stopMovie;
** 6 setDebugDisplayCallback(a, b) (0 is NULL); 7 getDebugDisplayCallback (appended to the output). Every op is 6 words.
** After each op: the log (count, entries: draw id, 1000+id update, 2000+id delete), the list ids head first (count, ids), hasStream, movie number (-1 empty),
** movieHoldTime, copyrightHoldTime, elapsedMovieTime, callback, user data, and the answer of kind 7 (else 0). */
#include <string>
#include <vector>
#include <stdlib.h>
#include <stdio.h>

typedef int Int;
typedef bool Bool;
typedef unsigned int UnsignedInt;
#define TRUE true
#define FALSE false
#define NULL 0

namespace {

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	Bool isEmpty() const { return s.empty(); }
	const char *str() const { return s.c_str(); }
	static AsciiString TheEmptyString;
};
AsciiString AsciiString::TheEmptyString;

std::vector<int> g_log;
int g_open_ok = 0;
unsigned g_time = 0;

class View
{
public:
	int id;
	View *m_next;
	View(int i) : id(i), m_next(NULL) {}
	virtual ~View() { g_log.push_back(2000 + id); }
	View *getNextView() { return m_next; }
	View *prependViewToList(View *list);
	void drawView() { g_log.push_back(id); }
	void updateView() { g_log.push_back(1000 + id); }
};

class VideoBuffer { public: bool allocate(int, int) { return true; } };
class VideoStream { public: void close() {} int width() { return 1; } int height() { return 1; } };
VideoStream g_stream;
struct VideoPlayerStandIn { VideoStream *open(AsciiString) { return g_open_ok ? &g_stream : NULL; } } g_vp;
VideoPlayerStandIn *TheVideoPlayer = &g_vp;
struct DisplayStringManagerStandIn { void freeDisplayString(void *) {} } g_dsm;
DisplayStringManagerStandIn *TheDisplayStringManager = &g_dsm;
unsigned timeGetTime() { return g_time; }

class Display
{
public:
	typedef void DebugDisplayCallback(int, void *, int, int, int, int, int);
	View *m_viewList;
	VideoBuffer *m_videoBuffer;
	VideoStream *m_videoStream;
	AsciiString m_currentlyPlayingMovie;
	void *m_copyrightDisplayString;
	Int m_copyrightHoldTime;
	Int m_movieHoldTime;
	UnsignedInt m_elapsedMovieTime;
	DebugDisplayCallback *m_debugDisplayCallback;
	void *m_debugDisplayUserData;
	Display() : m_viewList(NULL), m_videoBuffer(NULL), m_videoStream(NULL), m_copyrightDisplayString(NULL), m_copyrightHoldTime(0), m_movieHoldTime(0), m_elapsedMovieTime(0), m_debugDisplayCallback(NULL), m_debugDisplayUserData(NULL) {}
	VideoBuffer *createVideoBuffer() { return new VideoBuffer; }
	void deleteViews(void);
	void attachView(View *view);
	void drawViews(void);
	void updateViews(void);
	void stopMovie(void);
	void playLogoMovie(AsciiString movieName, Int minMovieLength, Int minCopyrightLength);
	void setDebugDisplayCallback(DebugDisplayCallback *callback, void *userData);
	DebugDisplayCallback *getDebugDisplayCallback();
};

#include "view_prepend.inc"
#include "display_views.inc"
#include "display_stop.inc"
#include "display_logo.inc"
#include "display_dbg.inc"

}

extern "C" int ref_lc3_display_run(const int *p, int np, int *out)
{
	Display d;
	int n = 0;
	for (int k = 0; k + 6 <= np; k += 6) {
		int kind = p[k], a = p[k + 1], b = p[k + 2], c = p[k + 3];
		g_log.clear();
		int ans = 0;
		switch (kind) {
		case 0: d.attachView(new View(a)); break;
		case 1: d.deleteViews(); break;
		case 2: d.drawViews(); break;
		case 3: d.updateViews(); break;
		case 4: {
			g_open_ok = p[k + 4];
			g_time = (unsigned)p[k + 5];
			char name[32];
			sprintf(name, "m%d", a);
			d.playLogoMovie(AsciiString(name), b, c);
			break;
		}
		case 5: d.stopMovie(); break;
		case 6: d.setDebugDisplayCallback((Display::DebugDisplayCallback *)(long)a, (void *)(long)b); break;
		case 7: ans = (int)(long)d.getDebugDisplayCallback(); break;
		}
		out[n++] = (int)g_log.size();
		for (size_t i = 0; i < g_log.size(); ++i) out[n++] = g_log[i];
		int cnt = 0;
		for (View *v = d.m_viewList; v; v = v->getNextView()) ++cnt;
		out[n++] = cnt;
		for (View *v = d.m_viewList; v; v = v->getNextView()) out[n++] = v->id;
		out[n++] = d.m_videoStream != NULL;
		out[n++] = d.m_currentlyPlayingMovie.isEmpty() ? -1 : atoi(d.m_currentlyPlayingMovie.str() + 1);
		out[n++] = d.m_movieHoldTime;
		out[n++] = d.m_copyrightHoldTime;
		out[n++] = (int)d.m_elapsedMovieTime;
		out[n++] = (int)(long)d.m_debugDisplayCallback;
		out[n++] = (int)(long)d.m_debugDisplayUserData;
		out[n++] = ans;
	}
	return n;
}
