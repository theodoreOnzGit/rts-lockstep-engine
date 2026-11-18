//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturefilter.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texturefilter.h
//!
//! The three global tables (`_MinTextureFilters`, `_MagTextureFilters`, `_MipMapFilters`) are the fields of
//! [`TextureFilterTables`], one of which the owner keeps. D3D constants are given as their DX8 values
//! (`D3DTEXF_NONE` 0, `POINT` 1, `LINEAR` 2, `ANISOTROPIC` 3; `D3DPTFILTERCAPS_*` from d3d8caps.h).
//! `Apply` and the `D3DTSS_MAXANISOTROPY` call in `_Init_Filters` set device texture stage states; they are
//! device-only: `apply_state` returns the values.

/// `MAX_TEXTURE_STAGES` (dx8wrapper.h:76).
pub const MAX_TEXTURE_STAGES: usize = 8;

/// `D3DTEXF_NONE`.
pub const D3DTEXF_NONE: u32 = 0;
/// `D3DTEXF_POINT`.
pub const D3DTEXF_POINT: u32 = 1;
/// `D3DTEXF_LINEAR`.
pub const D3DTEXF_LINEAR: u32 = 2;
/// `D3DTEXF_ANISOTROPIC`.
pub const D3DTEXF_ANISOTROPIC: u32 = 3;

/// `D3DPTFILTERCAPS_MINFLINEAR`.
pub const D3DPTFILTERCAPS_MINFLINEAR: u32 = 0x0000_0100;
/// `D3DPTFILTERCAPS_MINFANISOTROPIC`.
pub const D3DPTFILTERCAPS_MINFANISOTROPIC: u32 = 0x0000_0400;
/// `D3DPTFILTERCAPS_MIPFLINEAR`.
pub const D3DPTFILTERCAPS_MIPFLINEAR: u32 = 0x0001_0000;
/// `D3DPTFILTERCAPS_MAGFLINEAR`.
pub const D3DPTFILTERCAPS_MAGFLINEAR: u32 = 0x0200_0000;
/// `D3DPTFILTERCAPS_MAGFANISOTROPIC`.
pub const D3DPTFILTERCAPS_MAGFANISOTROPIC: u32 = 0x0400_0000;

/// `MipCountType` (texturefilter.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipCountType {
    /// generate all mipmap levels down to 1x1 size
    MipLevelsAll = 0,
    /// no mipmapping at all (just one mip level)
    MipLevels1,
    MipLevels2,
    MipLevels3,
    MipLevels4,
    MipLevels5,
    MipLevels6,
    MipLevels7,
    MipLevels8,
    MipLevels10,
    MipLevels11,
    MipLevels12,
    /// "This isn't to be used (use MIP_LEVELS_ALL instead), it is just an enum for creating static tables etc."
    MipLevelsMax,
}

/// `TextureFilterClass::FilterType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    None = 0,
    Fast = 1,
    Best = 2,
    Default = 3,
}

/// `FILTER_TYPE_COUNT`.
pub const FILTER_TYPE_COUNT: usize = 4;

/// `TextureFilterClass::TextureFilterMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFilterMode {
    Bilinear,
    Trilinear,
    Anisotropic,
}

/// `TextureFilterClass::TxtAddrMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxtAddrMode {
    Repeat = 0,
    Clamp,
}

/// `_MinTextureFilters`, `_MagTextureFilters`, `_MipMapFilters` (texturefilter.cpp:26), zero-initialised
/// globals indexed `[stage][filter type]`.
#[derive(Clone, Debug, Default)]
pub struct TextureFilterTables {
    pub min_texture_filters: [[u32; FILTER_TYPE_COUNT]; MAX_TEXTURE_STAGES],
    pub mag_texture_filters: [[u32; FILTER_TYPE_COUNT]; MAX_TEXTURE_STAGES],
    pub mip_map_filters: [[u32; FILTER_TYPE_COUNT]; MAX_TEXTURE_STAGES],
}

