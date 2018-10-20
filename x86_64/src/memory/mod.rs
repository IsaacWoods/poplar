pub mod kernel_map;
pub mod paging;

mod physical_address;
mod virtual_address;

pub use self::physical_address::PhysicalAddress;
pub use self::virtual_address::VirtualAddress;
