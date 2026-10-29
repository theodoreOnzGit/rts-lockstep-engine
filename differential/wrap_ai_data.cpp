/* Differential-test wrapper for the AI's data (Phase 1 layer 10, p1w13-sonnet, batch A27; upstream commit 0a05454d):
**   GameEngine/Source/GameLogic/AI/AI.cpp: TAiData::addSideInfo .. AI::parseSkirmishBuildList (with TheAIFieldParseTable),
**     AI::newOverride, AI::addSideInfo, AI::parseAiDataDefinition, TAiData::TAiData .. TAiData::loadPostProcess
**   GameEngine/Source/GameLogic/Map/SidesList.cpp: BuildListInfo::BuildListInfo .. BuildListInfo::loadPostProcess
** cut verbatim by build.rs into OUT_DIR/ai_data_gen/ai_data_a27.inc and compiled here with the real GameLogic/AI.h and
** GameLogic/SidesList.h, in the science build (the real Science.h, INI.h and Xfer; ScienceStore::getSciencePurchaseCost is the real
** Science.cpp's). Every class and global those files define is renamed here (A27...) so that nothing clashes with the other
** archives' AI, TAiData or BuildListInfo (wrap_partition.cpp's stand-ins, wrap_sides.cpp's SidesList.cpp).
**
** The AIData block reaches AI::parseAiDataDefinition through wrap_ini.cpp's INI::parseAIDataDefinition, which calls
** ref_ai_data_block_hook when it is set (ref_ai_data_reset sets it). The files are loaded with wrap_science.cpp's ref_science_load
** (the real INI::load: Science blocks fill the real ScienceStore the skill sets look sciences up in).
**
** TheAI: only its m_aiData is used (newOverride, addSideInfo, parseAiDataDefinition), so it is raw storage with that member set,
** never constructed (AI's constructor makes a Pathfinder; its vtable is in AI.cpp, not here). */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Common/INI.h"
#include "Common/Xfer.h"
#include "Common/XferCRC.h"
#ifndef CRCGEN_LOG
#define CRCGEN_LOG(m)
#endif

#define AI A27AI
#define TheAI A27TheAI
#define TAiData A27TAiData
#define AISideInfo A27AISideInfo
#define AISideBuildList A27AISideBuildList
#define TSkillSet A27TSkillSet
#define BuildListInfo A27BuildListInfo
#define private public
#define protected public
#include "GameLogic/AI.h"
#include "GameLogic/SidesList.h"
#undef private
#undef protected

AI *TheAI = NULL;

#include "ai_data_a27.inc"

