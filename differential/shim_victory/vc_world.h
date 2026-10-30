/* The world of VictoryConditions.cpp, the VictoryConditions fixture only (differential tests, p1w13-sonnet, layer 11 batch V1;
** differential/wrap_victory.cpp). Every header the file includes is this one (the stand-ins of differential/shim_victory); the real
** GameLogic/VictoryConditions.h is the file's own. Everything is compiled inside namespace vc (the wrapper opens it), so that no symbol
** collides with another fixture's. The players answer from the test's configuration (their questions are Player's, verified in
** tests/differential_player.rs); every call with an effect writes a line (vcLog). */
#ifndef VC_WORLD_H
#define VC_WORLD_H
#include <string>
#include <vector>
#include <cstdio>
#include <cstring>
#include <cstdarg>
#include <cwchar>

typedef int Int;
typedef unsigned int UnsignedInt;
typedef bool Bool;
typedef float Real;
typedef char Char;
typedef wchar_t WideChar;
#ifndef TRUE
#define TRUE true
#define FALSE false
#endif
#ifndef NULL
#define NULL 0
#endif
#define NEW new
#define DEBUG_LOG(x)
#define DEBUG_ASSERTCRASH(c, x)

void vcLog(const char *fmt, ...);

/* GameCommon.h: MAX_PLAYER_COUNT; Relationship; NetworkDefs.h: MAX_SLOTS (MAX_PLAYER + 1) */
enum { MAX_PLAYER_COUNT = 16 };
enum Relationship { ENEMIES = 0, NEUTRAL, ALLIES };
static const Int MAX_SLOTS = 8;

class AsciiString
{
public:
	AsciiString() {}
	AsciiString(const char *s) : m_s(s) {}
	void format(const char *fmt, ...)
	{
		char buf[256];
		va_list ap;
		va_start(ap, fmt);
		vsnprintf(buf, sizeof(buf), fmt, ap);
		va_end(ap);
		m_s = buf;
	}
	const char *str() const { return m_s.c_str(); }
	std::string m_s;
};
class UnicodeString
{
public:
	const WideChar *str() const { return m_s.c_str(); }
	std::wstring m_s;
};

typedef Int NameKeyType;
/* the name keys: "playerN" is 1000 + N, "FactionCivilian" 500, any other name 1 (the Rust host's) */
class NameKeyGenerator
{
public:
	NameKeyType nameToKey(const AsciiString &name)
	{
		int n = 0;
		char tail = 0;
		if (sscanf(name.str(), "player%d%c", &n, &tail) == 1) return 1000 + n;
		if (strcmp(name.str(), "FactionCivilian") == 0) return 500;
		return 1;
	}
};
extern NameKeyGenerator *TheNameKeyGenerator;
#define NAMEKEY(s) TheNameKeyGenerator->nameToKey(s)

class SubsystemInterface
{
public:
	virtual ~SubsystemInterface() {}
	virtual void init(void) = 0;
	virtual void reset(void) = 0;
	virtual void update(void) = 0;
};

/* KindOf.h: the one bit the file sets */
enum KindOfType { KINDOF_MP_COUNT_FOR_VICTORY = 33 };
class KindOfMaskType
{
public:
	KindOfMaskType() { memset(m_bits, 0, sizeof(m_bits)); }
	void set(Int i, Int value = 1) { if (value) m_bits[i / 32] |= 1u << (i % 32); else m_bits[i / 32] &= ~(1u << (i % 32)); }
	Bool test(Int i) const { return (m_bits[i / 32] >> (i % 32) & 1) != 0; }
	unsigned m_bits[4];
};

class PlayerTemplate
{
public:
	Int m_key;
};
class PlayerTemplateStore
{
public:
	const PlayerTemplate *findPlayerTemplate(NameKeyType key) { return key == 500 && m_hasCivilian ? &m_civilian : 0; }
	PlayerTemplate m_civilian;
	PlayerTemplate m_other;
	Bool m_hasCivilian;
};
extern PlayerTemplateStore *ThePlayerTemplateStore;