impl TextureFilterTables {
    /// `TextureFilterClass::_Init_Filters` (texturefilter.cpp:119) for the non-Xbox build. Stage 0 is set from
    /// the device's `TextureFilterCaps` (`dx8caps.TextureFilterCaps`, the argument); later stages copy the
    /// previous stage, except that an anisotropic "best" magnification or minimisation falls back to linear;
    /// "default" copies "best". `D3DTSS_MAXANISOTROPY = 2` is set on every stage (device-only, not translated).
    pub fn init_filters(&mut self, filter_type: TextureFilterMode, texture_filter_caps: u32) {
        self.min_texture_filters[0][FilterType::None as usize] = D3DTEXF_POINT;
        self.mag_texture_filters[0][FilterType::None as usize] = D3DTEXF_POINT;
        self.mip_map_filters[0][FilterType::None as usize] = D3DTEXF_NONE;

        self.min_texture_filters[0][FilterType::Fast as usize] = D3DTEXF_LINEAR;
        self.mag_texture_filters[0][FilterType::Fast as usize] = D3DTEXF_LINEAR;
        self.mip_map_filters[0][FilterType::Fast as usize] = D3DTEXF_POINT;

        self.mag_texture_filters[0][FilterType::Best as usize] = D3DTEXF_POINT;
        self.min_texture_filters[0][FilterType::Best as usize] = D3DTEXF_POINT;
        self.mip_map_filters[0][FilterType::Best as usize] = D3DTEXF_POINT;

        if texture_filter_caps & D3DPTFILTERCAPS_MAGFLINEAR != 0 {
            self.mag_texture_filters[0][FilterType::Best as usize] = D3DTEXF_LINEAR;
        }
        if texture_filter_caps & D3DPTFILTERCAPS_MINFLINEAR != 0 {
            self.min_texture_filters[0][FilterType::Best as usize] = D3DTEXF_LINEAR;
        }

        if filter_type == TextureFilterMode::Anisotropic {
            if texture_filter_caps & D3DPTFILTERCAPS_MAGFANISOTROPIC != 0 {
                self.mag_texture_filters[0][FilterType::Best as usize] = D3DTEXF_ANISOTROPIC;
            }
            if texture_filter_caps & D3DPTFILTERCAPS_MINFANISOTROPIC != 0 {
                self.min_texture_filters[0][FilterType::Best as usize] = D3DTEXF_ANISOTROPIC;
            }
        }
        if filter_type == TextureFilterMode::Anisotropic || filter_type == TextureFilterMode::Trilinear {
            if texture_filter_caps & D3DPTFILTERCAPS_MIPFLINEAR != 0 {
                self.mip_map_filters[0][FilterType::Best as usize] = D3DTEXF_LINEAR;
            }
        }

        for i in 1..MAX_TEXTURE_STAGES {
            self.min_texture_filters[i][FilterType::None as usize] = self.min_texture_filters[i - 1][FilterType::None as usize];
            self.mag_texture_filters[i][FilterType::None as usize] = self.mag_texture_filters[i - 1][FilterType::None as usize];
            self.mip_map_filters[i][FilterType::None as usize] = self.mip_map_filters[i - 1][FilterType::None as usize];

            self.min_texture_filters[i][FilterType::Fast as usize] = self.min_texture_filters[i - 1][FilterType::Fast as usize];
            self.mag_texture_filters[i][FilterType::Fast as usize] = self.mag_texture_filters[i - 1][FilterType::Fast as usize];
            self.mip_map_filters[i][FilterType::Fast as usize] = self.mip_map_filters[i - 1][FilterType::Fast as usize];

            if self.mag_texture_filters[i - 1][FilterType::Best as usize] == D3DTEXF_ANISOTROPIC {
                self.mag_texture_filters[i][FilterType::Best as usize] = D3DTEXF_LINEAR;
            } else {
                self.mag_texture_filters[i][FilterType::Best as usize] = self.mag_texture_filters[i - 1][FilterType::Best as usize];
            }

            if self.min_texture_filters[i - 1][FilterType::Best as usize] == D3DTEXF_ANISOTROPIC {
                self.min_texture_filters[i][FilterType::Best as usize] = D3DTEXF_LINEAR;
            } else {
                self.min_texture_filters[i][FilterType::Best as usize] = self.min_texture_filters[i - 1][FilterType::Best as usize];
            }

            self.mip_map_filters[i][FilterType::Best as usize] = self.mip_map_filters[i - 1][FilterType::Best as usize];
        }

        for i in 0..MAX_TEXTURE_STAGES {
            self.min_texture_filters[i][FilterType::Default as usize] = self.min_texture_filters[i][FilterType::Best as usize];
            self.mag_texture_filters[i][FilterType::Default as usize] = self.mag_texture_filters[i][FilterType::Best as usize];
            self.mip_map_filters[i][FilterType::Default as usize] = self.mip_map_filters[i][FilterType::Best as usize];
            // upstream: DX8Wrapper::Set_DX8_Texture_Stage_State(i, D3DTSS_MAXANISOTROPY, 2) — device-only, not translated
        }
    }

    /// `TextureFilterClass::_Set_Default_Min_Filter` (texturefilter.cpp:180).
    pub fn set_default_min_filter(&mut self, filter: FilterType) {
        for i in 0..MAX_TEXTURE_STAGES {
            self.min_texture_filters[i][FilterType::Default as usize] = self.min_texture_filters[i][filter as usize];
        }
    }

    /// `TextureFilterClass::_Set_Default_Mag_Filter` (texturefilter.cpp:189).
    pub fn set_default_mag_filter(&mut self, filter: FilterType) {
        for i in 0..MAX_TEXTURE_STAGES {
            self.mag_texture_filters[i][FilterType::Default as usize] = self.mag_texture_filters[i][filter as usize];
        }
    }

