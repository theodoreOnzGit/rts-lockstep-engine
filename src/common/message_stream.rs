// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/MessageStream.h
//   GeneralsMD/Code/GameEngine/Source/Common/MessageStream.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameMessage`, `GameMessageList`, `MessageStream` and `CommandList`: the messages the client's translators turn
//! input into, and the list of commands the logic executes each frame (layer 12).
//!
//! # Shape
//!
//! Upstream's messages are pool objects linked into intrusive doubly linked lists, and a message knows the list it is
//! on (its destructor removes it from that list). Here the messages live in a [`Messages`] arena and are named by a
//! [`MessageId`] (an index and a generation, so a freed message is detected, never reused); the lists are
//! [`ListId`]s into the same arena, and every link is upstream's, written in the order upstream writes it. A list
//! operation on a message that is on another list does what upstream's does (it rewrites the links and leaves the
//! other list pointing at it); using a message after it was freed is [`Undefined::UseAfterFree`].
//!
//! The arena makes [`Messages::STREAM`] (`TheMessageStream`) and [`Messages::COMMAND_LIST`] (`TheCommandList`); more
//! lists can be made with [`Messages::new_list`].
//!
//! # Arguments
//!
//! An argument is upstream's union, kept as its four words ([`ArgumentData`]): `getArgument(i)->integer` of an
//! argument appended as a location reads the location's `x` bits, as upstream's union does. A new argument is the
//! pool's zero-filled block (`allocateBlockImplementation` zeroes), so the words a smaller member does not write are
//! zero. `m_argCount` is an `UnsignedByte`: a 256th argument wraps the count to 0 (the arguments stay on the list),
//! after which `getArgumentDataType` answers `ARGUMENTDATATYPE_UNKNOWN` for every index.
//!
//! # Translators
//!
//! `MessageStream`'s translators are the client's (`CommandXlat`, `SelectionXlat`, the hot keys, ...). The list
//! mechanics are translated over the caller's translator type ([`Translators`]): `attachTranslator`'s priority order
//! (equal priorities keep their attach order), `findTranslator`, `removeTranslator`, and `propagateMessages`, which
//! hands each message to each translator through a closure. A translator may append messages to the stream while it
//! runs (they reach the later translators and itself), as upstream's do; it may not attach or remove translators
//! during propagation. `isInvalidDebugCommand` is compiled only in `_DEBUG`/`_INTERNAL` builds and is not translated.
//!
//! # Not translated
//!
//! `SubsystemInterface`'s `init`/`update`, which do nothing for these lists; `buildRegion` is [`build_region`].

use crate::common::base_type::{Coord3D, ICoord2D, IRegion2D};
use crate::common::ini::{IniError, Undefined};
use crate::logic::modules::plant;
pub use crate::common::message_types::*;

/// `GameMessage::Type` (an `enum`; a value read from a replay or the network can be any `Int`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageType(pub i32);

impl MessageType {
    /// `GameMessage::getCommandTypeAsAsciiString(t)`: the name for a value its `CHECK_IF` list names,
    /// `"Invalid command"` for a value at or above `MSG_COUNT`, `"UnknownMessage"` otherwise.
    pub fn command_string(self) -> &'static str {
        if !NOT_NAMED_BY_COMMAND_STRING.contains(&self) {
            if let Some((name, _)) = MESSAGE_TYPES.iter().find(|(_, t)| *t == self) {
                return name;
            }
        }
        if self >= MSG_COUNT && !plant(3937) {
            "Invalid command"
        } else {
            "UnknownMessage"
        }
    }
}

/// `enum GameMessageArgumentDataType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ArgumentDataType {
    Integer = 0,
    Real = 1,
    Boolean = 2,
    ObjectId = 3,
    DrawableId = 4,
    TeamId = 5,
    Location = 6,
    Pixel = 7,
    PixelRegion = 8,
    Timestamp = 9,
    WideChar = 10,
    Unknown = 11,
}

