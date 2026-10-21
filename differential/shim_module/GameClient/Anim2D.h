/* Stand-in for GameEngine/Include/GameClient/Anim2D.h, Module build only (differential tests, p1w12-sonnet, crate batch F): the collection and
** its template lookup, logged by differential/l9_crates_f.inc. Renamed ModTestAnim2D* by module_prelude.h. */
#ifndef SHIM_MODULE_ANIM2D_H
#define SHIM_MODULE_ANIM2D_H
#include "Common/AsciiString.h"
class Anim2DTemplate { public: int m_dummy; };
class Anim2DCollection
{
public:
	Anim2DTemplate *findTemplate( const AsciiString& name );
};
extern Anim2DCollection *TheAnim2DCollection;
#endif
