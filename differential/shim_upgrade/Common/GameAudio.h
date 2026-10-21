/* Stand-in for GameEngine/Include/Common/GameAudio.h, Upgrade/SpecialPower
** build only (differential tests, p1w2-upgrade): differential/shim/Common/
** GameAudio.h without its own AudioEventRTS (this build's is in
** AudioEventRTS.h). INI::parseAudioEventRTS calls getInfoForAudioEvent on
** TheAudio, which is null in the INI tests; the wrapper defines it to do
** nothing. */
#ifndef SHIM_UPGRADE_GAMEAUDIO_H
#define SHIM_UPGRADE_GAMEAUDIO_H
#include "Common/AudioAffect.h"
#include "Common/AsciiString.h"
#include "Common/GameMemory.h"
#include "Common/AudioEventRTS.h"
class DynamicAudioEventRTS : public MemoryPoolObject
{
public:
	AudioEventRTS m_event;
};
class AudioManager
{
public:
	virtual ~AudioManager() {}
	virtual void stopAudio( AudioAffect which ) = 0;
	void getInfoForAudioEvent( const AudioEventRTS *eventToFindAndFill ) const;
	/* --- p1w11-sonnet --- (the Module build renames this class; its die batch D defines the call) */
	UnsignedInt addAudioEvent( const AudioEventRTS *eventToAdd );
	const class MiscAudio *getMiscAudio( void ) const;   /* the crates (differential/l9_crates_f.inc) */
	/* --- end p1w11-sonnet --- */
};
extern AudioManager *TheAudio;
#endif
