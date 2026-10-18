/* Stand-in for GameEngine/Include/Common/Thing.h (layer 5), Module build only
** (differential tests, p1w3-modules): what Module.cpp uses of it, AsObject and
** AsDrawable, with the same shape as the real header (the virtual
** asObjectMeth / asDrawableMeth, and the friend functions). The real Thing
** brings Matrix3D, ThingTemplate and the whole object model. The classes are
** renamed by the wrapper's macros (module_prelude.h). */
#ifndef SHIM_MODULE_THING_H
#define SHIM_MODULE_THING_H
#include "Lib/BaseType.h"
class Object;
class Drawable;
class Thing
{
public:
	virtual ~Thing() {}
	friend inline Object *AsObject(Thing *thing) { return thing ? thing->asObjectMeth() : NULL; }
	friend inline Drawable *AsDrawable(Thing *thing) { return thing ? thing->asDrawableMeth() : NULL; }
	friend inline const Object *AsObject(const Thing *thing) { return thing ? thing->asObjectMeth() : NULL; }
	friend inline const Drawable *AsDrawable(const Thing *thing) { return thing ? thing->asDrawableMeth() : NULL; }
protected:
	virtual Object *asObjectMeth() { return NULL; }
	virtual Drawable *asDrawableMeth() { return NULL; }
	virtual const Object *asObjectMeth() const { return NULL; }
	virtual const Drawable *asDrawableMeth() const { return NULL; }
};
#endif
