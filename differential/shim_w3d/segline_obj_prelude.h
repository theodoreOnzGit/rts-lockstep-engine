/* Forced include (-include, after w3d_prelude.h and segline_prelude.h) for
** the segmented-line render object library, differential tests only (GitHub
** #169; see build.rs and differential/wrap_segline_obj.cpp).
**
** WW3D2/segline.cpp is compiled VERBATIM (w3d_gen/segline_obj_def.cpp, from
** its first #include to the end) against the renderer of the segmented-line
** library (segline_prelude.h's renaming: SegLineRendererClass is
** RefSegLineRendererClass, WW3D RefSegLineWW3D, ...). What it needs from the
** render object system is replaced here, the real headers' include guards
** defined first:
**   rendobj.h (RENDOBJ_H): RenderObjClass, only what SegmentedLineClass uses:
**     Bits and its enum and DEFAULT_BITS, Transform, the constructors and
**     operator= (rendobj.cpp's text), Set_Visible, Is_Visible, Set_Hidden,
**     Is_Hidden, Set_Animation_Hidden, Is_Animation_Hidden,
**     Set_Force_Visible, Is_Force_Visible, Get_Collision_Type,
**     Set_Collision_Type, Is_Not_Hidden_At_All,
**     Invalidate_Cached_Bounding_Volumes (rendobj.h's inline text),
**     Get_Screen_Size (answers the test's value), AT_MIN_LOD, AT_MAX_LOD
**     (rendobj.cpp's values), CLASSID_SEGLINE, and the Native_Screen_Size
**     pair operator= reaches.
**   predlod.h (PREDLOD_H): PredictiveLODOptimizerClass::Add_Object and
**     Add_Cost record what Prepare_LOD asked.
**   coltest.h (COLTEST_H): RayCollisionTestClass as far as Cast_Ray names
**     it (LineSegClass and CastResultStruct are the real lineseg.h and
**     castres.h; the wrapper's LineSegClass::Find_Intersection and
**     recalculate abort: Cast_Ray is compiled, never called; a gap of #169).
** WW3D's sorting statics (Is_Sorting_Enabled true, as the game never turns
** it off; Are_Static_Sort_Lists_Enabled; Add_To_Static_Sort_List aborts)
** are declared in segline_prelude.h's WW3D and defined by the wrapper. */
#ifndef SHIM_SEGLINE_OBJ_PRELUDE_H
#define SHIM_SEGLINE_OBJ_PRELUDE_H

/* renamed so that they cannot collide with the other libraries' stand-ins of the same names (part_prelude.h's
** RenderObjClass and PredictiveLODOptimizerClass): the test binaries link every library */
#define RenderObjClass RefSegLineObjRenderObjClass
#define PredictiveLODOptimizerClass RefSegLineObjPredictiveLODOptimizerClass
#define RayCollisionTestClass RefSegLineObjRayCollisionTestClass
#define CameraClass RefSegLineObjCameraClass

#define RENDOBJ_H
#define PREDLOD_H
#define COLTEST_H

#include "simplevec_this.h" /* w3d_gen: SimpleDynVecClass with this-> (build.rs); segline.h's PointLocations */
#include "sphere.h"
#include "aabox.h"

class CameraClass;

/* coltype.h */
enum { COLL_TYPE_ALL = 0x01 };

/* rendobj.h */
class RenderObjClass : public RefCountClass
{
public:
	enum {
		CLASSID_UNKNOWN = 0xFFFFFFFF,
		CLASSID_SEGLINE = 9 /* the enum's place; Class_ID is never read by the test */
	};
	RenderObjClass(void) : Bits(DEFAULT_BITS), Transform(1), NativeScreenSize(1.0f) {}
	RenderObjClass(const RenderObjClass & src) : Bits(src.Bits), Transform(src.Transform), NativeScreenSize(src.NativeScreenSize) {}
	RenderObjClass & operator = (const RenderObjClass & that)
	{
		// don't do anything if we're assigning this to this
		if (this != &that) {
			Set_Hidden(that.Is_Hidden());
			Set_Animation_Hidden(that.Is_Animation_Hidden());
			Set_Force_Visible(that.Is_Force_Visible());
			Set_Collision_Type(that.Get_Collision_Type());
			Set_Native_Screen_Size(that.Get_Native_Screen_Size());
		}
		return *this;
	}
	virtual ~RenderObjClass(void) {}

