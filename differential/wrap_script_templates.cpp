/* Differential-test wrapper for ScriptEngine::init's template tables (upstream commit 0a05454d):
**   GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp, lines 587-5266 of ScriptEngine::init
** (from "// Set up the script action templates." through the two loops that append "[index]" to the editor names and make the
** name keys), cut out unchanged by build.rs into OUT_DIR/script_templates_gen/init_templates.inc and compiled here as the body of
** a member function of a stand-in that holds the two arrays under their upstream names, on the real Template classes of
** GameLogic/Scripts.h (Scripts.cpp, with Template's constructor, is in the same build: upstream_sides). The stand-in is
** constructed on zeroed memory, as `NEW ScriptEngine` is (DynamicMemoryAllocator::allocateBytesImplementation memsets), so the
** fields Template() and init leave unset read as upstream's do.
**
** ref_script_templates(out, cap) runs it on a fresh name key generator (only init()) and writes, for the 109 conditions and then
** the 344 actions: str m_uiName, str m_uiName2, str m_internalName, u32 m_internalNameKey, i32 m_numUiStrings, 12 x str
** m_uiStrings, i32 m_numParameters, 12 x i32 m_parameters, str m_helpText; a str is u32 length and bytes, all little-endian.
** Returns the length written (or needed, when larger than cap). */
#include <stdlib.h>
#include <string.h>
#include <new>
#define private public
#define protected public
#include "GameLogic/Scripts.h"
#undef private
#undef protected

namespace {

struct EngineTemplates {
	ConditionTemplate m_conditionTemplates[Condition::NUM_ITEMS];
	ActionTemplate m_actionTemplates[ScriptAction::NUM_ITEMS];
	void initTemplates();
};

void EngineTemplates::initTemplates()
{
#include "init_templates.inc"
}

struct Out {
	unsigned char *p; size_t cap, len;
	void u32(unsigned v) { for (int k = 0; k < 4; ++k) { if (len < cap) p[len] = (unsigned char)(v >> (8 * k)); ++len; } }
	void str(const AsciiString &s) { unsigned n = (unsigned)s.getLength(); u32(n); for (unsigned k = 0; k < n; ++k) { if (len < cap) p[len] = (unsigned char)s.str()[k]; ++len; } }
	void tmpl(const Template &t) {
		str(t.m_uiName); str(t.m_uiName2); str(t.m_internalName); u32((unsigned)t.m_internalNameKey);
		u32((unsigned)t.m_numUiStrings);
		for (int k = 0; k < MAX_PARMS; ++k) str(t.m_uiStrings[k]);
		u32((unsigned)t.m_numParameters);
		for (int k = 0; k < MAX_PARMS; ++k) u32((unsigned)t.m_parameters[k]);
		str(t.m_helpText);
	}
};

} // namespace

/* Defined in wrap_sides.cpp, the object that also holds the TheScriptEngine stand-in Scripts.cpp refers to. Taking its address
** below links that object whenever this one is linked; otherwise the linker can take TheScriptEngine from another fixture's
** archive first and then meet wrap_sides.cpp's definition as a duplicate. */
extern "C" int ref_sides_constants(int *out);

extern "C" size_t ref_script_templates(unsigned char *out, size_t cap)
{
	int (*volatile anchor)(int *) = ref_sides_constants;
	(void)anchor;
	NameKeyGenerator *saved = TheNameKeyGenerator;
	TheNameKeyGenerator = new NameKeyGenerator;
	TheNameKeyGenerator->init();
	void *mem = calloc(1, sizeof(EngineTemplates));
	EngineTemplates *e = new (mem) EngineTemplates;
	e->initTemplates();
	Out o = { out, cap, 0 };
	for (int k = 0; k < Condition::NUM_ITEMS; ++k) o.tmpl(e->m_conditionTemplates[k]);
	for (int k = 0; k < ScriptAction::NUM_ITEMS; ++k) o.tmpl(e->m_actionTemplates[k]);
	e->~EngineTemplates();
	free(mem);
	delete TheNameKeyGenerator;
	TheNameKeyGenerator = saved;
	return o.len;
}
