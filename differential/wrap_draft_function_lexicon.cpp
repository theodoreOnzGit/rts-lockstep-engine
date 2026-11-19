/* Differential-test wrapper for the draft src/drafts/sweep_common_client/function_lexicon.rs (#215, group ui2, upstream
** commit 0a05454d), cut verbatim by build.rs (fn drafts_ui2) into OUT_DIR/draft_ui2_gen:
**   u2_fl_h.inc    GameEngine/Include/Common/FunctionLexicon.h, `struct TableEntry` to `FunctionLexicon( void );`
**                  (TableEntry, TableIndex), inside the stand-in class;
**   u2_fl_a.inc    GameEngine/Source/Common/System/FunctionLexicon.cpp, loadTable, keyToFunc, findFunction (to the
**                  `#ifdef NOT_IN_USE` of funcToName);
**   u2_fl_b.inc    the same file, validate to the end (validate, gameWinDrawFunc, winLayoutInitFunc).
** No edit. Stand-ins: the FunctionLexicon class declaring the members the cuts define (the real one is a
** SubsystemInterface with inline typed lookups); NameKeyType and NAMEKEY_INVALID as NameKeyGenerator.h has them;
** TheNameKeyGenerator (nameToKey: keys 1, 2, 3 ... in order of first sight, EXCEPT the name "n11", which gets
** NAMEKEY_INVALID, so that the tests exercise the table walks' stop at a key of NAMEKEY_INVALID, which upstream's
** terminator entry { NAMEKEY_INVALID, NULL, NULL } relies on; the real generator never returns it); AsciiString (a
** std::string holder); GameWinDrawFunc and WindowLayoutInitFunc as `void *` (their real types are function pointers
** the cuts only cast); DEBUG_LOG as nothing (it is compiled out of the release build). The tables are arrays of
** TableEntry with upstream's terminator, one per index. A function is an integer (0 = NULL) stored as `void *`.
**
** Entry point (extern "C", tests/differential_draft_function_lexicon.rs):
**   ref_u2_fl_run(in, n, out, cap) runs one sequence of operations (a fresh lexicon, every table NULL), `in` is steps:
**     [0, index, count, (name id, func) x count]  loadTable        -> the count keys the entries were given
**     [1, key, index]   findFunction(key, index)   -> the function    [3, key, index] gameWinDrawFunc
**     [4, key, index]   winLayoutInitFunc          -> the function    [5, key, index] keyToFunc(key, m_tables[index])
**     [2]               validate()                 -> 0 or 1
**   and writes the answers in order. Returns the number of words written. */
#include <string.h>
#include <stdint.h>
#include <string>
#include <map>
#include <vector>
#include "Lib/BaseType.h"

namespace u2fl {

enum NameKeyType { NAMEKEY_INVALID = 0, FORCE_NAMEKEYTYPE_LONG = 0x7fffffff };
struct AsciiString { std::string s; AsciiString(const char *c) : s(c) {} };
static const char *const NAMES[12] = { "n0","n1","n2","n3","n4","n5","n6","n7","n8","n9","n10","n11" };
struct NameKeyGenerator
{
	std::map<std::string, int> m;
	NameKeyType nameToKey(const AsciiString &name)
	{
		if (name.s == "n11") return NAMEKEY_INVALID;
		std::map<std::string, int>::iterator it = m.find(name.s);
		if (it != m.end()) return (NameKeyType)it->second;
		int k = (int)m.size() + 1;
		m[name.s] = k;
		return (NameKeyType)k;
	}
};
static NameKeyGenerator g_gen;
static NameKeyGenerator *TheNameKeyGenerator = &g_gen;

typedef void *GameWinDrawFunc;
typedef void *WindowLayoutInitFunc;
#define DEBUG_LOG(x)

class FunctionLexicon
{
public:
#include "u2_fl_h.inc"
	FunctionLexicon( void ) { for (int i = 0; i < MAX_FUNCTION_TABLES; i++) m_tables[i] = NULL; }
	Bool validate( void );
	GameWinDrawFunc gameWinDrawFunc( NameKeyType key, TableIndex index );
	WindowLayoutInitFunc winLayoutInitFunc( NameKeyType key, TableIndex index );
	void loadTable( TableEntry *table, TableIndex tableIndex );
	void *findFunction( NameKeyType key, TableIndex index );
	void *keyToFunc( NameKeyType key, TableEntry *table );
	TableEntry *m_tables[ MAX_FUNCTION_TABLES ];
};

#include "u2_fl_a.inc"
#include "u2_fl_b.inc"
}

extern "C" int ref_u2_fl_run(const int *in, int n, int *out, int cap)
{
	using namespace u2fl;
	g_gen.m.clear();
	FunctionLexicon lex;
	std::vector<FunctionLexicon::TableEntry> store[FunctionLexicon::MAX_FUNCTION_TABLES];
	int o = 0, i = 0;
	#define PUSH(v) do { if (o < cap) out[o] = (v); o++; } while (0)
	while (i < n) {
		int kind = in[i++];
		if (kind == 0) {
			int index = in[i++], count = in[i++];
			std::vector<FunctionLexicon::TableEntry> v;
			for (int k = 0; k < count; k++) {
				FunctionLexicon::TableEntry e;
				e.key = NAMEKEY_INVALID;
				e.name = NAMES[in[i++]];
				e.func = (void *)(intptr_t)in[i++];
				v.push_back(e);
			}
			FunctionLexicon::TableEntry t; t.key = NAMEKEY_INVALID; t.name = NULL; t.func = NULL;
			v.push_back(t);
			store[index] = v;
			lex.loadTable(store[index].data(), (FunctionLexicon::TableIndex)index);
			for (int k = 0; k < count; k++) PUSH((int)store[index][k].key);
		} else if (kind == 2) {
			PUSH(lex.validate() ? 1 : 0);
		} else {
			int key = in[i++], index = in[i++];
			void *f = NULL;
			if (kind == 1) f = lex.findFunction((NameKeyType)key, (FunctionLexicon::TableIndex)index);
			else if (kind == 3) f = lex.gameWinDrawFunc((NameKeyType)key, (FunctionLexicon::TableIndex)index);
			else if (kind == 4) f = lex.winLayoutInitFunc((NameKeyType)key, (FunctionLexicon::TableIndex)index);
			else f = lex.keyToFunc((NameKeyType)key, lex.m_tables[index]);
			PUSH((int)(intptr_t)f);
		}
	}
	return o;
}