impl ArgumentDataType {
    /// The enumeration value for `v`, or `None` outside it.
    pub fn from_i32(v: i32) -> Option<ArgumentDataType> {
        use ArgumentDataType::*;
        Some(match v {
            0 => Integer,
            1 => Real,
            2 => Boolean,
            3 => ObjectId,
            4 => DrawableId,
            5 => TeamId,
            6 => Location,
            7 => Pixel,
            8 => PixelRegion,
            9 => Timestamp,
            10 => WideChar,
            11 => Unknown,
            _ => return None,
        })
    }
}

/// `union GameMessageArgumentType`: its sixteen bytes as four little-endian words (the largest member is
/// `IRegion2D`). Each accessor reads its member's bytes, as a read of that member of the union does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArgumentData(pub [u32; 4]);

impl ArgumentData {
    /// `.integer`.
    pub fn integer(&self) -> i32 {
        self.0[0] as i32
    }
    /// `.real`.
    pub fn real(&self) -> f32 {
        f32::from_bits(self.0[0])
    }
    /// `.boolean` (a one-byte `bool`: the low byte of the first word, read as nonzero).
    pub fn boolean(&self) -> bool {
        self.0[0] & 0xff != 0
    }
    /// `.objectID`.
    pub fn object_id(&self) -> u32 {
        self.0[0]
    }
    /// `.drawableID`.
    pub fn drawable_id(&self) -> u32 {
        self.0[0]
    }
    /// `.teamID` (also `.squadID`).
    pub fn team_id(&self) -> u32 {
        self.0[0]
    }
    /// `.location`.
    pub fn location(&self) -> Coord3D {
        Coord3D { x: f32::from_bits(self.0[0]), y: f32::from_bits(self.0[1]), z: f32::from_bits(self.0[2]) }
    }
    /// `.pixel`.
    pub fn pixel(&self) -> ICoord2D {
        ICoord2D { x: self.0[0] as i32, y: self.0[1] as i32 }
    }
    /// `.pixelRegion`.
    pub fn pixel_region(&self) -> IRegion2D {
        IRegion2D {
            lo: ICoord2D { x: self.0[0] as i32, y: self.0[1] as i32 },
            hi: ICoord2D { x: self.0[2] as i32, y: self.0[3] as i32 },
        }
    }
    /// `.timestamp`.
    pub fn timestamp(&self) -> u32 {
        self.0[0]
    }
    /// `.wChar` (a two-byte `WideChar`).
    pub fn wide_char(&self) -> u16 {
        self.0[0] as u16
    }
}

/// `GameMessageArgument`: the data and its type (`m_next` is the order of [`GameMessage::arguments`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameMessageArgument {
    /// `m_data`.
    pub data: ArgumentData,
    /// `m_type`.
    pub ty: ArgumentDataType,
}

/// A message's handle in [`Messages`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MessageId {
    index: u32,
    generation: u32,
}

/// A list's handle in [`Messages`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListId(pub u32);

/// `GameMessage`.
#[derive(Clone, Debug, PartialEq)]
pub struct GameMessage {
    next: Option<MessageId>,
    prev: Option<MessageId>,
    list: Option<ListId>,
    ty: MessageType,
    player_index: i32,
    arg_count: u8,
    args: Vec<GameMessageArgument>,
}

