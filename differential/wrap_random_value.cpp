/* Differential-test wrapper for GameEngine/Source/Common/RandomValue.cpp
** (upstream commit 0a05454d8574207440a5fb15241b98ad0b435590).
**
** The upstream .cpp is #included unchanged so that its file-static helpers
** (randomValue, seedRandom) and seed arrays can be reached from extern "C"
** functions. Nothing in the upstream file is edited. */
#include "../upstream/GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp"

extern "C" {

void ref_seed_random(unsigned int seed, unsigned int *state6) { seedRandom(seed, state6); }
unsigned int ref_random_value(unsigned int *state6) { return randomValue(state6); }
float ref_mult_factor(void) { return theMultFactor; }

void ref_init_random(unsigned int seed) { InitRandom(seed); }
void ref_init_game_logic_random(unsigned int seed) { InitGameLogicRandom(seed); }
unsigned int ref_get_game_logic_random_seed(void) { return GetGameLogicRandomSeed(); }
unsigned int ref_get_game_logic_random_seed_crc(void) { return GetGameLogicRandomSeedCRC(); }

/* which: 0 = logic, 1 = client, 2 = audio */
void ref_get_state(int which, unsigned int *out6)
{
	const unsigned int *s = which == 0 ? theGameLogicSeed : which == 1 ? theGameClientSeed : theGameAudioSeed;
	for (int i = 0; i < 6; ++i) out6[i] = s[i];
}
void ref_set_state(int which, const unsigned int *in6)
{
	unsigned int *s = which == 0 ? theGameLogicSeed : which == 1 ? theGameClientSeed : theGameAudioSeed;
	for (int i = 0; i < 6; ++i) s[i] = in6[i];
}

static char ref_file[] = "differential";
int ref_logic_value(int lo, int hi) { return GetGameLogicRandomValue(lo, hi, ref_file, 0); }
int ref_client_value(int lo, int hi) { return GetGameClientRandomValue(lo, hi, ref_file, 0); }
int ref_audio_value(int lo, int hi) { return GetGameAudioRandomValue(lo, hi, ref_file, 0); }
float ref_logic_value_real(float lo, float hi) { return GetGameLogicRandomValueReal(lo, hi, ref_file, 0); }
float ref_client_value_real(float lo, float hi) { return GetGameClientRandomValueReal(lo, hi, ref_file, 0); }
float ref_audio_value_real(float lo, float hi) { return GetGameAudioRandomValueReal(lo, hi, ref_file, 0); }

/* The RandomVariable classes have no constructor (they live in unions), and
** setRange() reads m_type before writing it (only inside a no-op assert). */
float ref_logic_variable_get_value(int type, float lo, float hi)
{
	GameLogicRandomVariable v;
	memset(&v, 0, sizeof(v));
	v.setRange(lo, hi, (GameLogicRandomVariable::DistributionType)type);
	return v.getValue();
}
float ref_client_variable_get_value(int type, float lo, float hi)
{
	GameClientRandomVariable v;
	memset(&v, 0, sizeof(v));
	v.setRange(lo, hi, (GameClientRandomVariable::DistributionType)type);
	return v.getValue();
}

} /* extern "C" */
