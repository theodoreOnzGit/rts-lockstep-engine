/* Shim for GameEngine/Include/Common/Upgrade.h, differential tests only:
** stand-ins for what Xfer::xferUpgradeMask calls (NOT translated, NOT tested;
** the stand-ins abort if reached). UpgradeMaskType is BitFlags<128> upstream. */
#ifndef SHIM_UPGRADE_H
#define SHIM_UPGRADE_H
#include "Common/AsciiString.h"
class UpgradeMaskType
{
public:
	bool testForAll(const UpgradeMaskType &) const;
	void clear();
	void set(const UpgradeMaskType &);
	unsigned int m_bits[4];
};
class UpgradeTemplate
{
public:
	const UpgradeMaskType &getUpgradeMask() const;
	AsciiString getUpgradeName() const;
	UpgradeTemplate *friend_getNext();
};
class UpgradeCenter
{
public:
	UpgradeTemplate *firstUpgradeTemplate();
	const UpgradeTemplate *findUpgrade(const AsciiString &);
};
extern UpgradeCenter *TheUpgradeCenter;
#endif
