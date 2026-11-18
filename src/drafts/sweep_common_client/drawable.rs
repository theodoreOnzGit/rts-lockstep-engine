//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp   (the functions the verified `crate::client::drawable`
//!     does not have: the selection hooks, tint/selection colours, the UI overlay draws, the icon info)
//!   GeneralsMD/Code/GameEngine/Include/GameClient/Drawable.h    (`DrawableIconType`, `DrawableIconInfo`)
//!
//! The verified `Drawable` holds the model-condition state and the draw modules only. These are the members and
//! functions of the 2D overlay (health bar, pips, icons, captions, veterancy), kept in [`DrawableUi`], which a host
//! pairs with the verified drawable. Everything upstream reads through globals or the object (`TheGlobalData`,
//! `TheTacticalView`, `TheInGameUI`, `ThePlayerList`, `TheDisplay`, `TheDisplayStringManager`, the `Object`'s body,
//! contain and weapon-bonus state) is the [`DrawableUiHost`] trait. Compile-time switches in this file, as in the
//! shipped build: `SCALE_ICONS_WITH_ZOOM_ML` is NOT defined (the `#ifdef` branches are left out), `ALLOW_DEMORALIZE` is
//! not defined (so `drawDemoralized` is compiled out: not translated), `KRIS_BRUTAL_HACK_FOR_AIRCRAFT_CARRIER_DEBUGGING`
//! is commented out, `CLAMP_ICON_ZOOM_FACTOR(n)` is `(n)`, `ALLOW_ANIM_INQUIRIES` is not defined (so
//! `getAnimationScrubScalar` is compiled out).
//!
//! Excluded with reasons (listed in #213): `Drawable::validatePos` (`#ifdef _DEBUG`, debug crash only),
//! `Drawable::getAnimationScrubScalar` (`#ifdef ALLOW_ANIM_INQUIRIES`, undefined), `Drawable::drawDemoralized`
//! (`#ifdef ALLOW_DEMORALIZE`, undefined).

use super::anim2d::{game_make_color, Anim2DCollection, Anim2DHost, Anim2DId, Anim2DTemplate};

/// `LOGICFRAMES_PER_SECOND`.
pub const LOGICFRAMES_PER_SECOND: u32 = 30;
/// `HEALING_ICON_DISPLAY_TIME`.
pub const HEALING_ICON_DISPLAY_TIME: u32 = LOGICFRAMES_PER_SECOND * 3;
/// `BOMB_ICON_EXTRA_OFFSET`.
pub const BOMB_ICON_EXTRA_OFFSET: i32 = 5;
/// `FOREVER` (`0xffffffff`).
pub const FOREVER: u32 = 0xffff_ffff;
/// `NO_HOTKEY_SQUAD`.
pub const NO_HOTKEY_SQUAD: i32 = -1;
/// `NUM_HOTKEY_SQUADS`.
pub const NUM_HOTKEY_SQUADS: i32 = 10;
/// `NO_FORMATION_ID`.
pub const NO_FORMATION_ID: i32 = 0;

/// `DrawableIconType`. `ICON_DEMORALIZED_OBSOLETE` holds the fourth slot (`ALLOW_DEMORALIZE` undefined).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawableIconType {
    /// `ICON_DEFAULT_HEAL`.
    DefaultHeal = 0,
    /// `ICON_STRUCTURE_HEAL`.
    StructureHeal,
    /// `ICON_VEHICLE_HEAL`.
    VehicleHeal,
    /// `ICON_DEMORALIZED_OBSOLETE`.
    DemoralizedObsolete,
    /// `ICON_BOMB_TIMED`.
    BombTimed,
    /// `ICON_BOMB_REMOTE`.
    BombRemote,
    /// `ICON_DISABLED`.
    Disabled,
    /// `ICON_BATTLEPLAN_BOMBARD`.
    BattleplanBombard,
    /// `ICON_BATTLEPLAN_HOLDTHELINE`.
    BattleplanHoldTheLine,
    /// `ICON_BATTLEPLAN_SEARCHANDDESTROY`.
    BattleplanSearchAndDestroy,
    /// `ICON_EMOTICON`.
    Emoticon,
    /// `ICON_ENTHUSIASTIC`.
    Enthusiastic,
    /// `ICON_ENTHUSIASTIC_SUBLIMINAL`.
    EnthusiasticSubliminal,
    /// `ICON_CARBOMB`.
    Carbomb,
}

/// `MAX_ICONS`.
pub const MAX_ICONS: usize = 14;

/// `class DrawableIconInfo`: one animation per icon type and the frame each is kept until.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawableIconInfo {
    /// `m_icon[MAX_ICONS]` (ids into the Anim2D collection).
    pub icon: [Option<Anim2DId>; MAX_ICONS],
    /// `m_keepTillFrame[MAX_ICONS]`.
    pub keep_till_frame: [u32; MAX_ICONS],
}

impl DrawableIconInfo {
    /// `DrawableIconInfo::DrawableIconInfo()`: all NULL and 0.
    pub fn new() -> DrawableIconInfo {
        DrawableIconInfo { icon: [None; MAX_ICONS], keep_till_frame: [0; MAX_ICONS] }
    }

    /// `DrawableIconInfo::clear()`: delete every icon instance.
    pub fn clear(&mut self, anims: &mut Anim2DCollection) {
        for i in 0..MAX_ICONS {
            if let Some(id) = self.icon[i] {
                anims.delete_anim(id);
            }
            self.icon[i] = None;
            self.keep_till_frame[i] = 0;
        }
    }

    /// `DrawableIconInfo::killIcon(t)` (Drawable.cpp:173): delete the icon if there is one, and zero its keep-till frame.
    pub fn kill_icon(&mut self, t: DrawableIconType, anims: &mut Anim2DCollection) {
        let i = t as usize;
        if let Some(id) = self.icon[i] {
            anims.delete_anim(id);
            self.icon[i] = None;
            self.keep_till_frame[i] = 0;
        }
    }
}

/// `IRegion2D` (`lo`, `hi`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IRegion2D {
    /// `lo.x`.
    pub lo_x: i32,
    /// `lo.y`.
    pub lo_y: i32,
    /// `hi.x`.
    pub hi_x: i32,
    /// `hi.y`.
    pub hi_y: i32,
}

/// `Drawable::s_veterancyImage[]`, `s_fullAmmo`... as the sizes the draws need, plus their names for `drawImage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconImage {
    /// Image name (for the display host).
    pub name: Vec<u8>,
    /// `getImageWidth()`.
    pub width: i32,
    /// `getImageHeight()`.
    pub height: i32,
}

