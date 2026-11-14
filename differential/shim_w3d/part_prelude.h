/* Stand-ins for the particle emitter and buffer render objects (GitHub #49;
** differential tests only; see build.rs and differential/wrap_w3d_part.cpp).
** Included by the generated w3d_gen/part_emt_def.h before the class bodies
** of part_buf.h and part_emt.h (verbatim).
**
** TRANSCRIBED (stand-ins), because the real ones need the renderer, the
** scene or the whole render-object hierarchy (rendobj.h):
**   - RenderObjClass: only what the two classes use of it: the transform
**     (Get_Transform, Set_Transform, Get_Position, Transform), the scene
**     pointer (Scene, Is_In_Scene, Add, Notify_Added, Notify_Removed:
**     rendobj.cpp's own bodies, Add without the scene's list), the
**     visibility bits (Set_Hidden, Set_Visible, Set_Animation_Hidden,
**     Set_Force_Visible, Is_Not_Hidden_At_All, as rendobj.h's bit tests),
**     the class ids, AT_MIN_LOD and AT_MAX_LOD (rendobj.cpp's values) and
**     no-op bounding-volume caching (nothing logged reads it).
**   - SceneClass: Register and Unregister record nothing.
**   - WW3D: the clock (Get_Sync_Time, Get_Frame_Time, Get_Frame_Count) from
**     the wrapper's globals; sorting reported enabled and static sort lists
**     off, so ParticleBufferClass::Render draws directly.
**   - (#65) CameraClass, ViewportClass and PredictiveLODOptimizerClass, for
**     ParticleBufferClass::Prepare_LOD (renamed by macro to PartLod*, so that
**     no other library's stand-in of the same name can be merged with them):
**     the camera's Get_Position, Get_Viewport (ViewportClass's Width and
**     Height are camera.h's: Max - Min) and Get_View_Plane answer from fields
**     the driver sets; Add_Object records the object's Get_Cost() (the
**     optimizer's list and Optimize_LODs are not needed: Zero Hour never
**     calls Optimize_LODs). RenderObjClass's LOD defaults (rendobj.h's
**     inline bodies; Get_Cost is rendobj.cpp's, over Get_Num_Polys 0).
**     SceneClass::Register counts RELEASE registrations
**     (g_part_release_count, defined by wrap_part_lod.cpp); Remove goes
**     through Remove_Render_Object and Notify_Removed as rendobj.cpp's and
**     scene.cpp's do. The bounding-volume cache is kept as rendobj.h and
**     rendobj.cpp keep it (BOUNDING_VOLUMES_VALID, Get_Bounding_Sphere,
**     Update_Cached_Bounding_Volumes with ObjectScale 1).
**     Corrected 2026-11-14 (#65, batch b): the visibility bits are
**     rendobj.h's: DEFAULT_BITS is not visible, and Is_Not_Hidden_At_All
**     tests IS_NOT_HIDDEN | IS_NOT_ANIMATION_HIDDEN only (the stand-in tested
**     Visible too, as the Rust did, so the differential could not see it).
**     (#65, batch c) SceneClass's Add_Render_Object, Remove_Render_Object,
**     Register and Unregister are virtual, as scene.h declares them (the
**     bodies here stay the default; shim_w3d/hlod_release_prelude.h's
**     SimpleSceneClass overrides them with scene.cpp's); RenderObjClass has
**     its container (Get_Container, Set_Container: rendobj.cpp's), the
**     default Remove_Sub_Object (rendobj.h: 0), the default On_Frame_Update
**     (rendobj.h: nothing) and rendobj.cpp's Remove for a
**     contained object (Container->Remove_Sub_Object(this)).
**   - PointGroupClass, SegLineRendererClass, LineGroupClass: the setters
**     store their argument; Render records what was set (the wrapper logs
**     it); the line renderer's getters answer from the stored line
**     properties as seglinerenderer.h's Init does (W3D_ELINE_* flags).
*/
#ifndef SHIM_PART_PRELUDE_H
#define SHIM_PART_PRELUDE_H

