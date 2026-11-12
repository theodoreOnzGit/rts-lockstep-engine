/* Differential-test wrapper for W3DView::pickDrawable and what it reaches (GitHub #148, upstream commit 0a05454d), cut
** verbatim by build.rs into OUT_DIR/pick_drawable_gen:
**   pd_pick_type.inc     GameEngine/Include/GameClient/View.h, `enum PickType` (to the comment of `class View`);
**   pd_pick.inc          GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp, W3DView::pickDrawable (to the comment
**                        of screenToTerrain);
**   pd_cast.inc          GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp, RTS3DScene::castRay (to the banner
**                        of Visibility_Check);
**   pd_coll_type.inc     Libraries/Source/WWVegas/WW3D2/rendobj.h, Get_Collision_Type and Set_Collision_Type (to
**                        Is_Complete), inside the stand-in render object;
**   pd_hlod_cast.inc     WW3D2/hlod.cpp, HLodClass::Cast_Ray (to Cast_AABox);
**   pd_aabox_cast.inc    WW3D2/boxrobj.cpp, AABoxRenderObjClass::Cast_Ray (to its Cast_AABox);
**   pd_obbox_cast.inc    boxrobj.cpp, OBBoxRenderObjClass::Cast_Ray (to its Cast_AABox);
**   pd_colmath_head.inc  Libraries/Source/WWVegas/WWMath/colmathline.cpp, BoxTestStruct, BoxSideType, _box_normal and
**                        the prototype of Test_Aligned_Box (to the AAPlane collide);
**   pd_colmath_box.inc   colmathline.cpp, Collide(LineSeg, AABox), Collide(LineSeg, OBBox) and Test_Aligned_Box (to
**                        the end);
**   pd_model_draw.inc    GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp, setModelState's
**                        collision types (from "set collision type for render object" to "add render object to our
**                        scene"), as the body of the stand-in's `collisionTypes`;
**   pd_selectable.inc    W3DModelDraw.cpp, W3DModelDraw::setSelectable (to replaceIndicatorColor);
**   pd_pick_types.inc    GameEngine/Source/GameClient/SelectionInfo.cpp, getPickTypesForContext and
**                        getPickTypesForCurrentSelection (to translatePickTypesToKindof).
** No edit. The real WWMath headers (lineseg.h, castres.h, aabox.h, obbox.h, sphere.h, colmath.h) and WW3D2's coltest.h and
** coltype.h; RenderObjClass, the box render objects, HLodClass and CollisionMath are renamed by #define (Pd...) so that no
** symbol meets another archive's. The stand-ins: the render object (its bits, Is_Really_Visible, Is_Animation_Hidden, the
** bounding sphere, the user data), the HLod (its top LOD's models and additional models; sub-object transforms never
** dirty), the boxes (CachedBox), the scene's render list and its iterator, W3DDisplay::m_3DScene, W3DView (getPickRay gives
** the input's ray), TheWindowManager (the window under the cursor, a chain of windows each see-through or not), the
** drawables and their info, the template and object kinds, TheInGameUI (the GUI command, the selection).
**
** Entry point (extern "C", tests/differential_pick_drawable.rs):
**   ref_pick_drawable_seq(in, nin, out, cap): `in` is [n objects], then per object [drawable id (-1 none), collision
**   bits, really visible, sphere centre x, y, z and radius bits, number of top models, number of additional models], then
**   per model 18 words [0 AABox | 1 OBBox, collision bits, animation hidden, centre x y z, extent x y z bits, the basis'
**   9 bits (row-major)]; then steps of 20 words [kind, ...]:
**     kind 0  pickDrawable: [ray start x y z, end x y z bits, forceAttack, pickType, windows k, see-thru 0..2] ->
**             [0, the drawable's id, -2 none]
**     kind 1  castRay: [ray start, end bits, testAll, collisionType] -> [1, hit, collided object index (-1 none), the ray's
**             P1, Dir bits, Length bits]
**     kind 2  Collide: [0 AABox | 1 OBBox, ray start, end bits, box index (an object's model, by a flat index), result
**             Fraction bits, StartBad, ComputeContactPoint] -> [2, return, StartBad, Fraction bits, Normal bits, ContactPoint
**             bits]
**     kind 3  the model draw's collision types: [bits, the template's kinds (bit 0 SELECTABLE, 1 SHRUBBERY, 2 MINE, 3
**             FORCEATTACKABLE, 4 CLICK_THROUGH), an object, its kinds (bit 0 BRIDGE, 1 BRIDGE_TOWER, 2 STRUCTURE), rubble,
**             effectively dead] -> [3, the collision type]
**     kind 4  setSelectable: [bits, selectable] -> [4, the collision type]
**     kind 5  getPickTypesForContext: [forceAttack, a GUI command, its options, controllable, n selected (to 4), each -1
**             no object, 0, 1 flame weapon] -> [5, types] */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <list>
#include <vector>
#include "Lib/BaseType.h"
#include "wwmath.h"
#include "vector3.h"
#include "matrix3.h"
#include "matrix3d.h"
#define RenderObjClass PdRenderObjClass
#define CollisionMath PdCollisionMath
#define AABoxRenderObjClass PdAABoxRenderObjClass
#define OBBoxRenderObjClass PdOBBoxRenderObjClass
#define HLodClass PdHLodClass
#include "lineseg.h"
#include "castres.h"
#include "aabox.h"
#include "obbox.h"
#include "sphere.h"
#include "colmath.h"
#include "coltype.h"
#include "coltest.h"

