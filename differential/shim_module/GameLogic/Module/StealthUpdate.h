/* Stand-in for GameEngine/Include/GameLogic/Module/StealthUpdate.h, Module build only (differential tests, p1w13-sonnet, batch R): the one call OpenContain
** makes on the stealth update of a contained object (differential/l9_open_contain_r.inc logs it). */
#ifndef SHIM_MODULE_STEALTHUPDATE_H
#define SHIM_MODULE_STEALTHUPDATE_H
#include "Lib/BaseType.h"
#include "GameClient/Eva.h"   /* p1w13-sonnet, batch R27 */
class StealthUpdate
{
public:
	/* p1w13-sonnet, batch R27: StealthDetectorUpdate asks a detected object's Eva events (the test's) */
	EvaMessage getEnemyDetectionEvaEvent() const { return (EvaMessage)m_enemyEva; }
	EvaMessage getOwnDetectionEvaEvent() const { return (EvaMessage)m_ownEva; }
	int m_enemyEva = -1, m_ownEva = -1;
	void markAsDetected( UnsignedInt numFrames = 0 );
	Bool isTemporaryGrant() const { return m_temporaryGrant; }   /* p1w13-sonnet, batch R44: SupplyCenterDockUpdate (the test's) */
	Bool m_temporaryGrant = false;
	void receiveGrant( Bool active = TRUE, UnsignedInt frames = 0 );   /* p1w13-sonnet, batch R4: HelixContain grants stealth to its portable structure */
	Object *m_owner = 0;
	Bool allowedToStealth( Object *stealthOwner ) const;   /* p1w13-sonnet, batch R73: SpawnBehavior (the test's) */
};
#endif
