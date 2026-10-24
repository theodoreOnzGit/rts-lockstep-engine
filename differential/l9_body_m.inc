/* The stand-in body of the batch M and N drivers (p1w12-sonnet): see the class. */
namespace {

/* A body that keeps a health and a maximum and takes healing and damage as the scripted probe body of the Rust side does (module_probe.rs, AttemptHealing
** and AttemptDamage, minus the death), and logs them (and setAflame) as the probe's trace does. The scripted probe dies at zero health; a test that
** damages must keep the health above what it deals. */
class MBody : public BodyModuleInterface
{
public:
	Real m_health = 0.0f, m_max = 0.0f;
	Object *m_owner = 0;
	void attemptDamage( DamageInfo *info )
	{
		ulog("damage %d %08x %d %d %d", (int)m_owner->m_oid, realBits(info->in.m_amount), (int)info->in.m_damageType, (int)info->in.m_deathType, (int)info->in.m_sourceID);
		Real dealt;
		if (info->in.m_kill) dealt = m_health;
		else {
			Real amount = info->in.m_amount * 1.0f;
			dealt = amount < m_health ? amount : m_health;
		}
		m_health -= dealt;
		info->out.m_actualDamageDealt = dealt;
		info->out.m_actualDamageClipped = dealt;
		info->out.m_noEffect = false;
	}
	void attemptHealing( DamageInfo *info )
	{
		ulog("heal %d %08x %d", (int)m_owner->m_oid, realBits(info->in.m_amount), (int)info->in.m_sourceID);
		Real room = m_max - m_health;
		Real healed = (info->in.m_damageType == DAMAGE_HEALING && info->in.m_amount < room) ? info->in.m_amount : room;
		m_health += healed;
		info->out.m_actualDamageDealt = healed;
	}
	Real estimateDamage( DamageInfoInput & ) const { return 0.0f; }
	Real getHealth() const { return m_health; }
	Real getMaxHealth() const { return m_max; }
	Real getInitialHealth() const { return 0.0f; }
	Real getPreviousHealth() const { return 0.0f; }
	UnsignedInt getSubdualDamageHealRate() const { return 0; }
	Real getSubdualDamageHealAmount() const { return 0.0f; }
	Bool hasAnySubdualDamage() const { return false; }
	Real getCurrentSubdualDamageAmount() const { return 0.0f; }
	BodyDamageType getDamageState() const { return BODY_PRISTINE; }
	void setDamageState( BodyDamageType ) {}
	void setAflame( Bool setting ) { ulog("aflame %d", (int)setting); }
	void onVeterancyLevelChanged( VeterancyLevel, VeterancyLevel, Bool ) {}
	void setArmorSetFlag( ArmorSetType ) {}
	void clearArmorSetFlag( ArmorSetType ) {}
	Bool testArmorSetFlag( ArmorSetType ) { return false; }
	const DamageInfo *getLastDamageInfo() const { return m_lastDamage; }   /* batch R30: none, or the test's */
	const DamageInfo *m_lastDamage = 0;
	UnsignedInt getLastDamageTimestamp() const { return 0; }
	UnsignedInt getLastHealingTimestamp() const { return 0; }
	ObjectID getClearableLastAttacker() const { return INVALID_ID; }
	void clearLastAttacker() {}
	Bool getFrontCrushed() const { return false; }
	Bool getBackCrushed() const { return false; }
	void setInitialHealth( Int ) {}
	void setMaxHealth( Real, MaxHealthChangeType ) {}
	void setFrontCrushed( Bool ) {}
	void setBackCrushed( Bool ) {}
	void applyDamageScalar( Real ) {}
	Real getDamageScalar() const { return 1.0f; }
	void internalChangeHealth( Real ) {}
	void setIndestructible( Bool ) {}
	Bool isIndestructible( void ) const { return false; }
	void evaluateVisualCondition() {}
	void updateBodyParticleSystems() { ulog("bodyps %d", m_owner ? (int)m_owner->m_oid : 0); }   /* batch R29: StructureCollapseUpdate */
};

}  // namespace