/* ---- colmathline.cpp's box collides ---- */
#include "pd_colmath_head.inc"
#include "pd_colmath_box.inc"

/* ---- the render objects ---- */
class PdRenderObjClass
{
public:
	int Bits;
	int ReallyVisible;
	int AnimHidden;
	SphereClass Sphere;
	void *UserData;
	PdRenderObjClass() : Bits(COLL_TYPE_ALL), ReallyVisible(1), AnimHidden(0), UserData(NULL) {}
	virtual ~PdRenderObjClass() {}
	enum { COLL_TYPE_MASK = 0x000000FF };
#include "pd_coll_type.inc"
	int Is_Really_Visible( void ) { return ReallyVisible; }
	int Is_Animation_Hidden( void ) const { return AnimHidden; }
	const SphereClass & Get_Bounding_Sphere( void ) const { return Sphere; }
	void *Get_User_Data( void ) { return UserData; }
	virtual bool Cast_Ray( RayCollisionTestClass & raytest ) { return false; }
};

class PdAABoxRenderObjClass : public PdRenderObjClass
{
public:
	AABoxClass CachedBox;
	virtual bool Cast_Ray( RayCollisionTestClass & raytest );
};
class PdOBBoxRenderObjClass : public PdRenderObjClass
{
public:
	OBBoxClass CachedBox;
	virtual bool Cast_Ray( RayCollisionTestClass & raytest );
};
#include "pd_aabox_cast.inc"
#include "pd_obbox_cast.inc"

class PdHLodClass : public PdRenderObjClass
{
public:
	class ModelNodeClass { public: PdRenderObjClass *Model; int BoneIndex; };
	class ModelArrayClass
	{
	public:
		std::vector<ModelNodeClass> v;
		int Count( void ) const { return (int)v.size(); }
		ModelNodeClass & operator [] ( int i ) { return v[i]; }
	};
	int LodCount;
	ModelArrayClass Lod[1];
	ModelArrayClass AdditionalModels;
	PdHLodClass() : LodCount(1) {}
	bool Are_Sub_Object_Transforms_Dirty( void ) { return false; }
	void Update_Sub_Object_Transforms( void ) {}
	virtual bool Cast_Ray( RayCollisionTestClass & raytest );
};
#include "pd_hlod_cast.inc"

