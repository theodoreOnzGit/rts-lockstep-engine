//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PrisonBehavior.h
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PrisonBehavior.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/POWTruckBehavior.h
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/POWTruckBehavior.cpp
//!
//! **Not compiled into the shipped game** (everything is under `#ifdef ALLOW_SURRENDER`; see `crate::common::module_factory`'s
//! header). Both classes are `OpenContain` subclasses written in the port's pattern (a struct holding [`OpenContain`] and
//! implementing [`OpenContainVirtuals`]); their `UpdateBody` variants, the factory rows and the `World` calls marked
//! `// wire:` do not exist yet. `PrisonBehavior` holds prisoners (`DISABLED_HELD`) and, with `ShowPrisoners`, a drawable per
//! prisoner wandering in the yard polygon; `POWTruckBehavior` picks up a surrendered unit it collides with.

use std::sync::Arc;

#[cfg(any())]
use crate::common::base_type::{Coord3D, Region2D, TWO_PI};
#[cfg(any())]
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::{DrawableId, ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use crate::common::ini::{FieldParse, IniResult, MultiIniFieldParse, UserData};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::modules::open_contain::{OpenContain, OpenContainModuleData};
#[cfg(any())]
use crate::logic::modules::open_contain::OpenContainVirtuals;
#[cfg(any())]
use crate::logic::world::game_logic::World;
#[cfg(any())]
use crate::logic::world::module_call::ModuleHandle;
#[cfg(any())]
use crate::logic::world::partition::filters::point_inside_area_2d;
#[cfg(any())]
use crate::logic::world::update_body::UpdateBody;

/// `MAX_YARD_BONES` in `pickVisualLocation`.
pub const MAX_YARD_BONES: usize = 16;

/// `class PrisonVisual`: an object/drawable pair; `m_next` is the list order (a `Vec`, newest first as upstream pushes at the head).
#[derive(Clone, Debug, PartialEq)]
pub struct PrisonVisual {
    /// `m_objectID`: "object that is contained".
    pub object_id: ObjectId,
    /// `m_drawableID`: "associated visual prisoner drawable".
    pub drawable_id: DrawableId,
}

impl PrisonVisual {
    /// `PrisonVisual::PrisonVisual()`: both invalid.
    pub fn new() -> PrisonVisual {
        PrisonVisual { object_id: INVALID_ID, drawable_id: INVALID_DRAWABLE_ID }
    }
}

/// `class PrisonBehaviorModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PrisonBehaviorModuleData {
    /// The base.
    pub open_contain: OpenContainModuleData,
    /// `m_showPrisoners`: "Will the contained prisoners be shown in the prison yard". Default FALSE.
    pub show_prisoners: bool,
    /// `m_prisonYardBonePrefix`: "Bone prefix that makes up the prison yards".
    pub prison_yard_bone_prefix: Vec<u8>,
}

impl Default for PrisonBehaviorModuleData {
    /// `PrisonBehaviorModuleData()`.
    fn default() -> Self {
        PrisonBehaviorModuleData { open_contain: OpenContainModuleData::default(), show_prisoners: false, prison_yard_bone_prefix: Vec::new() }
    }
}

