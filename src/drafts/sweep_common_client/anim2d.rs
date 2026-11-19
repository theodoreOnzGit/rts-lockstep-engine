//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 programs (test: tests/differential_draft_anim2d.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/System/Anim2D.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Anim2D.h
//!
//! `Anim2DTemplate`, `Anim2D` and `Anim2DCollection`. Nothing of Anim2D was in the verified port beyond the saved
//! members (`crate::client::drawable_save::Anim2DSave`), so the whole file is drafted here. Differences of form:
//! - the collection owns the instances in an arena (`Anim2DId`); upstream's `m_collectionSystemNext/Prev` intrusive list
//!   is the order vector `instance_order` (head first, as `registerAnimation` pushes at the head);
//! - the template list is a vector with the newest at the head (`newTemplate` links at the head);
//! - `TheGameLogic->getFrame()`, `TheDisplay->drawImage`, `GameClientRandomValue` and the mapped image collection are
//!   the [`Anim2DHost`] trait;
//! - an INI `throw INI_INVALID_DATA` is `Err(IniErrorKind::InvalidData)`.
//! The `parseAnim2DDefinition` field table's parse functions are the `parse_*` fns below.

use crate::common::ini::IniErrorKind;
use crate::logic::modules::plant;

/// `Anim2DMode` (`ANIM_2D_INVALID` = 0 ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anim2DMode {
    /// `ANIM_2D_INVALID`.
    Invalid = 0,
    /// `ANIM_2D_ONCE`.
    Once,
    /// `ANIM_2D_ONCE_BACKWARDS`.
    OnceBackwards,
    /// `ANIM_2D_LOOP`.
    Loop,
    /// `ANIM_2D_LOOP_BACKWARDS`.
    LoopBackwards,
    /// `ANIM_2D_PING_PONG`.
    PingPong,
    /// `ANIM_2D_PING_PONG_BACKWARDS`.
    PingPongBackwards,
}

/// `Anim2DModeNames` (`INI::parseIndexList` over this list).
pub const ANIM2D_MODE_NAMES: [&str; 7] = ["NONE", "ONCE", "ONCE_BACKWARDS", "LOOP", "LOOP_BACKWARDS", "PING_PONG", "PING_PONG_BACKWARDS"];

/// `ANIM_2D_STATUS_NONE`.
pub const ANIM_2D_STATUS_NONE: u8 = 0x00;
/// `ANIM_2D_STATUS_FROZEN`.
pub const ANIM_2D_STATUS_FROZEN: u8 = 0x01;
/// `ANIM_2D_STATUS_REVERSED`: ping-pong direction tracking.
pub const ANIM_2D_STATUS_REVERSED: u8 = 0x02;
/// `ANIM_2D_STATUS_COMPLETE`: uni-directional animations at their last frame.
pub const ANIM_2D_STATUS_COMPLETE: u8 = 0x04;

/// `Anim2DTemplate::NUM_FRAMES_INVALID`.
pub const NUM_FRAMES_INVALID: u16 = 0;

/// An `const Image*` as the animation needs it: name and natural size (`getImageWidth` / `getImageHeight`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimImage {
    /// `Image::getName()`.
    pub name: Vec<u8>,
    /// `getImageWidth()`.
    pub width: i32,
    /// `getImageHeight()`.
    pub height: i32,
}

/// What the animations reach through globals.
pub trait Anim2DHost {
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
    /// `GameClientRandomValue(lo, hi)`.
    fn game_client_random_value(&mut self, lo: i32, hi: i32) -> i32;
    /// `TheMappedImageCollection->findImageByName(name)`.
    fn find_image_by_name(&self, name: &[u8]) -> Option<AnimImage>;
    /// `TheDisplay->drawImage(image, x0, y0, x1, y1, color)` with `GameMakeColor(255, 255, 255, 255 * alpha)`.
    fn draw_image(&mut self, image: &AnimImage, x0: i32, y0: i32, x1: i32, y1: i32, color: u32);
}

