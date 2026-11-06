/* Differential-test wrapper for the audio (GitHub #13; upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INIAudioEventInfo.cpp
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INIMiscAudio.cpp
**   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp (the AudioSettings table, the speaker translations,
**     INI::parseAudioSettingsDefinition, parseSpeakerType)
**   GeneralsMD/Code/GameEngine/Source/Common/Audio/AudioEventRTS.cpp
**   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp (INI::parseAudioEventRTS)
**   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/OptionsMenu.cpp (OptionPreferences' audio
**     getters)
** compiled with the REAL AudioEventInfo.h, AudioEventRTS.h, AudioSettings.h, MiscAudio.h, AudioRandomValue.h,
** LogicRandomValue.h and INI.h, on the stand-ins of differential/shim_audio (Common/GameAudio.h, audio_world.h; their
** definitions are below). build.rs cuts every upstream piece VERBATIM into OUT_DIR/audio_gen (see its comment).
** The INI is read by the real INI::load (wrap_ini.cpp's ref_ini_load, whose audio blocks reach the handlers here
** through ref_audio_block_hook); the random generators are RandomValue.cpp's (upstream_gameengine; the test sets
** their state with ref_set_state).
**
** What is transcribed rather than cut (the functions of GameAudio.cpp that live in the real AudioManager): the bodies
** of newAudioEventInfo, findAudioEventInfo, addTrackName and getInfoForAudioEvent, unchanged but for the map type
** (see shim_audio/Common/GameAudio.h).
**
** Entry points (extern "C", tests/differential_audio.rs documents the encodings): ref_audio_reset, ref_audio_pref,
** ref_audio_file, ref_audio_language, ref_audio_object, ref_audio_dump, ref_audio_event. */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <string>
#include <vector>
#include <map>
#define private public
#define protected public
#include "PreRTS.h"
#include "Common/INI.h"
#include "audio_world.h"
#include "Common/GameAudio.h"
#include "Common/AudioEventInfo.h"
#include "Common/AudioSettings.h"
#include "Common/AudioEventRTS.h"
#include "Common/MiscAudio.h"
#include "Common/AudioRandomValue.h"
#include "Common/AudioHandleSpecialValues.h"
#include "GameLogic/LogicRandomValue.h"
#include "ini_audio_event_info.inc"
#include "ini_misc_audio.inc"
#include "game_audio_table.inc"
#include "game_audio_speakers.inc"
#include "game_audio_settings.inc"
#include "ini_parse_audio_event_rts.inc"
#include "AudioEventRTS_cut.cpp"
#include "options_prefs.inc"
#undef private
#undef protected

/* ------------------------------------------------------------------------- */
/* AudioManager's functions of GameAudio.cpp (transcribed, see the header)    */
/* ------------------------------------------------------------------------- */

AudioManager *TheAudio = NULL;

AudioEventInfo *AudioManager::newAudioEventInfo( AsciiString audioName )
{
	AudioEventInfo *eventInfo = findAudioEventInfo(audioName);
	if (eventInfo) {
		return eventInfo;
	}

	m_allAudioEventInfo[audioName.str()] = newInstance(AudioEventInfo);
	return m_allAudioEventInfo[audioName.str()];
}

AudioEventInfo *AudioManager::findAudioEventInfo( AsciiString eventName ) const
{
	std::map<std::string, AudioEventInfo *>::const_iterator it = m_allAudioEventInfo.find(eventName.str());
	if (it == m_allAudioEventInfo.end()) {
		return NULL;
	}
	return (*it).second;
}

void AudioManager::addTrackName( const AsciiString& trackName )
{
	m_musicTracks.push_back(trackName);
}

void AudioManager::getInfoForAudioEvent( const AudioEventRTS *eventToFindAndFill ) const
{
	if (!eventToFindAndFill) {
		return;
	}
	if (eventToFindAndFill->getAudioEventInfo()) {
		return;
	}
	eventToFindAndFill->setAudioEventInfo(findAudioEventInfo(eventToFindAndFill->getEventName()));
}

const FieldParse *AudioManager::getFieldParseTable( void ) const
{
	return audioSettingsFieldParseTable;
}

/* ------------------------------------------------------------------------- */
/* The stand-ins of audio_world.h                                             */
/* ------------------------------------------------------------------------- */

