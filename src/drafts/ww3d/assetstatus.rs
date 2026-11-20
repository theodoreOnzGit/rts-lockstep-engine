//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (the report text; line order inside a category differs by design) (test: tests/differential_draft_ww3d7_misc.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetstatus.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetstatus.h

use std::collections::BTreeMap;

/// `AssetStatusClass::REPORT_*` (assetstatus.h).
pub const REPORT_LOAD_ON_DEMAND_ROBJ: usize = 0;
pub const REPORT_LOAD_ON_DEMAND_HANIM: usize = 1;
pub const REPORT_LOAD_ON_DEMAND_HTREE: usize = 2;
pub const REPORT_MISSING_ROBJ: usize = 3;
pub const REPORT_MISSING_HANIM: usize = 4;
pub const REPORT_MISSING_HTREE: usize = 5;
pub const REPORT_COUNT: usize = 6;

/// `ReportCategoryNames` (assetstatus.cpp:34).
pub const REPORT_CATEGORY_NAMES: [&str; REPORT_COUNT] = [
    "LOAD_ON_DEMAND_ROBJ",
    "LOAD_ON_DEMAND_HANIM",
    "LOAD_ON_DEMAND_HTREE",
    "MISSING_ROBJ",
    "MISSING_HANIM",
    "MISSING_HTREE",
];

/// `class AssetStatusClass` (assetstatus.h). Upstream has one static `Instance`; the owner holds one of these.
/// `HashTemplateClass<StringClass,int>` becomes a `BTreeMap` (upstream's iteration order is hash order, which
/// only affects the order of lines in the debug report).
#[derive(Clone, Debug)]
pub struct AssetStatusClass {
    reporting: bool,
    load_on_demand_reporting: bool,
    report_hash_tables: [BTreeMap<String, i32>; REPORT_COUNT],
}

impl AssetStatusClass {
    /// `AssetStatusClass::AssetStatusClass` (assetstatus.cpp:44): reporting on, load-on-demand reporting off.
    pub fn new() -> Self {
        AssetStatusClass {
            reporting: true,
            load_on_demand_reporting: false,
            report_hash_tables: Default::default(),
        }
    }

    /// `Enable_Reporting` (assetstatus.h).
    pub fn enable_reporting(&mut self, enable: bool) {
        self.reporting = enable;
    }

    /// `Enable_Load_On_Demand_Reporting` (assetstatus.h).
    pub fn enable_load_on_demand_reporting(&mut self, enable: bool) {
        self.load_on_demand_reporting = enable;
    }

    /// The text `AssetStatusClass::~AssetStatusClass` (assetstatus.cpp:52, `#ifdef WWDEBUG` only) writes to
    /// `asset_report.txt` when `Reporting` is set; `None` when it would write nothing. The file write
    /// (`RawFileClass`) is left to the caller.
    pub fn debug_report(&self) -> Option<String> {
        if !self.reporting && !crate::logic::modules::plant(19479) {
            return None;
        }
        let mut report = String::from("Load-on-demand and missing assets report\r\n\r\n");
        for i in 0..REPORT_COUNT {
            report.push_str("Category: ");
            report.push_str(REPORT_CATEGORY_NAMES[i]);
            report.push_str("\r\n\r\n");
            for (key, count) in &self.report_hash_tables[i] {
                report.push_str(key);
                let threshold = if crate::logic::modules::plant(19477) { 0 } else { 1 };
                if *count > threshold {
                    report.push_str(&format!("\t(reported {} times)", count));
                }
                report.push_str("\r\n");
            }
            report.push_str("\r\n");
        }
        Some(report)
    }

    /// `AssetStatusClass::Add_To_Report` (assetstatus.cpp:88): lower-cases the name (`_strlwr`, ASCII) and bumps
    /// its count (`Get` of a missing key is 0).
    fn add_to_report(&mut self, index: usize, name: &str) {
        let lower_case_name = if crate::logic::modules::plant(19474) { name.to_string() } else { name.to_ascii_lowercase() };
        let count = if crate::logic::modules::plant(19476) { 1 } else { self.report_hash_tables[index].get(&lower_case_name).copied().unwrap_or(0) + 1 };
        self.report_hash_tables[index].insert(lower_case_name, count);
    }

    /// `Report_Load_On_Demand_RObj` (assetstatus.cpp:97).
    pub fn report_load_on_demand_robj(&mut self, name: &str) {
        if self.load_on_demand_reporting || crate::logic::modules::plant(19475) {
            self.add_to_report(REPORT_LOAD_ON_DEMAND_ROBJ, name);
        }
    }

    /// `Report_Load_On_Demand_HAnim` (assetstatus.cpp:102).
    pub fn report_load_on_demand_hanim(&mut self, name: &str) {
        if self.load_on_demand_reporting {
            self.add_to_report(REPORT_LOAD_ON_DEMAND_HANIM, name);
        }
    }

    /// `Report_Load_On_Demand_HTree` (assetstatus.cpp:107).
    pub fn report_load_on_demand_htree(&mut self, name: &str) {
        if self.load_on_demand_reporting {
            self.add_to_report(REPORT_LOAD_ON_DEMAND_HTREE, name);
        }
    }

    /// `Report_Missing_RObj` (assetstatus.cpp:112).
    pub fn report_missing_robj(&mut self, name: &str) {
        if crate::logic::modules::plant(19478) && !self.load_on_demand_reporting {
            return;
        }
        self.add_to_report(REPORT_MISSING_ROBJ, name);
    }

    /// `Report_Missing_HAnim` (assetstatus.cpp:117).
    pub fn report_missing_hanim(&mut self, name: &str) {
        self.add_to_report(REPORT_MISSING_HANIM, name);
    }

    /// `Report_Missing_HTree` (assetstatus.cpp:122).
    pub fn report_missing_htree(&mut self, name: &str) {
        self.add_to_report(if crate::logic::modules::plant(19480) { REPORT_MISSING_HANIM } else { REPORT_MISSING_HTREE }, name);
    }
}

impl Default for AssetStatusClass {
    fn default() -> Self {
        Self::new()
    }
}