/// Everything a draw reads through the object and the globals, and the display calls it makes.
pub trait DrawableUiHost: Anim2DHost {
    /// `TheGlobalData->m_showObjectHealth`.
    fn show_object_health(&self) -> bool;
    /// `TheInGameUI && TheInGameUI->getMousedOverDrawableID() == getID()`.
    fn is_moused_over(&self) -> bool;
    /// `getObject() == NULL`.
    fn has_object(&self) -> bool;
    /// `obj->getControllingPlayer() == ThePlayerList->getLocalPlayer()`.
    fn is_controlled_by_local_player(&self) -> bool;
    /// `obj->isLocallyControlled()`.
    fn is_locally_controlled(&self) -> bool;
    /// `owner->getSquadNumberForObject(obj)`.
    fn squad_number_for_object(&self) -> i32;
    /// `obj->getFormationID()`.
    fn formation_id(&self) -> i32;
    /// `obj->getAmmoPipShowingInfo(numTotal, numFull)`.
    fn ammo_pip_showing_info(&self) -> Option<(i32, i32)>;
    /// `container->getContainerPipsToShow(numTotal, numFull)` for `obj->getContain()`; `None` without a container
    /// or when it shows none.
    fn container_pips_to_show(&self) -> Option<(i32, i32)>;
    /// Number of contained items that are `KINDOF_INFANTRY`.
    fn contained_infantry_count(&self) -> i32;
    /// `obj->getPosition()` plus `getGeometryInfo().getMaxHeightAbovePosition()`, `getBoundingSphereRadius()`.
    fn object_position_and_geometry(&self) -> ((f32, f32, f32), f32, f32);
    /// `TheGlobalData->m_ammoPipWorldOffset`, `m_ammoPipScreenOffset`.
    fn ammo_pip_offsets(&self) -> ((f32, f32, f32), (f32, f32));
    /// `TheGlobalData->m_containerPipWorldOffset`, `m_containerPipScreenOffset`.
    fn container_pip_offsets(&self) -> ((f32, f32, f32), (f32, f32));
    /// `TheTacticalView->worldToScreen(pos, &screen)`.
    fn world_to_screen(&self, pos: (f32, f32, f32)) -> Option<(i32, i32)>;
    /// `TheTacticalView->getZoom()`.
    fn view_zoom(&self) -> f32;
    /// `obj->getHealthBoxPosition(p)`.
    fn health_box_position(&self) -> (f32, f32, f32);
    /// `obj->getHealthBoxDimensions(h, w)`: `(height, width)`.
    fn health_box_dimensions(&self) -> Option<(f32, f32)>;
    /// `TheDisplay->drawImage(image, x0, y0, x1, y1, color)`; `color` 0xffffffff when upstream passes the default.
    fn display_draw_image(&mut self, image: &IconImage, x0: i32, y0: i32, x1: i32, y1: i32, color: u32);
    /// `TheDisplay->drawOpenRect(x, y, w, h, lineWidth, color)`.
    fn display_draw_open_rect(&mut self, x: i32, y: i32, w: i32, h: i32, line_width: f32, color: u32);
    /// `TheDisplay->drawFillRect(x, y, w, h, color)`.
    fn display_draw_fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32);
    /// `Drawable::s_fullAmmo`, `s_emptyAmmo` (`None` when the image was not found).
    fn ammo_images(&self) -> (Option<IconImage>, Option<IconImage>);
    /// `Drawable::s_fullContainer`, `s_emptyContainer`.
    fn container_images(&self) -> (Option<IconImage>, Option<IconImage>);
    /// `Drawable::s_veterancyImage[level]` for `obj->getVeterancyLevel()`; `None` also without an experience tracker.
    fn veterancy_image(&self) -> Option<IconImage>;
    /// `Drawable::s_animationTemplates[type]` as an index into the Anim2D collection's templates.
    fn animation_template(&self, t: DrawableIconType) -> Option<usize>;
    /// `obj->getPlayer()->getNumBattlePlansActive() > 0 && doesObjectQualifyForBattlePlan(obj)`.
    fn battle_plans_apply(&self) -> bool;
    /// `player->getBattlePlansActiveSpecific(plan)`: 0 bombardment, 1 hold the line, 2 search and destroy.
    fn battle_plan_active(&self, plan: i32) -> bool;
    /// Object facts for the healing icon.
    fn healing_facts(&self) -> HealingFacts;
    /// `obj->testWeaponBonusCondition(ENTHUSIASTIC)`, `(SUBLIMINAL)`.
    fn weapon_bonus_enthusiastic(&self) -> (bool, bool);
    /// `isKindOf(KINDOF_STRUCTURE)`, `KINDOF_HUGE_VEHICLE`, `KINDOF_VEHICLE`.
    fn kind_of_structure_hugevehicle_vehicle(&self) -> (bool, bool, bool);
    /// `obj->testWeaponSetFlag(WEAPONSET_CARBOMB)`.
    fn weapon_set_carbomb(&self) -> bool;
    /// The object's `StickyBombUpdate`, if any, with a target object: `(isTimedBomb, detonationFrame)`.
    fn sticky_bomb(&self) -> Option<(bool, u32)>;
    /// `obj->isDisabledByType(HACKED/PARALYZED/EMP/SUBDUED/UNDERPOWERED)`.
    fn disabled_for_icon(&self) -> bool;
    /// `UNDER_CONSTRUCTION` and `SOLD` status, `getConstructionPercent()`.
    fn construction_facts(&self) -> (bool, bool, f32);
    /// `obj->isKindOf(KINDOF_FORCEATTACKABLE)`.
    fn is_force_attackable(&self) -> bool;
    /// `body->getHealth()`, `getMaxHealth()`.
    fn health_and_max(&self) -> (f32, f32);
    /// `UNDER_CONSTRUCTION || (isDisabled() && !isDisabledByType(DISABLED_HELD))`.
    fn health_bar_blue(&self) -> bool;
    /// `m_conditionState.test(MODELCONDITION_REALLY_DAMAGED)`, `(MODELCONDITION_DAMAGED)`.
    fn model_condition_damaged(&self) -> (bool, bool);
    /// `TheGameText->fetch("CONTROLBAR:UnderConstructionDesc")` formatted with the percent, set on the display string.
    fn set_construct_text(&mut self, percent: f32);
    /// The construct / caption display strings: `(width, height)`; draw at a position.
    fn construct_text_width(&self) -> i32;
    /// `m_constructDisplayString->draw(x, y, color, dropColor)`.
    fn draw_construct_text(&mut self, x: i32, y: i32, color: u32, drop_color: u32);
    /// `m_captionDisplayString`: `(width, height)`, `None` when NULL.
    fn caption_size(&self) -> Option<(i32, i32)>;
    /// `m_captionDisplayString->draw(x, y, color, dropColor)`.
    fn draw_caption_text(&mut self, x: i32, y: i32, color: u32, drop_color: u32);
    /// `TheInGameUI->getDrawableCaptionColor()`.
    fn drawable_caption_color(&self) -> u32;
    /// `getDrawableGeometryInfo().getCenterPosition(*getPosition(), pos)`.
    fn drawable_center_position(&self) -> (f32, f32, f32);
    /// `TheDrawGroupInfo`: the group-numeral draw (`getGroupNumeralString(n)->draw(x, y, color, drop, dx, dy)`).
    fn draw_group_numeral(&mut self, group_num: i32, x: i32, y: i32, color: u32);
    /// `TheDisplayStringManager->getFormationLetterString()->draw(...)`.
    fn draw_formation_letter(&mut self, x: i32, y: i32, color: u32);
    /// `TheDrawGroupInfo` layout: `(usePlayerColor, colorForText, usingPixelOffsetX, pixelOffsetX, percentOffsetX,
    /// usingPixelOffsetY, pixelOffsetY, percentOffsetY)`.
    fn draw_group_info(&self) -> DrawGroupInfo;
    /// `owner->getPlayerColor()`.
    fn player_color(&self) -> u32;
    /// `obj->getContain()->clientVisibleContainedFlashAsSelected()` when there is a container.
    fn contain_flash_as_selected(&mut self);
    /// `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
    fn is_night(&self) -> bool;
}

