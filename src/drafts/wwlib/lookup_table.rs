//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/lookuptable.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/lookuptable.cpp
//!
//! Replacements: `RefCountClass`/`MultiListObjectClass`/`RefMultiListClass` -> plain ownership, `LookupTableMgrClass`'s
//! statics -> the `LookupTableMgrClass` value; `_TheFileFactory->Get_File(name)` -> the caller passes the file's bytes to
//! `get_table` (device/file-system access is not translated here); `SaveLoadSystemClass::Find_Persist_Factory` -> the
//! `Curve1D` enum below, picked by the factory chunk id. `Get_Value_Quick` keeps upstream's formula verbatim, which
//! multiplies the *count* by the converted index (an upstream defect, left as is).

use super::chunk_save::ChunkSaveClass;
use super::curve::{read_f32, LinearCurve1DClass};
use super::cardinal_spline::CardinalSpline1DClass;
use super::catmull_rom_spline::CatmullRomSpline1DClass;
use super::hermite_spline::HermiteSpline1DClass;
use super::saveload_ids::*;
use crate::ww::chunkio::{ChunkLoad, GameFile};
use crate::ww::vector2::Vector2;
use crate::ww::wwmath;

/// `LOOKUPTABLE_CHUNK_CURVE = 03071200` (octal, as upstream wrote it) and the next id.
pub const LOOKUPTABLE_CHUNK_CURVE: u32 = 0o3071200;
pub const LOOKUPTABLE_CHUNK_EXTENTS: u32 = LOOKUPTABLE_CHUNK_CURVE + 1;

/// The `Curve1DClass` descendants a persist factory can produce (`Curve1DClass *` with virtual dispatch upstream).
#[derive(Clone)]
pub enum Curve1D {
    Linear(LinearCurve1DClass),
    Hermite(HermiteSpline1DClass),
    CatmullRom(CatmullRomSpline1DClass),
    Cardinal(CardinalSpline1DClass),
}

impl Curve1D {
    /// `Curve1DClass::Key_Count`.
    pub fn key_count(&self) -> i32 {
        match self {
            Curve1D::Linear(c) => c.base.key_count(),
            Curve1D::Hermite(c) => c.base.key_count(),
            Curve1D::CatmullRom(c) => c.base.base.key_count(),
            Curve1D::Cardinal(c) => c.base.base.key_count(),
        }
    }

    /// `Curve1DClass::Get_Key`: (point, time, extra).
    pub fn get_key(&self, i: i32) -> (f32, f32, u32) {
        match self {
            Curve1D::Linear(c) => c.base.get_key(i),
            Curve1D::Hermite(c) => c.base.get_key(i),
            Curve1D::CatmullRom(c) => c.base.base.get_key(i),
            Curve1D::Cardinal(c) => c.base.base.get_key(i),
        }
    }

    /// `Curve1DClass::Evaluate` (virtual).
    pub fn evaluate(&mut self, time: f32) -> f32 {
        match self {
            Curve1D::Linear(c) => c.evaluate(time),
            Curve1D::Hermite(c) => c.evaluate(time),
            Curve1D::CatmullRom(c) => c.evaluate(time),
            Curve1D::Cardinal(c) => c.evaluate(time),
        }
    }

    /// `PersistClass::Get_Factory().Chunk_ID()`.
    pub fn factory_chunk_id(&self) -> u32 {
        match self {
            Curve1D::Linear(c) => c.get_factory_id(),
            Curve1D::Hermite(c) => c.get_factory_id(),
            Curve1D::CatmullRom(c) => c.get_factory_id(),
            Curve1D::Cardinal(c) => c.get_factory_id(),
        }
    }

    /// `PersistFactoryClass::Save(csave, curve)` -> the object's `Save`.
    pub fn save(&mut self, csave: &mut ChunkSaveClass) -> bool {
        match self {
            Curve1D::Linear(c) => c.save(csave),
            Curve1D::Hermite(c) => c.save(csave),
            Curve1D::CatmullRom(c) => c.save(csave),
            Curve1D::Cardinal(c) => c.save(csave),
        }
    }

