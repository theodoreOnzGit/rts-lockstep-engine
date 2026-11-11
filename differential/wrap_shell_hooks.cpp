/* Differential-test wrapper for the shell's script hooks (GitHub #142, #140, #139; upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/shell_hooks_gen:
**   sh_enum.inc    GameEngine/Include/GameClient/ShellHooks.h, the anonymous enum (`enum \n{` to before
**                  `extern char *TheShellHookNames[];`);
**   sh_names.inc   GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp, `char *TheShellHookNames[]=` and
**                  `void SignalUIInteraction(Int interaction)` (to the Surfaces comment);
**   sh_signal.inc  GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp, ScriptEngine::signalUIInteract (to the
**                  next function's comment);
**   sh_alloc.inc   the same file, ScriptEngine::allocateFlag (to findGroup's comment);
**   sh_flag.inc    the same file, ScriptEngine::evaluateFlag (to setFlag's comment);
**   sh_clear.inc   the same file, ScriptEngine::update's two lines that clear m_uiInteractions (the comment and the
**                  call), included as the body of a stand-in method.
** No edit. Everything is inside namespace shx, so this file's TheShellHookNames, SignalUIInteraction and
** TheScriptEngine do not meet the whole Scripts.cpp another wrapper compiles.
** The stand-ins: a ScriptEngine declaring the members the cuts use (ScriptEngine.h's names and types: m_uiInteractions,
** m_flags[MAX_FLAGS], m_numFlags); TFlag as ScriptEngine.h has it; a Condition (type FLAG, two Parameters) and a
** Parameter (its Int and string, friend_setInt) with the accessors evaluateFlag calls. DEBUG_LOGGING is not defined (a
** release build), so signalUIInteract's AppendDebugMessage is compiled out, as in the shipped game.
**
** Entry point (extern "C", tests/differential_shell_hooks.rs):
**   ref_shell_hooks_case(kind, in, nin, out, cap) -> words written.
**   kind 0: the enum's 30 values in ShellHooks.h's order (SHELL_SCRIPT_HOOK_TOTAL last), named here one by one.
**   kind 1: TheShellHookNames[0..SHELL_SCRIPT_HOOK_TOTAL), each name's bytes then a 0.
**   kind 2: a sequence. A fresh engine (m_numFlags 1, the flags cleared, the list empty) and 58 conditions, FLAG on
**           TheShellHookNames[j] with value v (condition 2j+v), their parameters' cached index 0. Steps of 2 words:
**             [0, i]  SignalUIInteraction(i)
**             [1, 0]  the frame ends: [10, count, names...] the list from the front, each name's bytes then a 0
**             [2, 0]  ScriptEngine::update's clear (sh_clear.inc)
**             [3, k]  evaluateFlag(condition k): [20, result, m_numFlags]
*/
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/AsciiString.h"

namespace shx {

std::vector<int> g_out;

typedef std::list<AsciiString> ListAsciiString;
typedef ListAsciiString::iterator ListAsciiStringIt;

#include "sh_enum.inc"

class Parameter
{
public:
	Int m_int;
	AsciiString m_string;
	Int getInt( void ) const { return m_int; }
	const AsciiString& getString( void ) const { return m_string; }
	void friend_setInt( Int i ) { m_int = i; }
};

class Condition
{
public:
	enum ConditionType { FLAG = 3 };
	Parameter m_parms[2];
	Int getNumParameters( void ) const { return 2; }
	ConditionType getConditionType( void ) const { return FLAG; }
	Parameter *getParameter( Int ndx ) { return &m_parms[ndx]; }
};

struct TFlag
{
	Bool value;
	AsciiString name;
};

class ScriptEngine
{
public:
	enum { MAX_FLAGS = 256 };
	ListAsciiString m_uiInteractions;
	TFlag m_flags[MAX_FLAGS];
	Int m_numFlags;
	void signalUIInteract( const AsciiString& hookName );
	Int allocateFlag( const AsciiString& name );
	Bool evaluateFlag( Condition *pCondition );
	void updateClear( void )
	{
#include "sh_clear.inc"
	}
};

ScriptEngine *TheScriptEngine = NULL;

#include "sh_names.inc"
#include "sh_signal.inc"
#include "sh_alloc.inc"
#include "sh_flag.inc"

void put_name(const char *s)
{
	for (const char *p = s; *p; ++p)
		g_out.push_back((unsigned char)*p);
	g_out.push_back(0);
}

}  // namespace shx

extern "C" int ref_shell_hooks_case(int kind, const int *in, int nin, int *out, int cap)
{
	using namespace shx;
	g_out.clear();
	if (kind == 0) {
		int v[] = {
			SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED, SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_HIGHLIGHTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_UNHIGHLIGHTED, SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_HIGHLIGHTED, SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_UNHIGHLIGHTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED, SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_UNHIGHLIGHTED, SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_HIGHLIGHTED, SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_UNHIGHLIGHTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED, SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_HIGHLIGHTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED, SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED,
			SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED, SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED,
			SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN, SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGOUT,
			SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME, SHELL_SCRIPT_HOOK_OPTIONS_OPENED,
			SHELL_SCRIPT_HOOK_OPTIONS_CLOSED, SHELL_SCRIPT_HOOK_SKIRMISH_OPENED, SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED,
			SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME, SHELL_SCRIPT_HOOK_LAN_OPENED, SHELL_SCRIPT_HOOK_LAN_CLOSED,
			SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME, SHELL_SCRIPT_HOOK_TOTAL,
		};
		g_out.assign(v, v + sizeof(v) / sizeof(v[0]));
	} else if (kind == 1) {
		for (int i = 0; i < SHELL_SCRIPT_HOOK_TOTAL; ++i)
			put_name(TheShellHookNames[i]);
	} else if (kind == 2) {
		ScriptEngine *engine = new ScriptEngine;
		engine->m_numFlags = 1;
		for (int i = 0; i < ScriptEngine::MAX_FLAGS; ++i) {
			engine->m_flags[i].value = FALSE;
			engine->m_flags[i].name.clear();
		}
		TheScriptEngine = engine;
		static Condition conds[2 * SHELL_SCRIPT_HOOK_TOTAL];
		for (int j = 0; j < SHELL_SCRIPT_HOOK_TOTAL; ++j)
			for (int v = 0; v < 2; ++v) {
				Condition &c = conds[2 * j + v];
				c.m_parms[0].m_int = 0;
				c.m_parms[0].m_string.set(TheShellHookNames[j]);
				c.m_parms[1].m_int = v;
				c.m_parms[1].m_string.clear();
			}
		for (int k = 0; k + 1 < nin; k += 2) {
			int op = in[k], a = in[k + 1];
			if (op == 0) {
				SignalUIInteraction(a);
			} else if (op == 1) {
				g_out.push_back(10);
				g_out.push_back((int)engine->m_uiInteractions.size());
				for (ListAsciiStringIt it = engine->m_uiInteractions.begin(); it != engine->m_uiInteractions.end(); ++it)
					put_name(it->str());
			} else if (op == 2) {
				engine->updateClear();
			} else if (op == 3) {
				Bool r = engine->evaluateFlag(&conds[a]);
				g_out.push_back(20);
				g_out.push_back(r ? 1 : 0);
				g_out.push_back(engine->m_numFlags);
			}
		}
		TheScriptEngine = NULL;
		delete engine;
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	memcpy(out, g_out.data(), n * sizeof(int));
	return n;
}
