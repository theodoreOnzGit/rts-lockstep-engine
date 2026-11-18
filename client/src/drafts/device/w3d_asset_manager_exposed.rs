//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManagerExposed.cpp

/// The asset manager calls `ReloadAllTextures` needs: `W3DAssetManager::Get_Instance()->Release_All_Textures()`.
pub trait ReleaseAllTextures {
    /// `WW3DAssetManager::Release_All_Textures`.
    fn release_all_textures(&mut self);
}

/// `ReloadAllTextures` (W3DAssetManagerExposed.cpp:30): drops every texture so the next use loads it again.
pub fn reload_all_textures(assets: &mut dyn ReleaseAllTextures) {
    assets.release_all_textures();
}
