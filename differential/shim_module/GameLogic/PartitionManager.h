/* Stand-in for GameEngine/Include/GameLogic/PartitionManager.h, Module build only (differential tests, p1w12-sonnet, crate batch F): the calls the
** crate modules make, logged by differential/l9_crates_f.inc. Renamed ModTestPartitionManager by module_prelude.h.
** Update batch M: the range scan of AutoHealBehavior -- the filters (constructed, never run) and the iterator over the list the test gives
** (renamed ModTest* by module_prelude.h, like the manager). */
#ifndef SHIM_MODULE_PARTITIONMANAGER_H
#define SHIM_MODULE_PARTITIONMANAGER_H
#include "Lib/BaseType.h"
#include "Common/GameMemory.h"
#include <vector>
class Object;
/* --- p1w12-sonnet --- */
enum DistanceCalculationType { FROM_CENTER_2D, FROM_CENTER_3D, FROM_BOUNDINGSPHERE_2D, FROM_BOUNDINGSPHERE_3D };
enum IterOrderType { ITER_FASTEST, ITER_SORTED_NEAR_TO_FAR, ITER_SORTED_FAR_TO_NEAR, ITER_SORTED_CHEAP_TO_EXPENSIVE, ITER_SORTED_EXPENSIVE_TO_CHEAP };
class ObjectIterator : public MemoryPoolObject
{
public:
	virtual Object *first() = 0;
	virtual Object *next() = 0;
};
class SimpleObjectIterator : public ObjectIterator
{
public:
	Object *first() { m_i = 0; return next(); }
	Object *next() { return m_i < m_list.size() ? m_list[m_i++] : 0; }
	std::vector<Object *> m_list;
	size_t m_i = 0;
};
class PartitionFilter { public: virtual ~PartitionFilter() {}
	/* --- p1w13-sonnet --- (update batch O: FireSpreadUpdate's own filter overrides it) */
	virtual Bool allow(Object *objOther) { return TRUE; }
	/* --- end p1w13-sonnet --- */ };
class PartitionFilterRelationship : public PartitionFilter
{
public:
	enum RelationshipAllowTypes { ALLOW_ALLIES = 2, ALLOW_ENEMIES = 1, ALLOW_NEUTRAL = 4 };
	PartitionFilterRelationship( const Object *obj, Int flags ) {}
};
class PartitionFilterAlive : public PartitionFilter { public: PartitionFilterAlive( void ) {} };
class PartitionFilterSameMapStatus : public PartitionFilter { public: PartitionFilterSameMapStatus( const Object *obj ) {} };
class MemoryPoolObjectHolder
{
	MemoryPoolObject *m_mpo;
public:
	MemoryPoolObjectHolder( MemoryPoolObject *mpo = 0 ) : m_mpo( mpo ) {}
	~MemoryPoolObjectHolder() { m_mpo->deleteInstance(); }
};
/* --- end p1w12-sonnet --- */
/* --- p1w13-sonnet --- (update batch O: HeightDieUpdate's scan for structures under it) */
#include "Common/KindOf.h"
#include "Common/Geometry.h"
class PartitionFilterAcceptByKindOf : public PartitionFilter
{
public:
	PartitionFilterAcceptByKindOf(const KindOfMaskType& mustBeSet, const KindOfMaskType& mustBeClear) {}
};
/* batch P (ActiveBody): the retaliation scan's two filters (constructed, never run: the scan is the list the test gives); the affiliation filter keeps its player */
class Player;
class PartitionFilterPlayerAffiliation : public PartitionFilter
{
public:
	PartitionFilterPlayerAffiliation( const Player *player, Int affiliation, Bool allow ) : m_player(player), m_affiliation(affiliation), m_allow(allow) {}
	const Player *m_player; Int m_affiliation; Bool m_allow;
};
class PartitionFilterOnMap : public PartitionFilter { public: PartitionFilterOnMap() {} };
/* --- end p1w13-sonnet --- */
class PartitionManager
{
public:
	void revealMapForPlayer( Int playerIndex );
	/* --- p1w12-sonnet --- */
	SimpleObjectIterator *iterateObjectsInRange( const Coord3D *pos, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters = 0, IterOrderType order = ITER_FASTEST );
	/* --- end p1w12-sonnet --- */
	/* --- p1w13-sonnet --- */
	Object *getClosestObject( const Object *obj, Real maxDist, DistanceCalculationType dc = FROM_CENTER_2D, PartitionFilter **filters = 0 );   /* update batch O: FireSpreadUpdate */
	Bool geomCollidesWithGeom(const Coord3D* pos1, const GeometryInfo& geom1, Real angle1, const Coord3D* pos2, const GeometryInfo& geom2, Real angle2) const;
	SimpleObjectIterator *iterateObjectsInRange( const Object *obj, Real maxDist, DistanceCalculationType dc, PartitionFilter **filters = 0, IterOrderType order = ITER_FASTEST );
	Real getDistanceSquared( const Object *obj, const Object *otherObj, DistanceCalculationType d, Coord3D *vec = 0 ) const;   /* batch P: ActiveBody */
	Real getDistanceSquared( const Object *obj, const Coord3D *pos, DistanceCalculationType d, Coord3D *vec = 0 ) const;   /* batch R: OpenContain */
	void registerObject( Object *obj );   /* batch R: OpenContain */
	void unRegisterObject( Object *obj );
	/* --- end p1w13-sonnet --- */
};
extern PartitionManager *ThePartitionManager;
#endif