impl GameMessage {
    /// `getType()`.
    pub fn get_type(&self) -> MessageType {
        self.ty
    }
    /// `getArgumentCount()` (an `UnsignedByte`: wraps at 256).
    pub fn get_argument_count(&self) -> u8 {
        self.arg_count
    }
    /// `getPlayerIndex()`.
    pub fn get_player_index(&self) -> i32 {
        self.player_index
    }
    /// `friend_setPlayerIndex(i)`.
    pub fn set_player_index(&mut self, i: i32) {
        self.player_index = i;
    }
    /// `next()`.
    pub fn next(&self) -> Option<MessageId> {
        self.next
    }
    /// `prev()`.
    pub fn prev(&self) -> Option<MessageId> {
        self.prev
    }
    /// `m_list`.
    pub fn list(&self) -> Option<ListId> {
        self.list
    }
    /// The argument list in order (`m_argList` through `m_next`), however many were appended.
    pub fn arguments(&self) -> &[GameMessageArgument] {
        &self.args
    }
    /// `getCommandAsAsciiString()`.
    pub fn command_string(&self) -> &'static str {
        self.ty.command_string()
    }

    /// `getArgument(argIndex)`: the argument's union, or the all-zero `junk` when there is none at that index (a
    /// `DEBUG_CRASH` upstream, nothing in a release build).
    pub fn get_argument(&self, arg_index: i32) -> ArgumentData {
        if arg_index < 0 {
            return ArgumentData::default();
        }
        self.args.get(arg_index as usize).map(|a| a.data).unwrap_or_default()
    }

    /// `getArgumentDataType(argIndex)`: `UNKNOWN` at or past `m_argCount`; a negative index is the first argument's
    /// type (the walk stops before it starts).
    pub fn get_argument_data_type(&self, arg_index: i32) -> ArgumentDataType {
        if (arg_index >= i32::from(self.arg_count) && !plant(3934)) || (plant(3934) && arg_index > i32::from(self.arg_count)) {
            return ArgumentDataType::Unknown;
        }
        let at = arg_index.max(0) as usize;
        self.args.get(at).map(|a| a.ty).unwrap_or(ArgumentDataType::Unknown)
    }

    fn alloc_arg(&mut self, data: [u32; 4], ty: ArgumentDataType) {
        self.args.push(GameMessageArgument { data: ArgumentData(data), ty });
        self.arg_count = if plant(3933) { self.arg_count.saturating_add(1) } else { self.arg_count.wrapping_add(1) };
    }

    /// `appendIntegerArgument`.
    pub fn append_integer_argument(&mut self, arg: i32) {
        self.alloc_arg([arg as u32, 0, 0, 0], ArgumentDataType::Integer);
    }
    /// `appendRealArgument`.
    pub fn append_real_argument(&mut self, arg: f32) {
        self.alloc_arg([arg.to_bits(), 0, 0, 0], ArgumentDataType::Real);
    }
    /// `appendBooleanArgument`.
    pub fn append_boolean_argument(&mut self, arg: bool) {
        self.alloc_arg([u32::from(arg), 0, 0, 0], ArgumentDataType::Boolean);
    }
    /// `appendObjectIDArgument`.
    pub fn append_object_id_argument(&mut self, arg: u32) {
        self.alloc_arg([arg, 0, 0, 0], ArgumentDataType::ObjectId);
    }
    /// `appendDrawableIDArgument`.
    pub fn append_drawable_id_argument(&mut self, arg: u32) {
        self.alloc_arg([arg, 0, 0, 0], ArgumentDataType::DrawableId);
    }
    /// `appendTeamIDArgument`.
    pub fn append_team_id_argument(&mut self, arg: u32) {
        self.alloc_arg([arg, 0, 0, 0], ArgumentDataType::TeamId);
    }
    /// `appendLocationArgument`.
    pub fn append_location_argument(&mut self, arg: Coord3D) {
        if plant(3939) {
            self.alloc_arg([arg.x.to_bits(), arg.y.to_bits(), 0, arg.z.to_bits()], ArgumentDataType::Location);
            return;
        }
        self.alloc_arg([arg.x.to_bits(), arg.y.to_bits(), arg.z.to_bits(), 0], ArgumentDataType::Location);
    }
    /// `appendPixelArgument`.
    pub fn append_pixel_argument(&mut self, arg: ICoord2D) {
        self.alloc_arg([arg.x as u32, arg.y as u32, 0, 0], ArgumentDataType::Pixel);
    }
    /// `appendPixelRegionArgument`.
    pub fn append_pixel_region_argument(&mut self, arg: IRegion2D) {
        self.alloc_arg([arg.lo.x as u32, arg.lo.y as u32, arg.hi.x as u32, arg.hi.y as u32], ArgumentDataType::PixelRegion);
    }
    /// `appendTimestampArgument`.
    pub fn append_timestamp_argument(&mut self, arg: u32) {
        self.alloc_arg([arg, 0, 0, 0], ArgumentDataType::Timestamp);
    }
    /// `appendWideCharArgument`.
    pub fn append_wide_char_argument(&mut self, arg: u16) {
        self.alloc_arg([u32::from(arg), 0, 0, 0], ArgumentDataType::WideChar);
    }
    /// An argument as a replay or the network carries it: the type and the union's words (the recorder's and the
    /// network's readers append through the typed functions above; this is their common form).
    pub fn append_argument(&mut self, arg: GameMessageArgument) {
        self.alloc_arg(arg.data.0, arg.ty);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ListLinks {
    first: Option<MessageId>,
    last: Option<MessageId>,
}

#[derive(Clone, Debug)]
struct Slot {
    generation: u32,
    message: Option<GameMessage>,
}

/// What `propagateMessages` does with a message after a translator saw it (`enum GameMessageDisposition`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// `KEEP_MESSAGE`.
    Keep,
    /// `DESTROY_MESSAGE`: deleted at once, not shown to the later translators.
    Destroy,
}