namespace {
std::map<unsigned, Object> g_objects;
std::map<unsigned, Drawable> g_drawables;
std::vector<std::string> g_files;
std::string g_language = "English";
std::map<std::string, std::string> g_prefs;
GameLogic g_logic;
GameClient g_client;
FileSystem g_fs;

std::vector<unsigned> g_out;
void w(unsigned x) { g_out.push_back(x); }
void wf(Real r) { unsigned u; memcpy(&u, &r, 4); g_out.push_back(u); }
void wstr(const AsciiString &s) { w((unsigned)s.getLength()); for (Int i = 0; i < s.getLength(); ++i) w((unsigned char)s.str()[i]); }
void wvec(const std::vector<AsciiString> &v) { w((unsigned)v.size()); for (size_t i = 0; i < v.size(); ++i) wstr(v[i]); }
int flush(unsigned *out, int cap)
{
	int n = (int)g_out.size();
	for (int i = 0; i < n && i < cap; ++i) out[i] = g_out[i];
	g_out.clear();
	return n;
}

/* every field of an AudioEventInfo, in the Rust struct's order */
void dumpInfo(const AudioEventInfo &i)
{
	wstr(i.m_audioName); wstr(i.m_filename);
	wf(i.m_volume); wf(i.m_volumeShift); wf(i.m_minVolume); wf(i.m_pitchShiftMin); wf(i.m_pitchShiftMax);
	w(i.m_delayMin); w(i.m_delayMax); w(i.m_limit); w(i.m_loopCount); w(i.m_priority); w(i.m_type); w(i.m_control);
	wvec(i.m_soundsMorning); wvec(i.m_sounds); wvec(i.m_soundsNight); wvec(i.m_soundsEvening);
	wvec(i.m_attackSounds); wvec(i.m_decaySounds);
	wf(i.m_lowPassFreq); wf(i.m_minDistance); wf(i.m_maxDistance); w(i.m_soundType);
}

void dumpSettings(const AudioSettings &s)
{
	wstr(s.m_audioRoot); wstr(s.m_soundsFolder); wstr(s.m_musicFolder); wstr(s.m_streamingFolder); wstr(s.m_soundsExtension);
	w(s.m_useDigital); w(s.m_useMidi); w(s.m_outputRate); w(s.m_outputBits); w(s.m_outputChannels);
	w(s.m_sampleCount2D); w(s.m_sampleCount3D); w(s.m_streamCount); w(s.m_globalMinRange); w(s.m_globalMaxRange);
	w(s.m_drawableAmbientFrames); w(s.m_fadeAudioFrames); w(s.m_maxCacheSize); wf(s.m_minVolume);
	for (int i = 0; i <= MAX_HW_PROVIDERS; ++i) wstr(s.m_preferred3DProvider[i]);
	wf(s.m_relative2DVolume); wf(s.m_defaultSoundVolume); wf(s.m_default3DSoundVolume); wf(s.m_defaultSpeechVolume);
	wf(s.m_defaultMusicVolume); w(s.m_defaultSpeakerType2D); w(s.m_defaultSpeakerType3D);
	wf(s.m_preferredSoundVolume); wf(s.m_preferred3DSoundVolume); wf(s.m_preferredSpeechVolume); wf(s.m_preferredMusicVolume);
	wf(s.m_microphoneDesiredHeightAboveTerrain); wf(s.m_microphoneMaxPercentageBetweenGroundAndCamera);
	wf(s.m_zoomMinDistance); wf(s.m_zoomMaxDistance); wf(s.m_zoomSoundVolumePercentageAmount);
}

void hookAudioEvent(INI *ini) { INI::parseAudioEventDefinition(ini); }
void hookSettings(INI *ini) { INI::parseAudioSettingsDefinition(ini); }
void hookDialog(INI *ini) { INI::parseDialogDefinition(ini); }
void hookMisc(INI *ini) { INI::parseMiscAudio(ini); }
void hookMusic(INI *ini) { INI::parseMusicTrackDefinition(ini); }
}

GameLogic *TheGameLogic = &g_logic;
GameClient *TheGameClient = &g_client;
FileSystem *TheFileSystem = &g_fs;

Object *GameLogic::findObjectByID( ObjectID id )
{
	std::map<unsigned, Object>::iterator it = g_objects.find((unsigned)id);
	return it == g_objects.end() ? NULL : &it->second;
}

Drawable *GameClient::findDrawableByID( DrawableID id )
{
	Object *o = TheGameLogic->findObjectByID((ObjectID)id);
	if (!o) return NULL;
	Drawable &d = g_drawables[(unsigned)id];
	d.m_object = o;
	return &d;
}

Bool FileSystem::doesFileExist( const Char *filename ) const
{
	for (size_t i = 0; i < g_files.size(); ++i) if (g_files[i] == filename) return TRUE;
	return FALSE;
}

AsciiString GetRegistryLanguage( void ) { return AsciiString(g_language.c_str()); }

OptionPreferences::OptionPreferences()
{
	for (std::map<std::string, std::string>::iterator it = g_prefs.begin(); it != g_prefs.end(); ++it)
		(*this)[AsciiString(it->first.c_str())] = AsciiString(it->second.c_str());
}

