/* Differential-test pieces for OCLSpecialPower (p1w13-sonnet, batch R22; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/OCLSpecialPower.cpp   (#included, unchanged)
** in wrap_module.cpp's unit, at its very end: the special power batch H driver (l9_powers_h.inc, kind 7) runs it through the r22* hooks below, because
** the stand-ins the power reaches are defined after that driver: the object creation lists OCL_A and OCL_B (l9_upgrades_b.inc), the angle overload of
** ObjectCreationList::createInternal ("oclat", l9_ocl_update_n.inc) and the terrain's findClosestEdgePoint (the same driver's offset), the partition
** manager's findPositionAround ("findpos", the contain driver's answer, l9_open_contain_r.inc). New here: the createOwner overload of createInternal
** (a line, "oclown") and findFarthestEdgePoint (an offset of its own). */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/OCLSpecialPower.cpp"

namespace {
Coord3D g_r22FarOffset;
TerrainLogic g_r22Terrain;
PartitionManager g_r22Pm;
}  // namespace

Coord3D TerrainLogic::findFarthestEdgePoint( const Coord3D *farthestFrom ) const
{
	Coord3D r;
	r.x = farthestFrom->x + g_r22FarOffset.x;
	r.y = farthestFrom->y + g_r22FarOffset.y;
	r.z = farthestFrom->z + g_r22FarOffset.z;
	return r;
}
Object *ObjectCreationList::createInternal(const Object *primaryObj, const Coord3D *primary, const Coord3D *secondary, Bool createOwner, UnsignedInt lifetimeFrames) const
{
	ulog("oclown %d %08x,%08x,%08x %08x,%08x,%08x %d %u", this == &g_upOcls[0] ? 0 : 1, realBits(primary->x), realBits(primary->y), realBits(primary->z),
		realBits(secondary->x), realBits(secondary->y), realBits(secondary->z), (int)createOwner, lifetimeFrames);
	return 0;
}

ModuleData *r22ReadOcl(INI *ini) { return OCLSpecialPower::friend_newModuleData(ini); }
SpecialPowerModule *r22NewOcl(Object *obj, const ModuleData *d) { return new OCLSpecialPower(obj, d); }
/* the data dump: the upgrades (count, then the science's name and the list's code each: 0 NULL, 1 OCL_A, 2 OCL_B), the default list's code, the create
** location, the adjust flag, the reference object's name */
void r22PutOclData(const ModuleData *md)
{
	const OCLSpecialPowerModuleData *d = (const OCLSpecialPowerModuleData *)md;
	putU32((unsigned)d->m_upgradeOCL.size());
	for (size_t j = 0; j < d->m_upgradeOCL.size(); ++j) {
		AsciiString nm = TheNameKeyGenerator->keyToName((NameKeyType)d->m_upgradeOCL[j].m_science);
		putStr(nm.str(), (int)nm.getLength());
		putU32((unsigned)oclCode(d->m_upgradeOCL[j].m_ocl));
	}
	putU32((unsigned)oclCode(d->m_defaultOCL));
	putI32((int)d->m_createLoc);
	putU32(d->m_isOCLAdjustPositionToPassable ? 1 : 0);
	putStr(d->m_referenceThingName.str(), (int)d->m_referenceThingName.getLength());
}
/* the scene: the terrain and the partition manager, the edge offsets and findPositionAround's answer (none) */
void r22Begin()
{
	TheTerrainLogic = &g_r22Terrain;
	ThePartitionManager = &g_r22Pm;
	g_ocEdgeOffset.x = g_ocEdgeOffset.y = g_ocEdgeOffset.z = 0.0f;
	g_r22FarOffset.x = g_r22FarOffset.y = g_r22FarOffset.z = 0.0f;
	g_ctFindPosHas = false;
}
void r22End() { TheTerrainLogic = 0; ThePartitionManager = 0; }
/* op 23: the closest edge's offset; 24: the farthest edge's; 25: findPositionAround finds (a != 0) the position (b, c, d) */
void r22Op(const unsigned *o)
{
	Coord3D c;
	memcpy(&c.x, &o[2], 4); memcpy(&c.y, &o[3], 4); memcpy(&c.z, &o[4], 4);
	if (o[0] == 23) { memcpy(&g_ocEdgeOffset.x, &o[1], 4); memcpy(&g_ocEdgeOffset.y, &o[2], 4); memcpy(&g_ocEdgeOffset.z, &o[3], 4); }
	if (o[0] == 24) { memcpy(&g_r22FarOffset.x, &o[1], 4); memcpy(&g_r22FarOffset.y, &o[2], 4); memcpy(&g_r22FarOffset.z, &o[3], 4); }
	if (o[0] == 25) { g_ctFindPosHas = o[1] != 0; g_ctFindPos = c; }
}