namespace pd {

std::vector<int> g_out;
static Int bitsOf(Real r) { Int i; memcpy(&i, &r, sizeof i); return i; }
static Real realOf(Int i) { Real r; memcpy(&r, &i, sizeof r); return r; }
static void rec(const std::vector<Int> &w) { g_out.insert(g_out.end(), w.begin(), w.end()); }
static Vector3 vec(const int *w) { return Vector3(realOf(w[0]), realOf(w[1]), realOf(w[2])); }
static void recVec(const Vector3 &v) { rec({bitsOf(v.X), bitsOf(v.Y), bitsOf(v.Z)}); }

/* ---- the scene ---- */
typedef std::vector<PdRenderObjClass *> RefRenderObjListClass;
class RefRenderObjListIterator
{
public:
	RefRenderObjListClass *l;
	size_t i;
	RefRenderObjListIterator( RefRenderObjListClass *list ) : l(list), i(0) {}
	void First( void ) { i = 0; }
	bool Is_Done( void ) { return i >= l->size(); }
	PdRenderObjClass *Peek_Obj( void ) { return (*l)[i]; }
	void Next( void ) { ++i; }
};
class RTS3DScene
{
public:
	RefRenderObjListClass RenderList;
	Bool castRay(RayCollisionTestClass & raytest, Bool testAll, Int collisionType);
};
#include "pd_cast.inc"
struct W3DDisplay { static RTS3DScene *m_3DScene; };
RTS3DScene *W3DDisplay::m_3DScene = NULL;

#include "pd_pick_type.inc"

/* ---- the drawables, the windows ---- */
class Object;
class Drawable;
struct DrawableInfo { Drawable *m_drawable; };
enum { KINDOF_SELECTABLE, KINDOF_SHRUBBERY, KINDOF_MINE, KINDOF_FORCEATTACKABLE, KINDOF_CLICK_THROUGH, KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_STRUCTURE };
enum { MODELCONDITION_RUBBLE = 5 };
enum DamageType { DAMAGE_FLAME = 6 };
class Object
{
public:
	Int m_kinds;
	Bool m_dead;
	Int m_flame;
	Bool isKindOf( Int k ) const { return (m_kinds >> k) & 1; }
	Bool isEffectivelyDead( void ) const { return m_dead; }
	Bool hasWeaponToDealDamageType( DamageType t ) const { return t == DAMAGE_FLAME && m_flame; }
};
struct ModelConditionFlags { Bool rubble; Bool test( Int f ) const { return f == MODELCONDITION_RUBBLE && rubble; } };
class Drawable
{
public:
	Int m_id;
	Object *m_object;
	ModelConditionFlags m_flags;
	Object *getObject( void ) { return m_object; }
	const ModelConditionFlags & getModelConditionFlags( void ) const { return m_flags; }
};
class ThingTemplate
{
public:
	Int m_kinds;
	Bool isKindOf( Int k ) const { return (m_kinds >> k) & 1; }
};
enum { WIN_STATUS_SEE_THRU = 0x00010000 };
class GameWindow
{
public:
	UnsignedInt m_status;
	GameWindow *m_parent;
	UnsignedInt winGetStatus( void ) { return m_status; }
	GameWindow *winGetParent( void ) { return m_parent; }
};
struct WindowManagerStub
{
	GameWindow *m_under;
	GameWindow *getWindowUnderCursor( Int x, Int y ) { return m_under; }
};
WindowManagerStub g_wm;
WindowManagerStub *TheWindowManager = &g_wm;

class W3DView
{
public:
	Vector3 m_start, m_end;
	void getPickRay( const ICoord2D *screen, Vector3 *rayStart, Vector3 *rayEnd ) { *rayStart = m_start; *rayEnd = m_end; }
	Drawable *pickDrawable( const ICoord2D *screen, Bool forceAttack, PickType pickType );
};
#include "pd_pick.inc"

class W3DModelDraw
{
public:
	PdRenderObjClass *m_renderObject;
	void setSelectable( Bool selectable );
	void collisionTypes( const ThingTemplate *tmplate, Drawable *draw )
	{
		if( m_renderObject )
		{
#include "pd_model_draw.inc"
		}
	}
};
#include "pd_selectable.inc"

/* ---- getPickTypesForContext ---- */
enum { ALLOW_SHRUBBERY_TARGET = 0x00000010, ALLOW_MINE_TARGET = 0x00000800 };
class CommandButton { public: UnsignedInt m_options; UnsignedInt getOptions( void ) const { return m_options; } };
typedef std::list<Drawable *> DrawableList;
typedef DrawableList::const_iterator DrawableListCIt;
struct InGameUIStub
{
	const CommandButton *m_command;
	Bool m_controllable;
	DrawableList m_selected;
	const CommandButton *getGUICommand( void ) const { return m_command; }
	Bool areSelectedObjectsControllable( void ) const { return m_controllable; }
	const DrawableList *getAllSelectedDrawables( void ) const { return &m_selected; }
};
InGameUIStub g_ui;
InGameUIStub *TheInGameUI = &g_ui;
UnsignedInt getPickTypesForCurrentSelection( Bool forceAttackMode );
#include "pd_pick_types.inc"

} /* namespace pd */

