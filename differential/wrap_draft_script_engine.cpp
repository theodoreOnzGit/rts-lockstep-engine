/* Differential-test wrapper for the ScriptEngine draft (#215, group lc2, upstream commit 0a05454d), src/drafts/sweep_script/script_engine.rs:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp, cut verbatim by build.rs: se_action.inc (`addActionTemplateInfo`),
**   se_cond.inc (`addConditionTemplateInfo`), se_seq.inc (`removeSequentialScript`), se_done.inc (`hasUnitCompletedSequentialScript`,
**   `hasTeamCompletedSequentialScript`, `getEnableVTune`, `setEnableVTune`), se_victory.inc (`debugVictory`);
**   GeneralsMD/Code/GameEngine/Include/GameLogic/Scripts.h: se_enum.inc, the first entries of `ScriptAction::ScriptActionType` up to `VICTORY`.
**
** Stand-ins (outside the cuts): AsciiString is a std::string with `==`; `Template` keeps the four string fields the cut touches (the real
** one also holds the parameter list); `ScriptEngine` keeps only the two template arrays the cut reads (sizes ScriptAction::NUM_ITEMS = 6 and
** Condition::NUM_ITEMS = 5, the test's own small sizes) and the cut's member declarations; `ScriptAction` has the enum prefix and a
** constructor that keeps the type; `newInstance(X)` is `new X`; `TheScriptActions->executeAction` records the type it was given;
** `DEBUG_LOG` is empty (compiled out of the release build); DO_VTUNE_STUFF is not defined (as in the shipped build).
**
** Entry points: ref_lc2_se_templates, ref_lc2_se_misc, ref_lc2_se_victory (tests/differential_draft_script_engine.rs). */
#include <string>

typedef int Int;
typedef bool Bool;
#define TRUE true
#define FALSE false
#define NULL 0
#define DEBUG_LOG(x)
#define newInstance(X) new X

namespace {

class AsciiString
{
public:
	std::string s;
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	bool operator==(const AsciiString &o) const { return s == o.s; }
	const char *str() const { return s.c_str(); }
};

class Object;
class Team;
class SequentialScript;

class ScriptAction
{
public:
	enum ScriptActionType
	{
#include "se_enum.inc"
		DEFEAT,
		NO_OP,
		NUM_ITEMS
	};
	int m_type;
	ScriptAction(ScriptActionType t) : m_type((int)t) {}
};

namespace Condition { enum { NUM_ITEMS = 5 }; }

struct Template
{
	AsciiString m_internalName;
	AsciiString m_uiName;
	AsciiString m_uiName2;
	AsciiString m_helpText;
};

class ScriptEngine
{
public:
	Template m_actionTemplates[ScriptAction::NUM_ITEMS];
	Template m_conditionTemplates[Condition::NUM_ITEMS];
	void addActionTemplateInfo( Template *actionTemplate);
	void addConditionTemplateInfo( Template *actionTemplate);
	void removeSequentialScript(SequentialScript *scriptToRemove);
	Bool hasUnitCompletedSequentialScript( Object *object, const AsciiString& sequentialScriptName );
	Bool hasTeamCompletedSequentialScript( Team *team, const AsciiString& sequentialScriptName );
	Bool getEnableVTune() const;
	void setEnableVTune(Bool value);
	void debugVictory( void );
};

int g_executed = -1;
class ScriptActions { public: void executeAction(ScriptAction *a) { g_executed = a->m_type; } };
ScriptActions *TheScriptActions = new ScriptActions;

#include "se_action.inc"
#include "se_cond.inc"
#include "se_seq.inc"
#include "se_done.inc"
#include "se_victory.inc"

const char *const IDS[6] = { "", "n0", "n1", "N0", "u", "h" };

AsciiString idStr(int id) { return AsciiString(IDS[id]); }

int strId(const AsciiString &a)
{
	for (int i = 0; i < 6; ++i) if (a.s == IDS[i]) return i;
	return -1;
}

}

extern "C" {

/* which 0: actions (6 entries), 1: conditions (5). tbl: 4 ids per entry (internal, ui, ui2, help); tpl: 4 ids. out: the table's ids after the call. */
int ref_lc2_se_templates(int which, const int *tbl, const int *tpl, int *out)
{
	ScriptEngine *e = new ScriptEngine;
	int n = which == 0 ? (int)ScriptAction::NUM_ITEMS : (int)Condition::NUM_ITEMS;
	Template *t = which == 0 ? e->m_actionTemplates : e->m_conditionTemplates;
	for (int i = 0; i < n; ++i)
	{
		t[i].m_internalName = idStr(tbl[4 * i]); t[i].m_uiName = idStr(tbl[4 * i + 1]);
		t[i].m_uiName2 = idStr(tbl[4 * i + 2]); t[i].m_helpText = idStr(tbl[4 * i + 3]);
	}
	Template in;
	in.m_internalName = idStr(tpl[0]); in.m_uiName = idStr(tpl[1]); in.m_uiName2 = idStr(tpl[2]); in.m_helpText = idStr(tpl[3]);
	if (which == 0) e->addActionTemplateInfo(&in); else e->addConditionTemplateInfo(&in);
	for (int i = 0; i < n; ++i)
	{
		out[4 * i] = strId(t[i].m_internalName); out[4 * i + 1] = strId(t[i].m_uiName);
		out[4 * i + 2] = strId(t[i].m_uiName2); out[4 * i + 3] = strId(t[i].m_helpText);
	}
	delete e;
	return 4 * n;
}

/* removeSequentialScript(NULL) (nothing), hasUnit, hasTeam, getEnableVTune, setEnableVTune(true), getEnableVTune, setEnableVTune(false), getEnableVTune */
int ref_lc2_se_misc(int *out)
{
	ScriptEngine *e = new ScriptEngine;
	e->removeSequentialScript(NULL);
	AsciiString n("seq");
	out[0] = e->hasUnitCompletedSequentialScript(NULL, n);
	out[1] = e->hasTeamCompletedSequentialScript(NULL, n);
	out[2] = e->getEnableVTune();
	e->setEnableVTune(true);
	out[3] = e->getEnableVTune();
	e->setEnableVTune(false);
	out[4] = e->getEnableVTune();
	delete e;
	return 5;
}

/* debugVictory: the type TheScriptActions->executeAction was given. */
int ref_lc2_se_victory(void)
{
	ScriptEngine *e = new ScriptEngine;
	g_executed = -1;
	e->debugVictory();
	delete e;
	return g_executed;
}

}