    /// `TextureFilterClass::_Set_Default_Mip_Filter` (texturefilter.cpp:198).
    pub fn set_default_mip_filter(&mut self, filter: FilterType) {
        for i in 0..MAX_TEXTURE_STAGES {
            self.mip_map_filters[i][FilterType::Default as usize] = self.mip_map_filters[i][filter as usize];
        }
    }
}

/// What `TextureFilterClass::Apply` sets on a stage: (min, mag, mip) filters and the U/V address modes as
/// `D3DTADDRESS_WRAP` (1) / `D3DTADDRESS_CLAMP` (3) values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureStageFilterState {
    pub min_filter: u32,
    pub mag_filter: u32,
    pub mip_filter: u32,
    pub address_u: u32,
    pub address_v: u32,
}

/// `class TextureFilterClass` (texturefilter.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureFilterClass {
    texture_min_filter: FilterType,
    texture_mag_filter: FilterType,
    mip_map_filter: FilterType,
    u_address_mode: TxtAddrMode,
    v_address_mode: TxtAddrMode,
}

impl TextureFilterClass {
    /// `TextureFilterClass::TextureFilterClass(mip_level_count)` (texturefilter.cpp:33): min and mag default, both
    /// address modes repeat; no mip mapping for a single level, otherwise the default mip filter.
    pub fn new(mip_level_count: MipCountType) -> Self {
        TextureFilterClass {
            texture_min_filter: FilterType::Default,
            texture_mag_filter: FilterType::Default,
            u_address_mode: TxtAddrMode::Repeat,
            v_address_mode: TxtAddrMode::Repeat,
            mip_map_filter: if mip_level_count != MipCountType::MipLevels1 { FilterType::Default } else { FilterType::None },
        }
    }

    /// `TextureFilterClass::Apply` (texturefilter.cpp:52): the device-state values for `stage`.
    /// upstream: DX8Wrapper::Set_DX8_Texture_Stage_State (MINFILTER, MAGFILTER, MIPFILTER, ADDRESSU, ADDRESSV) —
    /// device-only, not translated.
    pub fn apply_state(&self, tables: &TextureFilterTables, stage: usize) -> TextureStageFilterState {
        const D3DTADDRESS_WRAP: u32 = 1;
        const D3DTADDRESS_CLAMP: u32 = 3;
        TextureStageFilterState {
            min_filter: tables.min_texture_filters[stage][self.texture_min_filter as usize],
            mag_filter: tables.mag_texture_filters[stage][self.texture_mag_filter as usize],
            mip_filter: tables.mip_map_filters[stage][self.mip_map_filter as usize],
            address_u: match self.get_u_addr_mode() {
                TxtAddrMode::Repeat => D3DTADDRESS_WRAP,
                TxtAddrMode::Clamp => D3DTADDRESS_CLAMP,
            },
            address_v: match self.get_v_addr_mode() {
                TxtAddrMode::Repeat => D3DTADDRESS_WRAP,
                TxtAddrMode::Clamp => D3DTADDRESS_CLAMP,
            },
        }
    }

    /// `Get_Min_Filter` (texturefilter.h).
    pub fn get_min_filter(&self) -> FilterType {
        self.texture_min_filter
    }

    /// `Get_Mag_Filter` (texturefilter.h).
    pub fn get_mag_filter(&self) -> FilterType {
        self.texture_mag_filter
    }

    /// `Get_Mip_Mapping` (texturefilter.h).
    pub fn get_mip_mapping(&self) -> FilterType {
        self.mip_map_filter
    }

    /// `Set_Min_Filter` (texturefilter.h).
    pub fn set_min_filter(&mut self, filter: FilterType) {
        self.texture_min_filter = filter;
    }

    /// `Set_Mag_Filter` (texturefilter.h).
    pub fn set_mag_filter(&mut self, filter: FilterType) {
        self.texture_mag_filter = filter;
    }

    /// `TextureFilterClass::Set_Mip_Mapping` (texturefilter.cpp:171).
    pub fn set_mip_mapping(&mut self, mipmap: FilterType) {
        self.mip_map_filter = mipmap;
    }

    /// `Get_U_Addr_Mode` (texturefilter.h).
    pub fn get_u_addr_mode(&self) -> TxtAddrMode {
        self.u_address_mode
    }

    /// `Get_V_Addr_Mode` (texturefilter.h).
    pub fn get_v_addr_mode(&self) -> TxtAddrMode {
        self.v_address_mode
    }

    /// `Set_U_Addr_Mode` (texturefilter.h).
    pub fn set_u_addr_mode(&mut self, mode: TxtAddrMode) {
        self.u_address_mode = mode;
    }

    /// `Set_V_Addr_Mode` (texturefilter.h).
    pub fn set_v_addr_mode(&mut self, mode: TxtAddrMode) {
        self.v_address_mode = mode;
    }
}