extern "C" int ref_pick_drawable_seq(const int *in, int nin, int *out, int cap)
{
	using namespace pd;
	g_out.clear();
	RTS3DScene scene;
	W3DDisplay::m_3DScene = &scene;
	std::vector<PdHLodClass *> objs;
	std::vector<PdRenderObjClass *> models;
	std::vector<Drawable *> draws;
	std::vector<DrawableInfo *> infos;
	int k = 0;
	int n = in[k++];
	for (int o = 0; o < n; ++o) {
		const int *w = in + k;
		k += 9;
		PdHLodClass *h = new PdHLodClass;
		if (w[0] >= 0) {
			Drawable *d = new Drawable;
			d->m_id = w[0];
			d->m_object = NULL;
			DrawableInfo *di = new DrawableInfo;
			di->m_drawable = d;
			h->UserData = di;
			draws.push_back(d);
			infos.push_back(di);
		}
		h->Bits = w[1];
		h->ReallyVisible = w[2];
		h->Sphere.Center = vec(w + 3);
		h->Sphere.Radius = realOf(w[6]);
		int nTop = w[7], nAdd = w[8];
		for (int m = 0; m < nTop + nAdd; ++m) {
			const int *b = in + k;
			k += 18;
			PdRenderObjClass *r;
			if (b[0] == 0) {
				PdAABoxRenderObjClass *a = new PdAABoxRenderObjClass;
				a->CachedBox.Center = vec(b + 3);
				a->CachedBox.Extent = vec(b + 6);
				r = a;
			} else {
				PdOBBoxRenderObjClass *a = new PdOBBoxRenderObjClass;
				a->CachedBox.Center = vec(b + 3);
				a->CachedBox.Extent = vec(b + 6);
				for (int i = 0; i < 3; ++i)
					for (int j = 0; j < 3; ++j)
						a->CachedBox.Basis[i][j] = realOf(b[9 + 3 * i + j]);
				r = a;
			}
			r->Bits = b[1];
			r->AnimHidden = b[2];
			PdHLodClass::ModelNodeClass node;
			node.Model = r;
			node.BoneIndex = 0;
			if (m < nTop)
				h->Lod[0].v.push_back(node);
			else
				h->AdditionalModels.v.push_back(node);
			models.push_back(r);
		}
		scene.RenderList.push_back(h);
		objs.push_back(h);
	}
	W3DView view;
	for (; k + 20 <= nin; k += 20) {
		const int *w = in + k + 1;
		switch (in[k]) {
		case 0: {
			view.m_start = vec(w);
			view.m_end = vec(w + 3);
			GameWindow wins[3];
			Int nw = w[8];
			for (int i = 0; i < nw; ++i) {
				wins[i].m_status = w[9 + i] ? WIN_STATUS_SEE_THRU : 0;
				wins[i].m_parent = i + 1 < nw ? &wins[i + 1] : NULL;
			}
			g_wm.m_under = nw > 0 ? &wins[0] : NULL;
			ICoord2D screen = { 0, 0 };
			Drawable *d = view.pickDrawable(&screen, w[6] != 0, (PickType)w[7]);
			rec({0, d ? d->m_id : -2});
			break;
		}
		case 1: {
			LineSegClass ray(vec(w), vec(w + 3));
			CastResultStruct result;
			RayCollisionTestClass raytest(ray, &result, COLL_TYPE_ALL, false, false);
			Bool hit = scene.castRay(raytest, w[6] != 0, w[7]);
			Int idx = -1;
			for (size_t i = 0; i < objs.size(); ++i)
				if (raytest.CollidedRenderObj == objs[i])
					idx = (Int)i;
			rec({1, hit ? 1 : 0, idx});
			recVec(raytest.Ray.Get_P1());
			recVec(raytest.Ray.Get_Dir());
			rec({bitsOf(raytest.Ray.Get_Length())});
			break;
		}
		case 2: {
			LineSegClass ray(vec(w + 1), vec(w + 4));
			CastResultStruct result;
			result.Fraction = realOf(w[8]);
			result.StartBad = w[9] != 0;
			result.ComputeContactPoint = w[10] != 0;
			bool r = false;
			if (!models.empty()) {
				PdRenderObjClass *m = models[(size_t)(unsigned)w[7] % models.size()];
				if (w[0] == 0) {
					AABoxClass box;
					PdOBBoxRenderObjClass *ob = dynamic_cast<PdOBBoxRenderObjClass *>(m);
					PdAABoxRenderObjClass *ab = dynamic_cast<PdAABoxRenderObjClass *>(m);
					box.Center = ob ? ob->CachedBox.Center : ab->CachedBox.Center;
					box.Extent = ob ? ob->CachedBox.Extent : ab->CachedBox.Extent;
					r = PdCollisionMath::Collide(ray, box, &result);
				} else {
					OBBoxClass box;
					PdOBBoxRenderObjClass *ob = dynamic_cast<PdOBBoxRenderObjClass *>(m);
					PdAABoxRenderObjClass *ab = dynamic_cast<PdAABoxRenderObjClass *>(m);
					if (ob) {
						box = ob->CachedBox;
					} else {
						box.Center = ab->CachedBox.Center;
						box.Extent = ab->CachedBox.Extent;
						box.Basis.Make_Identity();
					}
					r = PdCollisionMath::Collide(ray, box, &result);
				}
			}
			rec({2, r ? 1 : 0, result.StartBad ? 1 : 0, bitsOf(result.Fraction)});
			recVec(result.Normal);
			recVec(result.ContactPoint);
			break;
		}
		case 3: {
			PdRenderObjClass ro;
			ro.Bits = w[0];
			ThingTemplate t;
			t.m_kinds = w[1];
			Object obj;
			obj.m_kinds = (w[3] & 1 ? 1 << KINDOF_BRIDGE : 0) | (w[3] & 2 ? 1 << KINDOF_BRIDGE_TOWER : 0) | (w[3] & 4 ? 1 << KINDOF_STRUCTURE : 0);
			obj.m_dead = w[5] != 0;
			obj.m_flame = 0;
			Drawable d;
			d.m_id = 0;
			d.m_object = w[2] ? &obj : NULL;
			d.m_flags.rubble = w[4] != 0;
			W3DModelDraw md;
			md.m_renderObject = &ro;
			md.collisionTypes(&t, &d);
			rec({3, ro.Get_Collision_Type()});
			break;
		}
		case 4: {
			PdRenderObjClass ro;
			ro.Bits = w[0];
			W3DModelDraw md;
			md.m_renderObject = &ro;
			md.setSelectable(w[1] != 0);
			rec({4, ro.Get_Collision_Type()});
			break;
		}
		case 5: {
			CommandButton cb;
			cb.m_options = (UnsignedInt)w[2];
			g_ui.m_command = w[1] ? &cb : NULL;
			g_ui.m_controllable = w[3] != 0;
			g_ui.m_selected.clear();
			Object os[4];
			Drawable ds[4];
			for (int i = 0; i < w[4] && i < 4; ++i) {
				os[i].m_kinds = 0;
				os[i].m_dead = FALSE;
				os[i].m_flame = w[5 + i] == 1;
				ds[i].m_id = i;
				ds[i].m_object = w[5 + i] < 0 ? NULL : &os[i];
				g_ui.m_selected.push_back(&ds[i]);
			}
			rec({5, (Int)getPickTypesForContext(w[0] != 0)});
			g_ui.m_selected.clear();
			break;
		}
		}
	}
	for (size_t i = 0; i < models.size(); ++i) delete models[i];
	for (size_t i = 0; i < objs.size(); ++i) delete objs[i];
	for (size_t i = 0; i < draws.size(); ++i) delete draws[i];
	for (size_t i = 0; i < infos.size(); ++i) delete infos[i];
	W3DDisplay::m_3DScene = NULL;
	int nn = (int)g_out.size();
	if (nn > cap)
		nn = cap;
	if (nn > 0)
		memcpy(out, &g_out[0], nn * sizeof(int));
	return nn;
}
