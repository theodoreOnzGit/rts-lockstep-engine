/* Stand-in for GameEngine/Include/GameClient/ParticleSys.h, Module build only (differential tests): DamDie.cpp includes it and uses nothing from it;
** p1w12-sonnet, update batch M: AutoHealBehavior.cpp makes particle systems and keeps the id. The manager's calls are defined by
** differential/l9_auto_heal_m.inc (findTemplate answers a distinct template for every name but "None"; createParticleSystem hands out ids 1, 2, 3, ...
** and logs; setPosition logs). The classes are renamed ModTest* by module_prelude.h, so that
** the symbols are this unit's own (the weapon fixture defines ParticleSystemManager::findTemplate for itself; the INI archive's
** parseParticleSystemTemplate is replaced, in AutoHealBehavior.h, by l9ahParsePsTemplate). */
#ifndef SHIM_MODULE_PARTICLESYS_H
#define SHIM_MODULE_PARTICLESYS_H
#include "Common/AsciiString.h"
#include "Lib/BaseType.h"
typedef UnsignedInt ParticleSystemID;
enum { INVALID_PARTICLE_SYSTEM_ID = 0 };
class ParticleSystemTemplate { public: int m_idx; };
/* batch R32: the emission volume types (ParticleSys.h, verbatim order) */
class ParticleSystemInfo { public: enum EmissionVolumeType { INVALID_VOLUME=0, POINT, LINE, BOX, SPHERE, CYLINDER }; };
class ParticleSystem
{
public:
	void setPosition( const Coord3D *pos );
	void attachToObject( const class Object *obj );   /* p1w13-sonnet, batch P (differential/l9_active_body_p.inc) */
	void attachToDrawable( const class Drawable *draw );   /* p1w13-sonnet, batch R27: a line (differential/l9_stealth_detector_r27.inc) */
	void destroy( void );
	void stop( void );   /* batch R30: a line (differential/l9_bonefx_r30.inc) */
	/* batch R32: the firestorm's radius (differential/l9_firestorm_r32.inc) */
	ParticleSystemInfo::EmissionVolumeType getEmisionVolumeType() const;
	void setEmissionVolumeSphereRadius( Real radius );
	void setEmissionVolumeCylinderRadius( Real radius );
	/* batch R33: EMPUpdate's sparks (differential/l9_emp_r33.inc) */
	void setSystemLifetime( UnsignedInt frames );
	void setInitialDelay( UnsignedInt delay );
	void setLifetimeRange( Real min, Real max );   /* batch R57: SlavedUpdate's welding sparks (a line: differential/l9_slaved_r57.inc) */
	ParticleSystemID getSystemID() const { return m_id; }
	ParticleSystemID m_id;
	int m_tmpl;
};
class ParticleSystemManager
{
public:
	ParticleSystemTemplate *findTemplate( const AsciiString &name ) const;
	ParticleSystem *createParticleSystem( const ParticleSystemTemplate *sysTemplate, Bool createSlaves = TRUE );
	void destroyParticleSystemByID( ParticleSystemID id );
	void destroyAttachedSystems( class Object *obj );   /* p1w13-sonnet, update batch O: differential/l9_height_die_o.inc */
	ParticleSystem *findParticleSystem( ParticleSystemID id );   /* p1w13-sonnet, batch P */
};
extern ParticleSystemManager *TheParticleSystemManager;
#endif
