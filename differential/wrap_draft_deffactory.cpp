/* Differential-test wrapper for the drafts src/drafts/wwlib/definitionfactorymgr.rs and definitionfactory.rs (#215, group wwlib3),
** upstream commit 0a05454d: GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionfactorymgr.h, definitionfactorymgr.cpp,
** definitionfactory.h, definitionfactory.cpp and definitionclassids.h, compiled UNCHANGED (copied by build.rs so that their quoted
** includes find the stand-ins). Force-included: shim_w3d/w3d_prelude.h (32-bit uint32 as on Win32, stricmp, the CRT names).
** Stand-ins: wwdebug.h (WWASSERT compiled out, as in the release build). The abstract DefinitionFactoryClass is derived here
** (TestFactory: a class id and a name, Create returns NULL; DefinitionClass is only forward-declared by the header). */
#include "definitionfactory.h"
#include "definitionfactorymgr.h"
#include "definitionfactory.cpp"
#include "definitionfactorymgr.cpp"
#include <string>
#include <stdint.h>

class TestFactory : public DefinitionFactoryClass {
public:
	uint32 id; std::string name;
	TestFactory(uint32 i, const char *n) : id(i), name(n) {}
	virtual DefinitionClass *Create(void) const { return 0; }
	virtual const char *Get_Name(void) const { return name.c_str(); }
	virtual uint32 Get_Class_ID(void) const { return id; }
	virtual bool Is_Displayed(void) const { return true; }
};

static const int NF = 8;

extern "C" int ref_df_run(const int *prog, int nops, int *out, int cap)
{
	TestFactory *f[NF];
	for (int i = 0; i < NF; i++) f[i] = 0;
	int n = 0;
	auto put = [&](int w) { if (n < cap) out[n] = w; n++; };
	auto idx = [&](DefinitionFactoryClass *p) { if (p == 0) return -1; for (int i = 0; i < NF; i++) if (f[i] == p) return i; return -2; };
	const int *p = prog;
	for (int k = 0; k < nops; k++) {
		int op = *p++;
		switch (op) {
		case 0: { int slot = *p++; uint32 id = (uint32)*p++; int nl = *p++; char nm[16]; for (int i = 0; i < nl; i++) nm[i] = (char)*p++; nm[nl] = 0;
			if (f[slot] == 0) f[slot] = new TestFactory(id, nm); break; }   /* the constructor registers it (Link_Factory) */
		case 1: { int slot = *p++; delete f[slot]; f[slot] = 0; break; }      /* the destructor unregisters it */
		case 2: put(idx(DefinitionFactoryMgrClass::Find_Factory((uint32)*p++))); break;
		case 3: { int nl = *p++; char nm[16]; for (int i = 0; i < nl; i++) nm[i] = (char)*p++; nm[nl] = 0; put(idx(DefinitionFactoryMgrClass::Find_Factory(nm))); break; }
		case 4: put(idx(DefinitionFactoryMgrClass::Get_First((uint32)*p++))); break;
		case 5: { int slot = *p++; uint32 sc = (uint32)*p++; if (f[slot]) put(idx(DefinitionFactoryMgrClass::Get_Next(f[slot], sc))); else put(-3); break; }
		case 6: put(idx(DefinitionFactoryMgrClass::Get_First())); break;
		case 7: { int slot = *p++; if (f[slot]) put(idx(DefinitionFactoryMgrClass::Get_Next(f[slot]))); else put(-3); break; }
		}
		/* the whole list, newest first */
		for (DefinitionFactoryClass *q = DefinitionFactoryMgrClass::Get_First(); q; q = DefinitionFactoryMgrClass::Get_Next(q)) put(idx(q));
		put(-9);
	}
	for (int i = 0; i < NF; i++) { delete f[i]; f[i] = 0; }
	return n;
}
