/* Stand-in for GameEngine/Include/GameLogic/Locomotor.h, Module build only (differential tests, p1w13-sonnet, batch R2: TransportContain.cpp includes it
** and compares a `const Locomotor*` with NULL). The real header needs the whole locomotor layer.
** Batch R8: the calls ParachuteContain makes on the current locomotor (differential/l9_open_contain_r.inc: the test's locomotor is the AI itself). */
#ifndef SHIM_MODULE_LOCOMOTOR_H
#define SHIM_MODULE_LOCOMOTOR_H
#include "Lib/BaseType.h"
class Locomotor
{
public:
	Real getPitchStiffness() const;
	Real getRollStiffness() const;
	Real getPitchDamping() const;
	Real getRollDamping() const;
	void setUltraAccurate(Bool u);
	void setCloseEnoughDist(Real dist);
	void setCloseEnoughDist3D(Bool setting);
	void setMaxLift(Real lift);   /* p1w13-sonnet, batch R46: JetSlowDeathBehavior (differential/l9_jet_death_r46.inc) */
	void setMaxBraking(Real braking);   /* p1w13-sonnet, batch R48: HelicopterSlowDeathBehavior (differential/l9_heli_death_r48.inc) */
	void setMaxTurnRate(Real turn);
	Real getPreferredHeight() const;   /* p1w13-sonnet, batch R60: SpectreGunshipDeploymentUpdate (differential/l9_spectre_deploy_r60.inc) */
	void setUsePreciseZPos(Bool u);
	void setAllowInvalidPosition(Bool allow);   /* p1w13-sonnet, batch R72: SpectreGunshipUpdate (a line: differential/l9_spectre_gunship_r72.inc) */   /* p1w13-sonnet, batch R57: SlavedUpdate (a line: differential/l9_slaved_r57.inc) */
};
#endif
