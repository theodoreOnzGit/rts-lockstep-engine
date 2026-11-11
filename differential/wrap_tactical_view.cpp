/* Differential-test wrapper for the tactical view's height, origin and screen-to-world mapping (GitHub #138, upstream
** commit 0a05454d), cut verbatim by build.rs into OUT_DIR/tactical_view_gen:
**   tv_view_h.inc      GameEngine/Include/GameClient/View.h, from the comment of `getScreenCornerWorldPointsAtZ`'s
**                      declaration to `virtual void forceRedraw() = 0;` (the declaration and the inline setWidth,
**                      getWidth, setHeight, getHeight, setOrigin, getOrigin), inside the stand-in View class;
**   tv_corners.inc     GameEngine/Source/GameClient/View.cpp, View::getScreenCornerWorldPointsAtZ (to the xfer's
**                      comment);
**   tv_size.inc        GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp, W3DView::setHeight, setWidth and
**                      setOrigin (to buildCameraTransform's comment);
**   tv_pick.inc        W3DView.cpp, W3DView::getPickRay (to setCameraTransform's comment);
**   tv_at_z.inc        W3DView.cpp, W3DView::screenToWorldAtZ (to cameraEnableSlaveMode);
**   tv_convert.inc     GameEngineDevice/Source/W3DDevice/Common/W3DConvert.cpp, PixelScreenToW3DLogicalScreen (to the
**                      end of the file);
**   tv_cam_inline.inc  Libraries/Source/WWVegas/WW3D2/camera.h, CameraClass::Get_Depth, Set_Projection_Type,
**                      Get_Projection_Type, Set_Viewport and Get_Viewport (to Set_Depth_Range);
**   tv_cam_plane.inc   WW3D2/camera.cpp, CameraClass::Set_View_Plane(hfov, vfov) and Set_Aspect_Ratio (to
**                      Get_View_Plane);
**   tv_cam_unproj.inc  camera.cpp, CameraClass::Un_Project (to Transform_To_View_Space).
** No edit. The stand-ins: a CameraClass declaring the members those functions use (camera.h's names and types;
** `Get_Position` is RenderObjClass's, the transform's translation), set up as its constructor does (camera.cpp:92: a
** perspective camera, the viewport (0,0)-(1,1), 4:3, clip 1 to the far plane given, `Set_View_Plane(DEG_TO_RADF(50))`),
** then the transform given; a W3DView holding m_3DCamera; and TheDisplay's size. WWMath (Vector2, Vector3, Matrix3D) is
** upstream's own headers; `tan` is the test binary's interposer (tests/support).
**
** Entry point (extern "C", tests/differential_tactical_view.rs):
**   ref_tactical_view_seq(in, nin, out, cap) runs one sequence and writes its words to out.
**   `in` is the setup, 18 words [display width, height, the view's m_width, m_height, the far clip's bits, the
**   transform's 12 floats' bits (row-major 3x4), unused], then steps of 4 words [kind, a, b, c]:
**     kind 0  setWidth(a)               kind 1  setHeight(a)            kind 2  setOrigin(a, b)
**     kind 3  the display's size (a, b)
**     kind 4  getPickRay((a, b))                          -> [4, start x, y, z, end x, y, z] (bits)
**     kind 5  screenToWorldAtZ((a, b), bits c)            -> [5, x, y, z]
**     kind 6  getScreenCornerWorldPointsAtZ(bits a)       -> [6, the four points' x, y, z, in the argument order]
**   After each step [100, m_width, m_height, m_originX, m_originY, AspectRatio, ViewPlane min x, y, max x, y,
**   Viewport min x, y, max x, y] (floats as bits). */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <vector>
#include "Lib/BaseType.h"
#include "wwmath.h"
#include "vector2.h"
#include "vector3.h"
#include "matrix3d.h"

namespace tvx {

std::vector<int> g_out;
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
static Real realOf(Int i) { Real r; memcpy(&r, &i, sizeof r); return r; }

/* ---- the display ---- */
struct DisplayStub
{
	UnsignedInt m_width, m_height;
	UnsignedInt getWidth() { return m_width; }
	UnsignedInt getHeight() { return m_height; }
};
DisplayStub g_display;
DisplayStub *TheDisplay = &g_display;

/* ---- CameraClass ---- */
class ViewportClass
{
public:
	Vector2 Min;
	Vector2 Max;
};
class CameraClass
{
public:
	enum ProjectionType { PERSPECTIVE = 0, ORTHO };
	ProjectionType Projection;
	ViewportClass Viewport;
	ViewportClass ViewPlane;
	float AspectRatio;
	float ZNear, ZFar;
	bool FrustumValid;
	Matrix3D Transform;

