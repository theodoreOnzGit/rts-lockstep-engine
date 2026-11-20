/* Differential-test wrapper for the W3D module factory draft (#215, group lc2, upstream commit 0a05454d), src/drafts/logic_common/w3d_module_factory.rs:
**   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/Thing/W3DModuleFactory.cpp from `void W3DModuleFactory::init( void )` to `}  // end init`,
**   cut verbatim by build.rs into w3d_factory_init.inc.
**
** Stand-ins (outside the cut): `ModuleFactory::init()` is empty (the logic modules are the logic factory's); `addModule( classname )` is ModuleFactory.h's macro
** reduced to what the draft models, its `AsciiString( #classname )` name: here it appends the stringised name to a list (the real one also takes the
** class's `friend_newModuleInstance`, `friend_newModuleData`, type and interface mask, which need the 19 draw classes: not compared).
**
** Entry point: ref_lc2_w3d_names (tests/differential_draft_w3d_module_factory.rs). */
#include <string>
#include <string.h>
#include <vector>

namespace {

std::vector<std::string> g_names;

class ModuleFactory
{
public:
	virtual ~ModuleFactory() {}
	virtual void init(void) {}
	void addNameInternal(const char *n) { g_names.push_back(n); }
#define addModule( classname ) addNameInternal( #classname )
};

class W3DModuleFactory : public ModuleFactory
{
public:
	virtual void init(void);
};

#include "w3d_factory_init.inc"

}

extern "C" {

/* The names addModule was given, in order, each followed by '\n'. Returns the byte count. */
int ref_lc2_w3d_names(char *out, int cap)
{
	g_names.clear();
	W3DModuleFactory f;
	f.init();
	std::string all;
	for (size_t i = 0; i < g_names.size(); ++i) { all += g_names[i]; all += '\n'; }
	if ((int)all.size() > cap) return -1;
	memcpy(out, all.data(), all.size());
	return (int)all.size();
}

}
