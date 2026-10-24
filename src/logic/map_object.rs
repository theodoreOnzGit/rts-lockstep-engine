// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/MapObject.h
//   GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp
//     (ParseObjectDataChunk, the reader of an `Object` chunk)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp
//     (the constructor, the accessors and the waypoint accessors; the
//     definitions of the MapObject class live in the device layer)
//   GeneralsMD/Code/GameEngine/Include/Common/MapReaderWriterInfo.h
//     (the K_OBJECTS_VERSION_* numbers)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John Ahlquist, April 2001 (MapObject.h), Matt Campbell,
//   December 2001 (MapUtil.cpp)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MapObject`: an object placed on a map (a unit, a building, a waypoint,
//! a decoration), with its position, angle, flags and the property `Dict`
//! the map gives it, and the reader of the `Object` chunks of a map.
//!
//! # Where the template pointer went
//!
//! Upstream stores `const ThingTemplate*`, found by name with
//! `TheThingFactory->findTemplate(name, FALSE)`. The template is not
//! translated yet, so the object keeps the name it is found by
//! ([`MapObject::thing_template`]). Whether that name names a template, and
//! which one, is resolved later, when the thing factory exists; a null
//! pointer upstream becomes that lookup failing then.
//!
//! # What the parse keeps, and what it leaves to the caller
//!
//! `ParseObjectDataChunk` builds the object, and then classifies it: a
//! waypoint (its property `waypointID` is an `Int`) goes into the waypoint
//! map (`m_waypoints`, [`ObjectParse::waypoints`]); a tech building or a
//! supply source (by the template's kind of) goes into a list of positions
//! that the map's client code reads. The kind of needs the template, so
//! that classification is a cut point: [`ObjectParse::objects`] keeps every
//! object in file order, and the caller classifies them once templates are
//! translated. Upstream deletes each object after its classification; here
//! the parse keeps them.
//!
//! # Not translated
//!
//! - The constructor's `props == NULL` branch, which sets default properties
//!   (`TheKey_objectInitialHealth` and the rest). Nothing in the engine calls
//!   it with NULL: the parse always passes the chunk's dictionary (an empty
//!   one for the version-1 format), and `duplicate()` passes its own. The
//!   Rust constructor takes a `Dict` only.
//! - The render object, shadow, bridge towers and `TheWorldDict` (the client
//!   and World Builder side).
//! - `validate`, `verifyValidTeam`, `verifyValidUniqueID`,
//!   `fastAssignAllUniqueIDs`, `countMapObjectsWithOwner`, `setWaypointID`,
//!   `setWaypointName`, `setName` and `duplicate` (World Builder; the
//!   accessors that the parse and the engine need are here).
//! - The road, bridge, light and scorch flags as meaning: the bit values are
//!   kept, the code that reads them is the client's.
//! - The `validateName` of the constructor: it returns its argument (the
//!   device file's version does).
//!
//! # Undefined upstream
//!
//! An angle that `normalizeAngle` never reduces (an infinity, or a value so
//! large that subtracting `2*PI` does not change it) makes upstream's
//! constructor hang. NaN is not one: `normalizeAngle` gives 0 for it. [`MapObject::new`] returns the error, and the parse
//! records it in [`ObjectParse::undefined`] and stops.

use std::collections::BTreeMap;

use crate::common::base_type::Coord3D;
use crate::common::dict::{DataType, Dict};
use crate::common::game_common::{try_normalize_angle, NormalizeAngleHangs};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::well_known_keys::{WellKnownKeys, WAYPOINT_ID, WAYPOINT_NAME};
use crate::formats::data_chunk::{ChunkResult, DataChunkInfo, DataChunkInput};

/// `K_OBJECTS_VERSION_1` (MapReaderWriterInfo.h): no dictionary.
pub const K_OBJECTS_VERSION_1: u16 = 1;
/// `K_OBJECTS_VERSION_2`: includes the dictionary.
pub const K_OBJECTS_VERSION_2: u16 = 2;
/// `K_OBJECTS_VERSION_3`: includes the dictionary.
pub const K_OBJECTS_VERSION_3: u16 = 3;

/// `MapObject::MO_SELECTED`: the runtime flags (not saved in the map file).
pub const MO_SELECTED: i32 = 0x01;
/// `MapObject::MO_LIGHT`.
pub const MO_LIGHT: i32 = 0x02;
/// `MapObject::MO_WAYPOINT`.
pub const MO_WAYPOINT: i32 = 0x04;
/// `MapObject::MO_SCORCH`.
pub const MO_SCORCH: i32 = 0x08;