/// `TheDrawGroupInfo`'s layout members.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawGroupInfo {
    /// `m_usePlayerColor`.
    pub use_player_color: bool,
    /// `m_colorForText`.
    pub color_for_text: u32,
    /// `m_usingPixelOffsetX`.
    pub using_pixel_offset_x: bool,
    /// `m_pixelOffsetX`.
    pub pixel_offset_x: i32,
    /// `m_percentOffsetX`.
    pub percent_offset_x: f32,
    /// `m_usingPixelOffsetY`.
    pub using_pixel_offset_y: bool,
    /// `m_pixelOffsetY`.
    pub pixel_offset_y: i32,
    /// `m_percentOffsetY`.
    pub percent_offset_y: f32,
}

/// What `drawHealing` reads of the object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealingFacts {
    /// `KINDOF_NO_HEAL_ICON`.
    pub no_heal_icon: bool,
    /// `OBJECT_STATUS_SOLD`.
    pub sold: bool,
    /// `body->getHealth() != body->getMaxHealth()`.
    pub health_not_max: bool,
    /// `TheGameLogic->getFrame()`.
    pub frame: u32,
    /// `body->getLastHealingTimestamp()`.
    pub last_healing_timestamp: u32,
    /// `KINDOF_STRUCTURE`.
    pub structure: bool,
    /// `KINDOF_VEHICLE`.
    pub vehicle: bool,
}

/// `REAL_TO_INT`: truncation toward zero.
fn real_to_int(r: f32) -> i32 {
    r as i32
}

/// A `Vector3` colour envelope's read side (`TintEnvelope` in `crate::client::drawable_save` holds the saved members;
/// the effect test and the colour are these two functions).
pub trait TintEnvelopeRead {
    /// `TintEnvelope::isEffective()`.
    fn is_effective(&self) -> bool;
    /// `TintEnvelope::getColor()`.
    fn get_color(&self) -> [f32; 3];
}

impl TintEnvelopeRead for crate::client::drawable_save::TintEnvelope {
    fn is_effective(&self) -> bool {
        self.affect
    }
    fn get_color(&self) -> [f32; 3] {
        self.current_color
    }
}

/// `TintEnvelope::setAttackFrames(frames)` (Drawable.cpp:5514): `m_attackRate = (m_peakColor - m_currentColor) *
/// (1 / max(1, frames))`.
pub fn tint_envelope_set_attack_frames(env: &mut crate::client::drawable_save::TintEnvelope, frames: u32) {
    let recip_frames = 1.0f32 / (frames.max(1) as f32);
    for i in 0..3 {
        env.attack_rate[i] = (env.peak_color[i] - env.current_color[i]) * recip_frames;
    }
}

/// The overlay members of `class Drawable`, and its functions.
#[derive(Clone, Debug, Default)]
pub struct DrawableUi {
    /// `m_selected`.
    pub selected: bool,
    /// `m_iconInfo` (NULL until `getIconInfo`).
    pub icon_info: Option<DrawableIconInfo>,
    /// `m_colorTintEnvelope`.
    pub color_tint_envelope: Option<crate::client::drawable_save::TintEnvelope>,
    /// `m_selectionFlashEnvelope`.
    pub selection_flash_envelope: Option<crate::client::drawable_save::TintEnvelope>,
    /// `m_lastConstructDisplayed` (-1 at construction upstream).
    pub last_construct_displayed: f32,
    /// `m_constructDisplayString != NULL`.
    pub has_construct_display_string: bool,
    /// `m_groupNumber != NULL` (a cached numeral string).
    pub group_number_set: bool,
}

impl DrawableUi {
    /// `Drawable::friend_setSelected()` (Drawable.cpp:932): if not already selected, set it and run `onSelected`.
    /// `flash_as_selected` is `Drawable::flashAsSelected` (verified elsewhere / not in this file).
    pub fn friend_set_selected<H: DrawableUiHost + ?Sized>(&mut self, host: &mut H, flash_as_selected: &mut dyn FnMut()) {
        if !self.selected {
            self.selected = true;
            self.on_selected(host, flash_as_selected);
        }
    }

    /// `Drawable::friend_clearSelected()` (Drawable.cpp:946).
    pub fn friend_clear_selected(&mut self) {
        if self.selected {
            self.selected = false;
            self.on_unselected();
        }
    }

    /// `Drawable::onSelected()` (Drawable.cpp:1010): `flashAsSelected()`, then the contain module's
    /// `clientVisibleContainedFlashAsSelected()` when the object has one.
    pub fn on_selected<H: DrawableUiHost + ?Sized>(&mut self, host: &mut H, flash_as_selected: &mut dyn FnMut()) {
        flash_as_selected();
        if host.has_object() {
            host.contain_flash_as_selected();
        }
    }

    /// `Drawable::onUnselected()` (Drawable.cpp:1030): nothing.
    pub fn on_unselected(&mut self) {}

    /// `Drawable::getTintColor()` (Drawable.cpp:1038): the tint envelope's colour when it is effective.
    pub fn get_tint_color(&self) -> Option<[f32; 3]> {
        match &self.color_tint_envelope {
            Some(e) if TintEnvelopeRead::is_effective(e) => Some(TintEnvelopeRead::get_color(e)),
            _ => None,
        }
    }

    /// `Drawable::getSelectionColor()` (Drawable.cpp:1054).
    pub fn get_selection_color(&self) -> Option<[f32; 3]> {
        match &self.selection_flash_envelope {
            Some(e) if TintEnvelopeRead::is_effective(e) => Some(TintEnvelopeRead::get_color(e)),
            _ => None,
        }
    }

    /// `Drawable::getIconInfo()` (Drawable.cpp:2816): made on first use.
    pub fn get_icon_info(&mut self) -> &mut DrawableIconInfo {
        self.icon_info.get_or_insert_with(DrawableIconInfo::new)
    }

    fn has_icon_info(&self) -> bool {
        self.icon_info.is_some()
    }