/// `GameMakeColor(r, g, b, a)`.
pub fn game_make_color(r: u32, g: u32, b: u32, a: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

/// `GameMakeColor( 255, 255, 255, 255 * m_alpha )`: the product is a `Real`, converted to the `UnsignedByte` the function
/// takes (truncated toward zero; defined for `0 <= 255 * alpha < 256`).
fn alpha_color(alpha: f32) -> u32 {
    let a = 255.0 * alpha;
    let a = if plant(14272) { (a + 0.5) as u32 } else { a as u32 };
    game_make_color(255, 255, 255, a)
}

/// `class Anim2DTemplate`.
#[derive(Clone, Debug)]
pub struct Anim2DTemplate {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_images`: `numFrames` slots, `None` for NULL (empty until `allocateImages`).
    pub images: Vec<Option<AnimImage>>,
    /// `m_numFrames` (`NUM_FRAMES_INVALID` until `allocateImages`).
    pub num_frames: u16,
    /// `m_framesBetweenUpdates`.
    pub frames_between_updates: u16,
    /// `m_animMode`.
    pub anim_mode: Anim2DMode,
    /// `m_randomizeStartFrame`.
    pub randomize_start_frame: bool,
}

impl Anim2DTemplate {
    /// `Anim2DTemplate::Anim2DTemplate(name)`.
    pub fn new(name: &[u8]) -> Anim2DTemplate {
        Anim2DTemplate {
            name: name.to_vec(),
            images: Vec::new(),
            num_frames: NUM_FRAMES_INVALID,
            frames_between_updates: 0,
            anim_mode: Anim2DMode::Loop,
            randomize_start_frame: false,
        }
    }

    /// `Anim2DTemplate::parseNumImages` (Anim2D.cpp:99): the count, which must be at least 1 (`minimumFrames`), then
    /// `allocateImages`. `num_frames` is the already parsed unsigned integer.
    pub fn parse_num_images(&mut self, num_frames: u32) -> Result<(), IniErrorKind> {
        let minimum_frames = if plant(14270) { 0 } else { 1 };
        if num_frames < minimum_frames {
            // upstream: DEBUG_CRASH("Invalid animation '%s', animations must have '%d' or more frames defined")
            return Err(IniErrorKind::InvalidData);
        }
        self.allocate_images(num_frames as u16);
        Ok(())
    }

    /// `Anim2DTemplate::allocateImages` (Anim2D.cpp:128): store the count and `numFrames` NULL slots.
    pub fn allocate_images(&mut self, num_frames: u16) {
        self.num_frames = num_frames;
        self.images = vec![None; num_frames as usize];
    }

    /// `Anim2DTemplate::parseImage` (Anim2D.cpp:146): the parsed mapped image (`None` when not found, which upstream
    /// tolerates) goes to the next free slot.
    pub fn parse_image(&mut self, image: Option<AnimImage>) -> Result<(), IniErrorKind> {
        self.store_image(image)
    }

    /// `Anim2DTemplate::parseImageSequence`: the number of frames must already be known; images
    /// `<base>000`, `<base>001` ... are looked up and stored; one that is not found is `INI_INVALID_DATA`.
    pub fn parse_image_sequence<H: Anim2DHost + ?Sized>(&mut self, host: &H, image_base_name: &[u8]) -> Result<(), IniErrorKind> {
        if self.num_frames == NUM_FRAMES_INVALID {
            return Err(IniErrorKind::InvalidData);
        }
        for i in 0..self.num_frames as i32 {
            let mut image_name = image_base_name.to_vec();
            image_name.extend_from_slice(format!("{:03}", i).as_bytes());
            let image = host.find_image_by_name(&image_name);
            if image.is_none() {
                return Err(IniErrorKind::InvalidData);
            }
            self.store_image(image)?;
        }
        Ok(())
    }

    /// `Anim2DTemplate::storeImage`: NULL is ignored; else the first empty slot; none left is `INI_INVALID_DATA`.
    pub fn store_image(&mut self, image: Option<AnimImage>) -> Result<(), IniErrorKind> {
        let image = match image {
            None => return Ok(()),
            Some(i) => i,
        };
        for slot in self.images.iter_mut().take(self.num_frames as usize) {
            if slot.is_none() {
                *slot = Some(image);
                return Ok(());
            }
        }
        Err(IniErrorKind::InvalidData)
    }

    /// `Anim2DTemplate::getFrame(frameNumber)`: NULL for a frame outside `[0, m_numFrames)`.
    pub fn get_frame(&self, frame_number: u16) -> Option<&AnimImage> {
        if frame_number >= self.num_frames {
            return None;
        }
        self.images.get(frame_number as usize).and_then(|i| i.as_ref())
    }
}

/// Index of an instance in the collection's arena.
pub type Anim2DId = usize;

/// `class Anim2D`.
#[derive(Clone, Debug)]
pub struct Anim2D {
    /// `m_currentFrame`.
    pub current_frame: u16,
    /// `m_lastUpdateFrame`.
    pub last_update_frame: u32,
    /// `m_template`: index into the collection's templates (by name here, since the vector reorders nothing).
    pub template: usize,
    /// `m_status`.
    pub status: u8,
    /// `m_minFrame`.
    pub min_frame: u16,
    /// `m_maxFrame`.
    pub max_frame: u16,
    /// `m_framesBetweenUpdates`.
    pub frames_between_updates: u32,
    /// `m_alpha`.
    pub alpha: f32,
    /// `m_collectionSystem != NULL`.
    pub registered: bool,
}

/// `class Anim2DCollection`.
#[derive(Clone, Debug, Default)]
pub struct Anim2DCollection {
    /// `m_templateList` as a vector; `newTemplate` links at the head, so the head is the LAST element here.
    pub templates: Vec<Anim2DTemplate>,
    /// The instance arena (`None`: freed).
    pub instances: Vec<Option<Anim2D>>,
    /// `m_instanceList`: ids from head to tail.
    pub instance_order: Vec<Anim2DId>,
}

impl Anim2DCollection {
    /// `Anim2DCollection::Anim2DCollection()`.
    pub fn new() -> Anim2DCollection {
        Anim2DCollection::default()
    }

    /// `Anim2DCollection::init()`: `ini.load("Data\\INI\\Animation2D.ini", INI_LOAD_OVERWRITE, NULL)` — the INI loader is
    /// the caller's (`crate::common::ini`); nothing else happens here.
    pub fn init(&mut self) {}

    /// `Anim2DCollection::update()`: every registered instance, head to tail, that is not frozen tries its next frame.
    pub fn update<H: Anim2DHost + ?Sized>(&mut self, host: &H) {
        let order = self.instance_order.clone();
        for id in order {
            let frozen = match self.instances.get(id).and_then(|a| a.as_ref()) {
                Some(a) => a.status & ANIM_2D_STATUS_FROZEN != 0,
                None => continue,
            };
            if !frozen {
                self.try_next_frame(id, host);
            }
        }
    }

    /// `Anim2DCollection::findTemplate(name)`: head to tail, so the newest definition of a name wins.
    pub fn find_template(&self, name: &[u8]) -> Option<usize> {
        if plant(14269) {
            (0..self.templates.len()).find(|&i| self.templates[i].name == name)
        } else {
            (0..self.templates.len()).rev().find(|&i| self.templates[i].name == name)
        }
    }

    /// `Anim2DCollection::getNextTemplate(animTemplate)` (Anim2D.cpp:808): `friend_getNextTemplate()`, the next one
    /// toward the tail; `None` for NULL input or at the end.
    pub fn get_next_template(&self, anim_template: Option<usize>) -> Option<usize> {
        match anim_template {
            Some(i) if i > 0 => Some(i - 1),
            _ => None,
        }
    }

    /// `Anim2DCollection::newTemplate(name)`: a new template linked at the head.
    pub fn new_template(&mut self, name: &[u8]) -> usize {
        self.templates.push(Anim2DTemplate::new(name));
        self.templates.len() - 1
    }

    /// `Anim2DCollection::registerAnimation(anim)` (Anim2D.cpp:839): link at the head of the instance list.
    pub fn register_animation(&mut self, id: Anim2DId) {
        if self.instances.get(id).and_then(|a| a.as_ref()).is_none() {
            return;
        }
        // upstream asserts the instance is not already in a list
        self.instance_order.insert(0, id);
    }

    /// `Anim2DCollection::unRegisterAnimation(anim)` (Anim2D.cpp:863): unlink; nothing if it is not registered here.
    pub fn un_register_animation(&mut self, id: Anim2DId) {
        match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) if a.registered => {}
            _ => return,
        }
        self.instance_order.retain(|&i| i != id);
    }

    /// `Anim2D::Anim2D(animTemplate, collectionSystem)`: returns the new instance's id.
    pub fn new_anim<H: Anim2DHost + ?Sized>(&mut self, host: &mut H, template: usize, register: bool) -> Anim2DId {
        let t = &self.templates[template];
        let mut a = Anim2D {
            current_frame: 0,
            template,
            status: ANIM_2D_STATUS_NONE,
            alpha: 1.0,
            min_frame: 0,
            max_frame: 0,
            frames_between_updates: 0,
            last_update_frame: 0,
            registered: false,
        };
        let randomize = t.randomize_start_frame;
        let num_frames = t.num_frames;
        let between = t.frames_between_updates as u32;
        self.instances.push(Some(a.clone()));
        let id = self.instances.len() - 1;
        // `randomizeCurrentFrame()` / `reset()` run before `m_minFrame` / `m_maxFrame` are set, as upstream orders it.
        if randomize {
            self.randomize_current_frame(id, host);
        } else {
            self.reset(id, host);
        }
        a = self.instances[id].take().unwrap();
        a.min_frame = 0;
        a.max_frame = num_frames.wrapping_sub(1);
        a.frames_between_updates = between;
        a.last_update_frame = 0;
        a.registered = register;
        self.instances[id] = Some(a);
        if register {
            self.register_animation(id);
        }
        id
    }

    /// `Anim2D::~Anim2D()`: unregister from the collection.
    pub fn delete_anim(&mut self, id: Anim2DId) {
        self.un_register_animation(id);
        if let Some(slot) = self.instances.get_mut(id) {
            *slot = None;
        }
    }

    /// `Anim2D::setCurrentFrame(frame)` (Anim2D.cpp:363): set the frame and stamp the logic frame.
    pub fn set_current_frame<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, frame: u16, host: &H) {
        if let Some(Some(a)) = self.instances.get_mut(id) {
            a.current_frame = frame;
            a.last_update_frame = host.logic_frame();
        }
    }

    /// `Anim2D::randomizeCurrentFrame()` (Anim2D.cpp:390): `GameClientRandomValue(0, numFrames - 1)`.
    pub fn randomize_current_frame<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, host: &mut H) {
        let n = match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) => self.templates[a.template].num_frames as i32,
            None => return,
        };
        let f = host.game_client_random_value(0, if plant(14268) { n } else { n - 1 });
        self.set_current_frame(id, f as u16, host);
    }

    /// `Anim2D::reset()`: to `m_minFrame` for the forward modes, `m_maxFrame` for the backward ones.
    pub fn reset<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, host: &H) {
        let (mode, min, max) = match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) => (self.templates[a.template].anim_mode, a.min_frame, a.max_frame),
            None => return,
        };
        match mode {
            Anim2DMode::Once | Anim2DMode::Loop | Anim2DMode::PingPong => self.set_current_frame(id, min, host),
            Anim2DMode::OnceBackwards | Anim2DMode::LoopBackwards | Anim2DMode::PingPongBackwards => self.set_current_frame(id, if plant(14266) { min } else { max }, host),
            Anim2DMode::Invalid => {} // upstream: DEBUG_CRASH("Unknown animation mode")
        }
    }

    /// `Anim2D::tryNextFrame()` (Anim2D.cpp:441).
    pub fn try_next_frame<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, host: &H) {
        let (mode, cur, min, max, between, last, status) = match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) => (self.templates[a.template].anim_mode, a.current_frame, a.min_frame, a.max_frame, a.frames_between_updates, a.last_update_frame, a.status),
            None => return,
        };
        if if plant(14264) { host.logic_frame().wrapping_sub(last) <= between } else { host.logic_frame().wrapping_sub(last) < between } {
            return;
        }
        match mode {
            Anim2DMode::Once => {
                if if plant(14261) { cur <= max } else { cur < max } {
                    self.set_current_frame(id, cur.wrapping_add(1), host);
                } else {
                    self.set_status(id, ANIM_2D_STATUS_COMPLETE);
                }
            }
            Anim2DMode::OnceBackwards => {
                if cur > min {
                    self.set_current_frame(id, cur.wrapping_sub(1), host);
                } else {
                    self.set_status(id, ANIM_2D_STATUS_COMPLETE);
                }
            }
            Anim2DMode::Loop => {
                if cur == max {
                    self.set_current_frame(id, if plant(14265) { 0 } else { min }, host);
                } else {
                    self.set_current_frame(id, cur.wrapping_add(1), host);
                }
            }
            Anim2DMode::LoopBackwards => {
                if if plant(14262) { cur >= min } else { cur > min } {
                    self.set_current_frame(id, cur.wrapping_sub(1), host);
                } else {
                    self.set_current_frame(id, max, host);
                }
            }
            Anim2DMode::PingPong | Anim2DMode::PingPongBackwards => {
                if status & ANIM_2D_STATUS_REVERSED != 0 {
                    if if plant(14263) { cur <= min } else { cur == min } {
                        self.set_current_frame(id, cur.wrapping_add(1), host);
                        self.clear_status(id, ANIM_2D_STATUS_REVERSED);
                    } else {
                        self.set_current_frame(id, cur.wrapping_sub(1), host);
                    }
                } else if cur == max {
                    self.set_current_frame(id, cur.wrapping_sub(1), host);
                    self.set_status(id, ANIM_2D_STATUS_REVERSED);
                } else {
                    self.set_current_frame(id, cur.wrapping_add(1), host);
                }
            }
            Anim2DMode::Invalid => {} // upstream: DEBUG_CRASH("Unknown animation mode")
        }
    }

    /// `Anim2D::setStatus(statusBits)`.
    pub fn set_status(&mut self, id: Anim2DId, bits: u8) {
        if let Some(Some(a)) = self.instances.get_mut(id) {
            a.status |= bits;
        }
    }

    /// `Anim2D::clearStatus(statusBits)`.
    pub fn clear_status(&mut self, id: Anim2DId, bits: u8) {
        if let Some(Some(a)) = self.instances.get_mut(id) {
            a.status &= !bits;
        }
    }

    /// `Anim2D::getCurrentFrameWidth()` (Anim2D.cpp:598): the current frame image's width, 0 without one.
    pub fn get_current_frame_width(&self, id: Anim2DId) -> u32 {
        self.current_image(id).map(|i| i.width as u32).unwrap_or(0)
    }

    /// `Anim2D::getCurrentFrameHeight()` (Anim2D.cpp:612).
    pub fn get_current_frame_height(&self, id: Anim2DId) -> u32 {
        self.current_image(id).map(|i| i.height as u32).unwrap_or(0)
    }

    fn current_image(&self, id: Anim2DId) -> Option<&AnimImage> {
        let a = self.instances.get(id)?.as_ref()?;
        self.templates[a.template].get_frame(a.current_frame)
    }

    /// `Anim2D::draw(x, y)`: the image at its natural size, tinted white with `alpha`; an unregistered, unfrozen
    /// animation then tries its next frame.
    pub fn draw<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, x: i32, y: i32, host: &mut H) {
        let (image, alpha) = match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) => (self.current_image(id).cloned(), a.alpha),
            None => return,
        };
        if let Some(image) = image {
            let color = alpha_color(alpha);
            host.draw_image(&image, x, y, x.wrapping_add(image.width), y.wrapping_add(image.height), color);
        }
        self.after_draw(id, host);
    }

    /// `Anim2D::draw(x, y, width, height)`: forced size.
    pub fn draw_sized<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, x: i32, y: i32, width: i32, height: i32, host: &mut H) {
        let (image, alpha) = match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) => (self.current_image(id).cloned(), a.alpha),
            None => return,
        };
        if let Some(image) = image {
            let color = alpha_color(alpha);
            host.draw_image(&image, x, y, x.wrapping_add(width), y.wrapping_add(if plant(14267) { width } else { height }), color);
        }
        self.after_draw(id, host);
    }

    /// The tail of both `draw`s: `m_collectionSystem == NULL && !FROZEN` -> `tryNextFrame()`.
    fn after_draw<H: Anim2DHost + ?Sized>(&mut self, id: Anim2DId, host: &H) {
        let (registered, frozen) = match self.instances.get(id).and_then(|a| a.as_ref()) {
            Some(a) => (a.registered, a.status & ANIM_2D_STATUS_FROZEN != 0),
            None => return,
        };
        if (!registered || plant(14271)) && !frozen {
            self.try_next_frame(id, host);
        }
    }
}
