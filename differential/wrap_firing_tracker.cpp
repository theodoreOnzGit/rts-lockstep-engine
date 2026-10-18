/* Differential-test wrapper for FiringTracker (upstream commit 0a05454d), p1w6-trackers:
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/FiringTracker.cpp  (#included below, unchanged)
** with the real headers FiringTracker.h, AudioHandleSpecialValues.h, GameType.h, Xfer.h, and the
** stand-ins of differential/shim_firing for the module base (UpdateModule), Object, Weapon,
** ThingTemplate, GameLogic, the audio manager and its event, the model-condition flags and the
** disabled mask.
**
** The classes are renamed (FireTest...) so that no other differential build's UpdateModule, Object
** or AudioManager can clash with these: one binary links all of them.
**
** Every call the tracker makes on a stand-in is appended, IN CALL ORDER, to one list (g_calls); the
** log of an operation is its results followed by that list. The order is the point: the tracker's
** result depends on it (see logic/firing_tracker.rs, "The decision"). The stand-ins model one
** thing beyond recording, because upstream does: Object::setWeaponBonusCondition and
** clearWeaponBonusCondition call weaponSetOnWeaponBonusChange when the word changes
** (Object.cpp:4674-4697), and Weapon::onWeaponBonusChange (Weapon.cpp:1959) recomputes
** m_whenWeCanFireAgain of a weapon that is reloading or between shots. Here a weapon flagged
** "reloading" gets m_possibleNext = frame + bonusDelay(word) on every change of the word, for
** every weapon of the owner, which is what makes the tracker's read of getPossibleNextShotFrame
** depend on whether it happens before or after the faerie-fire step.
**
** Entry point (extern "C", tests/differential_firing_tracker.rs):
**   ref_firing_run  a program of tracker operations on one tracker and one owner, logged word by word. */
#define Object FireTestObject
#define ThingTemplate FireTestThingTemplate
#define Weapon FireTestWeapon
#define GameLogic FireTestGameLogic
#define TheGameLogic FireTestTheGameLogic
#define AudioEventRTS FireTestAudioEventRTS
#define AudioManager FireTestAudioManager
#define TheAudio FireTestTheAudio
#define UpdateModule FireTestUpdateModule
#define ModuleData FireTestModuleData
#define Thing FireTestThing
#define ModelConditionFlags FireTestModelConditionFlags
#define DisabledMaskType FireTestDisabledMaskType
#define DISABLEDMASK_ALL FireTestDISABLEDMASK_ALL

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <vector>
#include "Common/XferSave.h"
#include "Common/XferLoad.h"
#include "GameLogic/FiringTracker.h"
#include "../upstream/GeneralsMD/Code/GameEngine/Source/GameLogic/Object/FiringTracker.cpp"

/* ---- the stand-ins' members -------------------------------------------------------------- */
namespace {
const int OBJECT_COUNT = 4;
const int MAX_WEAPONS = 4;
FireTestObject g_objects[OBJECT_COUNT];
FireTestThingTemplate g_template;
FireTestAudioEventRTS g_voice;
FireTestThing g_thing;
FireTestWeapon g_weapons[MAX_WEAPONS];
int g_weaponCount = 0;
UnsignedInt g_frame = 0;
bool g_playing = false;
AudioHandle g_nextHandle = 1;

/* a call the tracker made, in call order. kind: 1 setWeaponBonusCondition(a), 2 clearWeaponBonusCondition(a),
** 3 clearAndSetModelConditionFlags (a = clear bits, b = set bits; bit 0 slow, 1 mean, 2 fast),
** 4 reloadAllAmmo(a = now), 5 addAudioEvent (a = object id, name), 6 removeAudioEvent, 7 setWakeFrame(a) */
struct Call { int kind; unsigned a; unsigned b; std::vector<unsigned char> name; };
std::vector<Call> g_calls;

void record(int kind, unsigned a = 0, unsigned b = 0)
{
	Call c = { kind, a, b, std::vector<unsigned char>() };
	g_calls.push_back(c);
}

unsigned bits3(const FireTestModelConditionFlags &f)
{
	unsigned b = 0;
	for (int k = 0; k < 3; ++k) if (f.test(84 + k)) b |= 1u << k;
	return b;
}

/* the delay Weapon::onWeaponBonusChange would compute from the bonus (any function of the word will do) */
unsigned bonusDelay(unsigned word) { return 1u + ((word ^ (word >> 2)) % 13u); }

/* WeaponSet::weaponSetOnWeaponBonusChange, reduced to what the tracker can observe */
void weaponSetOnWeaponBonusChange(unsigned word)
{
	for (int i = 0; i < g_weaponCount; ++i)
		if (g_weapons[i].m_reloading) g_weapons[i].m_possibleNext = g_frame + bonusDelay(word);
}
}

