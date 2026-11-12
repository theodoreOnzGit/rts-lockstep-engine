/* Differential-test wrapper for the tactical view's world-to-screen mapping (GitHub #146, upstream commit 0a05454d),
** cut verbatim by build.rs into OUT_DIR/world_to_screen_gen:
**   w2s_view_enum.inc   GameEngine/Include/GameClient/View.h, the WorldToScreenReturn enum (from its comment to
**                       `public:` and View's constructor), inside the stand-in View class;
**   w2s_view_w2s.inc    View.h, the inline `worldToScreen` (to `worldToScreenTriReturn`'s declaration);
**   w2s_tri.inc         GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp, W3DView::worldToScreenTriReturn (to
**                       screenToWorld's comment);
**   w2s_iterate.inc     W3DView.cpp, W3DView::iterateDrawablesInRegion (to pickDrawable's comment);
**   w2s_convert.inc     GameEngineDevice/Source/W3DDevice/Common/W3DConvert.cpp, W3DLogicalScreenToPixelScreen (to the
**                       banner of PixelScreenToW3DLogicalScreen);
**   w2s_project.inc     Libraries/Source/WWVegas/WW3D2/camera.cpp, CameraClass::Get_View_Plane and Project (to the
**                       banner of Project_Camera_Space_Point);
**   w2s_frustum.inc     camera.cpp, CameraClass::Update_Frustum (to the banner of Device_To_View_Space);
**   w2s_clip.inc        camera.cpp, CameraClass::Get_Clip_Planes (to Get_Horizontal_FOV).
** No edit. Matrix4x4 (Init_Perspective, Make_Identity, operator * (Matrix4x4, Vector3)) is upstream's own matrix4.h, and
** Matrix3D::Get_Inverse is upstream's matrix3d.cpp (in upstream_wwmath), which calls the SDK's D3DXMatrixInverse: the
** stand-in in differential/wrap_wwmath.cpp (NOT the SDK's code). The stand-ins here: a CameraClass declaring the members
** these functions use (camera.h's names and types; `Get_Transform` is RenderObjClass's), with a FrustumClass and an
** OBBoxClass whose `Init` and `Set` record nothing (Project does not read them); a W3DView holding m_3DCamera, its size
** and origin, `pickDrawable` (the input's choice) and `getPickTypesForContext`; Drawables (a position and the next one);
** TheGameClient (the first drawable) and TheInGameUI (`isInForceAttackMode`, false). REAL_TO_INT is BaseType.h's, through
** differential/wrap_base_type.cpp's x87 helpers.
**
** Entry point (extern "C", tests/differential_world_to_screen.rs):
**   ref_world_to_screen_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in` is the setup, 22 words [m_width, m_height, m_originX, m_originY, ZNear bits, ZFar bits, ViewPlane min x, min y,
**   max x, max y bits, the transform's 12 floats' bits (row-major 3x4)], then [n], then n drawables' positions (3 bits
**   words each), then steps of 7 words [kind, a, b, c, d, e, f]:
**     kind 0  worldToScreenTriReturn((bits a, b, c))  -> [0, the return, s.x, s.y]
**     kind 1  worldToScreen((bits a, b, c))           -> [1, the bool, s.x, s.y]
**     kind 2  m_3DCamera->Project(dest, (a, b, c))    -> [2, the result, dest X, Y, Z bits]
**     kind 3  iterateDrawablesInRegion(&{(a, b), (c, d)}) with pickDrawable giving drawable e (-1: NULL; n: a drawable
**             not in TheGameClient's list, which the generator uses to reach the empty list's quirk), the callback
**             answering bit (index % 31) of f        -> [3, each drawable called back's index ..., -1, the count]
**     kind 4  iterateDrawablesInRegion(NULL), the same callback -> [3, ..., -1, the count]
**     kind 5  m_width = a, m_height = b, m_originX = c, m_originY = d (the size and origin, as setWidth/setHeight leave
**             them; the camera is not touched) */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <math.h>
#include <vector>
#include "Lib/BaseType.h"
#include "wwmath.h"
#include "vector2.h"
#include "vector3.h"
#include "vector4.h"
#include "matrix3d.h"
#include "matrix4.h"

