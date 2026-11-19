/* Differential-test wrapper for the control bar scheme drafts (#215, upstream commit 0a05454d),
** src/drafts/sweep_gui_logic/control_bar_scheme.rs (update_anim, set_control_bar_scheme), cut verbatim by build.rs (drafts_guilogic3) from
** GameClient/GUI/ControlBar/ControlBarScheme.cpp into:
**   cbs_anim.inc  ControlBarScheme::updateAnim (to ControlBarScheme::update);
**   cbs_find.inc  ControlBarSchemeManager::findControlBarScheme (to preloadAssets);
**   cbs_set.inc   ControlBarSchemeManager::setControlBarScheme (to `Update calls all the animation update calls`).
** Built on the REAL AsciiString (shim_ascii; classes renamed by build.rs).
** Stand-ins (outside the cuts): ControlBarSchemeAnimation (the enum of ControlBarScheme.h:103-107 and m_animType); ControlBarScheme (m_name,
** m_ScreenCreationRes, and animSlideRight and init, which log); ControlBarSchemeManager (m_schemeList as ControlBarScheme.h declares it, a
** std::list of pointers, m_currentScheme, m_multiplyer as a Coord2D); TheDisplay->getWidth() / getHeight() (UnsignedInt, as Display.h
** declares them); ControlBarScheme::init's body (the real one loads images). DEBUG_ASSERTCRASH is a no-op (release).
**
** Entry points (extern "C", tests/differential_draft_cbscheme.rs): ref_cbs_anim(type) -> 1 when animSlideRight was called;
** ref_cbs_set(in, n, out): in = [nSchemes, (nameLen, name.., resX, resY).., searchLen, search.., width, height]; out = [current place or -1,
** multiplier.x bits, multiplier.y bits, init calls]. */
#include <string.h>
#include <list>

DynamicMemoryAllocator *TheDynamicMemoryAllocator = 0;
static DynamicMemoryAllocator theCbsAllocator;

#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define NULL 0

static int theAnimCalls, theInitCalls;
struct ControlBarSchemeAnimation
{
	enum
	{
		CB_ANIM_SLIDE_RIGHT = 0,

		CB_ANIM_MAX
	};
	Int m_animType;
};
class ControlBarScheme
{
public:
	void updateAnim (ControlBarSchemeAnimation * anim);
	void animSlideRight(ControlBarSchemeAnimation *anim) { ++theAnimCalls; }
	void init() { ++theInitCalls; }
	AsciiString m_name;
	ICoord2D m_ScreenCreationRes;
};
typedef std::list<ControlBarScheme *> ControlBarSchemeList;
class ControlBarSchemeManager
{
public:
	ControlBarScheme *findControlBarScheme( AsciiString name );
	void setControlBarScheme(AsciiString schemeName);
	ControlBarSchemeList m_schemeList;
	ControlBarScheme *m_currentScheme;
	Coord2D m_multiplyer;
};
static UnsignedInt theW, theH;
struct DisplayStandIn { UnsignedInt getWidth() const { return theW; } UnsignedInt getHeight() const { return theH; } };
static DisplayStandIn theDisplayObj;
static DisplayStandIn *TheDisplay = &theDisplayObj;

#include "cbs_anim.inc"
#include "cbs_find.inc"
#include "cbs_set.inc"

extern "C" int ref_cbs_anim(int type)
{
	theAnimCalls = 0;
	ControlBarScheme s; ControlBarSchemeAnimation a; a.m_animType = type;
	s.updateAnim(&a);
	return theAnimCalls;
}

extern "C" int ref_cbs_set(const int *in, int nin, int *out)
{
	TheDynamicMemoryAllocator = &theCbsAllocator;
	theInitCalls = 0;
	ControlBarSchemeManager m;
	m.m_currentScheme = NULL; m.m_multiplyer.x = -1.0f; m.m_multiplyer.y = -1.0f;
	int at = 0;
	int n = in[at++];
	std::list<ControlBarScheme *> keep;
	for (int i = 0; i < n; ++i)
	{
		ControlBarScheme *s = new ControlBarScheme;
		int nl = in[at++];
		char name[64];
		for (int k = 0; k < nl; ++k) name[k] = (char)in[at++];
		name[nl] = 0;
		s->m_name = AsciiString(name);
		s->m_ScreenCreationRes.x = in[at++]; s->m_ScreenCreationRes.y = in[at++];
		m.m_schemeList.push_back(s);
	}
	if (!m.m_schemeList.empty()) m.m_currentScheme = m.m_schemeList.front(); /* the manager starts with the first scheme current, as the test's does */
	int sl = in[at++];
	char search[64];
	for (int k = 0; k < sl; ++k) search[k] = (char)in[at++];
	search[sl] = 0;
	theW = (UnsignedInt)in[at++]; theH = (UnsignedInt)in[at++];
	m.setControlBarScheme(AsciiString(search));
	int place = -1, i = 0;
	for (ControlBarSchemeList::iterator it = m.m_schemeList.begin(); it != m.m_schemeList.end(); ++it, ++i) if (*it == m.m_currentScheme) place = i;
	union { Real f; int i; } x, y;
	x.f = m.m_multiplyer.x; y.f = m.m_multiplyer.y;
	out[0] = place; out[1] = x.i; out[2] = y.i; out[3] = theInitCalls;
	return 4;
}