    fn kill_icon(&mut self, t: DrawableIconType, anims: &mut Anim2DCollection) {
        if let Some(info) = self.icon_info.as_mut() {
            info.kill_icon(t, anims);
        }
    }

    /// `Drawable::drawsAnyUIText()` (Drawable.cpp:2733). Upstream clears `m_groupNumber` (NULL) as a side effect when
    /// the squad number is out of range, which `group_number_set` models.
    pub fn draws_any_ui_text<H: DrawableUiHost + ?Sized>(&mut self, host: &H) -> bool {
        if !self.selected {
            return false;
        }
        if !host.has_object() || !host.is_locally_controlled() {
            return false;
        }
        let group_num = host.squad_number_for_object();
        if group_num > NO_HOTKEY_SQUAD && group_num < NUM_HOTKEY_SQUADS {
            return true;
        } else {
            self.group_number_set = false;
        }
        if host.formation_id() != NO_FORMATION_ID {
            return true;
        }
        false
    }

    /// `Drawable::clearEmoticon()` (Drawable.cpp:2824).
    pub fn clear_emoticon(&mut self, anims: &mut Anim2DCollection) {
        if !self.has_icon_info() {
            return;
        }
        self.kill_icon(DrawableIconType::Emoticon, anims);
    }

    /// `Drawable::setEmoticon(name, duration)`: translated here because `drawEmoticon` depends on it (the verified port
    /// does not have it). A duration of -1 (any negative) means FOREVER.
    pub fn set_emoticon<H: Anim2DHost + ?Sized>(&mut self, anims: &mut Anim2DCollection, host: &mut H, name: &[u8], duration: i32) {
        self.clear_emoticon(anims);
        if let Some(t) = anims.find_template(name) {
            let e = DrawableIconType::Emoticon as usize;
            if self.get_icon_info().icon[e].is_none() {
                let id = anims.new_anim(host, t, true);
                let frame = host.logic_frame();
                let info = self.get_icon_info();
                info.icon[e] = Some(id);
                info.keep_till_frame[e] = if duration >= 0 { frame.wrapping_add(duration as u32) } else { FOREVER };
            }
        }
    }

