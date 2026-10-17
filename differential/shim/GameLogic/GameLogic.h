/* Shim for GameEngine/Include/GameLogic/GameLogic.h, differential tests only.
** RandomValue.cpp includes it only for TheGameLogic->getFrame() inside
** DEBUG_RANDOM_* logging blocks, which upstream #undefs. Nothing is needed. */