/// `PrisonBehaviorModuleData::buildFieldParse`'s table: `ShowPrisoners`, `YardBonePrefix`.
static PRISON_FIELD_PARSE: [FieldParse<PrisonBehaviorModuleData>; 3] = [
    FieldParse::new("ShowPrisoners", |i, d, _| { d.show_prisoners = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("YardBonePrefix", |i, d, _| { d.prison_yard_bone_prefix = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl PrisonBehaviorModuleData {
    /// `buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)` then this table.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, PrisonBehaviorModuleData>) -> IniResult<()> {
        // wire: OpenContainModuleData::build_field_parse for the embedded base (as the other contain data do).
        p.add(&PRISON_FIELD_PARSE)?;
        Ok(())
    }
}

/// `class PrisonBehavior : public OpenContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct PrisonBehavior {
    oc: OpenContain,
    data: Arc<PrisonBehaviorModuleData>,
    /// `m_visualList`: the head is index 0 (upstream pushes new visuals at the head).
    pub visual_list: Vec<PrisonVisual>,
}

impl PrisonBehavior {
    /// `PrisonBehavior::PrisonBehavior(thing, moduleData)`: `m_visualList = NULL`.
    pub fn new(oc: OpenContain, data: Arc<PrisonBehaviorModuleData>) -> PrisonBehavior {
        PrisonBehavior { oc, data, visual_list: Vec::new() }
    }

}

#[cfg(any())] // not compiled: needs World/UpdateBody API that does not exist (the `// wire:` calls); upstream is under ALLOW_SURRENDER, undefined in the shipped build (#214)
impl PrisonBehavior {
    /// `PrisonBehavior::pickVisualLocation(pos)`: starts at the prison's position; with at least 3 yard bones
    /// (`getMultiLogicalBonePosition(prefix, 16, ...)`), the middle of their bounding box, then up to 32 random points of the box are
    /// tried (x then y from the logic random stream) and the first inside the yard polygon (`PointInsideArea2D`) wins; z stays the
    /// prison's.
    pub fn pick_visual_location(&self, world: &mut World, me: ModuleHandle) -> Coord3D {
        let us = world.module_object(me); // wire: getObject()
        // initialize the picked location to that of the prison center
        let mut picked_location = world.object_position(us); // wire: *us->getPosition()
        // get the positions of the bones that make up the prison yard area
        let yard_positions: Vec<Coord3D> = world.multi_logical_bone_position(us, &self.data.prison_yard_bone_prefix, MAX_YARD_BONES); // wire
        let yard_bones = yard_positions.len();
        // we must have at least 3 bone locations to make a yard polygon, otherwise we'll default to the object position
        if yard_bones >= 3 {
            // find the bounding region of the yard area
            let mut yard_region = Region2D::default();
            yard_region.lo.x = yard_positions[0].x;
            yard_region.lo.y = yard_positions[0].y;
            yard_region.hi.x = yard_positions[0].x;
            yard_region.hi.y = yard_positions[0].y;
            for p in yard_positions.iter().skip(1) {
                if p.x < yard_region.lo.x {
                    yard_region.lo.x = p.x;
                }
                if p.y < yard_region.lo.y {
                    yard_region.lo.y = p.y;
                }
                if p.x > yard_region.hi.x {
                    yard_region.hi.x = p.x;
                }
                if p.y > yard_region.hi.y {
                    yard_region.hi.y = p.y;
                }
            }
            // the default visual position will be in the middle of the yard region (Region2D::width() / height())
            picked_location.x = yard_region.lo.x + (yard_region.hi.x - yard_region.lo.x) / 2.0f32;
            picked_location.y = yard_region.lo.y + (yard_region.hi.y - yard_region.lo.y) / 2.0f32;
            // NOTE: pickedLocation.z is left alone at the object center Z
            // loop till we find a valid location that is inside the yard area
            let max_tries = 32;
            for _ in 0..max_tries {
                // pick a location
                let x = world.random.game_logic_random_value_real(yard_region.lo.x, yard_region.hi.x); // wire: the world's random streams
                let y = world.random.game_logic_random_value_real(yard_region.lo.y, yard_region.hi.y);
                let loc = Coord3D { x, y, z: picked_location.z };
                // must be inside the yard polygon
                if point_inside_area_2d(&loc, &yard_positions) {
                    // use this location, leave Z alone as the center of the prison
                    picked_location = loc;
                    break;
                }
            }
        }
        picked_location
    }

    /// `PrisonBehavior::addVisual(obj)`: a new drawable of the prisoner's template, coloured with the object's (night) indicator colour,
    /// placed at a picked yard location with a random orientation `[0, TWO_PI)`, its shroud-status object the prison; recorded at the
    /// list head.
    pub fn add_visual(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId) {
        // sanity
        if obj == INVALID_ID {
            return;
        }
        // create a drawable
        let draw = world.new_drawable_for_object_template(obj); // wire: TheThingFactory->newDrawable(obj->getTemplate())
        // set the color of the drawable to that of the object
        world.set_drawable_indicator_color_from_object(draw, obj); // wire: night colour when TheGlobalData->m_timeOfDay is TIME_OF_DAY_NIGHT
        // pick a location insid the prison yard
        let pos = self.pick_visual_location(world, me);
        // place drawable withing the prison yard area
        world.set_drawable_position(draw, &pos);
        let angle = world.random.game_logic_random_value_real(0.0, TWO_PI);
        world.set_drawable_orientation(draw, angle);
        let us = world.module_object(me);
        world.set_drawable_shroud_status_object(draw, us); // drawInfo->m_shroudStatusObjectID = getObject()->getID()
        // record this object/drawable pair
        self.visual_list.insert(0, PrisonVisual { object_id: obj, drawable_id: draw });
    }

    /// `PrisonBehavior::removeVisual(obj)`: finds and unlinks the object's visual, then destroys its drawable if still there.
    pub fn remove_visual(&mut self, world: &mut World, obj: ObjectId) {
        // sanity
        if obj == INVALID_ID {
            return;
        }
        let mut drawable_id = INVALID_DRAWABLE_ID;
        if let Some(pos) = self.visual_list.iter().position(|v| v.object_id == obj) {
            drawable_id = self.visual_list[pos].drawable_id;
            self.visual_list.remove(pos);
        }
        // find the drawable visual and destroy it
        world.destroy_drawable_by_id(drawable_id); // wire: findDrawableByID then destroyDrawable (a miss does nothing)
    }
}

#[cfg(any())] // not compiled: needs World/UpdateBody API that does not exist (the `// wire:` calls); upstream is under ALLOW_SURRENDER, undefined in the shipped build (#214)
impl OpenContainVirtuals for PrisonBehavior {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PrisonBehavior(self) // wire: new variant
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::PrisonBehavior(m) => Some(m),
            _ => None,
        }
    }

    /// `PrisonBehavior::onDelete()`: `OpenContain::onDelete()`, then every visual's drawable is destroyed and the list emptied.
    fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        // extend functionality
        crate::logic::modules::open_contain::base_on_delete(self, world, me); // wire
        // delete our list
        while !self.visual_list.is_empty() {
            let v = self.visual_list.remove(0);
            world.destroy_drawable_by_id(v.drawable_id);
        }
    }

    /// `PrisonBehavior::onContaining(obj, wasSelected)`: base first; the prisoner is `DISABLED_HELD`; with `ShowPrisoners` a visual is added.
    fn on_containing(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId, was_selected: bool) {
        // extend functionality
        crate::logic::modules::open_contain::base_on_containing(self, world, me, obj, was_selected); // wire
        // objects inside a prison are held
        world.set_disabled(obj, DISABLED_HELD);
        // if we show visuals, make one
        if self.data.show_prisoners {
            self.add_visual(world, me, obj);
        }
    }

    /// `PrisonBehavior::onRemoving(obj)`: with `ShowPrisoners` the visual goes; the prisoner is no longer held; then the base.
    fn on_removing(&mut self, world: &mut World, me: ModuleHandle, obj: ObjectId) {
        // if we show visuals, remove one
        if self.data.show_prisoners {
            self.remove_visual(world, obj);
        }
        // object is no longer held inside a garrisoned building
        world.clear_disabled(obj, DISABLED_HELD);
        // extend functionality
        crate::logic::modules::open_contain::base_on_removing(self, world, me, obj); // wire
    }
}