extern "C" {
int ref_fast_float2long_round(float f);
float ref_fast_float_trunc(float f);
}
/* BaseType.h line 231 (the shim leaves the asm helpers to differential/wrap_base_type.cpp) */
#define REAL_TO_INT(x) ((Int)(ref_fast_float2long_round(ref_fast_float_trunc(x))))

namespace w2s {

std::vector<int> g_out;
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
static Real realOf(Int i) { Real r; memcpy(&r, &i, sizeof r); return r; }
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }

/* ---- CameraClass ---- */
class ViewportClass
{
public:
	Vector2 Min;
	Vector2 Max;
};
/* FrustumClass::Init and OBBoxClass (frustum.h, obbox.h): Project does not read them */
class FrustumClass
{
public:
	void Init(const Matrix3D &, const Vector2 &, const Vector2 &, float, float) {}
};
class Matrix3Stub
{
public:
	void Set(const Matrix3D &) {}
};
class OBBoxClass
{
public:
	Vector3 Center;
	Vector3 Extent;
	Matrix3Stub Basis;
};
class CameraClass
{
public:
	enum ProjectionType { PERSPECTIVE = 0, ORTHO };
	enum ProjectionResType
	{
		INSIDE_FRUSTUM,
		OUTSIDE_FRUSTUM,
		OUTSIDE_NEAR_CLIP,
		OUTSIDE_FAR_CLIP,
	};
	ProjectionType Projection;
	ViewportClass ViewPlane;
	float ZNear, ZFar;
	Matrix3D Transform;
	mutable bool FrustumValid;
	mutable FrustumClass Frustum;
	mutable FrustumClass ViewSpaceFrustum;
	mutable OBBoxClass NearClipBBox;
	mutable Matrix4x4 ProjectionTransform;
	mutable Matrix3D CameraInvTransform;

	/* RenderObjClass::Get_Transform */
	const Matrix3D &Get_Transform(void) const { return Transform; }
	void Get_View_Plane(Vector2 & set_min,Vector2 & set_max) const;
	void Get_Clip_Planes(float & znear,float & zfar) const;
	ProjectionResType Project(Vector3 & dest,const Vector3 & ws_point) const;
	void Update_Frustum(void) const;
};
#include "w2s_project.inc"
#include "w2s_frustum.inc"
#include "w2s_clip.inc"

void W3DLogicalScreenToPixelScreen( Real logX, Real logY, Int *screenX, Int *screenY, Int screenWidth, Int screenHeight );
#include "w2s_convert.inc"

/* ---- Drawables, TheGameClient, TheInGameUI ---- */
class Drawable
{
public:
	Int m_index;
	Coord3D m_pos;
	Drawable *m_next;
	const Coord3D *getPosition() const { return &m_pos; }
	Drawable *getNextDrawable() const { return m_next; }
};
struct GameClientStub
{
	Drawable *m_first;
	Drawable *firstDrawable() { return m_first; }
};
GameClientStub g_client;
GameClientStub *TheGameClient = &g_client;
struct InGameUIStub
{
	Bool isInForceAttackMode() { return FALSE; }
};
InGameUIStub g_ui;
InGameUIStub *TheInGameUI = &g_ui;
enum PickType { PICK_TYPE_SELECTABLE = 1 };

/* ---- View and W3DView ---- */
class View
{
public:
	Int m_width, m_height;
	Int m_originX, m_originY;
#include "w2s_view_enum.inc"
	virtual ~View() {}
	virtual Int getWidth( void ) { return m_width; }
	virtual Int getHeight( void ) { return m_height; }
#include "w2s_view_w2s.inc"
	virtual WorldToScreenReturn worldToScreenTriReturn(const Coord3D *w, ICoord2D *s ) = 0;
};

class W3DView : public View
{
public:
	CameraClass *m_3DCamera;
	Drawable *m_picked;
	virtual WorldToScreenReturn worldToScreenTriReturn( const Coord3D *w, ICoord2D *s );
	Int iterateDrawablesInRegion( IRegion2D *screenRegion, Bool (*callback)( Drawable *draw, void *userData ), void *userData );
	/* the input's choice (the port casts no ray on the render objects) */
	Drawable *pickDrawable( const ICoord2D *, Bool, PickType ) { return m_picked; }
	Int getPickTypesForContext( Bool ) { return PICK_TYPE_SELECTABLE; }
};
#include "w2s_tri.inc"
#include "w2s_iterate.inc"