/// `class MapObject` (the data and the accessors; see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct MapObject {
    /// `m_location`: the centre of the object.
    location: Coord3D,
    /// `m_objectName`: the object's name, as the map gives it.
    object_name: Vec<u8>,
    /// `m_thingTemplate` as the name it is looked up by (see the module docs).
    thing_template: Option<Vec<u8>>,
    /// `m_angle`: normalised by `normalizeAngle` (counter-clockwise, degrees).
    angle: f32,
    /// `m_flags`: the map's flags (`FLAG_*` bits).
    flags: i32,
    /// `m_properties`: the property sheet.
    properties: Dict,
    /// `m_color`: the UI colour (bright green, `0xff << 8`, at creation).
    color: i32,
    /// `m_runtimeFlags`: the `MO_*` bits.
    runtime_flags: i32,
}

impl MapObject {
    /// `MapObject(loc, name, angle, flags, props, thingTemplate)` with `props`
    /// given (see the module docs for the NULL case). `thing_template` is the
    /// name the template was looked up by, or `None` for a null pointer.
    /// Fails where upstream's `normalizeAngle` never returns.
    pub fn new(
        location: Coord3D,
        name: &[u8],
        angle: f32,
        flags: i32,
        props: &Dict,
        thing_template: Option<&[u8]>,
    ) -> Result<Self, NormalizeAngleHangs> {
        Ok(MapObject {
            location,
            object_name: name.to_vec(),
            thing_template: thing_template.map(<[u8]>::to_vec),
            angle: try_normalize_angle(angle)?,
            flags,
            properties: props.clone(),
            color: 0xff << 8,
            runtime_flags: 0,
        })
    }

    /// `getLocation()`: the centre point.
    pub fn location(&self) -> Coord3D {
        self.location
    }

    /// `getAngle()`.
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// `getName()`.
    pub fn name(&self) -> &[u8] {
        &self.object_name
    }

    /// The name of the template this object was created from, if any (see
    /// the module docs: resolved later).
    pub fn thing_template(&self) -> Option<&[u8]> {
        self.thing_template.as_deref()
    }

    /// `getFlags()`.
    pub fn flags(&self) -> i32 {
        self.flags
    }

    /// `getProperties()`: the property sheet.
    pub fn properties(&self) -> &Dict {
        &self.properties
    }

    /// `getProperties()`, mutable.
    pub fn properties_mut(&mut self) -> &mut Dict {
        &mut self.properties
    }

    /// `getColor()`.
    pub fn color(&self) -> i32 {
        self.color
    }

    /// `setColor(color)`.
    pub fn set_color(&mut self, color: i32) {
        self.color = color;
    }

    /// `isWaypoint()`.
    pub fn is_waypoint(&self) -> bool {
        self.runtime_flags & MO_WAYPOINT != 0
    }

    /// `setIsWaypoint()`.
    pub fn set_is_waypoint(&mut self) {
        self.runtime_flags |= MO_WAYPOINT;
    }

    /// `getWaypointName()`: the `waypointName` property, `""` if it is not a
    /// string. The key is looked up (and made, if new) as upstream's
    /// `TheKey_waypointName` is on first use.
    pub fn waypoint_name(&self, keys: &WellKnownKeys, names: &mut NameKeyGenerator) -> Vec<u8> {
        let key = keys.key(WAYPOINT_NAME, names);
        self.properties.get_ascii_string(key).0
    }
}

/// `ParseObjectDataChunk`'s context: the name keys and well-known keys it
/// uses, the objects it has read, the waypoint map it fills, and the
/// undefined input it stopped on, if any.
#[derive(Clone, Debug)]
pub struct ObjectParse {
    /// `TheNameKeyGenerator`, which `readDict` and the well-known keys use.
    pub names: NameKeyGenerator,
    /// The well-known keys (`TheKey_*`), each made on first use.
    pub keys: WellKnownKeys,
    /// Every object read, in file order (see the module docs).
    pub objects: Vec<MapObject>,
    /// `m_waypoints`: the location of each waypoint by name; the later one
    /// wins, as `std::map`'s `operator[]` does.
    pub waypoints: BTreeMap<Vec<u8>, Coord3D>,
    /// The angle that `normalizeAngle` never reduces, if that is what stopped
    /// the parse.
    pub undefined: Option<NormalizeAngleHangs>,
}