namespace {
std::vector<unsigned char> g_dump;
void putU32(unsigned v) { for (int i = 0; i < 4; ++i) g_dump.push_back((unsigned char)(v >> (8 * i))); }
void putF(Real r) { unsigned u; memcpy(&u, &r, 4); putU32(u); }
void putStr(const AsciiString &s) { putU32((unsigned)s.getLength()); for (int i = 0; i < s.getLength(); ++i) g_dump.push_back((unsigned char)s.str()[i]); }
alignas(16) unsigned char g_aiMem[sizeof(AI)];

/* the INI hook: INI::parseAIDataDefinition (INIAiData.cpp) is AI::parseAiDataDefinition(ini) */
void aiDataBlock(INI *ini) { AI::parseAiDataDefinition(ini); }

void dumpData(TAiData *d)
{
	putF(d->m_structureSeconds); putF(d->m_teamSeconds); putU32((unsigned)d->m_resourcesWealthy); putU32((unsigned)d->m_resourcesPoor);
	putU32(d->m_forceIdleFramesCount); putF(d->m_structuresWealthyMod); putF(d->m_teamWealthyMod); putF(d->m_structuresPoorMod);
	putF(d->m_teamPoorMod); putF(d->m_teamResourcesToBuild); putF(d->m_guardInnerModifierAI); putF(d->m_guardOuterModifierAI);
	putF(d->m_guardInnerModifierHuman); putF(d->m_guardOuterModifierHuman); putU32(d->m_guardChaseUnitFrames);
	putU32(d->m_guardEnemyScanRate); putU32(d->m_guardEnemyReturnScanRate); putF(d->m_skirmishGroupFudgeValue); putF(d->m_repulsedDistance);
	putU32(d->m_enableRepulsors ? 1u : 0u); putF(d->m_alertRangeModifier); putF(d->m_aggressiveRangeModifier);
	putU32(d->m_forceSkirmishAI ? 1u : 0u); putU32(d->m_rotateSkirmishBases ? 1u : 0u); putU32(d->m_attackUsesLineOfSight ? 1u : 0u);
	putU32(d->m_attackIgnoreInsignificantBuildings ? 1u : 0u); putF(d->m_attackPriorityDistanceModifier); putF(d->m_maxRecruitDistance);
	putF(d->m_skirmishBaseDefenseExtraDistance); putF(d->m_wallHeight); putU32((unsigned)d->m_minInfantryForGroup);
	putU32((unsigned)d->m_minVehiclesForGroup); putF(d->m_minDistanceForGroup); putF(d->m_distanceRequiresGroup); putF(d->m_minClumpDensity);
	putU32((unsigned)d->m_infantryPathfindDiameter); putU32((unsigned)d->m_vehiclePathfindDiameter); putU32((unsigned)d->m_rebuildDelaySeconds);
	putF(d->m_supplyCenterSafeRadius); putF(d->m_aiDozerBoredRadiusModifier); putU32(d->m_aiCrushesInfantry ? 1u : 0u);
	putF(d->m_maxRetaliateDistance); putF(d->m_retaliateFriendsRadius);
	unsigned n = 0;
	for (AISideInfo *i = d->m_sideInfo; i; i = i->m_next) ++n;
	putU32(n);
	for (AISideInfo *i = d->m_sideInfo; i; i = i->m_next) {
		putStr(i->m_side); putU32((unsigned)i->m_easy); putU32((unsigned)i->m_normal); putU32((unsigned)i->m_hard);
		const TSkillSet *sets[5] = { &i->m_skillSet1, &i->m_skillSet2, &i->m_skillSet3, &i->m_skillSet4, &i->m_skillSet5 };
		for (int k = 0; k < 5; ++k) { putU32((unsigned)sets[k]->m_numSkills); for (int j = 0; j < MAX_AI_UPGRADES; ++j) putU32((unsigned)sets[k]->m_skills[j]); }
		putStr(i->m_baseDefenseStructure1);
	}
	n = 0;
	for (AISideBuildList *b = d->m_sideBuildLists; b; b = b->m_next) ++n;
	putU32(n);
	for (AISideBuildList *b = d->m_sideBuildLists; b; b = b->m_next) {
		putStr(b->m_side);
		unsigned m = 0;
		for (BuildListInfo *e = b->m_buildList; e; e = e->getNext()) ++m;
		putU32(m);
		for (BuildListInfo *e = b->m_buildList; e; e = e->getNext()) {
			putStr(e->m_buildingName); putStr(e->m_templateName); putF(e->m_location.x); putF(e->m_location.y); putF(e->m_location.z);
			putF(e->m_rallyPointOffset.x); putF(e->m_rallyPointOffset.y); putF(e->m_angle); putU32(e->m_isInitiallyBuilt ? 1u : 0u);
			putU32(e->m_numRebuilds); putStr(e->m_script); putU32((unsigned)e->m_health); putU32(e->m_whiner ? 1u : 0u);
			putU32(e->m_unsellable ? 1u : 0u); putU32(e->m_repairable ? 1u : 0u); putU32(e->m_automaticallyBuild ? 1u : 0u);
			putU32(e->m_selected ? 1u : 0u); putU32((unsigned)e->m_objectID); putU32(e->m_objectTimestamp); putU32(e->m_underConstruction ? 1u : 0u);
			for (int j = 0; j < BuildListInfo::MAX_RESOURCE_GATHERERS; ++j) putU32((unsigned)e->m_resourceGatherers[j]);
			putU32(e->m_isSupplyBuilding ? 1u : 0u); putU32((unsigned)e->m_desiredGatherers); putU32((unsigned)e->m_currentGatherers);
			putU32(e->m_priorityBuild ? 1u : 0u);
		}
	}
}
}	/* namespace */

extern "C" {
extern void (*ref_ai_data_block_hook)(INI *);

/* a new chain (AI::AI's TAiData) and the AIData block hook; call after ref_science_reset (the old chain is dropped, not freed) */
void ref_ai_data_reset(void)
{
	memset(g_aiMem, 0, sizeof(g_aiMem));
	TheAI = (AI *)g_aiMem;
	TheAI->m_aiData = NEW TAiData;
	ref_ai_data_block_hook = aiDataBlock;
}

/* the chain, current first: its length, then per TAiData its scalars, its side info and its build lists (the layout of
** tests/differential_ai_data.rs rust_dump). Returns the length (may exceed cap). */
int ref_ai_data_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	unsigned n = 0;
	for (TAiData *d = TheAI->m_aiData; d; d = d->m_next) ++n;
	putU32(n);
	for (TAiData *d = TheAI->m_aiData; d; d = d->m_next) dumpData(d);
	int len = (int)g_dump.size();
	if (len <= cap) memcpy(buf, g_dump.data(), (size_t)len);
	return len;
}

/* the XferCRC of the current TAiData's crc, then of its xfer */
void ref_ai_data_crc(unsigned *crc, unsigned *xferCrc)
{
	XferCRC a;
	a.open("ai");
	TheAI->m_aiData->crc(&a);
	*crc = a.getCRC();
	XferCRC b;
	b.open("ai");
	TheAI->m_aiData->xfer(&b);
	*xferCrc = b.getCRC();
}
}	/* extern "C" */
