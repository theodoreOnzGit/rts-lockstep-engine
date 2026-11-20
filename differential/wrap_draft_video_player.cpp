/* Differential-test wrapper for the VideoPlayer draft (#215, group lc2, upstream commit 0a05454d), src/drafts/logic_common/video_player.rs:
**   GeneralsMD/Code/GameEngine/Source/GameClient/VideoPlayer.cpp: vp_a.inc = `VideoBuffer::VideoBuffer` to just before `VideoPlayer::init`
**   (the buffer's constructor, Rect and free, and VideoPlayer's constructor and destructor), vp_b.inc = `VideoPlayer::deinit` to just before
**   the field-parse table (deinit, reset, update, focus, open, load, firstStream, closeAllStreams, remove, addVideo, removeVideo,
**   getNumVideos, both getVideo, and every VideoStream member), cut verbatim by build.rs;
**   GeneralsMD/Code/GameEngine/Include/GameClient/VideoPlayer.h: vp_h.inc = from the `VecVideo` typedefs to `extern VideoPlayerInterface *TheVideoPlayer;`
**   (Video, VideoBuffer, VideoStreamInterface, VideoStream, VideoPlayerInterface, VideoPlayer), verbatim.
**
** Not cut: `VideoPlayer::init` (loads two INI files: the host's), the field-parse table (INI::parseAsciiString), the Bink player (not released).
**
** Stand-ins (outside the cuts, in an anonymous namespace): AsciiString is a std::string with `==` (what the cuts use of it: copy, `==`,
** and storing); RectClass has the four floats and `Set` of WWMath/rect.h (rect.h line 63); SubsystemInterface is an empty base with a
** virtual destructor (VideoPlayerInterface redeclares init/reset/update/deinit itself); FieldParse is an empty struct (only its address is
** returned); `TheVideoPlayer` is the global the destructor compares; INI is not needed by the cuts. `TestStream` derives from VideoStream
** (its constructor is protected) and `TestBuffer` from VideoBuffer (its virtuals are pure) only to arm the protected fields; neither adds
** behaviour, except that TestStream's destructor records that the stream was deleted (VideoStream::close is `delete this`).
**
** Entry points (extern "C", tests/differential_draft_video_player.rs): ref_lc2_vb_run (a buffer: constructor, Rect, free),
** ref_lc2_vs_defaults (a fresh stream's answers), ref_lc2_vp_run (a player, an arena of streams and a program of operations). */
#include <string.h>
#include <stdint.h>
#include <string>
#include <vector>

typedef float Real;
typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
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
	bool operator==(const AsciiString &o) const { return s == o.s; }
};

class RectClass
{
public:
	float Left, Top, Right, Bottom;
	RectClass(float l, float t, float r, float b) : Left(l), Top(t), Right(r), Bottom(b) {}
	void Set(float left, float top, float right, float bottom) { Left = left; Top = top; Right = right; Bottom = bottom; }
};

class SubsystemInterface { public: virtual ~SubsystemInterface() {} };
struct FieldParse { int unused; };

struct Video;
class VideoPlayer;
class VideoPlayerInterface;
class VideoBuffer;
extern VideoPlayerInterface *TheVideoPlayer;

#include "vp_h.inc"

VideoPlayerInterface *TheVideoPlayer = NULL;
const FieldParse VideoPlayer::m_videoFieldParseTable[] = { { 0 } };
void VideoPlayer::init( void ) {} /* stand-in: the key function of the vtable; the real one loads two INI files (host) and is not called */

#include "vp_a.inc"
#include "vp_b.inc"

class TestStream : public VideoStream
{
public:
	bool *aliveFlag;
	TestStream(bool *flag) : aliveFlag(flag) { *aliveFlag = true; }
	virtual ~TestStream() { *aliveFlag = false; }
	void arm(VideoPlayer *p, VideoStream *n) { m_player = p; m_next = n; }
	VideoStream *nextStream() { return m_next; }
};