/* the module base: the owner is the thing's object; the wake frame is recorded */
FireTestUpdateModule::FireTestUpdateModule(FireTestThing *thing, const FireTestModuleData *) : m_object(thing->m_object) {}
void FireTestUpdateModule::setWakeFrame(FireTestObject *, UpdateSleepTime wakeDelay) { record(7, (unsigned)wakeDelay); }
void FireTestUpdateModule::crc(Xfer *) {}
void FireTestUpdateModule::xfer(Xfer *) {}
void FireTestUpdateModule::loadPostProcess(void) {}

/* the object */
void FireTestObject::setWeaponBonusCondition(WeaponBonusConditionType c)
{
	unsigned old = m_weaponBonus;
	m_weaponBonus |= (1u << c);
	record(1, (unsigned)c);
	if (old != m_weaponBonus) weaponSetOnWeaponBonusChange(m_weaponBonus);
}
void FireTestObject::clearWeaponBonusCondition(WeaponBonusConditionType c)
{
	unsigned old = m_weaponBonus;
	m_weaponBonus &= ~(1u << c);
	record(2, (unsigned)c);
	if (old != m_weaponBonus) weaponSetOnWeaponBonusChange(m_weaponBonus);
}
void FireTestObject::clearAndSetModelConditionFlags(const FireTestModelConditionFlags &clr, const FireTestModelConditionFlags &set)
{
	record(3, bits3(clr), bits3(set));
}
void FireTestObject::reloadAllAmmo(Bool now)
{
	record(4, now ? 1u : 0u);
}

/* the template: the rapid-fire voice of the owner */
const FireTestAudioEventRTS *FireTestThingTemplate::getPerUnitSound(const AsciiString &) const { return &g_voice; }

/* the weapon: the getters, from the fields the program sets */
UnsignedInt FireTestWeapon::getAutoReloadWhenIdleFrames() const { return m_autoReload; }
UnsignedInt FireTestWeapon::getContinuousFireCoastFrames() const { return m_coast; }
UnsignedInt FireTestWeapon::getPossibleNextShotFrame() const { return m_possibleNext; }
Int FireTestWeapon::getContinuousFireOneShotsNeeded() const { return m_one; }
Int FireTestWeapon::getContinuousFireTwoShotsNeeded() const { return m_two; }
UnsignedInt FireTestWeapon::getFireSoundLoopTime() const { return m_loopTime; }
FireTestAudioEventRTS FireTestWeapon::getFireSound() const { return FireTestAudioEventRTS(m_fireSound); }

/* the audio manager: addAudioEvent and removeAudioEvent are recorded; isCurrentlyPlaying is the
** client's answer for the operation (the program sets it) */
AudioHandle FireTestAudioManager::addAudioEvent(const FireTestAudioEventRTS *event)
{
	Call c = { 5, (unsigned)event->getObjectID(), 0, std::vector<unsigned char>() };
	const char *s = event->m_eventName.str();
	int n = event->m_eventName.getLength();
	for (int k = 0; k < n; ++k) c.name.push_back((unsigned char)s[k]);
	g_calls.push_back(c);
	return g_nextHandle++;
}
void FireTestAudioManager::removeAudioEvent(AudioHandle)
{
	record(6);
}
Bool FireTestAudioManager::isCurrentlyPlaying(AudioHandle) const { return g_playing; }

