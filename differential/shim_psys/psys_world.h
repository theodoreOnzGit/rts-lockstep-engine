/* Stand-ins for everything ParticleSys.cpp reaches outside itself (ParticleSys build only;
** differential tests, #46; build.rs, differential/wrap_particle_sys.cpp). The include block of
** ParticleSys.cpp is replaced by build.rs with an include of this file. The names are renamed by
** macro in PreRTS.h; here they are written with their upstream names.
**
** What each stands in for (the definitions are in wrap_particle_sys.cpp):
**  - Drawable and Object: attachment-less things with an id, a position, a transform and a
**    shroud state, which the test sets (the Rust test's host answers the same); an Object may
**    have a Drawable whose transform it reports (Object::getDrawable);
**  - TheGameClient (getFrame, findDrawableByID), TheGameLogic (getFrame, findObjectByID): the
**    frames the test gives and the scene's tables;
**  - TheTerrainLogic: flat ground at the height the test gives;
**  - TheGlobalData (m_useFX, m_particleScale, m_maxParticleCount, m_maxFieldParticleCount) and
**    TheGameLODManager (the dynamic particle priorities and the skip flag): the settings the test
**    gives (the Rust's ParticleSettings);
**  - TheDisplay: preloadTextureAssets does nothing (preloadAssets is not compared).
** For FXList.cpp: TheAudio, TheDisplay->createLightPulse, TheTacticalView->shake, TheGameClient->addScorch and
** createRayEffectByTemplate, and the tracer drawables of TheThingFactory (which knows every name) LOG their requests
** (psysLog*, the words the Rust test writes for its FxRequest list); ThePlayerList's local player is 0; the
** partition manager's cells are clear where x >= -40; the terrain has one layer, the flat ground; objects have a
** bounding circle radius; drawables have no bones. */
#ifndef SHIM_PSYS_WORLD_H
#define SHIM_PSYS_WORLD_H

#include "Lib/BaseType.h"
#include "Common/GameType.h"
#include "Common/AsciiString.h"
#include "Common/GameCommon.h"
#include "matrix3d.h"
#include "vector3.h"
#include <string>

class DrawModule;
class ThingTemplate;

class Drawable
{
public:
	DrawableID m_id;
	Coord3D m_pos;
	Matrix3D m_transform;
	Bool m_obscured;
	DrawableID getID() const { return m_id; }
	const Coord3D *getPosition() const { return &m_pos; }
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	Bool getFullyObscuredByShroud() const { return m_obscured; }
	/* FXList.cpp: an object's drawable has no bones (FXListAtBonePos plays nothing); a tracer (made by
	** TheThingFactory->newDrawable) records its template, position and expiry, and its one draw module its
	** parameters; the request is logged at setExpirationDate, the tracer's last call */
	Int getCurrentClientBonePositions(const char *, Int, Coord3D *, Matrix3D *, Int) const { return 0; }
	void setTransformMatrix(const Matrix3D *m) { m_transform = *m; }
	void setPosition(const Coord3D *p) { m_pos = *p; }
	DrawModule **getDrawModules();
	void setExpirationDate(UnsignedInt frame);
	const ThingTemplate *m_template;
	Real m_tracer[6];	/* speed, length, width, red, green, blue */
};

/* Geometry.h's bounding circle radius (Object::getGeometryInfo) */
class GeometryInfo
{
public:
	Real m_boundingCircleRadius;
	Real getBoundingCircleRadius() const { return m_boundingCircleRadius; }
};

class Player
{
public:
	Int getPlayerIndex() const { return m_index; }
	Int m_index;
};

class Object
{
public:
	ObjectID m_id;
	Coord3D m_pos;
	Matrix3D m_transform;
	ObjectShroudStatus m_shroud;
	Drawable *m_drawable;
	GeometryInfo m_geometry;
	ObjectID getID() const { return m_id; }
	const Coord3D *getPosition() const { return &m_pos; }
	const Matrix3D *getTransformMatrix() const { return &m_transform; }
	Drawable *getDrawable() const { return m_drawable; }
	ObjectShroudStatus getShroudedStatus(Int) const { return m_shroud; }
	/* FXList.cpp */
	const GeometryInfo &getGeometryInfo() const { return m_geometry; }
	Player *getControllingPlayer() const;
	void convertBonePosToWorldPos(const Coord3D *, const Matrix3D *, Coord3D *, Matrix3D *) const { abort(); }
};

/* FXList.cpp's requests, logged in order as words (the Rust test's FxRequest words, see wrap_particle_sys.cpp) */
void psysLog(unsigned w);
void psysLogReal(Real r);
void psysLogCoord(const Coord3D *c);
void psysLogName(const char *s);

