/* Differential-test wrapper for the campaign manager and the Generals' Challenge (GitHub #64; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameClient/System/CampaignManager.cpp, from `CampaignManager *TheCampaignManager`
**     to the end (the Campaign field table, INI::parseCampaignDefinition, Campaign, CampaignManager, Mission)
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ChallengeGenerals.cpp, from `ChallengeGenerals *TheChallengeGenerals`
**     to the end (the persona fields, INI::parseChallengeModeDefinition, the lookups)
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp's INI::parseAudioEventRTS (a Mission's BriefingVoice)
** cut by build.rs into OUT_DIR/campaign_gen (see its comment: verbatim but for three mechanical renames, each asserted
** once), compiled with the REAL GameClient/CampaignManager.h and GameClient/ChallengeGenerals.h, in the science build's
** setup (the real INI.h, Xfer.h, GameMemory's zero-filling stand-in), on the stand-ins of differential/shim_campaign for the
** three headers CampaignManager.cpp pulls in that this build does not have: Common/AudioEventRTS.h (the event's name),
** GameClient/GameClient.h (nothing used) and GameNetwork/GameInfo.h (TheChallengeGameInfo, made and dropped by the xfer,
** which the test does not run).
**
** Every class and global those files define is renamed here (Camp...), so that nothing clashes with another archive.
** The Campaign and ChallengeGenerals blocks of the real INI::load (wrap_ini.cpp's ref_ini_load_typed) reach the cut
** handlers through ref_campaign_block_hook, which ref_campaign_reset sets.
**
** Entry points (extern "C"; tests/differential_campaign.rs documents the layouts): ref_campaign_reset,
** ref_campaign_dump, ref_campaign_op, ref_challenge_dump, ref_challenge_lookup. */
#include <stdlib.h>
#include <string.h>
#include <vector>
#include "Common/INI.h"
#include "Common/Xfer.h"

#define CampaignManager CampCampaignManager
#define Campaign CampCampaign
#define Mission CampMission
#define TheCampaignManager CampTheCampaignManager
#define ChallengeGenerals CampChallengeGenerals
#define GeneralPersona CampGeneralPersona
#define TheChallengeGenerals CampTheChallengeGenerals
#define createChallengeGenerals campCreateChallengeGenerals
#define SkirmishGameInfo CampSkirmishGameInfo
#define TheChallengeGameInfo CampTheChallengeGameInfo
#define AudioEventRTS CampAudioEventRTS
#define TheAudio CampTheAudio
#define private public
#define protected public
#include "Common/AudioEventRTS.h"
#include "GameClient/CampaignManager.h"
#include "GameClient/ChallengeGenerals.h"
#include "GameClient/GameClient.h"
#include "GameNetwork/GameInfo.h"
#undef private
#undef protected

/* INI::parseAudioEventRTS's `TheAudio->getInfoForAudioEvent(theSound)`: the description lookup, which changes nothing a
** Mission holds (the real one is the audio differential's) */
struct CampAudioManager { void getInfoForAudioEvent( const AudioEventRTS * ) const { } };
static CampAudioManager g_audio;
static CampAudioManager *TheAudio = &g_audio;
SkirmishGameInfo *TheChallengeGameInfo = NULL;

#include "campaign_parse_audio_event_rts.inc"
#include "campaign_manager_cut.inc"
#include "challenge_generals_cut.inc"

namespace {
std::vector<unsigned char> g_dump;
void putU32(unsigned v) { for (int i = 0; i < 4; ++i) g_dump.push_back((unsigned char)(v >> (8 * i))); }
void putStr(const AsciiString &s) { putU32((unsigned)s.getLength()); for (int i = 0; i < s.getLength(); ++i) g_dump.push_back((unsigned char)s.str()[i]); }
int flush(unsigned char *buf, int cap)
{
	int len = (int)g_dump.size();
	if (len <= cap) memcpy(buf, g_dump.data(), (size_t)len);
	return len;
}

void campaignBlock(INI *ini) { campParseCampaignDefinition(ini); }
void challengeBlock(INI *ini) { campParseChallengeModeDefinition(ini); }

void dumpMission(const Mission *m)
{
	putStr(m->m_name); putStr(m->m_mapName); putStr(m->m_nextMission); putStr(m->m_movieLabel);
	for (int i = 0; i < MAX_OBJECTIVE_LINES; ++i) putStr(m->m_missionObjectivesLabel[i]);
	putStr(m->m_briefingVoice.getEventName()); putStr(m->m_locationNameLabel);
	for (int i = 0; i < MAX_DISPLAYED_UNITS; ++i) putStr(m->m_unitNames[i]);
	putU32((unsigned)m->m_voiceLength); putStr(m->m_generalName);
}
}	/* namespace */

