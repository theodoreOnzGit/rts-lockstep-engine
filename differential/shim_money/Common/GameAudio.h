/* Stand-in for GameEngine/Include/Common/GameAudio.h and the MiscAudio.h
** sounds, money tests only. Money::withdraw/deposit copy one of MiscAudio's
** AudioEventRTS, set its player index, and hand it to TheAudio's
** addAudioEvent: the stand-in logs each call (wrap_money.cpp). */
#ifndef SHIM_MONEY_GAMEAUDIO_H
#define SHIM_MONEY_GAMEAUDIO_H
#include "Lib/BaseType.h"
class AudioEventRTS
{
public:
	AudioEventRTS(Int id = 0) : m_id(id), m_playerIndex(0) {}
	void setPlayerIndex(Int ndx) { m_playerIndex = ndx; }
	Int m_id;
	Int m_playerIndex;
};
class MiscAudio
{
public:
	MiscAudio() : m_moneyDepositSound(1), m_moneyWithdrawSound(2) {}
	AudioEventRTS m_moneyDepositSound;
	AudioEventRTS m_moneyWithdrawSound;
};
class AudioManager
{
public:
	MiscAudio *getMiscAudio();
	void addAudioEvent(const AudioEventRTS *event);
};
extern AudioManager *TheAudio;
#endif