CameraClass g_camera;
W3DView g_view;
std::vector<Drawable> g_drawables;

static Bool answer(Drawable *draw, void *userData)
{
	UnsignedInt mask = *(UnsignedInt *)userData;
	g_out.push_back(draw->m_index);
	return ((mask >> (draw->m_index % 31)) & 1) != 0;
}

} // namespace w2s

extern "C" int ref_world_to_screen_seq(const int *in, int nin, int *out, int cap)
{
	using namespace w2s;
	g_out.clear();
	CameraClass &c = g_camera;
	c.Projection = CameraClass::PERSPECTIVE;
	c.FrustumValid = false;
	c.ZNear = realOf(in[4]);
	c.ZFar = realOf(in[5]);
	c.ViewPlane.Min.Set(realOf(in[6]), realOf(in[7]));
	c.ViewPlane.Max.Set(realOf(in[8]), realOf(in[9]));
	for (Int r = 0; r < 3; ++r)
		c.Transform[r].Set(realOf(in[10 + r * 4]), realOf(in[11 + r * 4]), realOf(in[12 + r * 4]), realOf(in[13 + r * 4]));
	g_view.m_3DCamera = &c;
	g_view.m_width = in[0];
	g_view.m_height = in[1];
	g_view.m_originX = in[2];
	g_view.m_originY = in[3];
	Int n = in[22];
	/* (one more drawable, index n, outside TheGameClient's list: what pickDrawable may give) */
	g_drawables.assign(n + 1, Drawable());
	g_drawables[n].m_index = n;
	g_drawables[n].m_next = NULL;
	for (Int i = 0; i < n; ++i) {
		g_drawables[i].m_index = i;
		g_drawables[i].m_pos.x = realOf(in[23 + i * 3]);
		g_drawables[i].m_pos.y = realOf(in[24 + i * 3]);
		g_drawables[i].m_pos.z = realOf(in[25 + i * 3]);
		g_drawables[i].m_next = (i + 1 < n) ? &g_drawables[i + 1] : NULL;
	}
	g_client.m_first = n > 0 ? &g_drawables[0] : NULL;
	for (int k = 23 + n * 3; k + 7 <= nin; k += 7) {
		const int *w = in + k;
		Coord3D p;
		p.x = realOf(w[1]);
		p.y = realOf(w[2]);
		p.z = realOf(w[3]);
		switch (w[0]) {
		case 0: {
			ICoord2D s; s.x = 12345; s.y = 12345;
			Int r = g_view.worldToScreenTriReturn(&p, &s);
			rec({0, r, s.x, s.y});
			break;
		}
		case 1: {
			ICoord2D s; s.x = 12345; s.y = 12345;
			Bool b = g_view.worldToScreen(&p, &s);
			rec({1, b ? 1 : 0, s.x, s.y});
			break;
		}
		case 2: {
			Vector3 world(p.x, p.y, p.z), dest(7, 7, 7);
			Int r = c.Project(dest, world);
			rec({2, r, bitsOf(dest.X), bitsOf(dest.Y), bitsOf(dest.Z)});
			break;
		}
		case 3:
		case 4: {
			IRegion2D region;
			region.lo.x = w[1];
			region.lo.y = w[2];
			region.hi.x = w[3];
			region.hi.y = w[4];
			g_view.m_picked = (w[5] >= 0 && w[5] <= n) ? &g_drawables[w[5]] : NULL;
			UnsignedInt mask = (UnsignedInt)w[6];
			g_out.push_back(3);
			Int count = g_view.iterateDrawablesInRegion(w[0] == 3 ? &region : NULL, answer, &mask);
			rec({-1, count});
			break;
		}
		case 5:
			g_view.m_width = w[1];
			g_view.m_height = w[2];
			g_view.m_originX = w[3];
			g_view.m_originY = w[4];
			break;
		}
	}
	int nout = (int)g_out.size();
	if (nout > cap)
		nout = cap;
	if (nout > 0)
		memcpy(out, &g_out[0], nout * sizeof(int));
	return nout;
}
