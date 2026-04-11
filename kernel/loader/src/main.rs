#![no_std]
#![no_main]

#[macro_use]
mod util;

use core::time::Duration;
use uefi::prelude::*;

#[entry]
fn main() -> Status {
    println!("Booting Poplar - loader v{}", env!("CARGO_PKG_VERSION"));
    println!(
        "Firmware: {} (revision {}; implementing UEFI {})",
        uefi::system::firmware_vendor(),
        uefi::system::firmware_revision(),
        uefi::system::uefi_revision()
    );

    boot::stall(Duration::from_secs(3));

    Status::SUCCESS
}