impl Snapshot for PrisonBehavior {
    /// `PrisonBehavior::crc(xfer)`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc(xfer)
    }

    /// `PrisonBehavior::xfer(xfer)`: version 1; the base; the visual count as an `UnsignedShort`; each visual's object id and drawable
    /// id. A load into a non-empty list is `SC_INVALID_DATA`; loaded visuals are pushed at the head (so the list reverses).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        // version
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        // extend base class
        self.oc.xfer(xfer)?;
        // count and data for the prison visuals
        let mut visual_count: u16 = self.visual_list.len() as u16;
        xfer.xfer_unsigned_short(&mut visual_count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            // write all data
            for v in self.visual_list.iter_mut() {
                xfer.xfer_object_id(&mut v.object_id)?;
                xfer.xfer_drawable_id(&mut v.drawable_id)?;
            }
        } else {
            // the visual list should be empty
            if !self.visual_list.is_empty() {
                // DEBUG_CRASH(( "PrisonBehavior::xfer - the visual list should be empty but is not\n" ));
                return Err(crate::common::xfer::XferStatus::ListNotEmpty); // upstream throws SC_INVALID_DATA, which XferStatus lacks; ListNotEmpty is this very case
            }
            // read each item
            for _ in 0..visual_count {
                let mut v = PrisonVisual::new();
                xfer.xfer_object_id(&mut v.object_id)?;
                xfer.xfer_drawable_id(&mut v.drawable_id)?;
                self.visual_list.insert(0, v);
            }
        }
        Ok(())
    }

    /// `PrisonBehavior::loadPostProcess()`: the base's.
    fn load_post_process(&mut self) {
        self.oc.load_post_process();
    }
}

