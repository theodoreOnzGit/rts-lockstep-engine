/* Differential-test wrapper for the draft src/drafts/sweep_common_client/particle_sys.rs (#215, group ui2, upstream
** commit 0a05454d), cut verbatim by build.rs (fn drafts_ui2) into OUT_DIR/draft_ui2_gen:
**   u2_fp_c.inc   GameEngine/Source/GameClient/System/ParticleSys.cpp, ParticleSystemManager::findParentTemplate (to the
**                 banner of destroyAttachedSystems).
** No edit. Stand-ins: AsciiString (a std::string holder with isEmpty and compare, strcmp's sign), ParticleSystemTemplate
** (m_slaveSystemName), ParticleSystemManager (m_templateMap, the cut function). **The map is a std::map ordered by name**
** (byte order), where upstream's TemplateMap is a hash_map with no defined order: which template is the n-th among
** several parents is unspecified upstream, and the draft documents that it orders them by name; the stand-in orders
** them the same way, so the test compares the counting logic, not the (undefined) hash order.
**
** Entry point (extern "C", tests/differential_draft_find_parent.rs):
**   ref_u2_fp_run(in, n, out, cap): `in` is [ntemplates, (name id, slave id) x ntemplates (a slave id of -1 is the empty
**   name; a repeated name id updates the template as newTemplate returns the existing one), nqueries, (name id or -1,
**   parentNum) x nqueries]; the output is one word per query: the found template's name id, or -1 for NULL. Names are
**   NAMES[id] below (case variants included). */
#include <string>
#include <map>
#include <string.h>
#include "Lib/BaseType.h"

namespace u2fp {
static const char *const NAMES[10] = { "FX", "fx", "Fx1", "Smoke", "smoke", "Fire", "FireA", "Fire2", "A", "B" };
class AsciiString
{
public:
	AsciiString() {}
	AsciiString(const char *c) : s(c) {}
	Bool isEmpty() const { return s.empty(); }
	Int compare(const AsciiString &o) const { return strcmp(s.c_str(), o.s.c_str()); }
	bool operator<(const AsciiString &o) const { return s < o.s; }
	std::string s;
};
class ParticleSystemTemplate { public: AsciiString m_slaveSystemName; AsciiString m_name; };
class ParticleSystemManager
{
public:
	ParticleSystemTemplate *findParentTemplate( const AsciiString &name, Int parentNum ) const;
	typedef std::map<AsciiString, ParticleSystemTemplate *> TemplateMap;
	TemplateMap m_templateMap;
};
#include "u2_fp_c.inc"
}

extern "C" int ref_u2_fp_run(const int *in, int n, int *out, int cap)
{
	using namespace u2fp;
	ParticleSystemManager m;
	int i = 0, o = 0;
	int nt = in[i++];
	for (int k = 0; k < nt; k++) {
		int name = in[i++], slave = in[i++];
		AsciiString key(NAMES[name]);
		ParticleSystemTemplate *t;
		ParticleSystemManager::TemplateMap::iterator it = m.m_templateMap.find(key);
		if (it == m.m_templateMap.end()) { t = new ParticleSystemTemplate; t->m_name = key; m.m_templateMap[key] = t; }
		else t = it->second;
		t->m_slaveSystemName = (slave < 0) ? AsciiString("") : AsciiString(NAMES[slave]);
	}
	int nq = in[i++];
	for (int q = 0; q < nq; q++) {
		int name = in[i++], pn = in[i++];
		AsciiString qn((name < 0) ? "" : NAMES[name]);
		ParticleSystemTemplate *t = m.findParentTemplate(qn, pn);
		int r = -1;
		if (t) for (int id = 0; id < 10; id++) if (t->m_name.s == NAMES[id]) r = id;
		if (o < cap) out[o] = r;
		o++;
	}
	(void)n;
	return o;
}
