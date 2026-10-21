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
class ParticleSystem
{
public:
	void setPosition( const Coord3D *pos );
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
};
extern ParticleSystemManager *TheParticleSystemManager;
#endif
