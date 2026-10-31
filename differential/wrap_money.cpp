/* Differential-test wrapper for Money (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/Common/RTS/Money.cpp
** (#included below, unchanged), with the stand-ins of differential/shim_money
** for TheAudio (MiscAudio's sounds, addAudioEvent), ThePlayerList
** (getNthPlayer, eight players) and the players' academy statistics
** (recordIncome). The stand-ins log what logic asks of the client and count
** the income records.
**
** Entry points (extern "C", tests/differential_money.rs):
**   ref_money_op     withdraw / deposit on a Money of a given amount and player
**   ref_money_parse  Money::parseMoneyAmount through the INI reader
**   ref_money_save / ref_money_load / ref_money_deep_crc  the Snapshot xfer */
/* The stand-ins of this file are named apart from the same names in other
** differential builds (wrap_big.cpp defines a TheAudio of its own): one
** binary links them all, and identical global or class symbols would clash.
** The upstream .cpp and headers below see these names only through the
** macros, so their own text is unchanged. */
#define AudioEventRTS MoneyTestAudioEventRTS
#define MiscAudio MoneyTestMiscAudio
#define AudioManager MoneyTestAudioManager
#define TheAudio MoneyTestTheAudio
#define Player MoneyTestPlayer
#define AcademyStats MoneyTestAcademyStats
#define PlayerList MoneyTestPlayerList
#define ThePlayerList MoneyTestThePlayerList

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <vector>
#include <utility>
#include "Common/INI.h"
#include "Common/INIException.h"
#include "Common/File.h"
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "Common/XferDeepCRC.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RTS/Money.cpp"

/* from wrap_ini.cpp: the in-memory file system of the INI build */
extern "C" void ref_ini_reset(void);
extern "C" void ref_ini_add_file(const char *name, const char *data, int len);

/* ---- stand-ins ----------------------------------------------------------- */
static std::vector<std::pair<int, int> > g_events;   /* (MiscAudio sound id, player index) */
static int g_recordIncomeCalls = 0;
static Player g_players[8];
static MiscAudio g_misc;
static AudioManager g_audio;
static PlayerList g_playerList;
AudioManager *TheAudio = &g_audio;
PlayerList *ThePlayerList = &g_playerList;

MiscAudio *AudioManager::getMiscAudio() { return &g_misc; }
void AudioManager::addAudioEvent(const AudioEventRTS *event) { g_events.push_back(std::make_pair(event->m_id, event->m_playerIndex)); }
Player *PlayerList::getNthPlayer(Int i) { return (i >= 0 && i < 8) ? &g_players[i] : NULL; }
void AcademyStats::recordIncome() { ++g_recordIncomeCalls; }

/* ---- INI: the block "MONEY" with one field (the reader of wrap_geometry.cpp) */
namespace {

class MoneyIniReader : public INI
{
public:
	void readBlock(const char *file, void *what, const FieldParse *table)
	{
		prepFile(AsciiString(file), INI_LOAD_OVERWRITE);
		try {
			readLine();
			if (!::strtok(m_buffer, getSeps())) throw INI_INVALID_DATA;
			initFromINI(what, table);
		} catch (...) {
			if (m_file) { m_file->close(); m_file = NULL; }
			throw;
		}
		if (m_file) { m_file->close(); m_file = NULL; }
	}
};

const FieldParse MoneyFields[] =
{
	{ "DefaultStartingMoney", Money::parseMoneyAmount, NULL, 0 },
	{ NULL, NULL, NULL, 0 }
};

/* a Money as the test sets it up: its player index, then its amount (a
** deposit records income, so the counter is reset before the operation) */
void setUp(Money &m, unsigned money0, int player0)
{
	m.setPlayerIndex(player0);
	if (money0) m.deposit(money0, FALSE);
	g_events.clear();
	g_recordIncomeCalls = 0;
}

}  // namespace

extern "C" {

/* op 0: withdraw(amount, playSound) -> out[0] the amount returned;
** op 1: deposit(amount, playSound)  -> out[0] 0.
** Both: out[1] the money after, out[2] the number of recordIncome calls,
** out[3] the number of client events n, then out[4 + 2k], out[5 + 2k] the
** event's sound id and player index, for k < n (at most 2 here). */
void ref_money_op(int op, unsigned money0, int player0, unsigned amount, int playSound, unsigned *out)
{
	Money m;
	setUp(m, money0, player0);
	out[0] = 0;
	if (op == 0) {
		out[0] = m.withdraw(amount, playSound != 0);
	} else {
		m.deposit(amount, playSound != 0);
	}
	out[1] = m.countMoney();
	out[2] = (unsigned)g_recordIncomeCalls;
	out[3] = (unsigned)g_events.size();
	for (size_t k = 0; k < g_events.size() && k < 2; ++k) {
		out[4 + 2 * k] = (unsigned)g_events[k].first;
		out[5 + 2 * k] = (unsigned)g_events[k].second;
	}
}

/* Money::parseMoneyAmount on `data` (one MONEY block). out[0]: 0 parsed,
** 1 an INIException (message into msg, *msgLen its length), 2 an INI_* status
** code; out[1] the amount. */
void ref_money_parse(const char *file, const char *data, int len, unsigned *out, char *msg, int msgCap, int *msgLen)
{
	ref_ini_reset();
	ref_ini_add_file(file, data, len);
	Money m;
	int status = 0;
	*msgLen = 0;
	try {
		MoneyIniReader ini;
		ini.readBlock(file, &m, MoneyFields);
	} catch (INIException &e) {
		status = 1;
		const char *s = e.mFailureMessage ? e.mFailureMessage : "";
		int n = (int)strlen(s);
		*msgLen = n;
		if (n + 1 <= msgCap) memcpy(msg, s, n + 1);
	} catch (...) {
		status = 2;
	}
	out[0] = (unsigned)status;
	out[1] = m.countMoney();
}

/* XferSave of Money(money0, player0) to `path`; 0, or the XferStatus + 1. */
int ref_money_save(const char *path, unsigned money0, int player0)
{
	Money m;
	setUp(m, money0, player0);
	XferSave x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(&m);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
	}
	return status;
}

/* XferLoad of `path` into Money(money0, player0): out[0] the status (0 none,
** else the XferStatus + 1), out[1] the amount. */
void ref_money_load(const char *path, unsigned money0, int player0, unsigned *out)
{
	Money m;
	setUp(m, money0, player0);
	XferLoad x;
	int status = 0;
	try {
		x.open(AsciiString(path));
		x.xferSnapshot(&m);
		x.close();
	} catch (XferStatus s) {
		status = (int)s + 1;
		try { x.close(); } catch (...) {}
	}
	out[0] = (unsigned)status;
	out[1] = m.countMoney();
}

unsigned ref_money_deep_crc(const char *path, unsigned money0, int player0)
{
	Money m;
	setUp(m, money0, player0);
	XferDeepCRC x;
	x.open(AsciiString(path));  /* XferDeepCRC writes the file it is given */
	x.xferSnapshot(&m);
	x.close();
	return x.getCRC();
}

} /* extern "C" */

/* layer 12 batch G1 (differential/wrap_game_info.cpp): the income records so far (GameInfo's starting cash deposits) */
extern "C" int ref_money_record_income_calls(void) { return g_recordIncomeCalls; }
