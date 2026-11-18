//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameMessageParser.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameMessageParser.h (GameMessageParserArgumentType's accessors)
//!
//! Groups a game message's arguments into runs of equal data type, for the network's packer: a run of 4 integers then 1
//! location is two entries, `(Integer, 4)` and `(Location, 1)`. Upstream's linked list (`m_first`, `m_last`,
//! `getNext`) is a `Vec` here, in the same order.

use crate::common::message_stream::{ArgumentDataType, GameMessage};

/// `GameMessageParserArgumentType`: a run of `m_argCount` arguments of data type `m_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameMessageParserArgumentType {
    /// `m_type`.
    pub ty: ArgumentDataType,
    /// `m_argCount`.
    pub arg_count: i32,
}

impl GameMessageParserArgumentType {
    /// `GameMessageParserArgumentType(type, argCount)`.
    pub fn new(ty: ArgumentDataType, arg_count: i32) -> Self {
        GameMessageParserArgumentType { ty, arg_count }
    }
    /// `getType()`.
    pub fn get_type(&self) -> ArgumentDataType {
        self.ty
    }
    /// `getArgCount()`.
    pub fn get_arg_count(&self) -> i32 {
        self.arg_count
    }
}

/// `class GameMessageParser`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameMessageParser {
    /// `m_first` .. `m_last`, in order.
    pub types: Vec<GameMessageParserArgumentType>,
    /// `m_argTypeCount`.
    pub arg_type_count: i32,
}

impl GameMessageParser {
    /// `GameMessageParser::GameMessageParser()`: no types.
    pub fn new() -> Self {
        GameMessageParser { types: Vec::new(), arg_type_count: 0 }
    }

    /// `GameMessageParser::GameMessageParser(msg)` (GameMessageParser.cpp:42): the arguments' data types, in runs.
    /// `lasttype` starts at `ARGUMENTDATATYPE_UNKNOWN`, so that a first run is closed only when it has a count.
    pub fn from_message(msg: &GameMessage) -> Self {
        let mut p = GameMessageParser::new();
        let arg_count = msg.get_argument_count();
        let mut lasttype = ArgumentDataType::Unknown;
        let mut this_type_count: i32 = 0;
        for i in 0..arg_count {
            let ty = msg.get_argument_data_type(i);
            if ty != lasttype {
                if this_type_count > 0 {
                    p.add_arg_type(lasttype, this_type_count);
                    p.arg_type_count += 1;
                }
                lasttype = ty;
                this_type_count = 0;
            }
            this_type_count += 1;
        }
        if this_type_count > 0 {
            p.add_arg_type(lasttype, this_type_count);
            p.arg_type_count += 1;
        }
        p
    }

    /// `GameMessageParser::addArgType(type, argCount)` (GameMessageParser.cpp:77): appends a run (the first run
    /// becomes `m_first` and `m_last`, a later one `m_last->setNext`). It does not count: the constructor does.
    pub fn add_arg_type(&mut self, ty: ArgumentDataType, arg_count: i32) {
        self.types.push(GameMessageParserArgumentType::new(ty, arg_count));
    }

    /// `getFirstArgumentType()`.
    pub fn get_first_argument_type(&self) -> Option<&GameMessageParserArgumentType> {
        self.types.first()
    }

    /// `getNumTypes()`.
    pub fn get_num_types(&self) -> i32 {
        self.arg_type_count
    }
}