type R<T> = Result<T, IniError>;

fn use_after_free() -> IniError {
    IniError::undefined(Undefined::UseAfterFree)
}

/// The messages and the lists that hold them.
#[derive(Clone, Debug)]
pub struct Messages {
    slots: Vec<Slot>,
    free: Vec<u32>,
    lists: Vec<ListLinks>,
}

impl Default for Messages {
    fn default() -> Self {
        Messages::new()
    }
}

impl Messages {
    /// `TheMessageStream`'s list.
    pub const STREAM: ListId = ListId(0);
    /// `TheCommandList`.
    pub const COMMAND_LIST: ListId = ListId(1);

    /// The two lists, empty (`GameMessageList()`).
    pub fn new() -> Messages {
        Messages { slots: Vec::new(), free: Vec::new(), lists: vec![ListLinks::default(); 2] }
    }

    /// Another empty `GameMessageList`.
    pub fn new_list(&mut self) -> ListId {
        self.lists.push(ListLinks::default());
        ListId(self.lists.len() as u32 - 1)
    }

    /// The message, or `None` when it was freed.
    pub fn get(&self, id: MessageId) -> Option<&GameMessage> {
        let s = self.slots.get(id.index as usize)?;
        if s.generation != id.generation {
            return None;
        }
        s.message.as_ref()
    }

    /// The message, mutable, or `None` when it was freed.
    pub fn get_mut(&mut self, id: MessageId) -> Option<&mut GameMessage> {
        let s = self.slots.get_mut(id.index as usize)?;
        if s.generation != id.generation {
            return None;
        }
        s.message.as_mut()
    }

    fn msg(&self, id: MessageId) -> R<&GameMessage> {
        self.get(id).ok_or_else(use_after_free)
    }

    fn msg_mut(&mut self, id: MessageId) -> R<&mut GameMessage> {
        self.get_mut(id).ok_or_else(use_after_free)
    }

    /// The message, or [`Undefined::UseAfterFree`].
    pub fn message(&self, id: MessageId) -> R<&GameMessage> {
        self.msg(id)
    }

    /// The message, mutable, or [`Undefined::UseAfterFree`].
    pub fn message_mut(&mut self, id: MessageId) -> R<&mut GameMessage> {
        self.msg_mut(id)
    }