    /// `Drawable::drawEmoticon(healthBarRegion)` (Drawable.cpp:2850).
    pub fn draw_emoticon<H: Anim2DHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, anims: &mut Anim2DCollection, host: &mut H) {
        let e = DrawableIconType::Emoticon as usize;
        let id = match self.icon_info.as_ref().and_then(|i| i.icon[e]) {
            Some(id) => id,
            None => return,
        };
        let now = host.logic_frame();
        let keep = self.icon_info.as_ref().unwrap().keep_till_frame[e];
        match health_bar_region {
            Some(r) if keep >= now => {
                let bar_width = r.hi_x - r.lo_x;
                let frame_width = anims.get_current_frame_width(id) as i32;
                let frame_height = anims.get_current_frame_height(id) as i32;
                let sx = (r.lo_x as f32 + (bar_width as f32 * 0.5) - (frame_width as f32 * 0.5)) as i32;
                let sy = r.hi_y - frame_height;
                anims.draw_sized(id, sx, sy, frame_width, frame_height, host);
            }
            _ => self.clear_emoticon(anims),
        }
    }

    /// `Drawable::drawAmmo(healthBarRegion)` (Drawable.cpp:2885).
    pub fn draw_ammo<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: &IRegion2D, host: &mut H) {
        if !(host.show_object_health() && (self.selected || host.is_moused_over()) && host.is_controlled_by_local_player()) {
            return;
        }
        let (num_total, num_full) = match host.ammo_pip_showing_info() {
            Some(v) => v,
            None => return,
        };
        let (full_ammo, empty_ammo) = host.ammo_images();
        let (full_ammo, empty_ammo) = match (full_ammo, empty_ammo) {
            (Some(f), Some(e)) => (f, e),
            _ => return,
        };
        let scale = 1.0f32;
        let box_width = real_to_int(empty_ammo.width as f32 * scale);
        let box_height = real_to_int(empty_ammo.height as f32 * scale);
        const SPACING: i32 = 1;
        let (mut pos, max_h, bounding_radius) = host.object_position_and_geometry();
        let (world_off, screen_off) = host.ammo_pip_offsets();
        pos.0 += world_off.0;
        pos.1 += world_off.1;
        pos.2 += world_off.2 + max_h;
        let screen_center = match host.world_to_screen(pos) {
            Some(s) => s,
            None => return,
        };
        let bounding = bounding_radius * scale;
        // "Left justify with health bar min"
        let mut posx = health_bar_region.lo_x;
        let posy = screen_center.1 + real_to_int(screen_off.1 * bounding);
        for i in 0..num_total {
            let img = if i < num_full { &full_ammo } else { &empty_ammo };
            host.display_draw_image(img, posx, posy + 1, posx + box_width, posy + 1 + box_height, 0xffff_ffff);
            posx += box_width + SPACING;
        }
    }

    /// `Drawable::drawContained(healthBarRegion)` (Drawable.cpp:2939).
    pub fn draw_contained<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: &IRegion2D, host: &mut H) {
        // `obj->getContain()` NULL: the host's pips query gives None; the display checks follow upstream's order.
        if host.container_pips_to_show().is_none() {
            return;
        }
        if !(host.show_object_health() && (self.selected || host.is_moused_over()) && host.is_controlled_by_local_player()) {
            return;
        }
        let (num_total, num_full) = host.container_pips_to_show().unwrap();
        if num_full == 0 {
            return;
        }
        let num_infantry = host.contained_infantry_count();
        let (full_c, empty_c) = host.container_images();
        let (full_c, empty_c) = match (full_c, empty_c) {
            (Some(f), Some(e)) => (f, e),
            _ => return,
        };
        let scale = 1.0f32;
        let box_width = real_to_int(empty_c.width as f32 * scale);
        let box_height = real_to_int(empty_c.height as f32 * scale);
        const SPACING: i32 = 1;
        let (mut pos, max_h, bounding_radius) = host.object_position_and_geometry();
        let (world_off, screen_off) = host.container_pip_offsets();
        pos.0 += world_off.0;
        pos.1 += world_off.1;
        pos.2 += world_off.2 + max_h;
        let screen_center = match host.world_to_screen(pos) {
            Some(s) => s,
            None => return,
        };
        let bounding = bounding_radius * scale;
        let mut posx = health_bar_region.lo_x;
        let posy = screen_center.1 + real_to_int(screen_off.1 * bounding);
        for i in 0..num_total {
            let infantry_color = game_make_color(0, 255, 0, 255);
            let non_infantry_color = game_make_color(0, 0, 255, 255);
            if i < num_full {
                let c = if i < num_infantry { infantry_color } else { non_infantry_color };
                host.display_draw_image(&full_c, posx, posy, posx + box_width, posy + box_height, c);
            } else {
                host.display_draw_image(&empty_c, posx, posy + 1, posx + box_width, posy + 1 + box_height, 0xffff_ffff);
            }
            posx += box_width + SPACING;
        }
    }

    fn ensure_icon<H: DrawableUiHost + ?Sized>(&mut self, t: DrawableIconType, anims: &mut Anim2DCollection, host: &mut H) -> Option<Anim2DId> {
        let i = t as usize;
        if self.get_icon_info().icon[i].is_none() {
            // `newInstance(Anim2D)( s_animationTemplates[ i ], TheAnim2DCollection )`
            let tpl = host.animation_template(t)?;
            let id = anims.new_anim(host, tpl, true);
            self.get_icon_info().icon[i] = Some(id);
        }
        self.get_icon_info().icon[i]
    }

    /// `Drawable::drawBattlePlans(healthBarRegion)` (Drawable.cpp:3013).
    pub fn draw_battle_plans<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, anims: &mut Anim2DCollection, host: &mut H) {
        if !host.has_object() {
            return;
        }
        let r = match health_bar_region {
            Some(r) => *r,
            None => return,
        };
        if !host.battle_plans_apply() {
            return;
        }
        let plans = [
            (0, DrawableIconType::BattleplanBombard, 0),
            (1, DrawableIconType::BattleplanHoldTheLine, 1),
            (2, DrawableIconType::BattleplanSearchAndDestroy, 2),
        ];
        for (plan, icon, x_frames) in plans {
            if host.battle_plan_active(plan) {
                let id = match self.ensure_icon(icon, anims, host) {
                    Some(id) => id,
                    None => continue,
                };
                let frame_width = anims.get_current_frame_width(id) as i32;
                let frame_height = anims.get_current_frame_height(id) as i32;
                let sx = r.lo_x;
                let sy = r.lo_y + frame_height;
                anims.draw_sized(id, sx + frame_width * x_frames, sy, frame_width, frame_height, host);
            } else {
                self.kill_icon(icon, anims);
            }
        }
    }

    fn group_text_position(r: &IRegion2D, info: &DrawGroupInfo) -> (i32, i32) {
        let mut x = r.lo_x;
        let mut y = r.lo_y;
        let width = r.hi_x - r.lo_x;
        if info.using_pixel_offset_x {
            x += info.pixel_offset_x;
        } else {
            x += (width as f32 * info.percent_offset_x) as i32;
        }
        if info.using_pixel_offset_y {
            y += info.pixel_offset_y;
        } else {
            // upstream: `healthBarRegion->width() * m_percentOffsetY` (the width, not the height)
            y += (width as f32 * info.percent_offset_y) as i32;
        }
        (x, y)
    }

    /// `Drawable::drawUIText()` (Drawable.cpp:3115), without the `KRIS_BRUTAL_HACK...` block (commented out upstream).
    /// `health_bar_region` is `computeHealthRegion`'s result (see [`compute_health_region`]).
    pub fn draw_ui_text<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, host: &mut H) {
        let r = match health_bar_region {
            Some(r) => *r,
            None => return,
        };
        let info = host.draw_group_info();
        let group_num = host.squad_number_for_object();
        let color = if info.use_player_color { host.player_color() } else { info.color_for_text };
        if group_num > NO_HOTKEY_SQUAD && group_num < NUM_HOTKEY_SQUADS {
            let (x, y) = Self::group_text_position(&r, &info);
            self.group_number_set = true;
            host.draw_group_numeral(group_num, x, y, color);
        }
        if host.formation_id() != NO_FORMATION_ID {
            // "draw an F, here"
            let p = host.health_box_position();
            let mut screen_center = match host.world_to_screen(p) {
                Some(s) => s,
                None => return,
            };
            let (_h, w) = match host.health_box_dimensions() {
                Some(d) => d,
                None => return,
            };
            let scale = 1.3f32 / host.view_zoom();
            screen_center.0 += ((w * scale * 0.5) + 10.0) as i32;
            host.draw_formation_letter(screen_center.0, screen_center.1, color);
        }
    }

    /// `Drawable::drawHealing(healthBarRegion)` (Drawable.cpp:3236).
    pub fn draw_healing<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, anims: &mut Anim2DCollection, host: &mut H) {
        let f = host.healing_facts();
        // "we do show show icons for things that explicitly forbid it"
        if f.no_heal_icon || f.sold {
            return;
        }
        let mut show_healing = false;
        if f.health_not_max {
            // "because so many things init health early in game"
            if f.frame > HEALING_ICON_DISPLAY_TIME && f.frame.wrapping_sub(f.last_healing_timestamp) <= HEALING_ICON_DISPLAY_TIME {
                show_healing = true;
            }
        }
        let type_index = if f.structure {
            DrawableIconType::StructureHeal
        } else if f.vehicle {
            DrawableIconType::VehicleHeal
        } else {
            DrawableIconType::DefaultHeal
        };
        if show_healing {
            if let Some(r) = health_bar_region {
                if let Some(id) = self.ensure_icon(type_index, anims, host) {
                    let bar_width = r.hi_x - r.lo_x;
                    let frame_width = anims.get_current_frame_width(id) as i32;
                    let frame_height = anims.get_current_frame_height(id) as i32;
                    let sx = real_to_int(r.lo_x as f32 + (bar_width as f32 * 0.75) - (frame_width as f32 * 0.5));
                    let sy = real_to_int((r.lo_y - frame_height) as f32);
                    anims.draw_sized(id, sx, sy, frame_width, frame_height, host);
                }
            }
        } else {
            self.kill_icon(type_index, anims);
        }
    }

    /// `Drawable::drawEnthusiastic(healthBarRegion)` (Drawable.cpp:3330).
    pub fn draw_enthusiastic<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, anims: &mut Anim2DCollection, host: &mut H) {
        let (enthusiastic, subliminal) = host.weapon_bonus_enthusiastic();
        match health_bar_region {
            Some(r) if enthusiastic => {
                let icon_index = if subliminal { DrawableIconType::EnthusiasticSubliminal } else { DrawableIconType::Enthusiastic };
                if let Some(id) = self.ensure_icon(icon_index, anims, host) {
                    let bar_width = r.hi_x - r.lo_x;
                    let (structure, huge_vehicle, vehicle) = host.kind_of_structure_hugevehicle_vehicle();
                    let scale = if structure || huge_vehicle {
                        1.00f32
                    } else if vehicle {
                        0.75f32
                    } else {
                        0.5f32
                    };
                    let frame_width = (anims.get_current_frame_width(id) as f32 * scale) as i32;
                    let frame_height = (anims.get_current_frame_height(id) as f32 * scale) as i32;
                    let sx = real_to_int(r.lo_x as f32 + (bar_width as f32 * 0.25) - (frame_width as f32 * 0.5));
                    let sy = (r.hi_y as f64 + (frame_height as f64 * 0.25)) as i32;
                    anims.draw_sized(id, sx, sy, frame_width, frame_height, host);
                }
            }
            _ => {
                self.kill_icon(DrawableIconType::Enthusiastic, anims);
                self.kill_icon(DrawableIconType::EnthusiasticSubliminal, anims);
            }
        }
    }

    /// `Drawable::drawBombed(healthBarRegion)` (Drawable.cpp:3459). `seconds_per_logic_frame` is
    /// `SECONDS_PER_LOGICFRAME_REAL` (1/30).
    pub fn draw_bombed<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, anims: &mut Anim2DCollection, host: &mut H) {
        let now = host.logic_frame();
        if host.weapon_set_carbomb() && host.is_controlled_by_local_player() {
            let cb = self.ensure_icon(DrawableIconType::Carbomb, anims, host);
            if let (Some(id), Some(r)) = (cb, health_bar_region) {
                let bar_width = r.hi_x - r.lo_x;
                let bar_height = r.hi_y - r.lo_y;
                let frame_width = anims.get_current_frame_width(id) as i32;
                let frame_height = anims.get_current_frame_height(id) as i32;
                let size = real_to_int(bar_width as f32 * 0.5);
                let frame_height = real_to_int((size as f32 / frame_width as f32) * frame_height as f32);
                let frame_width = size;
                let sx = real_to_int(r.lo_x as f32 + (bar_width as f32 * 0.5) - (frame_width as f32 * 0.5));
                let sy = real_to_int(r.lo_y as f32 + bar_height as f32 * 0.5) + BOMB_ICON_EXTRA_OFFSET;
                anims.draw_sized(id, sx, sy, frame_width, frame_height, host);
                self.get_icon_info().keep_till_frame[DrawableIconType::Carbomb as usize] = FOREVER;
            }
        } else {
            self.kill_icon(DrawableIconType::Carbomb, anims);
        }

        // StickyBombUpdate (only when it has a target object)
        if let Some((is_timed, detonation_frame)) = host.sticky_bomb() {
            let bt = DrawableIconType::BombTimed as usize;
            let br = DrawableIconType::BombRemote as usize;
            if is_timed {
                if self.get_icon_info().icon[bt].is_none() {
                    let remote = self.new_icon_raw(DrawableIconType::BombRemote, anims, host);
                    let timed = self.new_icon_raw(DrawableIconType::BombTimed, anims, host);
                    self.get_icon_info().icon[br] = remote;
                    self.get_icon_info().icon[bt] = timed;
                    if let Some(tid) = timed {
                        // REAL_TO_INT_CEIL((dieFrame - now) * SECONDS_PER_LOGICFRAME_REAL)
                        let secs_f = (detonation_frame.wrapping_sub(now) as f32) * (1.0f32 / LOGICFRAMES_PER_SECOND as f32);
                        let mut seconds = secs_f.ceil() as u32;
                        let tpl = anims.instances[tid].as_ref().unwrap().template;
                        let num_frames = anims.templates[tpl].num_frames as u32;
                        if seconds > num_frames.wrapping_sub(1) {
                            seconds = num_frames.wrapping_sub(1);
                        }
                        anims.instances[tid].as_mut().unwrap().min_frame = (num_frames - seconds - 1) as u16;
                        anims.reset(tid, host);
                    }
                }
                if let (Some(tid), Some(r)) = (self.get_icon_info().icon[bt], health_bar_region) {
                    let bar_width = r.hi_x - r.lo_x;
                    let bar_height = r.hi_y - r.lo_y;
                    let frame_width = anims.get_current_frame_width(tid) as i32;
                    let frame_height = anims.get_current_frame_height(tid) as i32;
                    let size = real_to_int(bar_width as f32 * 0.65);
                    let frame_height = real_to_int((size as f32 / frame_width as f32) * frame_height as f32);
                    let frame_width = size;
                    let sx = real_to_int(r.lo_x as f32 + (bar_width as f32 * 0.5) - (frame_width as f32 * 0.5));
                    let sy = real_to_int(r.lo_y as f32 + bar_height as f32 * 0.5) + BOMB_ICON_EXTRA_OFFSET;
                    if let Some(rid) = self.get_icon_info().icon[br] {
                        anims.draw_sized(rid, sx, sy, frame_width, frame_height, host);
                    }
                    self.get_icon_info().keep_till_frame[br] = now.wrapping_add(1);
                    anims.draw_sized(tid, sx, sy, frame_width, frame_height, host);
                    self.get_icon_info().keep_till_frame[bt] = now.wrapping_add(1);
                }
            } else {
                let rid = self.ensure_icon(DrawableIconType::BombRemote, anims, host);
                if let (Some(rid), Some(r)) = (rid, health_bar_region) {
                    let bar_width = r.hi_x - r.lo_x;
                    let bar_height = r.hi_y - r.lo_y;
                    let frame_width = anims.get_current_frame_width(rid) as i32;
                    let frame_height = anims.get_current_frame_height(rid) as i32;
                    let size = real_to_int(bar_width as f32 * 0.65);
                    let frame_height = real_to_int((size as f32 / frame_width as f32) * frame_height as f32);
                    let frame_width = size;
                    let sx = real_to_int(r.lo_x as f32 + (bar_width as f32 * 0.5) - (frame_width as f32 * 0.5));
                    let sy = real_to_int(r.lo_y as f32 + bar_height as f32 * 0.5) + BOMB_ICON_EXTRA_OFFSET;
                    anims.draw_sized(rid, sx, sy, frame_width, frame_height, host);
                    self.get_icon_info().keep_till_frame[br] = now.wrapping_add(1);
                }
            }
        }

        if self.has_icon_info() {
            let bt = DrawableIconType::BombTimed as usize;
            let br = DrawableIconType::BombRemote as usize;
            if self.get_icon_info().keep_till_frame[bt] <= now {
                self.kill_icon(DrawableIconType::BombTimed, anims);
            }
            if self.get_icon_info().keep_till_frame[br] <= now {
                self.kill_icon(DrawableIconType::BombRemote, anims);
            }
        }
    }

    /// `newInstance(Anim2D)( s_animationTemplates[ t ], TheAnim2DCollection )` without storing it in the icon slot.
    fn new_icon_raw<H: DrawableUiHost + ?Sized>(&mut self, t: DrawableIconType, anims: &mut Anim2DCollection, host: &mut H) -> Option<Anim2DId> {
        let tpl = host.animation_template(t)?;
        Some(anims.new_anim(host, tpl, true))
    }

    /// `Drawable::drawDisabled(healthBarRegion)` (Drawable.cpp:3638).
    pub fn draw_disabled<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, anims: &mut Anim2DCollection, host: &mut H) {
        if host.disabled_for_icon() {
            let id = self.ensure_icon(DrawableIconType::Disabled, anims, host);
            if let (Some(id), Some(r)) = (id, health_bar_region) {
                let bar_height = r.hi_y - r.lo_y;
                let frame_width = anims.get_current_frame_width(id) as i32;
                let frame_height = anims.get_current_frame_height(id) as i32;
                let sx = r.lo_x;
                let sy = r.hi_y - (frame_height + bar_height);
                anims.draw_sized(id, sx, sy, frame_width, frame_height, host);
            }
        } else {
            self.kill_icon(DrawableIconType::Disabled, anims);
        }
    }

    /// `Drawable::drawConstructPercent(healthBarRegion)` (Drawable.cpp:3696).
    pub fn draw_construct_percent<H: DrawableUiHost + ?Sized>(&mut self, host: &mut H) {
        let (under_construction, sold, percent) = host.construction_facts();
        if !host.has_object() || !under_construction || sold {
            if self.has_construct_display_string {
                // TheDisplayStringManager->freeDisplayString( m_constructDisplayString )
                self.has_construct_display_string = false;
            }
            return;
        }
        if !self.has_construct_display_string {
            self.has_construct_display_string = true;
        }
        if self.last_construct_displayed != percent {
            host.set_construct_text(percent);
            self.last_construct_displayed = percent;
        }
        let pos = host.drawable_center_position();
        // `TheTacticalView->worldToScreen( &pos, &screen )` (its bool is not read upstream)
        let mut screen = host.world_to_screen(pos).unwrap_or((0, 0));
        if screen.0 < 1 {
            return;
        }
        let color = game_make_color(255, 255, 255, 255);
        let drop_color = game_make_color(0, 0, 0, 255);
        screen.0 -= host.construct_text_width() / 2;
        host.draw_construct_text(screen.0, screen.1, color, drop_color);
    }

    /// `Drawable::drawCaption(healthBarRegion)` (Drawable.cpp:3761).
    pub fn draw_caption<H: DrawableUiHost + ?Sized>(&mut self, host: &mut H) {
        let (width, height) = match host.caption_size() {
            Some(s) => s,
            None => return,
        };
        let pos = host.drawable_center_position();
        let mut screen = host.world_to_screen(pos).unwrap_or((0, 0));
        screen.0 -= width / 2;
        let x_pos = screen.0 - 1;
        let y_pos = screen.1 - 1;
        host.display_draw_fill_rect(x_pos, y_pos, width + 2, height + 2, game_make_color(0, 0, 0, 125));
        host.display_draw_open_rect(x_pos, y_pos, width + 2, height + 2, 1.0, game_make_color(20, 20, 20, 255));
        let color = host.drawable_caption_color();
        let drop_color = game_make_color(0, 0, 0, 255);
        host.draw_caption_text(screen.0, screen.1, color, drop_color);
    }

    /// `Drawable::drawVeterancy(healthBarRegion)` (Drawable.cpp:3797).
    pub fn draw_veterancy<H: DrawableUiHost + ?Sized>(&mut self, host: &mut H) {
        let image = match host.veterancy_image() {
            Some(i) => i,
            None => return,
        };
        let scale = 1.3f32 / host.view_zoom();
        let obj_scale = 1.0f32;
        let vet_box_width = image.width as f32 * obj_scale;
        let vet_box_height = image.height as f32 * obj_scale;
        let p = host.health_box_position();
        let mut screen_center = match host.world_to_screen(p) {
            Some(s) => s,
            None => return,
        };
        let (_h, w) = match host.health_box_dimensions() {
            Some(d) => d,
            None => return,
        };
        let _ = scale;
        screen_center.0 += (w * (1.3f32 / host.view_zoom()) * 0.5) as i32;
        host.display_draw_image(
            &image,
            screen_center.0 + 1,
            screen_center.1 + 1,
            (screen_center.0 as f32 + 1.0 + vet_box_width) as i32,
            (screen_center.1 as f32 + 1.0 + vet_box_height) as i32,
            0xffff_ffff,
        );
    }

    /// `Drawable::drawHealthBar(healthBarRegion)` (Drawable.cpp:3849).
    pub fn draw_health_bar<H: DrawableUiHost + ?Sized>(&mut self, health_bar_region: Option<&IRegion2D>, host: &mut H) {
        let r = match health_bar_region {
            Some(r) => *r,
            None => return,
        };
        if !(host.show_object_health() && (self.selected || host.is_moused_over())) {
            return;
        }
        if !host.has_object() {
            return;
        }
        if host.is_force_attackable() {
            return;
        }
        let (health, max_health) = host.health_and_max();
        if max_health == 0.0 || health == 0.0 {
            return;
        }
        let health_ratio = health / max_health;
        let (color, outline_color);
        if host.health_bar_blue() {
            // blue to cyan
            color = game_make_color(0, (health_ratio * 255.0) as u32, 255, 255);
            outline_color = game_make_color(0, (health_ratio * 128.0) as u32, 128, 255);
        } else {
            // red to green
            let (mut in_r, mut in_g, in_b): (f32, f32, f32);
            in_b = 0.0; // health bars do not display blue...
            if health_ratio >= 0.5 {
                in_r = 1.0 - ((health_ratio - 0.5) / 0.5);
                in_g = 1.0;
            } else {
                in_r = 1.0;
                in_g = 1.0 - ((0.5 - health_ratio) / 0.5);
            }
            let out_r = in_r * 0.5;
            let out_g = in_g * 0.5;
            let out_b = 0.0f32;
            let (really_damaged, damaged) = host.model_condition_damaged();
            if really_damaged {
                // average the above color with red
                in_r = (1.0 + in_r) * 0.5;
                in_g *= 0.5;
            } else if !damaged {
                // average the above color with green
                in_g = (1.0 + in_g) * 0.5;
                in_r *= 0.5;
            }
            color = game_make_color((255.0 * in_r as f64) as u32, (255.0 * in_g as f64) as u32, (255.0 * in_b as f64) as u32, 255);
            outline_color = game_make_color((255.0 * out_r as f64) as u32, (255.0 * out_g as f64) as u32, (255.0 * out_b as f64) as u32, 255);
        }
        let health_box_width = (r.hi_x - r.lo_x) as f32;
        let health_box_height = 3.0f32.max((r.hi_y - r.lo_y) as f32);
        let health_box_outline_size = 1.0f32;
        host.display_draw_open_rect(r.lo_x, r.lo_y, health_box_width as i32, health_box_height as i32, health_box_outline_size, outline_color);
        host.display_draw_fill_rect(r.lo_x + 1, r.lo_y + 1, ((health_box_width - 2.0) * health_ratio) as i32, (health_box_height - 2.0) as i32, color);
    }
}

