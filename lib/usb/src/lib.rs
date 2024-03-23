#![no_std]

pub mod descriptor;
pub mod hid;
pub mod setup;

use ptah::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeviceControlMessage {
    UseConfiguration(u8),
}