	float Get_Depth(void) const;
	void Set_Projection_Type(ProjectionType ptype);
	ProjectionType Get_Projection_Type(void);
	void Set_Viewport(const Vector2 & min,const Vector2 & max);
	void Get_Viewport(Vector2 & set_min,Vector2 & set_max) const;
	void Set_View_Plane(float hfov,float vfov = -1);
	void Set_Aspect_Ratio(float width_to_height);
	void Un_Project(Vector3 & dest,const Vector2 & view_point) const;
	/* RenderObjClass::Get_Position: the transform's translation */
	Vector3 Get_Position(void) const { return Transform.Get_Translation(); }
};
#include "tv_cam_inline.inc"
#include "tv_cam_plane.inc"
#include "tv_cam_unproj.inc"

void PixelScreenToW3DLogicalScreen( Int screenX, Int screenY, Real *logX, Real *logY, Int screenWidth, Int screenHeight );
#include "tv_convert.inc"

/* ---- View and W3DView ---- */
class View
{
public:
	Int m_width, m_height;
	Int m_originX, m_originY;
	virtual ~View() {}
	virtual void screenToWorldAtZ( const ICoord2D *s, Coord3D *w, Real z ) = 0;
#include "tv_view_h.inc"
};
#include "tv_corners.inc"

class W3DView : public View
{
public:
	CameraClass *m_3DCamera;
	virtual void setWidth( Int width );
	virtual void setHeight( Int height );
	virtual void setOrigin( Int x, Int y);
	virtual void screenToWorldAtZ( const ICoord2D *s, Coord3D *w, Real z );
	void getPickRay(const ICoord2D *screen, Vector3 *rayStart, Vector3 *rayEnd);
	void forceRedraw() {}
};
#include "tv_size.inc"
#include "tv_pick.inc"
#include "tv_at_z.inc"

CameraClass g_camera;
W3DView g_view;

static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }

static void state(void)
{
	CameraClass &c = g_camera;
	rec({100, g_view.m_width, g_view.m_height, g_view.m_originX, g_view.m_originY, bitsOf(c.AspectRatio),
		bitsOf(c.ViewPlane.Min.X), bitsOf(c.ViewPlane.Min.Y), bitsOf(c.ViewPlane.Max.X), bitsOf(c.ViewPlane.Max.Y),
		bitsOf(c.Viewport.Min.X), bitsOf(c.Viewport.Min.Y), bitsOf(c.Viewport.Max.X), bitsOf(c.Viewport.Max.Y)});
}

} // namespace tvx

extern "C" int ref_tactical_view_seq(const int *in, int nin, int *out, int cap)
{
	using namespace tvx;
	g_out.clear();
	g_display.m_width = (UnsignedInt)in[0];
	g_display.m_height = (UnsignedInt)in[1];
	/* CameraClass::CameraClass (camera.cpp:92), then the transform */
	CameraClass &c = g_camera;
	c.Projection = CameraClass::PERSPECTIVE;
	c.Viewport.Min.Set(0, 0);
	c.Viewport.Max.Set(1, 1);
	c.AspectRatio = 4.0f / 3.0f;
	c.ZNear = 1.0f;
	c.ZFar = 1000.0f;
	c.FrustumValid = false;
	c.Set_View_Plane(DEG_TO_RADF(50.0f));
	c.ZFar = realOf(in[4]);
	for (Int r = 0; r < 3; ++r)
		c.Transform[r].Set(realOf(in[5 + r * 4]), realOf(in[6 + r * 4]), realOf(in[7 + r * 4]), realOf(in[8 + r * 4]));
	g_view.m_3DCamera = &c;
	g_view.m_width = in[2];
	g_view.m_height = in[3];
	g_view.m_originX = g_view.m_originY = 0;
	for (int k = 18; k + 4 <= nin; k += 4) {
		const int *w = in + k;
		switch (w[0]) {
		case 0: g_view.setWidth(w[1]); break;
		case 1: g_view.setHeight(w[1]); break;
		case 2: g_view.setOrigin(w[1], w[2]); break;
		case 3: g_display.m_width = (UnsignedInt)w[1]; g_display.m_height = (UnsignedInt)w[2]; break;
		case 4: {
			ICoord2D s; s.x = w[1]; s.y = w[2];
			Vector3 a, b;
			g_view.getPickRay(&s, &a, &b);
			rec({4, bitsOf(a.X), bitsOf(a.Y), bitsOf(a.Z), bitsOf(b.X), bitsOf(b.Y), bitsOf(b.Z)});
			break;
		}
		case 5: {
			ICoord2D s; s.x = w[1]; s.y = w[2];
			Coord3D p;
			g_view.screenToWorldAtZ(&s, &p, realOf(w[3]));
			rec({5, bitsOf(p.x), bitsOf(p.y), bitsOf(p.z)});
			break;
		}
		case 6: {
			Coord3D p[4];
			g_view.getScreenCornerWorldPointsAtZ(&p[0], &p[1], &p[2], &p[3], realOf(w[1]));
			std::vector<Int> r(1, 6);
			for (Int i = 0; i < 4; ++i) {
				r.push_back(bitsOf(p[i].x));
				r.push_back(bitsOf(p[i].y));
				r.push_back(bitsOf(p[i].z));
			}
			rec(r);
			break;
		}
		}
		state();
	}
	int n = (int)g_out.size();
	if (n > cap)
		n = cap;
	if (n > 0)
		memcpy(out, &g_out[0], n * sizeof(int));
	return n;
}