class Team;
class Player
{
public:
	Int getPlayerIndex() const { return m_index; }
	Team *getDefaultTeam() const { return m_defaultTeam; }
	Relationship getRelationship(const Team *that) const;
	Bool hasAnyObjects() const { return m_objects; }
	Bool hasAnyUnits() const { return m_units; }
	Bool hasAnyBuildings(KindOfMaskType mask) const
	{
		/* (the file asks with the victory count's bit) */
		return mask.test(KINDOF_MP_COUNT_FOR_VICTORY) && m_buildings;
	}
	UnicodeString getPlayerDisplayName() const { UnicodeString u; wchar_t b[16]; swprintf(b, 16, L"P%d", m_index); u.m_s = b; return u; }
	NameKeyType getPlayerNameKey() const { return m_nameKey; }
	void killPlayer() { vcLog("kill %d", m_index); }
	Bool isLocalPlayer() const { return m_local; }
	Bool isPlayerObserver() const { return m_observer; }
	const PlayerTemplate *getPlayerTemplate() const { return m_template; }
	Int m_index;
	Team *m_defaultTeam;
	Bool m_objects, m_units, m_buildings, m_local, m_observer;
	NameKeyType m_nameKey;
	const PlayerTemplate *m_template;
	unsigned m_alliedTo;   /* bit q: this player's relationship to player q's default team is ALLIES */
	unsigned m_neutralTo;  /* bit q: NEUTRAL (else ENEMIES) */
};
class Team
{
public:
	Player *m_owner;
};
inline Relationship Player::getRelationship(const Team *that) const
{
	if (!that) return NEUTRAL;
	Int q = that->m_owner->m_index;
	if (m_alliedTo >> q & 1) return ALLIES;
	return (m_neutralTo >> q & 1) ? NEUTRAL : ENEMIES;
}
class PlayerList
{
public:
	Player *getNthPlayer(Int i) { return i >= 0 && i < MAX_PLAYER_COUNT ? m_slots[i] : 0; }
	Player *getNeutralPlayer() { return m_neutral; }
	Player *m_slots[MAX_PLAYER_COUNT];
	Player *m_neutral;
};
extern PlayerList *ThePlayerList;

class Recorder
{
public:
	Bool isMultiplayer() { return m_multiplayer; }
	Bool m_multiplayer;
};
extern Recorder *TheRecorder;
class GameLogic
{
public:
	UnsignedInt getFrame() { return m_frame; }
	UnsignedInt m_frame;
};
extern GameLogic *TheGameLogic;
class PartitionManager
{
public:
	void revealMapForPlayerPermanently(Int playerIndex) { vcLog("reveal %d", playerIndex); }
};
extern PartitionManager *ThePartitionManager;
class GameClient
{
public:
	void updateFakeDrawables() { vcLog("fakedrawables"); }
};
extern GameClient *TheGameClient;
class InGameUI
{
public:
	void message(AsciiString label, ...)
	{
		va_list ap;
		va_start(ap, label);
		const WideChar *name = va_arg(ap, const WideChar *);
		va_end(ap);
		char n[64];
		wcstombs(n, name, sizeof(n));
		vcLog("message %s %s", label.str(), n);
	}
};
extern InGameUI *TheInGameUI;
class AudioEventRTS
{
public:
	AudioEventRTS(const char *name) : m_name(name) {}
	std::string m_name;
};
class GameAudio
{
public:
	void addAudioEvent(AudioEventRTS *e) { vcLog("audio %s", e->m_name.c_str()); }
};
extern GameAudio *TheAudio;
class Radar
{
public:
	void forceOn(Bool force) { vcLog("radarforce %d", force ? 1 : 0); }
};
extern Radar *TheRadar;
enum InGameChatType { INGAME_CHAT_ALLIES, INGAME_CHAT_EVERYONE, INGAME_CHAT_PLAYERS };
inline void SetInGameChatType(InGameChatType t) { vcLog("chat %d", (int)t); }
inline void PopulateInGameDiplomacyPopup(void) { vcLog("diplomacy"); }
class GameSlot
{
public:
	Bool isAI() const { return m_ai; }
	void setLastFrameInGame(UnsignedInt frame) { vcLog("slotlastframe %d %u", m_idx, frame); }
	Bool m_ai;
	Int m_idx;
};
class GameInfo
{
public:
	GameSlot *getSlot(Int idx) { return idx >= 0 && idx < MAX_SLOTS ? &m_slots[idx] : 0; }
	GameSlot m_slots[MAX_SLOTS];
};
extern GameInfo *TheGameInfo;

#endif
