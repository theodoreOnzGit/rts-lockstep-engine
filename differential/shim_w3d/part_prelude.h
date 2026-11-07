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

class RenderObjClass;
class RenderInfoClass;
class CameraClass;
class StringClass;

class SceneClass
{
public:
	enum RegType { ON_FRAME_UPDATE = 0, LIGHT, RELEASE };
	void Register(RenderObjClass *, RegType) {}
	void Unregister(RenderObjClass *, RegType) {}
	void Add_Render_Object(RenderObjClass *obj);
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

	RenderObjClass(void) : Transform(1), Scene(NULL), Visible(1), Hidden(0), AnimHidden(0), ForceVisible(0) {}
	RenderObjClass(const RenderObjClass &src) : RefCountClass(src), Transform(src.Transform), Scene(NULL), Visible(src.Visible), Hidden(src.Hidden),
		AnimHidden(src.AnimHidden), ForceVisible(src.ForceVisible) {}
	RenderObjClass &operator=(const RenderObjClass &) { return *this; }
	virtual ~RenderObjClass(void) {}

	virtual void Notify_Added(SceneClass *scene) { Scene = scene; }
	virtual void Notify_Removed(SceneClass *) { Scene = NULL; }
	void Add(SceneClass *scene) { Scene = scene; Scene->Add_Render_Object(this); }
	void Remove(void) { Scene = NULL; }
	bool Is_In_Scene(void) const { return Scene != NULL; }

	const Matrix3D &Get_Transform(void) const { return Transform; }
	void Set_Transform(const Matrix3D &m) { Transform = m; }
	Vector3 Get_Position(void) const { return Transform.Get_Translation(); }

	virtual void Set_Hidden(int onoff) { Hidden = onoff; }
	virtual void Set_Visible(int onoff) { Visible = onoff; }
	virtual void Set_Animation_Hidden(int onoff) { AnimHidden = onoff; }
	virtual void Set_Force_Visible(int onoff) { ForceVisible = onoff; }
	virtual int Is_Not_Hidden_At_All(void) { return Visible && !Hidden && !AnimHidden; }

	virtual void Set_LOD_Bias(float) {}
	virtual bool Is_Complete(void) { return false; }
	virtual void Add_Dependencies_To_List(DynamicVectorClass<StringClass> &, bool = false) {}
	virtual void Update_Cached_Bounding_Volumes(void) const {}
	void Invalidate_Cached_Bounding_Volumes(void) const {}
	void Validate_Cached_Bounding_Volumes(void) const {}
	const SphereClass &Get_Bounding_Sphere(void) const { return CachedBoundingSphere; }

	Matrix3D Transform;
	SceneClass *Scene;
	mutable SphereClass CachedBoundingSphere;
	mutable AABoxClass CachedBoundingBox;
	int Visible, Hidden, AnimHidden, ForceVisible;
};

inline void SceneClass::Add_Render_Object(RenderObjClass *obj) { obj->Notify_Added(this); }

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
