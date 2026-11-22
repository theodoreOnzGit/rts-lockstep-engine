/* Differential-test wrapper for the small helpers of the big WOL menu drafts (#215, group wol7, upstream commit 0a05454d):
**   src/drafts/ui/wol_quick_match_menu.rs `next_token_space` (`data.nextToken(&idStr, " ")`) and the `atoi` of wol_lobby_menu.rs, wol_game_setup_menu.rs and
**   wol_quick_match_menu.rs (the C library's `atoi`: upstream calls it, it is not upstream code).
** `AsciiString::nextToken` is cut VERBATIM by build.rs (fn drafts_wol7) from AsciiString.cpp (`skipSeps`, `skipNonSeps`: small_skip.inc; `nextToken`: small_token.inc).
** Stand-in: AsciiString, the members the cut calls (`isEmpty`, `peek`, `getBufferForRead`, `clear`, `set(const char *)`), over a std::string that holds the text and the
** terminator the way upstream's buffer does. Excluded inputs: a NUL byte in the text (a C string), and for `atoi` a number of more than 18 digits (`strtol` saturates, the draft wraps).
** Entry points: ref_wol7_atoi(text) and ref_wol7_next_token(text, token_out, rest_out, cap) -> the return value (1 / 0). */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include "Lib/BaseType.h"

namespace wol7_small {
class AsciiString
{
public:
	std::vector<char> b; /* the text and its NUL */
	AsciiString() { b.push_back(0); }
	char *peek() { return &b[0]; }
	Bool isEmpty() const { return b[0] == 0; }
	void clear() { b.assign(1, 0); }
	void set(const char *s) { std::string t(s); b.assign(t.begin(), t.end()); b.push_back(0); }
	char *getBufferForRead(Int len) { b.assign(len, 0); return &b[0]; }
	Bool nextToken(AsciiString *tok, const char *seps = NULL);
};
#include "small_skip.inc"
#include "small_token.inc"
}

extern "C" int ref_wol7_atoi(const char *s) { return atoi(s); }
extern "C" int ref_wol7_next_token(const char *text, char *tok_out, char *rest_out, int cap)
{
	wol7_small::AsciiString data, tok;
	data.set(text);
	bool r = data.nextToken(&tok, " ");
	snprintf(tok_out, cap, "%s", tok.peek());
	snprintf(rest_out, cap, "%s", data.peek());
	return r ? 1 : 0;
}