extern "C" {

extern void (*ref_audio_block_hook[5])(INI *);

/* A new AudioManager: no descriptions or tracks, zeroed settings (value-initialised) and misc sounds; no files,
** objects or preferences; English; the block hooks installed. */
void ref_audio_reset(void)
{
	TheAudio = new AudioManager;
	TheAudio->m_audioSettings = new AudioSettings();
	TheAudio->m_miscAudio = new MiscAudio();
	g_objects.clear(); g_drawables.clear(); g_files.clear(); g_prefs.clear();
	g_language = "English";
	ref_audio_block_hook[0] = hookAudioEvent;
	ref_audio_block_hook[1] = hookSettings;
	ref_audio_block_hook[2] = hookDialog;
	ref_audio_block_hook[3] = hookMisc;
	ref_audio_block_hook[4] = hookMusic;
}

void ref_audio_pref(const char *key, const char *value) { g_prefs[key] = value; }
void ref_audio_file(const char *name) { g_files.push_back(name); }
void ref_audio_language(const char *lang) { g_language = lang; }
void ref_audio_object(unsigned id, float x, float y, float z, int player)
{
	Object o; o.m_id = (ObjectID)id; o.m_pos.x = x; o.m_pos.y = y; o.m_pos.z = z; o.m_player.m_index = player;
	g_objects[id] = o;
}

/* Every description (by name), the track list, the settings, the misc sounds' names (in the field table's order) and
** the device's preferred provider and speaker. */
int ref_audio_dump(unsigned *out, int cap)
{
	w((unsigned)TheAudio->m_allAudioEventInfo.size());
	for (std::map<std::string, AudioEventInfo *>::iterator it = TheAudio->m_allAudioEventInfo.begin(); it != TheAudio->m_allAudioEventInfo.end(); ++it)
		dumpInfo(*it->second);
	wvec(TheAudio->m_musicTracks);
	dumpSettings(*TheAudio->m_audioSettings);
	for (const FieldParse *f = MiscAudio::m_fieldParseTable; f->token; ++f)
		wstr(((AudioEventRTS *)((char *)TheAudio->m_miscAudio + f->offset))->getEventName());
	wstr(TheAudio->m_pref3DProvider); wstr(TheAudio->m_prefSpeaker);
	return flush(out, cap);
}

static void dumpEventState(AudioEventRTS &e)
{
	wstr(e.m_filenameToLoad); w(e.m_playingAudioIndex); wf(e.m_delay);
	wf(e.m_pitchShift); wf(e.m_volumeShift); w(e.m_loopCount); wstr(e.m_attackName); wstr(e.m_decayName);
	w(e.m_portionToPlayNext); w(e.m_isLogicalAudio);
}

/* An event of `name` (its description looked up): owner 0 none, 1 an object `id`, 2 a drawable `id`, 3 the position
** `pos`; `logical`; then for each of `ops` (bit 0 of each 2-bit pair: generateFilename; bit 1: generatePlayInfo,
** low bits first) its state; then isPositionalAudio, getVolume, getCurrentPosition (a flag and x, y, z), the owner type
** and getPlayerIndex. */
int ref_audio_event(const char *name, int owner, unsigned id, const float *pos, int logical, int playerIndex, int nOps, unsigned ops,
	unsigned *out, int cap)
{
	AudioEventRTS *e;
	Coord3D p; p.x = pos[0]; p.y = pos[1]; p.z = pos[2];
	if (owner == 1) e = new AudioEventRTS(AsciiString(name), (ObjectID)id);
	else if (owner == 2) e = new AudioEventRTS(AsciiString(name), (DrawableID)id);
	else if (owner == 3) e = new AudioEventRTS(AsciiString(name), &p);
	else e = new AudioEventRTS(AsciiString(name));
	/* what upstream's constructors leave uninitialised, as the Rust starts it: the union (the plain constructor), the
	** position (the object and drawable constructors) and the portion to play (all of them) */
	if (owner == 0) e->m_objectID = INVALID_ID;
	if (owner == 1 || owner == 2) e->m_positionOfAudio.zero();
	e->m_portionToPlayNext = PP_Attack;
	e->setIsLogicalAudio(logical != 0);
	e->setPlayerIndex(playerIndex);
	TheAudio->getInfoForAudioEvent(e);
	w(e->getAudioEventInfo() != NULL);
	if (e->getAudioEventInfo()) {
		for (int k = 0; k < nOps; ++k) {
			unsigned op = (ops >> (2 * k)) & 3;
			if (op & 1) e->generateFilename();
			if (op & 2) e->generatePlayInfo();
			dumpEventState(*e);
		}
	}
	w(e->isPositionalAudio()); wf(e->getVolume());
	const Coord3D *c = e->getCurrentPosition();
	w(c != NULL); if (c) { wf(c->x); wf(c->y); wf(c->z); } else { wf(0); wf(0); wf(0); }
	w(e->getOwnerType()); w(e->getPlayerIndex());
	delete e;
	return flush(out, cap);
}

}