    /// `SimplePersistFactoryClass::Load` for a Curve1D factory id (`None` when it is not a 1D curve factory).
    pub fn load_by_factory(chunk_id: u32, cload: &mut ChunkLoad) -> Option<Curve1D> {
        match chunk_id {
            WWMATH_CHUNKID_LINEARCURVE1D => {
                let mut c = LinearCurve1DClass::default();
                c.load(cload);
                Some(Curve1D::Linear(c))
            }
            WWMATH_CHUNKID_HERMITESPLINE1D => {
                let mut c = HermiteSpline1DClass::new();
                c.load(cload);
                Some(Curve1D::Hermite(c))
            }
            WWMATH_CHUNKID_CATMULLROMSPLINE1D => {
                let mut c = CatmullRomSpline1DClass::default();
                c.load(cload);
                Some(Curve1D::CatmullRom(c))
            }
            WWMATH_CHUNKID_CARDINALSPLINE1D => {
                let mut c = CardinalSpline1DClass::default();
                c.load(cload);
                Some(Curve1D::Cardinal(c))
            }
            _ => None,
        }
    }
}

/// `class LookupTableClass`.
#[derive(Clone)]
pub struct LookupTableClass {
    name: String,
    min_input_value: f32,
    max_input_value: f32,
    oo_max_minus_min: f32,
    output_samples: Vec<f32>,
}

impl LookupTableClass {
    /// `LookupTableClass::LookupTableClass(int sample_count = 256)`.
    pub fn new(sample_count: i32) -> Self {
        LookupTableClass {
            name: String::new(),
            min_input_value: 0.0,
            max_input_value: 0.0,
            oo_max_minus_min: 0.0,
            output_samples: vec![0.0; sample_count as usize],
        }
    }

    /// `LookupTableClass::Init` (lookuptable.cpp:77).
    pub fn init(&mut self, name: &str, curve: &mut Curve1D) {
        self.name = name.to_string();
        // Store the min and max input values for the table
        self.min_input_value = curve.get_key(0).1;
        self.max_input_value = curve.get_key(curve.key_count() - 1).1;
        self.oo_max_minus_min = 1.0 / (self.max_input_value - self.min_input_value);
        // Sample the curve and store the output values
        let len = self.output_samples.len();
        for i in 0..len {
            let x = self.min_input_value
                + (self.max_input_value - self.min_input_value) * (i as f32) / ((len - 1) as f32);
            self.output_samples[i] = curve.evaluate(x);
        }
    }

    /// `LookupTableClass::Get_Value` (lookuptable.h).
    pub fn get_value(&self, input: f32) -> f32 {
        if input <= self.min_input_value {
            return self.output_samples[0];
        }
        if input >= self.max_input_value {
            return self.output_samples[self.output_samples.len() - 1];
        }
        let normalized_input =
            ((self.output_samples.len() - 1) as f32) * (input - self.min_input_value) * self.oo_max_minus_min;
        let input0 = wwmath::floor(normalized_input);
        let index0 = wwmath::float_to_long(input0);
        let index1 = index0 + 1;
        let lerp = normalized_input - input0;
        let (s0, s1) = (self.output_samples[index0 as usize], self.output_samples[index1 as usize]);
        s0 + lerp * (s1 - s0)
    }

    /// `LookupTableClass::Get_Value_Quick` (lookuptable.h), verbatim including the multiplication.
    pub fn get_value_quick(&self, input: f32) -> f32 {
        if input <= self.min_input_value {
            return self.output_samples[0];
        }
        if input >= self.max_input_value {
            return self.output_samples[self.output_samples.len() - 1];
        }
        let index = ((self.output_samples.len() - 1) as i32)
            * wwmath::float_to_long((input - self.min_input_value) * self.oo_max_minus_min);
        self.output_samples[index as usize]
    }

