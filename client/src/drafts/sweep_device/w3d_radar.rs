//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/System/W3DRadar.cpp
//!
//! Sweep 2 (#213): `W3DRadar::initializeTextureFormats` and `deleteResources`, with `findFormat`. The device's texture format
//! support is a closure. (The client draws the radar through Bevy images; these keep the format choice documented.)

use rts_lockstep_engine::formats::texture::format::Ww3dFormat;

/// `findFormat` (W3DRadar.cpp:84): the first format of the preference list the device supports; `Unknown` (and a
/// `DEBUG_CRASH`) when none.
pub fn find_format(formats: &[Ww3dFormat], supports: &impl Fn(Ww3dFormat) -> bool) -> Ww3dFormat {
    for f in formats {
        if supports(*f) {
            return *f;
        }
    }
    Ww3dFormat::Unknown
}

/// The three formats `initializeTextureFormats` chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadarTextureFormats {
    pub terrain: Ww3dFormat,
    pub overlay: Ww3dFormat,
    pub shroud: Ww3dFormat,
}

/// `W3DRadar::initializeTextureFormats` (W3DRadar.cpp:103): most preferred first. Terrain: R8G8B8, X8R8G8B8, R5G6B5,
/// X1R5G5B5; overlay and shroud: A8R8G8B8, A4R4G4B4.
pub fn initialize_texture_formats(supports: &impl Fn(Ww3dFormat) -> bool) -> RadarTextureFormats {
    let terrain_formats = [Ww3dFormat::R8G8B8, Ww3dFormat::X8R8G8B8, Ww3dFormat::R5G6B5, Ww3dFormat::X1R5G5B5];
    let overlay_formats = [Ww3dFormat::A8R8G8B8, Ww3dFormat::A4R4G4B4];
    let shroud_formats = [Ww3dFormat::A8R8G8B8, Ww3dFormat::A4R4G4B4];
    RadarTextureFormats {
        terrain: find_format(&terrain_formats, supports),
        overlay: find_format(&overlay_formats, supports),
        shroud: find_format(&shroud_formats, supports),
    }
}

/// The resources `deleteResources` drops: textures released (`Release_Ref`) and images deleted (`deleteInstance`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadarResources {
    pub terrain_texture: bool,
    pub terrain_image: bool,
    pub overlay_texture: bool,
    pub overlay_image: bool,
    pub shroud_texture: bool,
    pub shroud_image: bool,
}

/// `W3DRadar::deleteResources` (W3DRadar.cpp:140): each texture released then each image deleted and the members NULLed
/// (terrain, overlay, then shroud — the shroud pair follows the overlay's pattern in the part of the function after the
/// overlay image; checked against W3DRadar.cpp:140-185).
pub fn delete_resources(r: &mut RadarResources) {
    *r = RadarResources::default();
}
