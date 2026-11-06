/* Stand-ins for what AudioEventRTS.cpp and INI::parseAudioSettingsDefinition reach outside the audio (audio build
** only; differential tests, GitHub #13; build.rs, differential/wrap_audio.cpp). Renamed by shim_audio/PreRTS.h; here
** they are written with their upstream names.
**  - TheFileSystem->doesFileExist: the names the test lists (ref_audio_add_file), exact bytes;
**  - GetRegistryLanguage(): the language the test sets;
**  - TheGameLogic->findObjectByID, TheGameClient->findDrawableByID: the objects the test places (an id, a position
**    and a controlling player index); a drawable is its object (the Rust's client drawables are keyed by object id);
**  - OptionPreferences: the user's Options.ini, a map of its keys (empty unless the test sets one); its getters are
**    cut VERBATIM from OptionsMenu.cpp by build.rs. */
#ifndef SHIM_AUDIO_WORLD_H
#define SHIM_AUDIO_WORLD_H
#include "Lib/BaseType.h"
#include "Common/AsciiString.h"
#include "Common/GameType.h"
#include <map>

class Player
{
public:
	Int m_index;
	Int getPlayerIndex() const { return m_index; }
};

class Object
{
public:
	ObjectID m_id;
	Coord3D m_pos;
	Player m_player;
	const Coord3D *getPosition() const { return &m_pos; }
	Player *getControllingPlayer() { return &m_player; }
};

class Drawable
{
public:
	Object *m_object;
	const Coord3D *getPosition() const { return m_object->getPosition(); }
	Object *getObject() { return m_object; }
};

class GameLogic
{
public:
	Object *findObjectByID( ObjectID id );
};
extern GameLogic *TheGameLogic;

class GameClient
{
public:
	Drawable *findDrawableByID( DrawableID id );
};
extern GameClient *TheGameClient;

class FileSystem
{
public:
	Bool doesFileExist( const Char *filename ) const;
};
extern FileSystem *TheFileSystem;

AsciiString GetRegistryLanguage( void );

class OptionPreferences : public std::map<AsciiString, AsciiString>
{
public:
	OptionPreferences();
	AsciiString getPreferred3DProvider( void );
	AsciiString getSpeakerType( void );
	Real getSoundVolume( void );
	Real get3DSoundVolume( void );
	Real getSpeechVolume( void );
	Real getMusicVolume( void );
};
#endif