    /// `newInstance(GameMessage)(type)`: on no list; `m_playerIndex` is the local player's index, which the caller
    /// passes (`ThePlayerList->getLocalPlayer()->getPlayerIndex()`).
    pub fn new_message(&mut self, ty: MessageType, local_player_index: i32) -> MessageId {
        let message = GameMessage {
            next: None,
            prev: None,
            list: None,
            ty,
            player_index: local_player_index,
            arg_count: 0,
            args: Vec::new(),
        };
        if let Some(index) = self.free.pop() {
            let s = &mut self.slots[index as usize];
            s.message = Some(message);
            MessageId { index, generation: s.generation }
        } else {
            self.slots.push(Slot { generation: 0, message: Some(message) });
            MessageId { index: self.slots.len() as u32 - 1, generation: 0 }
        }
    }

    /// `msg->deleteInstance()`: the destructor removes the message from the list it is on, then frees it.
    pub fn delete_message(&mut self, id: MessageId) -> R<()> {
        if let Some(list) = self.msg(id)?.list {
            self.remove_message(list, id)?;
        }
        let s = &mut self.slots[id.index as usize];
        s.message = None;
        s.generation = s.generation.wrapping_add(1);
        self.free.push(id.index);
        Ok(())
    }

    /// `getFirstMessage()`.
    pub fn first_message(&self, list: ListId) -> Option<MessageId> {
        self.lists[list.0 as usize].first
    }

    /// `m_lastMessage`.
    pub fn last_message(&self, list: ListId) -> Option<MessageId> {
        self.lists[list.0 as usize].last
    }

    /// The list's messages from first through `next`, or [`Undefined::UseAfterFree`] when the walk reaches a freed
    /// one. A walk longer than the arena is a cycle (upstream loops for ever): [`Undefined::EndlessLoop`].
    pub fn list_messages(&self, list: ListId) -> R<Vec<MessageId>> {
        let mut out = Vec::new();
        let mut at = self.first_message(list);
        while let Some(id) = at {
            if out.len() > self.slots.len() {
                return Err(IniError::undefined(Undefined::EndlessLoop));
            }
            out.push(id);
            at = self.msg(id)?.next;
        }
        Ok(out)
    }

    fn set_next(&mut self, id: MessageId, next: Option<MessageId>) -> R<()> {
        self.msg_mut(id)?.next = next;
        Ok(())
    }

    fn set_prev(&mut self, id: MessageId, prev: Option<MessageId>) -> R<()> {
        self.msg_mut(id)?.prev = prev;
        Ok(())
    }

    /// `GameMessageList::appendMessage(msg)`.
    pub fn append_message(&mut self, list: ListId, msg: MessageId) -> R<()> {
        self.set_next(msg, None)?;
        let links = self.lists[list.0 as usize];
        if let Some(last) = links.last {
            self.set_next(last, Some(msg))?;
            self.set_prev(msg, Some(last))?;
            self.lists[list.0 as usize].last = Some(msg);
        } else {
            self.lists[list.0 as usize] = ListLinks { first: Some(msg), last: Some(msg) };
            self.set_prev(msg, None)?;
        }
        self.msg_mut(msg)?.list = Some(list);
        Ok(())
    }

    /// `GameMessageList::insertMessage(msg, messageToInsertAfter)`.
    pub fn insert_message(&mut self, list: ListId, msg: MessageId, after: MessageId) -> R<()> {
        let after_next = self.msg(after)?.next;
        self.set_next(msg, after_next)?;
        self.set_prev(msg, Some(after))?;
        match self.msg(msg)?.next {
            Some(n) => self.set_prev(n, Some(msg))?,
            None if !plant(3931) => self.lists[list.0 as usize].last = Some(msg),
            None => {}
        }
        self.set_next(after, Some(msg))?;
        self.msg_mut(msg)?.list = Some(list);
        Ok(())
    }

