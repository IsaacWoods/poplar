/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#[derive(Clone,Copy,Debug)]
pub struct ProcessId(pub(self) u16);

/*
 * The actual representation of a process is platform-dependent, so we use this to refer to the
 * process and leave the details up to the architecture crate.
 */
pub struct ProcessRef
{
    id  : ProcessId,
}
