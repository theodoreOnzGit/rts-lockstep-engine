/* Differential-test wrapper for the drawable overlay draft (#215, group lc3, upstream commit 0a05454d), src/drafts/sweep_common_client/drawable.rs. Cut verbatim by build.rs
** from GameEngine/Source/GameClient/Drawable.cpp:
**   dr_region.inc     computeHealthRegion (to drawsAnyUIText);
**   dr_pips.inc       drawAmmo and drawContained (to drawBattlePlans);
**   dr_construct.inc  drawConstructPercent, drawCaption, drawVeterancy and drawHealthBar (to clearAndSetModelConditionState).
**
** Stand-ins (outside the cuts): class Drawable with only the members the cuts read (m_selected, m_constructDisplayString, m_lastConstructDisplayed, m_captionDisplayString,
** m_conditionState, the s_ images) and the functions; Object (position, geometry, ammo and contain facts, health box, experience, status bits, body); the contain module and its
** contained items; Image (a width and a height); TheGlobalData (the show flag, the pip offsets); TheInGameUI (the moused-over id and the caption colour); ThePlayerList's local player;
** TheTacticalView (worldToScreen: false when z > 500, else (x*1.7, y*2.3-40) truncated; getZoom); TheDisplay (drawImage with its default colour, drawOpenRect, drawFillRect, all logged);
** DisplayString and TheDisplayStringManager (new / free / setText / getWidth / getSize / draw, logged); UnicodeString::format and TheGameText->fetch (the percent is captured); `max`;
** REAL_TO_INT is `(Int)(x)` (the real one is x87 assembly for a truncation); GameMakeColor is Color.h's, read unchanged. Compiled as the shipped game: SCALE_ICONS_WITH_ZOOM_ML undefined.
**
** Entry point ref_lc3_dr_run(prog, nprog, out): layout in tests/differential_draft_drawable.rs. */
#include "Lib/BaseType.h"
#include "GameClient/Color.h"
#include <vector>
#include <list>
#include <string.h>
#ifndef NULL
#define NULL 0
#endif
#ifndef TRUE
#define TRUE true
#define FALSE false
#endif
#undef REAL_TO_INT
#define REAL_TO_INT(x) ((Int)(x))
#define CLAMP_ICON_ZOOM_FACTOR(n) (n)
#undef max
#undef min
template <class T> inline T max(T a, T b) { return a > b ? a : b; }

typedef int DrawableID;
enum KindOfType { KINDOF_INFANTRY = 1, KINDOF_FORCEATTACKABLE = 2 };
enum ObjectStatus { OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_SOLD };
enum ModelConditionFlagType { MODELCONDITION_DAMAGED, MODELCONDITION_REALLY_DAMAGED };
enum DisabledType { DISABLED_HELD };
enum VeterancyLevel { LEVEL_REGULAR = 0 };

namespace {

std::vector<int> g_ev;
unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }

class Image
{
public:
	int id, w, h;
	Int getImageWidth() const { return w; }
	Int getImageHeight() const { return h; }
};

struct UnicodeString
{
	float percent;
	void format(const UnicodeString &, double v) { percent = (float)v; }
};
struct GameTextStandIn { UnicodeString fetch(const char *) { return UnicodeString(); } } g_gt;
GameTextStandIn *TheGameText = &g_gt;

class DisplayString
{
public:
	int w, h;
	void setText(const UnicodeString &t) { g_ev.push_back(4); g_ev.push_back((int)fbits(t.percent)); }
	Int getWidth() { return w; }
	void getSize(Int *pw, Int *ph) { *pw = w; *ph = h; }
	void draw(Int x, Int y, Color c, Color d) { g_ev.push_back(5); g_ev.push_back(x); g_ev.push_back(y); g_ev.push_back(c); g_ev.push_back(d); }
};
DisplayString g_constructDs, g_captionDs;
int g_constructW = 0;
struct DisplayStringManagerStandIn
{
	DisplayString *newDisplayString() { g_constructDs.w = g_constructW; return &g_constructDs; }
	void freeDisplayString(DisplayString *) {}
} g_dsm;
DisplayStringManagerStandIn *TheDisplayStringManager = &g_dsm;

