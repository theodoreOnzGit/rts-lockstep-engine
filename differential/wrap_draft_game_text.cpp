/* Differential-test wrapper for the draft src/drafts/sweep_common_client/game_text.rs (#215, group ui2, upstream
** commit 0a05454d), cut verbatim by build.rs (fn drafts_ui2) into OUT_DIR/draft_ui2_gen:
**   u2_game_text.inc   GameEngine/Source/GameClient/GameText.cpp, GameTextManager::removeLeadingAndTrailing (to
**                      readToEndOfQuote's comment banner).
** No edit. The stand-in: a GameTextManager class declaring the one member the cut defines (the real class holds the
** string tables). `iswspace` is the C library's (glibc, a C-locale test binary).
**
** Entry point (extern "C", tests/differential_draft_game_text.rs):
**   ref_u2_gt_trim(in, n, out, cap): the n chars in `in` (each 0..127, 0 allowed) are copied into a NUL-terminated
**   buffer, removeLeadingAndTrailing runs on it, and the resulting C string (up to the first NUL) is written to out;
**   returns its length. */
#include <wctype.h>
#include <string.h>
#include <vector>
#include "Lib/BaseType.h"

namespace u2gt {
class GameTextManager
{
public:
	void removeLeadingAndTrailing ( Char *m_buffer );
};
#include "u2_game_text.inc"
}

extern "C" int ref_u2_gt_trim(const int *in, int n, int *out, int cap)
{
	std::vector<char> b(n + 1);
	for (int i = 0; i < n; i++) b[i] = (char)in[i];
	b[n] = 0;
	u2gt::GameTextManager m;
	m.removeLeadingAndTrailing(b.data());
	int len = (int)strlen(b.data());
	for (int i = 0; i < len && i < cap; i++) out[i] = (unsigned char)b[i];
	return len;
}
