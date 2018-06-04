/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

pub const ZERO_OP: u8 = 0x00;
pub const ONE_OP: u8 = 0x01;
pub const ONES_OP: u8 = 0xFF;

pub const BYTE_PREFIX: u8 = 0x0A;
pub const WORD_PREFIX: u8 = 0x0B;
pub const DWORD_PREFIX: u8 = 0x0C;
pub const STRING_PREFIX: u8 = 0x0D;
pub const QWORD_PREFIX: u8 = 0x0E;

pub const NULL_NAME: u8 = 0x00;
pub const DUAL_NAME_PREFIX: u8 = 0x2E;
pub const MULTI_NAME_PREFIX: u8 = 0x2F;

pub const SCOPE_OP: u8 = 0x10;

pub const EXT_OP_PREFIX: u8 = 0x5b;
pub const OP_REGION_OP: u8 = 0x80;
