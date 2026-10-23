/* Stand-in for GameEngine/Include/GameLogic/Module/DozerAIUpdate.h, Module build only (differential tests, p1w13-sonnet): the sabotage crates (batch
** R13) include it and use nothing of it; ConvertToHijackedVehicleCrateCollide (batch R13e) cancels a dozer's tasks. The real header needs the AI layer;
** this is its task enum (DozerAIUpdate.h:71-80, verbatim) and the one call, answered by differential/l9_crates_r13e.inc. */
#ifndef SHIM_MODULE_DOZERAIUPDATE_H
#define SHIM_MODULE_DOZERAIUPDATE_H
enum DozerTask // These enums are saved in the game save file, so DO NOT renumber them. jba.
{
	DOZER_TASK_INVALID = -1,
	DOZER_TASK_FIRST = 0,
	DOZER_TASK_BUILD = DOZER_TASK_FIRST,		///< go build something
	DOZER_TASK_REPAIR = 1,											///< go repair something
	DOZER_TASK_FORTIFY = 2,											///< go fortify something

	DOZER_NUM_TASKS													// keep this last
};
class DozerAIInterface
{
public:
	virtual void cancelTask( DozerTask task ) = 0;							///< cancel this task from the queue, if it's the current task the dozer will stop working on it
};
#endif
