use std::poplar::caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING};

fn main() {
    std::poplar::syscall::early_log("Hello, World!").unwrap();
    // println!("Hello, world!");
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_PADDING, CAP_PADDING, CAP_PADDING]);
