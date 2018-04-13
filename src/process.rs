/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

pub enum ProcessMessage
{
    /// This drops to usermode and starts executing the process this message is sent to. The call
    /// to `message` is diverging for this message.
    DropToUsermode,
}