/// `class POWTruckBehaviorModuleData : public OpenContainModuleData`: no fields of its own (its field table is just the terminator).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowTruckBehaviorModuleData {
    /// The base.
    pub open_contain: OpenContainModuleData,
}

/// `class POWTruckBehavior : public OpenContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct PowTruckBehavior {
    oc: OpenContain,
    #[allow(dead_code)]
    data: Arc<PowTruckBehaviorModuleData>,
}

impl PowTruckBehavior {
    /// `POWTruckBehavior::POWTruckBehavior(thing, moduleData)`.
    pub fn new(oc: OpenContain, data: Arc<PowTruckBehaviorModuleData>) -> PowTruckBehavior {
        PowTruckBehavior { oc, data }
    }
}

#[cfg(any())] // not compiled: needs World/UpdateBody API that does not exist (the `// wire:` calls); upstream is under ALLOW_SURRENDER, undefined in the shipped build (#214)
impl OpenContainVirtuals for PowTruckBehavior {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PowTruckBehavior(self) // wire: new variant
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::PowTruckBehavior(m) => Some(m),
            _ => None,
        }
    }

    /// `POWTruckBehavior::onCollide(other, loc, normal)`: ignores a NULL other and one whose AI is missing or not surrendered; otherwise
    /// our `POWTruckAIUpdateInterface::loadPrisoner(other)` picks the prisoner up.
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) {
        let us = world.module_object(me);
        // sanity
        if other == INVALID_ID {
            return;
        }
        // if other isn't slated to be picked up by us, ignore
        if !world.ai_is_surrendered(other) {
            // wire: otherAi == NULL || otherAi->isSurrendered() == FALSE
            return;
        }
        // get our AI info and pick up the prisoner (the DEBUG_ASSERTCRASHes for no AI / no POWTruckAI are compiled out of release)
        world.pow_truck_load_prisoner(us, other); // wire: POWTruckAIUpdate::loadPrisoner (see pow_truck_ai_update.rs)
    }
}

impl Snapshot for PowTruckBehavior {
    /// `POWTruckBehavior::crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc(xfer)
    }
    /// `POWTruckBehavior::xfer`: version 1, then the base.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        self.oc.xfer(xfer)
    }
    /// `POWTruckBehavior::loadPostProcess`: the base's.
    fn load_post_process(&mut self) {
        self.oc.load_post_process();
    }
}