#include "always.h"
#include "refcount.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "sphere.h"
#include "aabox.h"
#include "shader.h"
#include "w3d_file.h"
#include "sharebuf.h"

#ifndef WWPROFILE
#define WWPROFILE(name)
#endif

#define CameraClass PartLodCameraClass
#define ViewportClass PartLodViewportClass
#define PredictiveLODOptimizerClass PartLodOptimizerClass

class RenderObjClass;
class RenderInfoClass;
class StringClass;

/* camera.h's ViewportClass (Width, Height) */
class ViewportClass
{
public:
	ViewportClass(void) : Min(0, 0), Max(1, 1) {}
	float Width(void) const { return Max.X - Min.X; }
	float Height(void) const { return Max.Y - Min.Y; }
	Vector2 Min;
	Vector2 Max;
};

/* What Prepare_LOD reads of the camera. */
class CameraClass
{
public:
	Vector3 Get_Position(void) const { return Position; }
	const ViewportClass &Get_Viewport(void) const { return Viewport; }
	void Get_View_Plane(Vector2 &set_min, Vector2 &set_max) const { set_min = ViewPlaneMin; set_max = ViewPlaneMax; }
	Vector3 Position;
	ViewportClass Viewport;
	Vector2 ViewPlaneMin, ViewPlaneMax;
};

class PredictiveLODOptimizerClass
{
public:
	static void Add_Object(RenderObjClass *robj);
};

extern int g_part_release_count;
void ref_part_lod_note_release(RenderObjClass *obj); /* wrap_part_lod.cpp */

class SceneClass
{
public:
	enum RegType { ON_FRAME_UPDATE = 0, LIGHT, RELEASE };
	virtual ~SceneClass(void) {}
	virtual void Register(RenderObjClass *obj, RegType for_what) { if (for_what == RELEASE) { g_part_release_count++; ref_part_lod_note_release(obj); } }
	virtual void Unregister(RenderObjClass *, RegType) {}
	virtual void Add_Render_Object(RenderObjClass *obj);
	virtual void Remove_Render_Object(RenderObjClass *obj);
};

class WW3D
{
public:
	static unsigned int Get_Sync_Time(void);
	static unsigned int Get_Frame_Time(void);
	static unsigned int Get_Frame_Count(void);
	static bool Is_Sorting_Enabled(void) { return true; }
	static bool Are_Static_Sort_Lists_Enabled(void) { return false; }
	static void Add_To_Static_Sort_List(RenderObjClass *, unsigned int) {}
};

class RenderObjClass : public RefCountClass
{
public:
	enum {
		CLASSID_PARTICLEEMITTER = 14,
		CLASSID_PARTICLEBUFFER = 15,
	};
	static const float AT_MIN_LOD;
	static const float AT_MAX_LOD;

	RenderObjClass(void) : Transform(1), Scene(NULL), Container(NULL), BVValid(false), Visible(0), Hidden(0), AnimHidden(0), ForceVisible(0) {}
	RenderObjClass(const RenderObjClass &src) : RefCountClass(src), Transform(src.Transform), Scene(NULL), Container(NULL), BVValid(false), Visible(src.Visible),
		Hidden(src.Hidden), AnimHidden(src.AnimHidden), ForceVisible(src.ForceVisible) {}
	RenderObjClass &operator=(const RenderObjClass &) { return *this; }
	virtual ~RenderObjClass(void) {}

	virtual void Notify_Added(SceneClass *scene) { Scene = scene; }
	virtual void Notify_Removed(SceneClass *) { Scene = NULL; }
	void Add(SceneClass *scene) { Scene = scene; Scene->Add_Render_Object(this); }
	/* ~~rendobj.cpp's Remove for a top-level object (no container)~~ (#65 batch c) rendobj.cpp's Remove */
	void Remove(void)
	{
		if (Container == NULL) {
			if (Scene != NULL) {
				Scene->Remove_Render_Object(this);
				return;
			}
		} else {
			Container->Remove_Sub_Object(this);
			return;
		}
	}
	/* rendobj.h / rendobj.cpp */
	RenderObjClass *Get_Container(void) const { return Container; }
	virtual void Set_Container(RenderObjClass *con) { Container = con; }
	virtual int Remove_Sub_Object(RenderObjClass *) { return 0; }
	/* rendobj.h (#65 batch c: the scene's update loop calls it through the base) */
	virtual void On_Frame_Update(void) {}
	bool Is_In_Scene(void) const { return Scene != NULL; }

