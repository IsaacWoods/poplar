/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

/// The actual representation of a process is left up to the architecure crate, so we use this to
/// represent a particular process outside it.
#[derive(Clone,Copy,Debug)]
pub struct ProcessId(pub u16);
