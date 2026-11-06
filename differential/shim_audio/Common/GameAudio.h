/* Stand-in for GameEngine/Include/Common/GameAudio.h, audio build only (differential tests, GitHub #13; build.rs,
** differential/wrap_audio.cpp). The real header declares AudioManager, an abstract subsystem of some ninety virtual
** functions, most of them the Miles device's. Kept from it verbatim: the AudioHandle typedef and the forward
** declarations. AudioManager here has only what the code compiled in this build calls: the description table
** (`m_allAudioEventInfo`, an ORDERED map by name, as the Rust's BTreeMap; upstream's hash order is unspecified and
** never compared), the track list, the settings and the miscellaneous sounds, and the device's preferred provider
** and speaker, which `parseAudioSettingsDefinition` sets. The functions upstream defines in GameAudio.cpp
** (newAudioEventInfo, findAudioEventInfo, addTrackName, the friend getters, getInfoForAudioEvent and the speaker
** translations) are cut VERBATIM from it by build.rs where they are self-contained (the speaker translations);
** the others are transcribed in wrap_audio.cpp next to their upstream text. The class is renamed by
** shim_audio/PreRTS.h. */
#ifndef SHIM_AUDIO_GAMEAUDIO_H
#define SHIM_AUDIO_GAMEAUDIO_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/AudioAffect.h"
#include <map>
#include <string>
#include <vector>

class AudioEventRTS;
struct AudioEventInfo;
struct AudioSettings;
struct MiscAudio;
struct FieldParse;
typedef UnsignedInt AudioHandle;

class AudioManager
{
public:
	std::map<std::string, AudioEventInfo *> m_allAudioEventInfo;
	std::vector<AsciiString> m_musicTracks;
	AudioSettings *m_audioSettings;
	MiscAudio *m_miscAudio;
	AsciiString m_pref3DProvider;
	AsciiString m_prefSpeaker;

	AudioEventInfo *newAudioEventInfo( AsciiString audioName );
	AudioEventInfo *findAudioEventInfo( AsciiString eventName ) const;
	void addTrackName( const AsciiString& trackName );
	const AudioSettings *getAudioSettings( void ) const { return m_audioSettings; }
	AudioSettings *friend_getAudioSettings( void ) { return m_audioSettings; }
	MiscAudio *friend_getMiscAudio( void ) { return m_miscAudio; }
	const FieldParse *getFieldParseTable( void ) const;
	void getInfoForAudioEvent( const AudioEventRTS *eventToFindAndFill ) const;
	UnsignedInt translateSpeakerTypeToUnsignedInt( const AsciiString& speakerType );
	AsciiString translateUnsignedIntToSpeakerType( UnsignedInt speakerType );
	void setPreferredProvider( AsciiString providerNdx ) { m_pref3DProvider = providerNdx; }
	void setPreferredSpeaker( AsciiString speakerType ) { m_prefSpeaker = speakerType; }
	Bool isCurrentlyPlaying( AudioHandle ) { return TRUE; }
};
extern AudioManager *TheAudio;
#endif