	static const float	AT_MIN_LOD;
	static const float	AT_MAX_LOD;

	virtual float					Get_Screen_Size(CameraClass &camera);

	virtual int						Is_Not_Hidden_At_All(void)													{ return ((Bits & IS_NOT_HIDDEN_AT_ALL) == IS_NOT_HIDDEN_AT_ALL); }
	virtual int						Is_Visible(void) const														{ return (Bits & IS_VISIBLE); }
	virtual void					Set_Visible(int onoff)														{ if (onoff) { Bits |= IS_VISIBLE; } else { Bits &= ~IS_VISIBLE; } }
	virtual int						Is_Hidden(void) const														{ return !(Bits & IS_NOT_HIDDEN); }
	virtual void					Set_Hidden(int onoff)														{ if (onoff) { Bits &= ~IS_NOT_HIDDEN; } else { Bits |= IS_NOT_HIDDEN; } }
	virtual int						Is_Animation_Hidden(void) const											{ return !(Bits & IS_NOT_ANIMATION_HIDDEN); }
	virtual void					Set_Animation_Hidden(int onoff)											{ if (onoff) { Bits &= ~IS_NOT_ANIMATION_HIDDEN; } else { Bits |= IS_NOT_ANIMATION_HIDDEN; } }
	virtual int						Is_Force_Visible(void) const												{ return Bits & IS_FORCE_VISIBLE; }
	virtual void					Set_Force_Visible(int onoff)          									{ if (onoff) { Bits |= IS_FORCE_VISIBLE; } else { Bits &= ~IS_FORCE_VISIBLE; } }
	virtual int						Get_Collision_Type(void) const											{ return (Bits & COLL_TYPE_MASK); }
	virtual void					Set_Collision_Type(int type)												{ Bits &= ~COLL_TYPE_MASK; Bits |= (type & COLL_TYPE_MASK) | COLL_TYPE_ALL; }
	virtual float					Get_Native_Screen_Size(void) const										{ return NativeScreenSize; }
	virtual void					Set_Native_Screen_Size(float screensize)								{ NativeScreenSize = screensize; }
	void								Invalidate_Cached_Bounding_Volumes(void) const						{ Bits &= ~BOUNDING_VOLUMES_VALID; }

	enum
	{
		COLL_TYPE_MASK =		0x000000FF,
		IS_VISIBLE =					0x00000100,
		IS_NOT_HIDDEN =				0x00000200,
		IS_NOT_ANIMATION_HIDDEN =	0x00000400,
		IS_FORCE_VISIBLE =			0x00000800,
		BOUNDING_VOLUMES_VALID =	0x00002000,
		IS_REALLY_VISIBLE =			IS_VISIBLE | IS_NOT_HIDDEN | IS_NOT_ANIMATION_HIDDEN,
		IS_NOT_HIDDEN_AT_ALL =     IS_NOT_HIDDEN | IS_NOT_ANIMATION_HIDDEN,
		DEFAULT_BITS =					0x01 /* COLL_TYPE_ALL */ | IS_NOT_HIDDEN | IS_NOT_ANIMATION_HIDDEN,
	};

	mutable unsigned long		Bits;
	Matrix3D							Transform;
	float								NativeScreenSize;
};

/* predlod.h */
class PredictiveLODOptimizerClass
{
public:
	static void Add_Object(RenderObjClass *robj);
	static void Add_Cost(float cost);
};

/* coltest.h (lineseg.h and castres.h are the real ones, through aabox.h) */
class RayCollisionTestClass
{
public:
	LineSegClass Ray;
	CastResultStruct *Result;
	RenderObjClass *CollidedRenderObj;
	int CollisionType;
};

#endif