/* the logic: the frame, and the objects by id (1..OBJECT_COUNT; 0 and anything above are not objects) */
UnsignedInt FireTestGameLogic::getFrame() const { return g_frame; }
FireTestObject *FireTestGameLogic::findObjectByID(ObjectID id) const
{
	if (id < 1 || id > (ObjectID)OBJECT_COUNT) return NULL;
	return const_cast<FireTestObject *>(&g_objects[id - 1]);
}

namespace {
FireTestGameLogic g_logic;
FireTestAudioManager g_audio;
}
FireTestGameLogic *FireTestTheGameLogic = &g_logic;
FireTestAudioManager *FireTestTheAudio = &g_audio;
FireTestDisabledMaskType FireTestDISABLEDMASK_ALL;

/* ---- the log --------------------------------------------------------------------------- */
namespace {
struct Log
{
	unsigned *w; int cap; int n;
	void put(unsigned v) { if (n < cap) w[n] = v; ++n; }
};

std::vector<unsigned char> readFile(const char *path)
{
	std::vector<unsigned char> b;
	FILE *f = fopen(path, "rb");
	if (!f) return b;
	int c;
	while ((c = fgetc(f)) != EOF) b.push_back((unsigned char)c);
	fclose(f);
	return b;
}

/* the owner's bonus word, then the calls of the operation (count, then each), then clear them */
void putCalls(Log &lg, unsigned bonus)
{
	lg.put(bonus);
	lg.put((unsigned)g_calls.size());
	for (size_t k = 0; k < g_calls.size(); ++k) {
		const Call &c = g_calls[k];
		lg.put((unsigned)c.kind);
		if (c.kind == 3) { lg.put(c.a); lg.put(c.b); }
		else if (c.kind == 5) {
			lg.put(c.a);
			lg.put((unsigned)c.name.size());
			for (size_t j = 0; j < c.name.size(); ++j) lg.put(c.name[j]);
		}
		else if (c.kind != 6) lg.put(c.a);
	}
	g_calls.clear();
}

void putSave(Log &lg, FiringTracker *t, const char *tmpPath)
{
	XferSave xs;
	xs.open(AsciiString(tmpPath));
	xs.xferSnapshot(t);
	xs.close();
	std::vector<unsigned char> bytes = readFile(tmpPath);
	lg.put((unsigned)bytes.size());
	for (size_t k = 0; k < bytes.size(); ++k) lg.put(bytes[k]);
}
}

