/* Stand-in for GameEngine/Include/GameLogic/AI.h, Module build only (differential tests, p1w12-sonnet, ReplaceObjectUpgrade): TheAI->pathfinder() and
** the two calls that module makes on it (answered by differential/l9_replace_object.inc). Renamed ModTestAI and ModTestPathfinder by
** module_prelude.h. */
#ifndef SHIM_MODULE_AI_H
#define SHIM_MODULE_AI_H
class Object;
class Pathfinder
{
public:
	void addObjectToPathfindMap(Object *obj);
	void removeObjectFromPathfindMap(Object *obj);
};
class AI
{
public:
	Pathfinder *pathfinder() { return &m_pathfinder; }
	Pathfinder m_pathfinder;
};
extern AI *TheAI;
#endif