/// `computeHealthRegion(draw, region)` (Drawable.cpp:2685, a file-static used by the draws above).
pub fn compute_health_region<H: DrawableUiHost + ?Sized>(host: &H) -> Option<IRegion2D> {
    if !host.has_object() {
        return None;
    }
    let p = host.health_box_position();
    let screen_center = host.world_to_screen(p)?;
    let (mut health_box_height, mut health_box_width) = host.health_box_dimensions()?;
    // scale the health bars according to the zoom
    let zoom = host.view_zoom();
    let width_scale = 1.0f32 / zoom;
    let height_scale = 1.0f32;
    health_box_width *= width_scale;
    health_box_height *= height_scale;
    let _ = health_box_height;
    // do this so health bar doesn't get too skinny or fat after scaling
    health_box_height = 3.0;
    let lo_x = (screen_center.0 as f32 - health_box_width * 0.45) as i32;
    let lo_y = (screen_center.1 as f32 - health_box_height * 0.5) as i32;
    let hi_x = (lo_x as f32 + health_box_width) as i32;
    let hi_y = (lo_y as f32 + health_box_height) as i32;
    Some(IRegion2D { lo_x, lo_y, hi_x, hi_y })
}

/// The members `Drawable::imitateStealthLook` copies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StealthLookState {
    /// `m_stealthOpacity`.
    pub stealth_opacity: f32,
    /// `m_explicitOpacity`.
    pub explicit_opacity: f32,
    /// `m_effectiveStealthOpacity`.
    pub effective_stealth_opacity: f32,
    /// `m_hidden`.
    pub hidden: bool,
    /// `m_hiddenByStealth`.
    pub hidden_by_stealth: bool,
    /// `m_stealthLook`.
    pub stealth_look: i32,
    /// `m_secondMaterialPassOpacity`.
    pub second_material_pass_opacity: f32,
}