extern "C" {

/* spec: the weapon count W (1..4), then W weapons of six words (auto-reload frames, coast frames,
**   one-shots-needed, two-shots-needed, loop time, fire sound index 0..3), then the initial bonus word.
**   The fire sounds are "FIRE_ONE", "FIRE_LOOP", "FIRE_THREE", "FIRE_FOUR"; the rapid-fire voice is
**   "VOICE_RAPID"; the owner is object 1, victims are objects 1..4 (0 = a ground shot).
** ops: quadruples (code, a, b, c):
**   0 shotFired(weapon a, victim b) after advancing the frame by c          (no result)
**   1 update() after advancing the frame by a                                the sleep it returns
**   2 set the owner's bonus word to a, directly                              (no result)
**   3 delete the tracker and make a new one                                  (no result; the calls are logged)
**   4 getNumConsecutiveShotsAtVictim(object b; 0 = NULL)                     the count
**   5 getLastShotVictim()                                                    the victim id
**   6 the client's looping-sound answer: playing = a != 0                    (no result)
**   7 xfer save of the module's own part                                     the byte count, the bytes
**   8 xfer load of the blob into the tracker                                 the status (0 ok, else 1 + XferStatus),
**                                                                              then the bytes of a save
**   9 weapon a: next-shot frame = b, reloading = c != 0                      (no result)
**  10 victim b's faerie-fire status = c != 0                                 (no result)
** Every operation then logs the bonus word and the calls the tracker made (see putCalls).
** Returns the number of words, or -1 if they do not fit in cap, -2 for a bad program. */
int ref_firing_run(const int *spec, int nSpec, const int *ops, int nOps, const unsigned char *blob, int blobLen,
	const char *tmpPath, unsigned *out, int cap)
{
	const char *soundNames[4] = { "FIRE_ONE", "FIRE_LOOP", "FIRE_THREE", "FIRE_FOUR" };
	int nWeapons = spec[0];
	if (nWeapons < 1 || nWeapons > MAX_WEAPONS || nSpec != 1 + 6 * nWeapons + 1) return -2;
	const int *p = spec + 1;
	g_weaponCount = nWeapons;
	for (int w = 0; w < nWeapons; ++w, p += 6) {
		FireTestWeapon &wp = g_weapons[w];
		wp = FireTestWeapon();
		wp.m_autoReload = (UnsignedInt)p[0];
		wp.m_coast = (UnsignedInt)p[1];
		wp.m_one = p[2];
		wp.m_two = p[3];
		wp.m_loopTime = (UnsignedInt)p[4];
		wp.m_fireSound.set(soundNames[p[5] & 3]);
	}
	for (int i = 0; i < OBJECT_COUNT; ++i) {
		g_objects[i].m_id = (ObjectID)(i + 1);
		g_objects[i].m_template = &g_template;
		g_objects[i].m_weaponBonus = 0;
		g_objects[i].m_faerie = false;
	}
	g_objects[0].m_weaponBonus = (UnsignedInt)*p;
	g_voice.setObjectID(INVALID_ID);
	g_voice.m_eventName.set("VOICE_RAPID");
	g_thing.m_object = &g_objects[0];   /* the owner is object 1 */
	g_frame = 0;
	g_playing = false;
	g_nextHandle = 1;
	g_calls.clear();

	FiringTracker *t = new FiringTracker(&g_thing, NULL);
	Log lg = { out, cap, 0 };
	FireTestObject &owner = g_objects[0];
	putCalls(lg, owner.m_weaponBonus);   /* the constructor's setWakeFrame */
	for (int i = 0; i < nOps; ++i) {
		const int code = ops[4 * i], a = ops[4 * i + 1], b = ops[4 * i + 2], c = ops[4 * i + 3];
		switch (code) {
		case 0:
			if (a < 0 || a >= nWeapons || b < 0 || b > OBJECT_COUNT) { delete t; return -2; }
			g_frame += (UnsignedInt)c;
			t->shotFired(&g_weapons[a], (ObjectID)b);
			break;
		case 1:
			g_frame += (UnsignedInt)a;
			lg.put((unsigned)(int)t->update());
			break;
		case 2:
			owner.m_weaponBonus = (UnsignedInt)a;
			break;
		case 3:
			delete t;
			t = new FiringTracker(&g_thing, NULL);
			break;
		case 4:
			lg.put((unsigned)t->getNumConsecutiveShotsAtVictim(b >= 1 && b <= OBJECT_COUNT ? &g_objects[b - 1] : NULL));
			break;
		case 5:
			lg.put((unsigned)t->getLastShotVictim());
			break;
		case 6:
			g_playing = a != 0;
			break;
		case 7:
			putSave(lg, t, tmpPath);
			break;
		case 8: {
			FILE *f = fopen(tmpPath, "wb");
			if (f) { fwrite(blob, 1, (size_t)blobLen, f); fclose(f); }
			unsigned status = 0;
			XferLoad xl;
			try {
				xl.open(AsciiString(tmpPath));
				xl.xferSnapshot(t);
				xl.close();
			} catch (XferStatus s) {
				status = 1u + (unsigned)s;
				try { xl.close(); } catch (...) {}
			}
			lg.put(status);
			putSave(lg, t, tmpPath);
			break; }
		case 9:
			if (a < 0 || a >= nWeapons) { delete t; return -2; }
			g_weapons[a].m_possibleNext = (UnsignedInt)b;
			g_weapons[a].m_reloading = c != 0;
			break;
		case 10:
			if (b < 1 || b > OBJECT_COUNT) { delete t; return -2; }
			g_objects[b - 1].m_faerie = c != 0;
			break;
		default:
			delete t;
			return -2;
		}
		putCalls(lg, owner.m_weaponBonus);
	}
	delete t;   /* the destructor's removeAudioEvent */
	putCalls(lg, owner.m_weaponBonus);
	return lg.n <= cap ? lg.n : -1;
}

} /* extern "C" */