/* the player's first stream is the protected m_firstStream (upstream's derived players set it when they open a stream) */
class ArmedPlayer : public VideoPlayer
{
public:
	void setFirst(VideoStream *s) { m_firstStream = s; }
};

class TestBuffer : public VideoBuffer
{
public:
	bool isValid;
	TestBuffer(Type t) : VideoBuffer(t), isValid(false) {}
	virtual Bool allocate(UnsignedInt, UnsignedInt) { return TRUE; }
	virtual void free(void) { VideoBuffer::free(); }
	virtual void *lock(void) { return NULL; }
	virtual void unlock(void) {}
	virtual Bool valid(void) { return isValid; }
	void set(UnsignedInt w, UnsignedInt h, UnsignedInt tw, UnsignedInt th, UnsignedInt pitch, UnsignedInt x, UnsignedInt y)
	{
		m_width = w; m_height = h; m_textureWidth = tw; m_textureHeight = th; m_pitch = pitch; m_xPos = x; m_yPos = y;
	}
	void log(unsigned int *out)
	{
		out[0] = (unsigned int)m_format; out[1] = m_xPos; out[2] = m_yPos; out[3] = m_width; out[4] = m_height;
		out[5] = m_textureWidth; out[6] = m_textureHeight; out[7] = m_pitch;
	}
};

const char *const NAMES[8] = { "", "a", "A", "ab", "b", "movie1", "Movie1", "x" };
const char *const FILES[6] = { "", "f0.bik", "f1.bik", "F0.bik", "f2", "g" };
const char *const COMMENTS[4] = { "", "c0", "c1", "C0" };

int idOf(const AsciiString &a, const char *const *table, int n)
{
	for (int i = 0; i < n; ++i)
		if (a.s == table[i]) return i;
	return -1;
}

void logVideo(std::vector<int> &log, const Video *v)
{
	if (v == NULL) { log.push_back(-1); log.push_back(-1); log.push_back(-1); return; }
	log.push_back(idOf(v->m_internalName, NAMES, 8));
	log.push_back(idOf(v->m_filename, FILES, 6));
	log.push_back(idOf(v->m_commentForWB, COMMENTS, 4));
}

}