	const Matrix3D &Get_Transform(void) const { return Transform; }
	void Set_Transform(const Matrix3D &m) { Transform = m; }
	Vector3 Get_Position(void) const { return Transform.Get_Translation(); }

	virtual void Set_Hidden(int onoff) { Hidden = onoff; }
	virtual void Set_Visible(int onoff) { Visible = onoff; }
	virtual void Set_Animation_Hidden(int onoff) { AnimHidden = onoff; }
	virtual void Set_Force_Visible(int onoff) { ForceVisible = onoff; }
	virtual int Is_Not_Hidden_At_All(void) { return !Hidden && !AnimHidden; }

	/* rendobj.h's LOD defaults; Get_Cost is rendobj.cpp's */
	virtual int Get_Num_Polys(void) const { return 0; }
	virtual void Increment_LOD(void) {}
	virtual void Decrement_LOD(void) {}
	virtual float Get_Cost(void) const { int polycount = Get_Num_Polys(); float cost = (polycount != 0) ? polycount : 0.000001f; return cost; }
	virtual float Get_Value(void) const { return AT_MIN_LOD; }
	virtual float Get_Post_Increment_Value(void) const { return AT_MAX_LOD; }
	virtual void Set_LOD_Level(int) {}
	virtual int Get_LOD_Level(void) const { return 0; }
	virtual int Get_LOD_Count(void) const { return 1; }
	virtual void Set_LOD_Bias(float) {}
	virtual bool Is_Complete(void) { return false; }
	virtual void Add_Dependencies_To_List(DynamicVectorClass<StringClass> &, bool = false) {}
	virtual void Get_Obj_Space_Bounding_Sphere(SphereClass &sphere) const { sphere.Center.Set(0, 0, 0); sphere.Radius = 0; }
	virtual void Get_Obj_Space_Bounding_Box(AABoxClass &box) const { box.Center.Set(0, 0, 0); box.Extent.Set(0, 0, 0); }
	/* rendobj.cpp (ObjectScale 1) */
	virtual void Update_Cached_Bounding_Volumes(void) const
	{
		Get_Obj_Space_Bounding_Box(CachedBoundingBox);
		Get_Obj_Space_Bounding_Sphere(CachedBoundingSphere);
		Get_Transform().mulVector3(CachedBoundingSphere.Center);
		CachedBoundingSphere.Radius = 1.0f * CachedBoundingSphere.Radius;
		CachedBoundingBox.Transform(Get_Transform());
		Validate_Cached_Bounding_Volumes();
	}
	void Invalidate_Cached_Bounding_Volumes(void) const { BVValid = false; }
	void Validate_Cached_Bounding_Volumes(void) const { BVValid = true; }
	const SphereClass &Get_Bounding_Sphere(void) const
	{
		if (!BVValid) {
			Update_Cached_Bounding_Volumes();
		}
		return CachedBoundingSphere;
	}

	Matrix3D Transform;
	SceneClass *Scene;
	RenderObjClass *Container;
	mutable SphereClass CachedBoundingSphere;
	mutable AABoxClass CachedBoundingBox;
	mutable bool BVValid;
	int Visible, Hidden, AnimHidden, ForceVisible;
};

inline void SceneClass::Add_Render_Object(RenderObjClass *obj) { obj->Notify_Added(this); }
/* scene.cpp's SceneClass::Remove_Render_Object */
inline void SceneClass::Remove_Render_Object(RenderObjClass *obj) { obj->Notify_Removed(this); }

/* What the particle buffer handed its renderer at the last Render. */
struct PartDrawRecord
{
	int kind; /* 0 none, 1 point group, 2 line runs, 3 line group */
};