extern "C" {
extern void (*ref_campaign_block_hook[2])(INI *);

/* A new TheCampaignManager (`CampaignManager()`) and TheChallengeGenerals (`createChallengeGenerals()`: MSGNEW, so zero-
** filled, as the real allocator fills it: the constructor initialises no persona's `m_bStartsEnabled` nor the images
** after the portraits); the block hooks set. The old ones are dropped. */
void ref_campaign_reset(void)
{
	TheCampaignManager = NEW CampaignManager;
	TheChallengeGenerals = createChallengeGenerals();
	ref_campaign_block_hook[0] = campaignBlock;
	ref_campaign_block_hook[1] = challengeBlock;
}

/* The campaign list, in order: its length, then per campaign its name, first mission, name label, final movie, challenge
** flag, player faction and missions (each mission's every field); then the current campaign's and mission's names (a
** presence flag before each), the victory flag, the rank points (the member, not getRankPoints' 0), the difficulty,
** getCurrentMissionNumber and getCurrentMap. Returns the length (may exceed cap). */
int ref_campaign_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	CampaignManager *cm = TheCampaignManager;
	putU32((unsigned)cm->m_campaignList.size());
	for (CampaignManager::CampaignListIt it = cm->m_campaignList.begin(); it != cm->m_campaignList.end(); ++it) {
		Campaign *c = *it;
		putStr(c->m_name); putStr(c->m_firstMission); putStr(c->m_campaignNameLabel); putStr(c->m_finalMovieName);
		putU32(c->m_isChallengeCampaign ? 1u : 0u); putStr(c->m_playerFactionName);
		putU32((unsigned)c->m_missions.size());
		for (Campaign::MissionListIt m = c->m_missions.begin(); m != c->m_missions.end(); ++m) dumpMission(*m);
	}
	putU32(cm->getCurrentCampaign() ? 1u : 0u);
	if (cm->getCurrentCampaign()) putStr(cm->getCurrentCampaign()->m_name);
	putU32(cm->getCurrentMission() ? 1u : 0u);
	if (cm->getCurrentMission()) putStr(cm->getCurrentMission()->m_name);
	putU32(cm->isVictorious() ? 1u : 0u); putU32((unsigned)cm->m_currentRankPoints); putU32((unsigned)cm->getGameDifficulty());
	putU32((unsigned)cm->getCurrentMissionNumber()); putStr(cm->getCurrentMap());
	return flush(buf, cap);
}

/* An operation on TheCampaignManager: 0 setCampaign(a), 1 setCampaignAndMission(a, b), 2 gotoNextMission() (the
** result: whether a mission came back), 3 SetVictorious(n != 0), 4 setRankPoints(n), 5 setGameDifficulty(n). Returns
** gotoNextMission's result for 2, else 0. */
int ref_campaign_op(int op, const char *a, const char *b, int n)
{
	CampaignManager *cm = TheCampaignManager;
	switch (op) {
	case 0: cm->setCampaign(AsciiString(a)); return 0;
	case 1: cm->setCampaignAndMission(AsciiString(a), AsciiString(b)); return 0;
	case 2: return cm->gotoNextMission() ? 1 : 0;
	case 3: cm->SetVictorious(n != 0); return 0;
	case 4: cm->setRankPoints(n); return 0;
	default: cm->setGameDifficulty((GameDifficulty)n); return 0;
	}
}

/* The twelve personas, each: m_bStartsEnabled, then its strings in the class's order (the images left out: the
** build has no TheMappedImageCollection, so parseMappedImage stores nothing; the port keeps their names). */
int ref_challenge_dump(unsigned char *buf, int cap)
{
	g_dump.clear();
	const GeneralPersona *p = TheChallengeGenerals->getChallengeGenerals();
	for (int i = 0; i < NUM_GENERALS; ++i) {
		const GeneralPersona &g = p[i];
		putU32(g.m_bStartsEnabled ? 1u : 0u);
		putStr(g.m_strBioName); putStr(g.m_strBioDOB); putStr(g.m_strBioBirthplace); putStr(g.m_strBioStrategy);
		putStr(g.m_strBioRank); putStr(g.m_strBioBranch); putStr(g.m_strBioClassNumber); putStr(g.m_strCampaign);
		putStr(g.m_strPlayerTemplateName); putStr(g.m_strPortraitMovieLeftName); putStr(g.m_strPortraitMovieRightName);
		putStr(g.m_strDefeated); putStr(g.m_strVictorious); putStr(g.m_strSelectionSound); putStr(g.m_strTauntSound1);
		putStr(g.m_strTauntSound2); putStr(g.m_strTauntSound3); putStr(g.m_strWinSound); putStr(g.m_strLossSound);
		putStr(g.m_strPreviewSound); putStr(g.m_strNameSound);
	}
	return flush(buf, cap);
}

/* A lookup: 0 getPlayerGeneralByCampaignName, 1 getGeneralByGeneralName, 2 getGeneralByTemplateName; the persona's index,
** or -1 for NULL. */
int ref_challenge_lookup(int kind, const char *name)
{
	const GeneralPersona *g = kind == 0 ? TheChallengeGenerals->getPlayerGeneralByCampaignName(AsciiString(name))
		: kind == 1 ? TheChallengeGenerals->getGeneralByGeneralName(AsciiString(name))
		: TheChallengeGenerals->getGeneralByTemplateName(AsciiString(name));
	return g ? (int)(g - TheChallengeGenerals->getChallengeGenerals()) : -1;
}
}	/* extern "C" */