struct DisplayStandIn
{
	void drawImage(const Image *im, Int x0, Int y0, Int x1, Int y1, Color c = 0xFFFFFFFF) { g_ev.push_back(1); g_ev.push_back(im->id); g_ev.push_back(x0); g_ev.push_back(y0); g_ev.push_back(x1); g_ev.push_back(y1); g_ev.push_back(c); }
	void drawOpenRect(Int x, Int y, Int w, Int h, Real lw, Color c) { g_ev.push_back(2); g_ev.push_back(x); g_ev.push_back(y); g_ev.push_back(w); g_ev.push_back(h); g_ev.push_back((int)fbits(lw)); g_ev.push_back(c); }
	void drawFillRect(Int x, Int y, Int w, Int h, Color c) { g_ev.push_back(3); g_ev.push_back(x); g_ev.push_back(y); g_ev.push_back(w); g_ev.push_back(h); g_ev.push_back(c); }
} g_display;
DisplayStandIn *TheDisplay = &g_display;

float g_zoom = 1.0f;
struct TacticalViewStandIn
{
	bool worldToScreen(const Coord3D *p, ICoord2D *s) { if (p->z > 500.0f) return false; s->x = (Int)(p->x * 1.7f); s->y = (Int)(p->y * 2.3f - 40.0f); return true; }
	Real getZoom() { return g_zoom; }
} g_tv;
TacticalViewStandIn *TheTacticalView = &g_tv;

struct GlobalDataStandIn { Bool m_showObjectHealth; Coord3D m_ammoPipWorldOffset, m_containerPipWorldOffset; Coord2D m_ammoPipScreenOffset, m_containerPipScreenOffset; } g_gd;
GlobalDataStandIn *TheGlobalData = &g_gd;

struct Player {} g_localPlayer, g_otherPlayer;
struct PlayerListStandIn { Player *getLocalPlayer() { return &g_localPlayer; } } g_pl;
PlayerListStandIn *ThePlayerList = &g_pl;

int g_mousedOver = 0;
Color g_captionColor = 0;
struct InGameUIStandIn { DrawableID getMousedOverDrawableID() { return g_mousedOver; } Color getDrawableCaptionColor() { return g_captionColor; } } g_igu;
InGameUIStandIn *TheInGameUI = &g_igu;

class Object;
class ContainModuleInterface
{
public:
	bool showPips; int total, full;
	std::list<Object *> items;
	bool getContainerPipsToShow(Int &t, Int &f) { if (!showPips) return false; t = total; f = full; return true; }
	const std::list<Object *> *getContainedItemsList() { return &items; }
};
typedef std::list<Object *> ContainedItemsList;
struct StatusBits { bool under, sold; bool test(ObjectStatus s) const { return s == OBJECT_STATUS_UNDER_CONSTRUCTION ? under : sold; } };
struct BodyModuleInterface { Real health, maxHealth; Real getHealth() { return health; } Real getMaxHealth() { return maxHealth; } };
struct GeometryInfo { Real maxH, radius; Real getMaxHeightAbovePosition() const { return maxH; } Real getBoundingSphereRadius() const { return radius; } };

class Object
{
public:
	Player *ctrl;
	bool ammoShow; int ammoTotal, ammoFull;
	ContainModuleInterface *contain;
	Coord3D position; GeometryInfo geom;
	bool hasExp;
	Coord3D hbPos; bool hbOk; Real hbH, hbW;
	StatusBits status; bool disabled, held, forceAttackable, infantry;
	BodyModuleInterface body; Real constructPercent;
	Player *getControllingPlayer() const { return ctrl; }
	bool getAmmoPipShowingInfo(Int &t, Int &f) const { if (!ammoShow) return false; t = ammoTotal; f = ammoFull; return true; }
	ContainModuleInterface *getContain() const { return contain; }
	const Coord3D *getPosition() const { return &position; }
	const GeometryInfo &getGeometryInfo() const { return geom; }
	void *getExperienceTracker() const { return hasExp ? (void *)this : NULL; }
	VeterancyLevel getVeterancyLevel() const { return LEVEL_REGULAR; }
	void getHealthBoxPosition(Coord3D &p) const { p = hbPos; }
	bool getHealthBoxDimensions(Real &h, Real &w) const { if (!hbOk) return false; h = hbH; w = hbW; return true; }
	const StatusBits &getStatusBits() const { return status; }
	bool isDisabled() const { return disabled; }
	bool isDisabledByType(DisabledType) const { return held; }
	bool isKindOf(KindOfType k) const { return k == KINDOF_FORCEATTACKABLE ? forceAttackable : infantry; }
	BodyModuleInterface *getBodyModule() { return &body; }
	Real getConstructionPercent() const { return constructPercent; }
};