class PointGroupClass
{
public:
	enum PointModeEnum { TRIS, QUADS, SCREENSPACE };
	enum FlagsType { TRANSFORM };
	PointGroupClass(void) : Mode(TRIS), Frccl2(0), Texture(NULL), DefSize(0), DefColor(1, 1, 1), DefAlpha(1), DefOrientation(0), DefFrame(0),
		Locs(NULL), Diffuse(NULL), Apt(NULL), Sizes(NULL), Orientations(NULL), Frames(NULL), Count(0) {}
	virtual ~PointGroupClass(void) {}
	void Set_Arrays(ShareBufferClass<Vector3> *locs, ShareBufferClass<Vector4> *diffuse = NULL, ShareBufferClass<unsigned int> *apt = NULL,
		ShareBufferClass<float> *sizes = NULL, ShareBufferClass<unsigned char> *orientations = NULL, ShareBufferClass<unsigned char> *frames = NULL,
		int active_point_count = -1, float = 0.0f, float = 0.0f, float = 0.0f, float = 0.0f)
	{
		Locs = locs; Diffuse = diffuse; Apt = apt; Sizes = sizes; Orientations = orientations; Frames = frames; Count = active_point_count;
	}
	void Set_Point_Size(float size) { DefSize = size; }
	void Set_Point_Color(Vector3 color) { DefColor = color; }
	void Set_Point_Alpha(float alpha) { DefAlpha = alpha; }
	void Set_Point_Orientation(unsigned char o) { DefOrientation = o; }
	void Set_Point_Frame(unsigned char f) { DefFrame = f; }
	void Set_Point_Mode(PointModeEnum mode) { Mode = mode; }
	void Set_Flag(FlagsType, bool) {}
	void Set_Texture(TextureClass *t) { Texture = t; }
	TextureClass *Get_Texture(void) { return Texture; }
	TextureClass *Peek_Texture(void) { return Texture; }
	void Set_Shader(ShaderClass s) { Shader = s; }
	ShaderClass Get_Shader(void) { return Shader; }
	unsigned char Get_Frame_Row_Column_Count_Log2(void) { return Frccl2; }
	/* pointgr.cpp: MIN(frccl2, 4) */
	void Set_Frame_Row_Column_Count_Log2(unsigned char frccl2) { Frccl2 = MIN(frccl2, 4); }
	void Render(RenderInfoClass &);

	PointModeEnum Mode;
	unsigned char Frccl2;
	TextureClass *Texture;
	ShaderClass Shader;
	float DefSize;
	Vector3 DefColor;
	float DefAlpha;
	unsigned char DefOrientation, DefFrame;
	ShareBufferClass<Vector3> *Locs;
	ShareBufferClass<Vector4> *Diffuse;
	ShareBufferClass<unsigned int> *Apt;
	ShareBufferClass<float> *Sizes;
	ShareBufferClass<unsigned char> *Orientations, *Frames;
	int Count;
};