extern "C" {

/* out: after the constructor: format, xPos, yPos, width, height, textureWidth, textureHeight, pitch (8 words); then Rect's four floats' bits;
** then after free(): the same eight words. */
int ref_lc2_vb_run(int fmt, unsigned w, unsigned h, unsigned tw, unsigned th, unsigned pitch, unsigned x, unsigned y, int valid,
                   float x1, float y1, float x2, float y2, unsigned int *out)
{
	TestBuffer b((VideoBuffer::Type)fmt);
	b.log(out);
	b.set(w, h, tw, th, pitch, x, y);
	b.isValid = valid != 0;
	RectClass r = b.Rect(x1, y1, x2, y2);
	memcpy(out + 8, &r.Left, 4);
	memcpy(out + 9, &r.Top, 4);
	memcpy(out + 10, &r.Right, 4);
	memcpy(out + 11, &r.Bottom, 4);
	b.free();
	b.log(out + 12);
	return 20;
}

/* out: next is NULL, isFrameReady, frameIndex, frameCount, height, width, after calling update, frameDecompress, frameNext, frameGoto(3) (which do nothing). */
int ref_lc2_vs_defaults(int *out)
{
	bool alive = false;
	TestStream *s = new TestStream(&alive);
	s->update(); s->frameDecompress(); s->frameNext(); s->frameGoto(3);
	out[0] = s->next() == NULL;
	out[1] = s->isFrameReady();
	out[2] = s->frameIndex();
	out[3] = s->frameCount();
	out[4] = s->height();
	out[5] = s->width();
	s->close();
	out[6] = alive;
	return 7;
}

/* prog: [nstreams, hasPlayer x nstreams, chainLength, chain indices..., ops...] with each op 4 words [kind, a, b, c]:
**   0 addVideo(name a, file b, comment c)   1 removeVideo(name a)   2 remove(stream a)   3 update   4 close(stream a)   5 closeAllStreams
**   6 reset   7 getVideo(name a)   8 getVideo(index a)
** The arena streams are linked head to tail along the chain, the first stream is chain[0]. After each op: the first stream's index (-1 NULL),
** for each stream alive (0/1) and its next's index (-1 NULL, -2 not alive), the video count, each video (internal name, file, comment ids),
** and for the getVideo ops the answer (3 words, -1s for NULL). Returns the number of words written. */
int ref_lc2_vp_run(const int *prog, int nprog, int *out)
{
	int p = 0;
	int n = prog[p++];
	ArmedPlayer *player = new ArmedPlayer;
	bool alive[16];
	TestStream *ts[16];
	std::vector<int> hasPlayer;
	for (int i = 0; i < n; ++i) hasPlayer.push_back(prog[p++]);
	for (int i = 0; i < n; ++i) ts[i] = new TestStream(&alive[i]);
	int chainLen = prog[p++];
	std::vector<int> chain;
	for (int i = 0; i < chainLen; ++i) chain.push_back(prog[p++]);
	for (int i = 0; i < n; ++i) ts[i]->arm(hasPlayer[i] ? player : NULL, NULL);
	for (int i = 0; i + 1 < chainLen; ++i) ts[chain[i]]->arm(hasPlayer[chain[i]] ? player : NULL, ts[chain[i + 1]]);
	player->setFirst(chainLen > 0 ? ts[chain[0]] : NULL);

	std::vector<int> log;
	while (p + 4 <= nprog)
	{
		int kind = prog[p], a = prog[p + 1], b = prog[p + 2], c = prog[p + 3];
		p += 4;
		int ans[3] = { 0, 0, 0 };
		bool answered = false;
		switch (kind)
		{
		case 0: { Video v; v.m_internalName = NAMES[a]; v.m_filename = FILES[b]; v.m_commentForWB = COMMENTS[c]; player->addVideo(&v); break; }
		case 1: { Video v; v.m_internalName = NAMES[a]; v.m_filename = "ignored"; player->removeVideo(&v); break; }
		case 2: player->remove(ts[a]); break;
		case 3: player->update(); break;
		case 4: ts[a]->close(); break;
		case 5: player->closeAllStreams(); break;
		case 6: player->reset(); break;
		case 7: { AsciiString t(NAMES[a]); const Video *v = player->getVideo(t); std::vector<int> l; logVideo(l, v); ans[0] = l[0]; ans[1] = l[1]; ans[2] = l[2]; answered = true; break; }
		case 8: { const Video *v = player->getVideo(a); std::vector<int> l; logVideo(l, v); ans[0] = l[0]; ans[1] = l[1]; ans[2] = l[2]; answered = true; break; }
		}
		VideoStream *first = player->firstStream() ? static_cast<VideoStream *>(player->firstStream()) : NULL;
		int firstIdx = -1;
		for (int i = 0; i < n; ++i) if ((uintptr_t)static_cast<VideoStream *>(ts[i]) == (uintptr_t)first) firstIdx = i;
		log.push_back(first == NULL ? -1 : firstIdx);
		for (int i = 0; i < n; ++i)
		{
			log.push_back(alive[i] ? 1 : 0);
			if (!alive[i]) { log.push_back(-2); continue; }
			VideoStream *nx = ts[i]->nextStream();
			int idx = -1;
			for (int j = 0; j < n; ++j) if ((uintptr_t)static_cast<VideoStream *>(ts[j]) == (uintptr_t)nx) idx = j; /* a freed stream's address still names it */
			log.push_back(nx == NULL ? -1 : idx);
		}
		log.push_back(player->getNumVideos());
		for (int i = 0; i < player->getNumVideos(); ++i) logVideo(log, player->getVideo(i));
		if (answered) { log.push_back(ans[0]); log.push_back(ans[1]); log.push_back(ans[2]); }
	}
	for (size_t i = 0; i < log.size(); ++i) out[i] = log[i];
	/* the remaining live streams are leaked on purpose (they may be dangling in the list); the player is leaked too */
	return (int)log.size();
}

}
