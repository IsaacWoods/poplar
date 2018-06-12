#![no_std]
#![feature(alloc)]
#![feature(core_intrinsics)]
#![feature(type_ascription)]
#![feature(string_retain)]
#![feature(pattern)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![feature(global_allocator)]
#![feature(box_syntax)]

extern crate alloc;
extern crate bit_field;
extern crate bitflags;
extern crate spin;
extern crate volatile;
#[macro_use]
extern crate log;
extern crate heap_allocator as allocator;
extern crate libmessage;

pub mod arch;
pub mod fs;
pub mod node;
pub mod process;

pub use arch::Architecture;

use alloc::{rc::Rc, String};
use allocator::LockedHoleAllocator;
use fs::{ramdisk::Ramdisk, FileManager};
use libmessage::NodeId;
use node::NodeManager;
use process::ProcessMessage;

#[global_allocator]
pub static ALLOCATOR: LockedHoleAllocator = LockedHoleAllocator::empty();

pub fn kernel_main<A>(architecture: &mut A) -> !
where
    A: Architecture,
{
    trace!("Control passed to kernel crate");

    let mut node_manager = NodeManager::new();
    // TODO: make kernel node

    let mut file_manager = FileManager::new();

    // Register ramdisk
    let ramdisk_mapping = architecture
        .get_module_mapping("ramdisk")
        .expect("Couldn't load ramdisk");
    file_manager.mount("/ramdisk", Rc::new(Ramdisk::new(&ramdisk_mapping)));

    let test_file = file_manager.open("/ramdisk/test_file").unwrap();
    info!(
        "Test file contents: {}",
        core::str::from_utf8(&file_manager.read(&test_file).unwrap()).unwrap()
    );

    let test_process_image = file_manager.open("/ramdisk/test_process.elf").unwrap();
    let (image_start, image_end) = unsafe {
        file_manager
            .get_physical_mapping(&test_process_image)
            .unwrap()
    };
    let test_process = architecture.create_process(image_start, image_end);

    let test_process_id = node_manager.add_node(test_process);
    node_manager
        .get_node(test_process_id)
        .unwrap()   // TODO: handle proplerly
        .message(NodeId(0), ProcessMessage::DropToUsermode); // TODO: use kernel's node id

    loop {}
}
