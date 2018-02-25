/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#[derive(Clone,Copy,Debug)]
pub struct ProcessId(pub(self) u16);

pub struct Process
{
    id  : ProcessId,
}

impl Process
{
    pub fn new(id : ProcessId) -> Process
    {
        Process
        {
            id,
        }
    }
}

pub struct ProcessList
{
    next_pid : ProcessId,
}

impl ProcessList
{
    pub fn new() -> ProcessList
    {
        ProcessList
        {
            next_pid : ProcessId(1),    // XXX: 0 is reserved
        }
    }

    pub fn next_pid(&mut self) -> ProcessId
    {
        let pid = self.next_pid;
        self.next_pid.0 += 1;
        pid
    }
}