/// `Drawable::imitateStealthLook(otherDraw)` (Drawable.cpp:1144). `other_effectively_hidden` is
/// `otherDraw.isDrawableEffectivelyHidden()` (it feeds both `m_hidden` and `m_hiddenByStealth`).
pub fn imitate_stealth_look(this: &mut StealthLookState, other: &StealthLookState, other_effectively_hidden: bool) {
    this.stealth_opacity = other.stealth_opacity;
    this.explicit_opacity = other.explicit_opacity;
    this.effective_stealth_opacity = other.effective_stealth_opacity;
    this.hidden = other_effectively_hidden;
    this.hidden_by_stealth = other_effectively_hidden;
    this.stealth_look = other.stealth_look;
    this.second_material_pass_opacity = other.second_material_pass_opacity;
}

/// What the draw module list calls need: `getObjectDrawInterface()` results for the three small pass-throughs.
pub trait DrawModuleList {
    /// Number of draw modules after `getDrawModules()` has applied dirty condition flags.
    fn module_count(&mut self) -> usize;
    /// `(*dm)->getObjectDrawInterface()` has an interface for module `i`.
    fn has_object_draw_interface(&self, i: usize) -> bool;
    /// `di->notifyDrawModuleDependencyCleared()` for module `i`.
    fn notify_draw_module_dependency_cleared(&mut self, i: usize);
    /// `di->clientOnly_getRenderObjInfo(pos, radius, transform)` for module `i`.
    fn client_only_get_render_obj_info(&self, i: usize) -> Option<((f32, f32, f32), f32, [[f32; 4]; 3])>;
    /// `(*dm)->onDrawableBoundToObject()` for module `i`.
    fn on_drawable_bound_to_object(&mut self, i: usize);
    /// `getModuleList(MODULETYPE_DRAW)` without applying the dirty flags (`getDrawModulesNonDirty`): the count.
    fn module_count_non_dirty(&self) -> usize;
}