impl Default for ObjectParse {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectParse {
    /// An empty context, with the name generator initialised.
    pub fn new() -> Self {
        let mut names = NameKeyGenerator::new();
        names.init();
        ObjectParse {
            names,
            keys: WellKnownKeys::new(),
            objects: Vec::new(),
            waypoints: BTreeMap::new(),
            undefined: None,
        }
    }
}

/// `ParseObjectDataChunk`: reads one `Object` chunk into `ctx.objects` and
/// fills `ctx.waypoints` for a waypoint. Versions 1 to 3: from version 2 on,
/// the chunk holds the property dictionary, and the height of the position
/// is read but dropped for versions up to 2.
pub fn parse_object_data_chunk(
    file: &mut DataChunkInput<ObjectParse>,
    info: &DataChunkInfo,
    ctx: &mut ObjectParse,
) -> ChunkResult<bool> {
    let read_dict = info.version >= K_OBJECTS_VERSION_2;
    let x = file.read_real()?;
    let y = file.read_real()?;
    let mut z = file.read_real()?;
    if info.version <= K_OBJECTS_VERSION_2 {
        z = 0.0;
    }
    let loc = Coord3D { x, y, z };
    let angle = file.read_real()?;
    let flags = file.read_int()?;
    let name = file.read_ascii_string()?;
    let d = if read_dict {
        file.read_dict(&mut ctx.names)?
    } else {
        Dict::new()
    };

    // `newInstance(MapObject)(loc, name, angle, flags, &d, findTemplate(name, FALSE))`
    let mut obj = match MapObject::new(loc, &name, angle, flags, &d, Some(&name)) {
        Ok(o) => o,
        Err(e) => {
            ctx.undefined = Some(e);
            return Ok(false);
        }
    };
    // if (getProperties()->getType(TheKey_waypointID) == Dict::DICT_INT)
    let waypoint_id = ctx.keys.key(WAYPOINT_ID, &mut ctx.names);
    if obj.properties().get_type(waypoint_id) == DataType::Int {
        obj.set_is_waypoint();
        // (*m_waypoints)[getWaypointName()] = loc;
        let wp_name = obj.waypoint_name(&ctx.keys, &mut ctx.names);
        ctx.waypoints.insert(wp_name, loc);
    }
    // The tech-building and supply-source classification needs the template's
    // kind of: a cut point (see the module docs).
    ctx.objects.push(obj);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::dict::DictValue;
    use crate::common::name_key_generator::NameKeyType;

    fn loc() -> Coord3D {
        Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }
    }

    #[test]
    fn angle_is_normalised_and_colour_is_bright_green() {
        let d = Dict::new();
        let m = MapObject::new(loc(), b"Tree", 370.0, 0x10, &d, Some(b"Tree")).unwrap();
        assert_eq!(
            m.angle().to_bits(),
            try_normalize_angle(370.0).unwrap().to_bits()
        );
        assert_eq!(m.color(), 0xff << 8);
        assert_eq!(m.flags(), 0x10);
        assert_eq!(m.name(), b"Tree");
        assert_eq!(m.thing_template(), Some(&b"Tree"[..]));
        assert!(!m.is_waypoint());
        assert_eq!(
            MapObject::new(loc(), b"x", f32::NAN, 0, &d, None)
                .unwrap()
                .angle(),
            0.0
        );
        assert!(MapObject::new(loc(), b"x", f32::INFINITY, 0, &d, None).is_err());
    }

    #[test]
    fn the_dictionary_is_copied() {
        let mut d = Dict::new();
        d.set_int(NameKeyType(5), 9).unwrap();
        let m = MapObject::new(loc(), b"x", 0.0, 0, &d, None).unwrap();
        assert_eq!(m.properties().get(NameKeyType(5)), Some(&DictValue::Int(9)));
        assert_eq!(m.thing_template(), None);
    }

    #[test]
    fn waypoint_runtime_bit() {
        let d = Dict::new();
        let mut m = MapObject::new(loc(), b"x", 0.0, 0, &d, None).unwrap();
        m.set_is_waypoint();
        assert!(m.is_waypoint());
        assert_eq!(m.runtime_flags & MO_SELECTED, 0);
    }
}