    /// `LookupTableClass::Get_Name`.
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

/// `class LookupTableMgrClass` (its statics held in a value).
#[derive(Default)]
pub struct LookupTableMgrClass {
    tables: Vec<LookupTableClass>,
}

impl LookupTableMgrClass {
    /// `LookupTableMgrClass::Init` (lookuptable.cpp:131): adds "DefaultTable".
    pub fn init(&mut self) {
        let mut default_table = LookupTableClass::new(2);
        let mut default_curve = LinearCurve1DClass::default();
        default_curve.base.add_key(0.5, 0.0, 0);
        default_curve.base.add_key(0.5, 1.0, 0);
        let mut curve = Curve1D::Linear(default_curve);
        default_table.init("DefaultTable", &mut curve);
        self.add_table(default_table);
    }

    /// `LookupTableMgrClass::Shutdown`.
    pub fn shutdown(&mut self) {
        self.reset();
    }

    /// `LookupTableMgrClass::Reset`.
    pub fn reset(&mut self) {
        self.tables.clear();
    }

    /// `LookupTableMgrClass::Add_Table`.
    pub fn add_table(&mut self, table: LookupTableClass) -> bool {
        self.tables.push(table);
        true
    }

    /// `LookupTableMgrClass::Remove_Table` (by name; tables have no identity here).
    pub fn remove_table(&mut self, name: &str) -> bool {
        match self.tables.iter().position(|t| t.get_name().eq_ignore_ascii_case(name)) {
            Some(p) => {
                self.tables.remove(p);
                true
            }
            None => false,
        }
    }

    /// `LookupTableMgrClass::Get_Table` (lookuptable.cpp:195). `file` is the bytes `_TheFileFactory->Get_File(name)`
    /// would open (`None` when `try_to_load` is false or the file is missing).
    pub fn get_table(&mut self, name: &str, file: Option<&[u8]>) -> Option<&LookupTableClass> {
        // check if we already have this table loaded...
        if let Some(p) = self.tables.iter().position(|t| t.get_name().eq_ignore_ascii_case(name)) {
            return Some(&self.tables[p]);
        }
        // otherwise we can try to load it.
        let bytes = file?;
        let mut cload = ChunkLoad::new(GameFile::new(bytes)?);
        let (curve, _, _) = Self::load_table_desc(&mut cload);
        let mut curve = curve?;
        let mut new_table = LookupTableClass::new(256);
        new_table.init(name, &mut curve);
        self.add_table(new_table);
        self.tables.last()
    }

    /// `LookupTableMgrClass::Save_Table_Desc` (lookuptable.cpp:240).
    pub fn save_table_desc(csave: &mut ChunkSaveClass, curve: &mut Curve1D, min_corner: &Vector2, max_corner: &Vector2) {
        // save the curve
        csave.begin_chunk(LOOKUPTABLE_CHUNK_CURVE);
        csave.begin_chunk(curve.factory_chunk_id());
        curve.save(csave);
        csave.end_chunk();
        csave.end_chunk();

        // save the extents
        csave.begin_chunk(LOOKUPTABLE_CHUNK_EXTENTS);
        csave.write_floats(&[min_corner.x]);
        csave.write_floats(&[min_corner.y]);
        csave.write_floats(&[max_corner.x]);
        csave.write_floats(&[max_corner.y]);
        csave.end_chunk();
    }

    /// `LookupTableMgrClass::Load_Table_Desc` (lookuptable.cpp:275): (curve, min corner, max corner).
    pub fn load_table_desc(cload: &mut ChunkLoad) -> (Option<Curve1D>, Vector2, Vector2) {
        let mut curve: Option<Curve1D> = None;
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                LOOKUPTABLE_CHUNK_CURVE => {
                    let _ = cload.open_chunk();
                    let id = cload.cur_chunk_id().unwrap_or(0);
                    curve = Curve1D::load_by_factory(id, cload); // factory == NULL asserts upstream
                    let _ = cload.close_chunk();
                }
                LOOKUPTABLE_CHUNK_EXTENTS => {
                    xmin = read_f32(cload);
                    ymin = read_f32(cload);
                    xmax = read_f32(cload);
                    ymax = read_f32(cload);
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        (curve, Vector2::new(xmin, ymin), Vector2::new(xmax, ymax))
    }
}