    /// `GameMessageList::removeMessage(msg)` (of `list`, whichever list the message is on).
    pub fn remove_message(&mut self, list: ListId, msg: MessageId) -> R<()> {
        let (next, prev) = {
            let m = self.msg(msg)?;
            (m.next, m.prev)
        };
        match next {
            Some(n) => self.set_prev(n, prev)?,
            None => self.lists[list.0 as usize].last = prev,
        }
        match prev {
            Some(p) => self.set_next(p, next)?,
            None => self.lists[list.0 as usize].first = next,
        }
        if !plant(3932) {
            self.msg_mut(msg)?.list = None;
        }
        Ok(())
    }

    /// `GameMessageList::containsMessageOfType(type)`.
    pub fn contains_message_of_type(&self, list: ListId, ty: MessageType) -> R<bool> {
        let mut at = self.first_message(list);
        let mut steps = 0usize;
        while let Some(id) = at {
            let m = self.msg(id)?;
            if m.ty == ty {
                return Ok(true);
            }
            steps += 1;
            if steps > self.slots.len() {
                return Err(IniError::undefined(Undefined::EndlessLoop));
            }
            at = m.next;
        }
        Ok(false)
    }

    /// `MessageStream::appendMessage(type)` (of any list): a new message appended.
    pub fn append_new_message(&mut self, list: ListId, ty: MessageType, local_player_index: i32) -> R<MessageId> {
        let msg = self.new_message(ty, local_player_index);
        self.append_message(list, msg)?;
        Ok(msg)
    }

    /// `MessageStream::insertMessage(type, messageToInsertAfter)`: a new message inserted.
    pub fn insert_new_message(&mut self, list: ListId, ty: MessageType, after: MessageId, local_player_index: i32) -> R<MessageId> {
        let msg = self.new_message(ty, local_player_index);
        self.insert_message(list, msg, after)?;
        Ok(msg)
    }

    /// `CommandList::destroyAllMessages()` (also `CommandList::reset()`): each message deleted in order (its destructor
    /// removes it from the list it is on), then the list emptied.
    pub fn destroy_all_messages(&mut self, list: ListId) -> R<()> {
        let mut at = self.first_message(list);
        let mut steps = 0usize;
        while let Some(id) = at {
            steps += 1;
            if steps > self.slots.len() + 1 {
                return Err(IniError::undefined(Undefined::EndlessLoop));
            }
            let next = self.msg(id)?.next;
            self.delete_message(id)?;
            at = next;
        }
        self.lists[list.0 as usize] = ListLinks::default();
        Ok(())
    }

    /// `CommandList::appendMessageList(list)`: each message from `first` through `next` appended to `to`, reading
    /// `next` before the append rewrites it.
    pub fn append_message_list(&mut self, to: ListId, first: Option<MessageId>) -> R<()> {
        let mut at = first;
        let mut steps = 0usize;
        while let Some(id) = at {
            steps += 1;
            if steps > self.slots.len() + 1 {
                return Err(IniError::undefined(Undefined::EndlessLoop));
            }
            let next = self.msg(id)?.next;
            self.append_message(to, id)?;
            at = next;
        }
        Ok(())
    }

    /// The end of `MessageStream::propagateMessages`: the stream's messages appended to the command list, and the
    /// stream emptied.
    pub fn transfer_stream_to_command_list(&mut self) -> R<()> {
        let first = self.first_message(Messages::STREAM);
        self.append_message_list(Messages::COMMAND_LIST, first)?;
        self.lists[Messages::STREAM.0 as usize] = ListLinks::default();
        Ok(())
    }
}

/// `MessageStream::TranslatorData`.
#[derive(Clone, Debug, PartialEq)]
pub struct TranslatorData<T> {
    /// `m_id`.
    pub id: u32,
    /// `m_translator` (`NULL` allowed: `propagateMessages` skips it).
    pub translator: Option<T>,
    /// `m_priority`.
    pub priority: u32,
}

