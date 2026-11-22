/* Differential-test wrapper for the WOL lobby's small rank image lookup (#215, group wol6, upstream commit 0a05454d):
** src/drafts/ui/wol_lobby_menu.rs `lookup_small_rank_image` (with `rank_for_points`). Cut verbatim by build.rs (fn drafts_wol6) from
** WOLLobbyMenu.cpp: lobby_rank.inc, `rankNames[]` and `LookupSmallRankImage` (to `insertPlayerInListbox`).
** Stand-ins: AsciiString (`format`, `operator=(const char *)`, `str`; the shim's text is vsnprintf-based like upstream's), `Image` (a name),
** TheRankPointValues->m_ranks (the case's table: index 0 is 0, then ascending, ending in INT_MAX as a sentinel the unbounded `while` stops at),
** TheMappedImageCollection->findImageByName (logs `find <name>` and returns the image if the case has one by that name), DEBUG_ASSERTLOG compiled out
** (release). Excluded inputs (upstream reads past the table): `rankPoints == INT_MAX`.
** Entry point ref_wol6_rank_run(in, out, cap): a case is `<side> <rankPoints> <n images> <names...>` then 14 table values; the output is the stand-in calls
** and `img <name|none>`. */
#include <stdint.h>
#include <stdio.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"
#define DEBUG_ASSERTLOG(x, y)

namespace wol6_rank {
static std::string w6log;
class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	AsciiString &operator=(const char *c) { std::string t(c); s = t; return *this; }
	const char *str() const { return s.c_str(); }
	void format(const char *fmt, ...)
	{
		char b[2048];
		va_list a;
		va_start(a, fmt);
		vsnprintf(b, sizeof b, fmt, a);
		va_end(a);
		s = b;
	}
};
struct Image { std::string name; };
static std::vector<Image> w6_images;
struct RankPointValue { Int m_ranks[14]; };
static RankPointValue w6_values;
static RankPointValue *TheRankPointValues = &w6_values;
class MappedImageCollection
{
public:
	const Image *findImageByName(AsciiString n)
	{
		w6log += "find " + n.s + "\n";
		for (size_t i = 0; i < w6_images.size(); ++i) if (w6_images[i].name == n.s) return &w6_images[i];
		return NULL;
	}
};
static MappedImageCollection w6_mic;
static MappedImageCollection *TheMappedImageCollection = &w6_mic;
#include "lobby_rank.inc"
static std::string run(const char *in)
{
	w6log.clear();
	w6_images.clear();
	int side, rp, n, used = 0;
	sscanf(in, "%d %d %d%n", &side, &rp, &n, &used);
	in += used;
	for (int i = 0; i < n; ++i)
	{
		char nm[128];
		sscanf(in, " %127s%n", nm, &used);
		in += used;
		Image im;
		im.name = nm;
		w6_images.push_back(im);
	}
	for (int i = 0; i < 14; ++i)
	{
		sscanf(in, " %d%n", &w6_values.m_ranks[i], &used);
		in += used;
	}
	const Image *img = LookupSmallRankImage(side, rp);
	w6log += std::string("img ") + (img ? img->name : "none") + "\n";
	return w6log;
}
}

extern "C" int ref_wol6_rank_run(const char *in, char *out, int cap)
{
	std::string r = wol6_rank::run(in);
	int len = (int)r.size() < cap - 1 ? (int)r.size() : cap - 1;
	memcpy(out, r.data(), len);
	out[len] = 0;
	return len;
}