class GameClient
{
public:
	UnsignedInt getFrame() { return m_frame; }
	Drawable *findDrawableByID(DrawableID id);
	UnsignedInt m_frame;
	/* FXList.cpp */
	void createRayEffectByTemplate(const Coord3D *start, const Coord3D *end, const ThingTemplate *tmpl);
	void addScorch(const Coord3D *pos, Real radius, Scorches type);
};
extern GameClient *TheGameClient;

class GameLogic
{
public:
	UnsignedInt getFrame() { return m_frame; }
	Object *findObjectByID(ObjectID id);
	UnsignedInt m_frame;
};
extern GameLogic *TheGameLogic;

class TerrainLogic
{
public:
	Real getGroundHeight(Real, Real, Coord3D *normal = NULL) const { (void)normal; return m_ground; }
	Real m_ground;
	/* FXList.cpp: one layer, the flat ground */
	PathfindLayerEnum getLayerForDestination(const Coord3D *) const { return LAYER_GROUND; }
	Real getLayerHeight(Real, Real, PathfindLayerEnum, Coord3D *normal = NULL) const { (void)normal; return m_ground; }
};
extern TerrainLogic *TheTerrainLogic;

class GlobalData
{
public:
	Bool m_useFX;
	Real m_particleScale;
	Int m_maxParticleCount;
	Int m_maxFieldParticleCount;
};
extern GlobalData *TheGlobalData;

class GameLODManager
{
public:
	Int getMinDynamicParticlePriority() { return m_minPriority; }
	Int getMinDynamicParticleSkipPriority() { return m_minSkipPriority; }
	/* (#160) GameLOD.h lines 227-230 (copied): the count and the mask, as upstream's (the stand-in held a fixed answer) */
	Bool isParticleSkipped() { return (++m_numParticleGenerations & m_dynamicParticleSkipMask) != m_dynamicParticleSkipMask; }
	Int m_minPriority;
	Int m_minSkipPriority;
	Int m_numParticleGenerations;
	Int m_dynamicParticleSkipMask;
};
extern GameLODManager *TheGameLODManager;

class Display
{
public:
	void preloadTextureAssets(AsciiString) {}
	void createLightPulse(const Coord3D *pos, const RGBColor *color, Real innerRadius, Real attenuationWidth, UnsignedInt increaseFrameTime, UnsignedInt decayFrameTime);
};
extern Display *TheDisplay;

/* ---- FXList.cpp's other stand-ins */
class ThingTemplate { public: std::string m_name; };
class ThingFactory
{
public:
	/* every name is a template (the Rust's requests name the template, whatever it is) */
	const ThingTemplate *findTemplate(const AsciiString &name, Bool check = TRUE);
	Drawable *newDrawable(const ThingTemplate *tmpl);
};
extern ThingFactory *TheThingFactory;

class TracerDrawInterface
{
public:
	void setTracerParms(Real speed, Real length, Real width, const RGBColor &color, Real initialOpacity);
	Drawable *m_owner;
};
class DrawModule
{
public:
	TracerDrawInterface *getTracerDrawInterface() { return &m_tracer; }
	TracerDrawInterface m_tracer;
};

class AudioEventRTS
{
public:
	AudioEventRTS(const AsciiString &name) : m_name(name), m_hasPos(FALSE), m_player(-1) {}
	void setPosition(const Coord3D *p) { m_pos = *p; m_hasPos = TRUE; }
	void setPlayerIndex(Int i) { m_player = i; }
	AsciiString m_name;
	Coord3D m_pos;
	Bool m_hasPos;
	Int m_player;
};
class GameAudio
{
public:
	void addAudioEvent(const AudioEventRTS *e);
};
extern GameAudio *TheAudio;

class PlayerList
{
public:
	Player *getLocalPlayer() { return &m_local; }
	Player m_local;
};
extern PlayerList *ThePlayerList;

/* the local player's view of the cells: clear where x >= -40, else shrouded (the Rust host's rule) */
class PartitionManager
{
public:
	CellShroudStatus getShroudStatusForPlayer(Int, const Coord3D *loc) const { return loc->x >= -40.0f ? CELLSHROUD_CLEAR : CELLSHROUD_SHROUDED; }
};
extern PartitionManager *ThePartitionManager;

/* View.h's CameraShakeType, verbatim (lines 75-84) */
class View
{
public:
	enum CameraShakeType 
	{ 
		SHAKE_SUBTLE = 0, 
		SHAKE_NORMAL, 
		SHAKE_STRONG, 
		SHAKE_SEVERE, 
		SHAKE_CINE_EXTREME,		//Added for cinematics ONLY
		SHAKE_CINE_INSANE,		//Added for cinematics ONLY
		SHAKE_COUNT 
	};
	void shake(const Coord3D *epicenter, CameraShakeType shakeType);
};
extern View *TheTacticalView;

#endif