struct ConditionState { bool damaged, reallyDamaged; bool test(ModelConditionFlagType f) const { return f == MODELCONDITION_DAMAGED ? damaged : reallyDamaged; } };
struct DrawableGeom { Coord3D center; void getCenterPosition(const Coord3D &, Coord3D &out) const { out = center; } };

class Drawable
{
public:
	Object *m_obj; Bool m_selected; DrawableID m_id;
	DisplayString *m_constructDisplayString; Real m_lastConstructDisplayed; DisplayString *m_captionDisplayString;
	ConditionState m_conditionState; DrawableGeom m_geom; Coord3D m_pos;
	static const Image *s_fullAmmo, *s_emptyAmmo, *s_fullContainer, *s_emptyContainer;
	static const Image *s_veterancyImage[4];
	Object *getObject() const { return m_obj; }
	Bool isSelected() const { return m_selected; }
	DrawableID getID() const { return m_id; }
	const DrawableGeom &getDrawableGeometryInfo() const { return m_geom; }
	const Coord3D *getPosition() const { return &m_pos; }
	void drawAmmo(const IRegion2D *healthBarRegion);
	void drawContained(const IRegion2D *healthBarRegion);
	void drawConstructPercent(const IRegion2D *healthBarRegion);
	void drawCaption(const IRegion2D *healthBarRegion);
	void drawVeterancy(const IRegion2D *healthBarRegion);
	void drawHealthBar(const IRegion2D *healthBarRegion);
};
const Image *Drawable::s_fullAmmo = NULL;
const Image *Drawable::s_emptyAmmo = NULL;
const Image *Drawable::s_fullContainer = NULL;
const Image *Drawable::s_emptyContainer = NULL;
const Image *Drawable::s_veterancyImage[4] = { NULL, NULL, NULL, NULL };

#include "dr_region.inc"
#include "dr_pips.inc"
#include "dr_construct.inc"

}

namespace {
int rd(const int *p, int &k) { return p[k++]; }
float fl(const int *p, int &k) { int b = p[k++]; float f; memcpy(&f, &b, 4); return f; }
void rdc(const int *p, int &k, Coord3D &c) { c.x = fl(p, k); c.y = fl(p, k); c.z = fl(p, k); }
}