class SegLineRendererClass
{
public:
	enum TextureMapMode { UNIFORM_WIDTH_TEXTURE_MAP = 0, UNIFORM_LENGTH_TEXTURE_MAP, TILED_TEXTURE_MAP };
	SegLineRendererClass(void) : Texture(NULL), Width(0), FreezeRandom(0) { memset(&Props, 0, sizeof(Props)); }
	void Init(const W3dEmitterLinePropertiesStruct &props) { Props = props; }
	void Set_Texture(TextureClass *t) { Texture = t; }
	TextureClass *Get_Texture(void) const { return Texture; }
	void Set_Shader(ShaderClass s) { Shader = s; }
	ShaderClass Get_Shader(void) const { return Shader; }
	void Set_Width(float w) { Width = w; }
	/* seglinerenderer.cpp */
	void Scale(float scale) { Width *= scale; Props.NoiseAmplitude *= scale; }
	void Set_Freeze_Random(int onoff) { FreezeRandom = onoff; }
	int Get_Texture_Mapping_Mode(void) const { return (Props.Flags & W3D_ELINE_TEXTURE_MAP_MODE_MASK) >> W3D_ELINE_TEXTURE_MAP_MODE_OFFSET; }
	int Is_Merge_Intersections(void) const { return (Props.Flags & W3D_ELINE_MERGE_INTERSECTIONS) != 0; }
	int Is_Freeze_Random(void) const { return (Props.Flags & W3D_ELINE_FREEZE_RANDOM) != 0; }
	int Is_Sorting_Disabled(void) const { return (Props.Flags & W3D_ELINE_DISABLE_SORTING) != 0; }
	int Are_End_Caps_Enabled(void) const { return (Props.Flags & W3D_ELINE_END_CAPS) != 0; }
	int Get_Current_Subdivision_Level(void) const { return Props.SubdivisionLevel; }
	float Get_Noise_Amplitude(void) const { return Props.NoiseAmplitude; }
	float Get_Merge_Abort_Factor(void) const { return Props.MergeAbortFactor; }
	float Get_Texture_Tile_Factor(void) const { return Props.TextureTileFactor; }
	Vector2 Get_UV_Offset_Rate(void) const { return Vector2(Props.UPerSec, Props.VPerSec); }
	void Render(RenderInfoClass &, const Matrix3D &, unsigned int point_count, Vector3 *points, const SphereClass &, Vector4 *rgbas);

	W3dEmitterLinePropertiesStruct Props;
	TextureClass *Texture;
	ShaderClass Shader;
	float Width;
	int FreezeRandom;
};

class LineGroupClass
{
public:
	enum FlagsType { TRANSFORM };
	enum LineModeType { TETRAHEDRON, PRISM };
	LineGroupClass(void) : Texture(NULL), Mode(TETRAHEDRON), DefColor(1, 1, 1), DefAlpha(1), DefSize(0), DefUCoord(0), DefTail(0, 0, 0, 0),
		Head(NULL), Tail(NULL), Diffuse(NULL), TailDiffuse(NULL), Alt(NULL), Sizes(NULL), UCoords(NULL), Count(0) {}
	virtual ~LineGroupClass(void) {}
	void Set_Flag(FlagsType, bool) {}
	void Set_Texture(TextureClass *t) { Texture = t; }
	TextureClass *Get_Texture(void) { return Texture; }
	TextureClass *Peek_Texture(void) { return Texture; }
	void Set_Shader(ShaderClass s) { Shader = s; }
	ShaderClass Get_Shader(void) { return Shader; }
	void Set_Line_Mode(LineModeType m) { Mode = m; }
	void Set_Line_Color(const Vector3 &c) { DefColor = c; }
	void Set_Line_Alpha(float a) { DefAlpha = a; }
	void Set_Line_Size(float s) { DefSize = s; }
	void Set_Line_UCoord(float u) { DefUCoord = u; }
	void Set_Tail_Diffuse(const Vector4 &t) { DefTail = t; }
	void Set_Arrays(ShareBufferClass<Vector3> *startlocs, ShareBufferClass<Vector3> *endlocs, ShareBufferClass<Vector4> *diffuse,
		ShareBufferClass<Vector4> *tail_diffuse, ShareBufferClass<unsigned int> *alt, ShareBufferClass<float> *sizes,
		ShareBufferClass<float> *ucoords, int active_line_count)
	{
		Head = startlocs; Tail = endlocs; Diffuse = diffuse; TailDiffuse = tail_diffuse; Alt = alt; Sizes = sizes; UCoords = ucoords;
		Count = active_line_count;
	}
	void Render(RenderInfoClass &);

	TextureClass *Texture;
	ShaderClass Shader;
	LineModeType Mode;
	Vector3 DefColor;
	float DefAlpha, DefSize, DefUCoord;
	Vector4 DefTail;
	ShareBufferClass<Vector3> *Head, *Tail;
	ShareBufferClass<Vector4> *Diffuse, *TailDiffuse;
	ShareBufferClass<unsigned int> *Alt;
	ShareBufferClass<float> *Sizes, *UCoords;
	int Count;
};

#endif
