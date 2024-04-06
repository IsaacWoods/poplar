#![no_std]

pub mod descriptor;
pub mod hid;
pub mod setup;

use ptah::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeviceControlMessage {
    UseConfiguration(u8),
    UseInterface(u8, u8),
    OpenEndpoint(u8),
}