/// `MessageStream`'s translator list, in priority order (lower first), over the client's translator type.
#[derive(Clone, Debug, PartialEq)]
pub struct Translators<T> {
    list: Vec<TranslatorData<T>>,
    next_id: u32,
}

impl<T> Default for Translators<T> {
    fn default() -> Self {
        Translators { list: Vec::new(), next_id: 1 }
    }
}

impl<T> Translators<T> {
    /// `MessageStream()`: none attached, the first id 1.
    pub fn new() -> Translators<T> {
        Translators::default()
    }

    /// `attachTranslator(translator, priority)`: placed before the first with a greater priority (after every equal
    /// one), its id the next (`m_nextTranslatorID++`, an `UnsignedInt` that wraps).
    pub fn attach(&mut self, translator: Option<T>, priority: u32) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let at = self.list.iter().position(|t| t.priority > priority || (plant(3935) && t.priority == priority)).unwrap_or(self.list.len());
        self.list.insert(at, TranslatorData { id, translator, priority });
        id
    }

    /// `findTranslator(id)`: the first with that id.
    pub fn find(&self, id: u32) -> Option<&T> {
        self.list.iter().find(|t| t.id == id).and_then(|t| t.translator.as_ref())
    }

    /// `findTranslator(id)`, mutable.
    pub fn find_mut(&mut self, id: u32) -> Option<&mut T> {
        self.list.iter_mut().find(|t| t.id == id).and_then(|t| t.translator.as_mut())
    }

    /// `removeTranslator(id)`: the first with that id, deleted.
    pub fn remove(&mut self, id: u32) -> Option<TranslatorData<T>> {
        let at = self.list.iter().position(|t| t.id == id)?;
        Some(self.list.remove(at))
    }

    /// The attached translators in order.
    pub fn list(&self) -> &[TranslatorData<T>] {
        &self.list
    }

    /// `propagateMessages()`: for each translator in order, each message of the stream from the first through `next`
    /// (read after the translator ran, so a message it appended is seen) is given to `translate`; a message it answers
    /// [`Disposition::Destroy`] is deleted. Then the stream's messages go to the command list and the stream is emptied.
    pub fn propagate_messages(
        &mut self,
        messages: &mut Messages,
        mut translate: impl FnMut(&mut T, &mut Messages, MessageId) -> R<Disposition>,
    ) -> R<()> {
        for t in self.list.iter_mut() {
            let mut at = messages.first_message(Messages::STREAM);
            let mut steps = 0usize;
            while let Some(msg) = at {
                // a walk longer than every message made is round a cycle of links: upstream never ends
                steps += 1;
                if steps > messages.slots.len() + 1 {
                    return Err(IniError::undefined(Undefined::EndlessLoop));
                }
                match t.translator.as_mut() {
                    Some(tr) => {
                        let early = messages.msg(msg)?.next;
                        let disp = translate(tr, messages, msg)?;
                        at = if plant(3936) { early } else { messages.msg(msg)?.next };
                        if disp == Disposition::Destroy {
                            messages.delete_message(msg)?;
                        }
                    }
                    None => at = messages.msg(msg)?.next,
                }
            }
        }
        messages.transfer_stream_to_command_list()
    }
}

/// `buildRegion(anchor, dest, region)`: the rectangle the two corners span (each axis's lower value in `lo`; on a
/// tie `dest`'s is `lo`, `anchor`'s `hi`).
pub fn build_region(anchor: ICoord2D, dest: ICoord2D) -> IRegion2D {
    let (lo_x, hi_x) = if anchor.x < dest.x { (anchor.x, dest.x) } else { (dest.x, anchor.x) };
    let (lo_y, hi_y) = if anchor.y < dest.y && !plant(3938) { (anchor.y, dest.y) } else { (dest.y, anchor.y) };
    IRegion2D { lo: ICoord2D { x: lo_x, y: lo_y }, hi: ICoord2D { x: hi_x, y: hi_y } }
}