/// `Drawable::clientOnly_getFirstRenderObjInfo(pos, boundingSphereRadius, transform)` (Drawable.cpp:667): only the FIRST
/// draw module (`*dm`) is asked, and only if it has an object draw interface. Client only.
pub fn client_only_get_first_render_obj_info<M: DrawModuleList + ?Sized>(m: &mut M) -> Option<((f32, f32, f32), f32, [[f32; 4]; 3])> {
    if m.module_count() == 0 {
        return None;
    }
    if m.has_object_draw_interface(0) {
        return m.client_only_get_render_obj_info(0);
    }
    None
}

/// `Drawable::notifyDrawableDependencyCleared()` (Drawable.cpp:4703): every draw module with an object draw
/// interface is told.
pub fn notify_drawable_dependency_cleared<M: DrawModuleList + ?Sized>(m: &mut M) {
    let n = m.module_count();
    for i in 0..n {
        if m.has_object_draw_interface(i) {
            m.notify_draw_module_dependency_cleared(i);
        }
    }
}

/// `Drawable::getDrawModulesNonDirty()` (Drawable.cpp:3978): the module list WITHOUT applying the dirty condition
/// flags (the verified `get_draw_modules` is the one that does); the count is what the list's NULL terminator gives.
pub fn get_draw_modules_non_dirty<M: DrawModuleList + ?Sized>(m: &M) -> usize {
    m.module_count_non_dirty()
}

/// What `Drawable::friend_bindToObject` sets from the object.
pub trait BindToObjectHost {
    /// `getObject()` is not NULL.
    fn has_object(&self) -> bool;
    /// `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
    fn is_night(&self) -> bool;
    /// `setIndicatorColor(getObject()->getNightIndicatorColor())`.
    fn set_indicator_color_night(&mut self);
    /// `setIndicatorColor(getObject()->getIndicatorColor())`.
    fn set_indicator_color_day(&mut self);
    /// `getObject()->isKindOf(KINDOF_FS_FAKE)`.
    fn is_fs_fake(&self) -> bool;
    /// `ThePlayerList->getLocalPlayer()->getRelationship(getObject()->getTeam())` is `ALLIES` or `NEUTRAL`.
    fn local_relationship_allies_or_neutral(&self) -> bool;
    /// `setTerrainDecal(TERRAIN_DECAL_SHADOW_TEXTURE)`.
    fn set_terrain_decal_shadow_texture(&mut self);
    /// `setTerrainDecal(TERRAIN_DECAL_NONE)`.
    fn set_terrain_decal_none(&mut self);
}

/// `Drawable::friend_bindToObject(obj)` (Drawable.cpp:4162): the object's indicator colour (night or day), the fake
/// faction-structure shadow decal by relationship, then every draw module's `onDrawableBoundToObject()`.
pub fn friend_bind_to_object<B: BindToObjectHost + ?Sized, M: DrawModuleList + ?Sized>(b: &mut B, m: &mut M) {
    if b.has_object() {
        if b.is_night() {
            b.set_indicator_color_night();
        } else {
            b.set_indicator_color_day();
        }
        if b.is_fs_fake() {
            if b.local_relationship_allies_or_neutral() {
                b.set_terrain_decal_shadow_texture();
            } else {
                b.set_terrain_decal_none();
            }
        }
    }
    let n = m.module_count();
    for i in 0..n {
        m.on_drawable_bound_to_object(i);
    }
}

/// Used only so `Anim2DTemplate` is referenced by this file's docs.
#[allow(dead_code)]
fn _template_type_marker(_: &Anim2DTemplate) {}
