/* Differential-test wrapper for the composite render object's bounding-volume arithmetic (GitHub #215, upstream commit 0a05454d), group "ww3d6":
** src/drafts/ww3d/composite.rs (SphereClass, MinMaxAABoxClass, aabox_from_minmax, CompositeRenderObjClass::update_obj_space_bounding_volumes)
** against WWMath/sphere.h (SphereClass::Add_Sphere), WWMath/aabox.h (MinMaxAABoxClass::Init / Add_Box, AABoxClass::Init(MinMax)), all REAL headers, and
** WW3D2/composite.cpp's CompositeRenderObjClass::Update_Obj_Space_Bounding_Volumes, cut VERBATIM by build.rs `drafts_ww3d6` (comp_update.inc).
**
** Stand-ins, each for something the cut calls outside itself: the class around the function (composite.h derives from RenderObjClass, the scene graph) is
** CompositeRenderObjClass holding ObjSphere, ObjBox and the sub-objects; RenderObjClass is a scripted sub-object (Get_Obj_Space_Bounding_Sphere / _Box,
** Release_Ref); Get_Num_Sub_Objects / Get_Sub_Object index the script; Invalidate_Cached_Bounding_Volumes counts its calls; Get_Container returns NULL.
**
** Entry point (extern "C", tests/differential_draft_composite.rs), uint32 words, floats by bits:
**   ref_w6_comp_run(in, out) -> n
**   op 0 [0, s1 (center 3, radius), s2]            SphereClass::Add_Sphere -> [center 3, radius]
**   op 1 [1, n, n x (center 3, extent 3)]          MinMaxAABoxClass::Init(first) then Add_Box(rest), AABoxClass::Init(minmax)
**                                                  -> [min 3, max 3, center 3, extent 3]
**   op 2 [2, n, n x (sphere 4, box 6)]             Update_Obj_Space_Bounding_Volumes -> [ObjSphere 4, ObjBox 6, Invalidate calls] */
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>
#include "always.h"
#include "vector3.h"
#include "matrix3d.h"
#include "sphere.h"
#include "aabox.h"

#ifndef WWASSERT
#define WWASSERT(x) ((void)0)
#endif

typedef uint32_t W;
static float F(W w) { float f; memcpy(&f, &w, 4); return f; }
static W B(float f) { W w; memcpy(&w, &f, 4); return w; }

namespace {

class W6SubObject {
public:
	SphereClass sphere;
	AABoxClass box;
	void Get_Obj_Space_Bounding_Sphere(SphereClass & s) const { s = sphere; }
	void Get_Obj_Space_Bounding_Box(AABoxClass & b) const { b = box; }
	void Release_Ref(void) const {}
	void Update_Obj_Space_Bounding_Volumes(void) {} /* only called on a container, and Get_Container returns NULL */
};

#define RenderObjClass W6SubObject

class CompositeRenderObjClass {
public:
	void Update_Obj_Space_Bounding_Volumes(void);
	int Get_Num_Sub_Objects(void) const { return (int)subs.size(); }
	RenderObjClass * Get_Sub_Object(int i) const { return const_cast<RenderObjClass *>(&subs[i]); }
	void Invalidate_Cached_Bounding_Volumes(void) { invalidates++; }
	RenderObjClass * Get_Container(void) const { return NULL; }
	SphereClass ObjSphere;
	AABoxClass ObjBox;
	std::vector<W6SubObject> subs;
	int invalidates = 0;
};

#include "comp_update.inc"

#undef RenderObjClass

}

extern "C" int ref_w6_comp_run(const W *in, W *out)
{
	W *o = out;
	const W *p = in;
	int op = (int)*p++;
	if (op == 0) {
		SphereClass a(Vector3(F(p[0]), F(p[1]), F(p[2])), F(p[3]));
		SphereClass b(Vector3(F(p[4]), F(p[5]), F(p[6])), F(p[7]));
		a.Add_Sphere(b);
		*o++ = B(a.Center.X); *o++ = B(a.Center.Y); *o++ = B(a.Center.Z); *o++ = B(a.Radius);
	} else if (op == 1) {
		int n = (int)*p++;
		MinMaxAABoxClass mm;
		for (int i = 0; i < n; i++, p += 6) {
			AABoxClass bx(Vector3(F(p[0]), F(p[1]), F(p[2])), Vector3(F(p[3]), F(p[4]), F(p[5])));
			if (i == 0) mm.Init(bx); else mm.Add_Box(bx);
		}
		AABoxClass res;
		res.Init(mm);
		*o++ = B(mm.MinCorner.X); *o++ = B(mm.MinCorner.Y); *o++ = B(mm.MinCorner.Z);
		*o++ = B(mm.MaxCorner.X); *o++ = B(mm.MaxCorner.Y); *o++ = B(mm.MaxCorner.Z);
		*o++ = B(res.Center.X); *o++ = B(res.Center.Y); *o++ = B(res.Center.Z);
		*o++ = B(res.Extent.X); *o++ = B(res.Extent.Y); *o++ = B(res.Extent.Z);
	} else {
		int n = (int)*p++;
		CompositeRenderObjClass c;
		c.ObjSphere.Init(Vector3(7, 8, 9), 5); /* what the object held before: the empty case must overwrite it */
		c.ObjBox.Center.Set(1, 2, 3);
		c.ObjBox.Extent.Set(4, 5, 6);
		for (int i = 0; i < n; i++, p += 10) {
			W6SubObject s;
			s.sphere.Init(Vector3(F(p[0]), F(p[1]), F(p[2])), F(p[3]));
			s.box.Center.Set(F(p[4]), F(p[5]), F(p[6]));
			s.box.Extent.Set(F(p[7]), F(p[8]), F(p[9]));
			c.subs.push_back(s);
		}
		c.Update_Obj_Space_Bounding_Volumes();
		*o++ = B(c.ObjSphere.Center.X); *o++ = B(c.ObjSphere.Center.Y); *o++ = B(c.ObjSphere.Center.Z); *o++ = B(c.ObjSphere.Radius);
		*o++ = B(c.ObjBox.Center.X); *o++ = B(c.ObjBox.Center.Y); *o++ = B(c.ObjBox.Center.Z);
		*o++ = B(c.ObjBox.Extent.X); *o++ = B(c.ObjBox.Extent.Y); *o++ = B(c.ObjBox.Extent.Z);
		*o++ = (W)c.invalidates;
	}
	return (int)(o - out);
}
