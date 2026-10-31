/* Differential-test wrapper for Object::updateObjValuesFromMapProperties (upstream commit 0a05454d):
**   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp, the function cut verbatim by build.rs into
**   OUT_DIR/objmp_gen/object_map_props_cut.inc.
** Compiled inside namespace omp on stand-ins (no upstream header): a Dict keyed by name, the well-known keys as their
** names, an AsciiString over std::string, the object's body, experience tracker, AI and locomotor, the upgrade center,
** the drawable and the audio system. Every call that acts logs a line; queries answer from the program's configuration.
** AudioEventInfo::isPermanentSound and DynamicAudioEventInfo's override functions are copied from upstream
** (AudioEventInfo.h, DynamicAudioEventInfo.cpp) onto the fields they touch, as is Drawable::mangleCustomAudioName.
**
** Entry point (extern "C", tests/differential_object_map_props.rs):
**   ref_objmp_run(words, n, out, cap)  runs one program (layout in the test) and writes the log */
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <string>
#include <vector>
#include <map>

namespace omp {

typedef bool Bool;
typedef int Int;
typedef float Real;
typedef unsigned int UnsignedInt;
#define TRUE true
#define FALSE false
#define INT_TO_REAL(x) ((Real)(x))
#define DEBUG_ASSERTCRASH(c, m) ((void)0)
#define BitTest(x, i) (((x) & (i)) != 0)
#define BitSet(x, i) ((x) |= (i))
#define BitClear(x, i) ((x) &= ~(i))
#define newInstance(T) new T

std::string g_log;
void logf(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
void logf(const char *fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    g_log += buf;
    g_log += '\n';
}
unsigned fb(Real r) {
    unsigned u;
    memcpy(&u, &r, 4);
    return u;
}

class AsciiString {
public:
    std::string s;
    AsciiString() {}
    AsciiString(const char *p) : s(p) {}
    const char *str() const { return s.c_str(); }
    Bool isEmpty() const { return s.empty(); }
    void clear() { s.clear(); }
    void concat(const AsciiString &o) { s += o.s; }
    void format(const char *fmt, ...) {
        char buf[512];
        va_list ap;
        va_start(ap, fmt);
        vsnprintf(buf, sizeof buf, fmt, ap);
        va_end(ap);
        s = buf;
    }
};

typedef std::string NameKeyType;
#define NAMEKEY(x) (NameKeyType((x).str()))
struct NameKeyGenerator {
    AsciiString keyToName(const NameKeyType &k) { return AsciiString(k.c_str()); }
};
NameKeyGenerator g_names;
NameKeyGenerator *TheNameKeyGenerator = &g_names;

#define KEY(n) static const NameKeyType TheKey_##n = #n;
KEY(objectName) KEY(objectMaxHPs) KEY(objectInitialHealth) KEY(objectVeterancy) KEY(objectAggressiveness)
KEY(objectRecruitableAI) KEY(objectSelectable) KEY(objectStoppingDistance) KEY(objectEnabled) KEY(objectPowered)
KEY(objectIndestructible) KEY(objectUnsellable) KEY(objectTargetable) KEY(objectVisualRange)
KEY(objectShroudClearingDistance) KEY(objectGrantUpgrade) KEY(objectTime) KEY(objectWeather) KEY(objectSoundAmbient)
KEY(objectSoundAmbientCustomized) KEY(objectSoundAmbientEnabled) KEY(objectSoundAmbientLooping)
KEY(objectSoundAmbientLoopCount) KEY(objectSoundAmbientMinVolume) KEY(objectSoundAmbientVolume)
KEY(objectSoundAmbientMinRange) KEY(objectSoundAmbientMaxRange) KEY(objectSoundAmbientPriority)

/* a value of the one type the key has; a missing key answers 0 / false / empty with exists FALSE */
struct DictValue {
    int type; /* 0 bool, 1 int, 2 real, 3 string */
    Int i;
    Real r;
    AsciiString s;
};
class Dict {
public:
    std::map<std::string, DictValue> m;
    template <class T> T get(const NameKeyType &k, Bool *exists, int type, T DictValue::*f, T dflt) const {
        std::map<std::string, DictValue>::const_iterator it = m.find(k);
        if (it == m.end() || it->second.type != type) {
            if (exists) *exists = FALSE;
            return dflt;
        }
        if (exists) *exists = TRUE;
        return it->second.*f;
    }
    Bool getBool(const NameKeyType &k, Bool *exists) const { return get<Int>(k, exists, 0, &DictValue::i, 0) != 0; }
    Int getInt(const NameKeyType &k, Bool *exists) const { return get<Int>(k, exists, 1, &DictValue::i, 0); }
    Real getReal(const NameKeyType &k, Bool *exists) const { return get<Real>(k, exists, 2, &DictValue::r, 0.0f); }
    AsciiString getAsciiString(const NameKeyType &k, Bool *exists) const { return get<AsciiString>(k, exists, 3, &DictValue::s, AsciiString()); }
};

typedef Int AttitudeType;
typedef Int AudioPriority;
enum VeterancyLevel { LEVEL_REGULAR = 0, LEVEL_VETERAN, LEVEL_ELITE, LEVEL_HEROIC };
enum ObjectScriptStatusBit {
    OBJECT_STATUS_SCRIPT_DISABLED = 0x01,
    OBJECT_STATUS_SCRIPT_UNPOWERED = 0x02,
    OBJECT_STATUS_SCRIPT_UNSELLABLE = 0x04,
    OBJECT_STATUS_SCRIPT_UNSTEALTHED = 0x08,
    OBJECT_STATUS_SCRIPT_TARGETABLE = 0x10,
};
enum ModelConditionFlagType { MODELCONDITION_NIGHT = 0, MODELCONDITION_SNOW = 1 };
static const char *mc_name(ModelConditionFlagType t) { return t == MODELCONDITION_NIGHT ? "NIGHT" : "SNOW"; }
enum AudioControl { AC_LOOP = 0x0001 };

struct BodyModuleInterface {
    void setMaxHealth(Real m) { logf("body maxhealth %08x", fb(m)); }
    void setInitialHealth(Int p) { logf("body initialhealth %d", p); }
    void setIndestructible(Bool on) { logf("body indestructible %d", (int)on); }
};
struct ExperienceTracker {
    Bool trainable;
    Bool isTrainable() const { return trainable; }
    void setVeterancyLevel(VeterancyLevel l, Bool provideFeedback = TRUE) { logf("veterancy %d %d", (int)l, (int)provideFeedback); }
};
struct Locomotor {
    void setCloseEnoughDist(Real d) { logf("loco closeenough %08x", fb(d)); }
};
struct AIUpdateInterface {
    Locomotor *loco;
    void setAttitude(AttitudeType t) { logf("ai attitude %d", t); }
    void setIsRecruitable(Bool on) { logf("ai recruitable %d", (int)on); }
    Locomotor *getCurLocomotor() { return loco; }
};

struct UpgradeTemplate {
    AsciiString name;
};
struct UpgradeCenter {
    std::vector<UpgradeTemplate> known;
    const UpgradeTemplate *findUpgrade(const AsciiString &n) {
        for (size_t i = 0; i < known.size(); ++i)
            if (known[i].name.s == n.s) return &known[i];
        return NULL;
    }
};
UpgradeCenter g_upgrades;
UpgradeCenter *TheUpgradeCenter = &g_upgrades;

/* AudioEventInfo: the fields the customisation reads or overrides */
struct AudioEventInfo {
    AsciiString m_audioName;
    Int m_control;
    Int m_loopCount;
    Real m_volume;
    Real m_minVolume;
    Real m_minDistance;
    Real m_maxDistance;
    AudioPriority m_priority;
    Bool isPermanentSound() const { return BitTest(m_control, AC_LOOP) && (m_loopCount == 0); } /* AudioEventInfo.h */
};
struct DynamicAudioEventInfo : public AudioEventInfo {
    enum { OVERRIDE_NAME = 0, OVERRIDE_LOOP_FLAG, OVERRIDE_LOOP_COUNT, OVERRIDE_VOLUME, OVERRIDE_MIN_VOLUME, OVERRIDE_MIN_RANGE, OVERRIDE_MAX_RANGE, OVERRIDE_PRIORITY };
    unsigned m_overriddenFields;
    AsciiString m_originalName;
    DynamicAudioEventInfo(const AudioEventInfo &b) : AudioEventInfo(b), m_overriddenFields(0) {}
    /* DynamicAudioEventInfo.cpp, the bit sets written on the stand-in's mask */
    void overrideAudioName(const AsciiString &newName) {
        m_originalName = m_audioName;
        m_overriddenFields |= 1u << OVERRIDE_NAME;
        m_audioName = newName;
    }
    void overrideLoopFlag(Bool newLoopFlag) {
        m_overriddenFields |= 1u << OVERRIDE_LOOP_FLAG;
        if (newLoopFlag)
            BitSet(m_control, AC_LOOP);
        else
            BitClear(m_control, AC_LOOP);
    }
    void overrideLoopCount(Int n) { m_overriddenFields |= 1u << OVERRIDE_LOOP_COUNT; m_loopCount = n; }
    void overrideVolume(Real v) { m_overriddenFields |= 1u << OVERRIDE_VOLUME; m_volume = v; }
    void overrideMinVolume(Real v) { m_overriddenFields |= 1u << OVERRIDE_MIN_VOLUME; m_minVolume = v; }
    void overrideMinRange(Real v) { m_overriddenFields |= 1u << OVERRIDE_MIN_RANGE; m_minDistance = v; }
    void overrideMaxRange(Real v) { m_overriddenFields |= 1u << OVERRIDE_MAX_RANGE; m_maxDistance = v; }
    void overridePriority(AudioPriority p) { m_overriddenFields |= 1u << OVERRIDE_PRIORITY; m_priority = p; }
    void deleteInstance() { delete this; }
};
struct AudioManager {
    std::vector<AudioEventInfo> table;
    std::vector<DynamicAudioEventInfo *> added;
    const AudioEventInfo *findAudioEventInfo(const AsciiString &n) {
        for (size_t i = 0; i < table.size(); ++i)
            if (table[i].m_audioName.s == n.s) return &table[i];
        return NULL;
    }
    void addAudioEventInfo(DynamicAudioEventInfo *i) { added.push_back(i); }
};
AudioManager g_audio;
AudioManager *TheAudio = &g_audio;

struct Drawable {
    Int id;
    Bool hasBase;
    AudioEventInfo base;
    Bool enabledFromScript;
    Int getID() const { return id; }
    void clearModelConditionState(ModelConditionFlagType t) { logf("clearmc %s", mc_name(t)); }
    void setModelConditionState(ModelConditionFlagType t) { logf("setmc %s", mc_name(t)); }
    void setCustomSoundAmbientOff() { logf("soundoff"); }
    const AudioEventInfo *getBaseSoundAmbientInfo() const { return hasBase ? &base : NULL; }
    void enableAmbientSoundFromScript(Bool on) { logf("enable %d", (int)on); }
    Bool getAmbientSoundEnabledFromScript() const { return enabledFromScript; }
    /* Drawable.cpp */
    void mangleCustomAudioName(DynamicAudioEventInfo *audioToMangle) const {
        AsciiString customizedName;
        customizedName.format(" CUSTOM %d ", (Int)getID()); // Note space at beginning prevents collision with any names from INI file
        customizedName.concat(audioToMangle->m_audioName);
        audioToMangle->overrideAudioName(customizedName);
    }
    void setCustomSoundAmbientInfo(DynamicAudioEventInfo *a) {
        logf("install [%s] %d %d %08x %08x %08x %08x %d %u", a->m_audioName.str(), a->m_control, a->m_loopCount, fb(a->m_volume), fb(a->m_minVolume),
             fb(a->m_minDistance), fb(a->m_maxDistance), a->m_priority, a->m_overriddenFields);
    }
};

class Object {
public:
    Int m_id;
    BodyModuleInterface m_bodyObj;
    Bool m_hasBody;
    ExperienceTracker m_exp;
    ExperienceTracker *m_experienceTracker;
    AIUpdateInterface m_aiObj;
    Locomotor m_locoObj;
    Bool m_hasAI;
    Bool m_selectable;
    Real m_visionRange;
    Real m_shroudClearingRange;
    Drawable m_drawableObj;
    Bool m_hasDrawable;

    void setName(const AsciiString &n) { logf("name [%s]", n.str()); }
    BodyModuleInterface *getBodyModule() { return m_hasBody ? &m_bodyObj : NULL; }
    AIUpdateInterface *getAIUpdateInterface() { return m_hasAI ? &m_aiObj : NULL; }
    Bool isSelectable() const { return m_selectable; }
    void setSelectable(Bool on) {
        logf("selectable %d", (int)on);
        m_selectable = on;
    }
    void setScriptStatus(ObjectScriptStatusBit bit, Bool on) { logf("scriptstatus %d %d", (int)bit, (int)on); }
    void giveUpgrade(const UpgradeTemplate *u) { logf("upgrade [%s]", u->name.str()); }
    Drawable *getDrawable() { return m_hasDrawable ? &m_drawableObj : NULL; }
    void updateObjValuesFromMapProperties(Dict *properties);
};

#include "object_map_props_cut.inc"

static const char *KEYS[31] = {
    "objectName", "objectMaxHPs", "objectInitialHealth", "objectVeterancy", "objectAggressiveness", "objectRecruitableAI",
    "objectSelectable", "objectStoppingDistance", "objectEnabled", "objectPowered", "objectIndestructible", "objectUnsellable",
    "objectTargetable", "objectVisualRange", "objectShroudClearingDistance", "objectGrantUpgrade0", "objectGrantUpgrade1",
    "objectGrantUpgrade2", "objectGrantUpgrade3", "objectTime", "objectWeather", "objectSoundAmbient",
    "objectSoundAmbientCustomized", "objectSoundAmbientEnabled", "objectSoundAmbientLooping", "objectSoundAmbientLoopCount",
    "objectSoundAmbientMinVolume", "objectSoundAmbientVolume", "objectSoundAmbientMinRange", "objectSoundAmbientMaxRange",
    "objectSoundAmbientPriority",
};
static const char *STRINGS[9] = {"", "UpA", "UpB", "UpC", "UpX", "SndA", "SndB", "SndX", "Bob"};
static Real rf(unsigned u) {
    Real r;
    memcpy(&r, &u, 4);
    return r;
}
/* the audio table entry for audio name i (0 SndA, 1 SndB): the same derivation as the Rust side */
static AudioEventInfo audio_entry(int i, const char *name) {
    AudioEventInfo a;
    a.m_audioName = AsciiString(name);
    a.m_control = i & 1;
    a.m_loopCount = i >= 1 ? 0 : 3;
    a.m_volume = 0.25f * (Real)(i + 1);
    a.m_minVolume = 0.1f * (Real)(i + 1);
    a.m_minDistance = 10.0f * (Real)(i + 1);
    a.m_maxDistance = 100.0f * (Real)(i + 1);
    a.m_priority = i + 1;
    return a;
}

int run(const unsigned *w, int n, char *out, int cap) {
    g_log.clear();
    (void)n;
    unsigned flags = w[0];
    Object *o = new Object();
    o->m_id = 7;
    o->m_hasBody = flags & 1;
    o->m_exp.trainable = (flags >> 1) & 1;
    o->m_experienceTracker = &o->m_exp;
    o->m_hasAI = (flags >> 2) & 1;
    o->m_aiObj.loco = ((flags >> 3) & 1) ? &o->m_locoObj : NULL;
    o->m_selectable = (flags >> 4) & 1;
    o->m_hasDrawable = (flags >> 5) & 1;
    o->m_drawableObj.id = 7;
    o->m_drawableObj.hasBase = (flags >> 6) & 1;
    o->m_drawableObj.enabledFromScript = (flags >> 7) & 1;
    o->m_visionRange = -1.0f;
    o->m_shroudClearingRange = -1.0f;
    AudioEventInfo &b = o->m_drawableObj.base;
    b.m_audioName = AsciiString("Base");
    b.m_control = w[1];
    b.m_loopCount = (Int)w[2];
    b.m_volume = rf(w[3]);
    b.m_minVolume = rf(w[4]);
    b.m_minDistance = rf(w[5]);
    b.m_maxDistance = rf(w[6]);
    b.m_priority = (Int)w[7];
    g_upgrades.known.clear();
    for (int i = 0; i < 3; ++i)
        if ((w[8] >> i) & 1) {
            UpgradeTemplate u;
            u.name = AsciiString(STRINGS[1 + i]);
            g_upgrades.known.push_back(u);
        }
    g_audio.table.clear();
    for (int i = 0; i < 2; ++i)
        if ((w[9] >> i) & 1) g_audio.table.push_back(audio_entry(i, STRINGS[5 + i]));
    Dict d;
    int entries = (int)w[10];
    for (int e = 0; e < entries; ++e) {
        const unsigned *x = w + 11 + 3 * e;
        DictValue v;
        v.type = (int)x[1];
        v.i = (Int)x[2];
        v.r = rf(x[2]);
        v.s = AsciiString(STRINGS[x[2] % 9]);
        d.m[KEYS[x[0] % 31]] = v;
    }
    o->updateObjValuesFromMapProperties(&d);
    logf("ranges %08x %08x", fb(o->m_visionRange), fb(o->m_shroudClearingRange));
    for (size_t i = 0; i < g_audio.added.size(); ++i) delete g_audio.added[i];
    g_audio.added.clear();
    delete o;
    int len = (int)g_log.size();
    if (len >= cap) len = cap - 1;
    memcpy(out, g_log.data(), len);
    out[len] = 0;
    return len;
}

} // namespace omp

extern "C" int ref_objmp_run(const unsigned *words, int n, char *out, int cap) { return omp::run(words, n, out, cap); }