/* prog layout: see the test. Every float is its bits. */
extern "C" int ref_lc3_dr_run(const int *p, int np, int *out)
{
	int k = 0, n = 0;
	Image fullAmmo, emptyAmmo, fullCont, emptyCont, vet;
	Object obj; Object infantry[6]; ContainModuleInterface contain;
	Drawable d;
	memset(&obj, 0, sizeof obj);
	d.m_obj = &obj;
	d.m_selected = rd(p, k) != 0;
	g_mousedOver = rd(p, k) ? 1 : 0;
	d.m_id = 1;
	g_gd.m_showObjectHealth = rd(p, k) != 0;
	bool hasObject = rd(p, k) != 0;
	obj.ctrl = rd(p, k) ? &g_localPlayer : &g_otherPlayer;
	obj.ammoShow = rd(p, k) != 0; obj.ammoTotal = rd(p, k); obj.ammoFull = rd(p, k);
	contain.showPips = rd(p, k) != 0; contain.total = rd(p, k); contain.full = rd(p, k);
	int hasContain = rd(p, k); int nItems = rd(p, k); int nInf = rd(p, k);
	for (int i = 0; i < nItems; ++i) { memset(&infantry[i], 0, sizeof infantry[i]); infantry[i].infantry = i < nInf; contain.items.push_back(&infantry[i]); }
	obj.contain = hasContain ? &contain : NULL;
	rdc(p, k, obj.position); obj.geom.maxH = fl(p, k); obj.geom.radius = fl(p, k);
	rdc(p, k, g_gd.m_ammoPipWorldOffset); g_gd.m_ammoPipScreenOffset.x = fl(p, k); g_gd.m_ammoPipScreenOffset.y = fl(p, k);
	rdc(p, k, g_gd.m_containerPipWorldOffset); g_gd.m_containerPipScreenOffset.x = fl(p, k); g_gd.m_containerPipScreenOffset.y = fl(p, k);
	g_zoom = fl(p, k);
	rdc(p, k, obj.hbPos); obj.hbOk = rd(p, k) != 0; obj.hbH = fl(p, k); obj.hbW = fl(p, k);
	int imgFlags = rd(p, k);
	fullAmmo.id = 1; emptyAmmo.id = 2; fullCont.id = 3; emptyCont.id = 4; vet.id = 5;
	fullAmmo.w = fullAmmo.h = 4; emptyAmmo.w = rd(p, k); emptyAmmo.h = rd(p, k); fullCont.w = fullCont.h = 4; emptyCont.w = rd(p, k); emptyCont.h = rd(p, k); vet.w = rd(p, k); vet.h = rd(p, k);
	Drawable::s_fullAmmo = (imgFlags & 1) ? &fullAmmo : NULL; Drawable::s_emptyAmmo = (imgFlags & 2) ? &emptyAmmo : NULL;
	Drawable::s_fullContainer = (imgFlags & 4) ? &fullCont : NULL; Drawable::s_emptyContainer = (imgFlags & 8) ? &emptyCont : NULL;
	Drawable::s_veterancyImage[0] = (imgFlags & 16) ? &vet : NULL;
	obj.hasExp = rd(p, k) != 0;
	obj.status.under = rd(p, k) != 0; obj.status.sold = rd(p, k) != 0; obj.disabled = rd(p, k) != 0; obj.held = rd(p, k) != 0; obj.forceAttackable = rd(p, k) != 0;
	obj.body.health = fl(p, k); obj.body.maxHealth = fl(p, k); obj.constructPercent = fl(p, k);
	d.m_conditionState.damaged = rd(p, k) != 0; d.m_conditionState.reallyDamaged = rd(p, k) != 0;
	rdc(p, k, d.m_geom.center); d.m_pos = obj.position;
	int hasCaption = rd(p, k); g_captionDs.w = rd(p, k); g_captionDs.h = rd(p, k); g_captionColor = rd(p, k);
	d.m_captionDisplayString = hasCaption ? &g_captionDs : NULL;
	g_constructW = rd(p, k);
	d.m_constructDisplayString = rd(p, k) ? &g_constructDs : NULL; if (d.m_constructDisplayString) g_constructDs.w = g_constructW;
	d.m_lastConstructDisplayed = -1.0f;
	if (!hasObject) d.m_obj = NULL;
	int nOps = rd(p, k);
	for (int o = 0; o < nOps; ++o) {
		int kind = rd(p, k);
		IRegion2D r; r.lo.x = rd(p, k); r.lo.y = rd(p, k); r.hi.x = rd(p, k); r.hi.y = rd(p, k);
		int hasRegion = rd(p, k);
		g_ev.clear();
		int ans = 0;
		switch (kind) {
		case 0: { IRegion2D rr; rr.lo.x = rr.lo.y = rr.hi.x = rr.hi.y = 0x5a5a5a5a; bool ok = computeHealthRegion(&d, rr); ans = ok; if (ok) { g_ev.push_back(rr.lo.x); g_ev.push_back(rr.lo.y); g_ev.push_back(rr.hi.x); g_ev.push_back(rr.hi.y); } break; }
		case 1: d.drawAmmo(&r); break;
		case 2: d.drawContained(&r); break;
		case 3: d.drawConstructPercent(hasRegion ? &r : NULL); break;
		case 4: d.drawCaption(hasRegion ? &r : NULL); break;
		case 5: d.drawVeterancy(hasRegion ? &r : NULL); break;
		case 6: d.drawHealthBar(hasRegion ? &r : NULL); break;
		}
		out[n++] = (int)g_ev.size();
		for (size_t i = 0; i < g_ev.size(); ++i) out[n++] = g_ev[i];
		out[n++] = ans;
		out[n++] = d.m_constructDisplayString != NULL;
		out[n++] = (int)fbits(d.m_lastConstructDisplayed);
	}
	return n;
}
