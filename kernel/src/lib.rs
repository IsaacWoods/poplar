#![no_std]
#![feature(alloc)]
#![feature(core_intrinsics)]
#![feature(type_ascription)]
#![feature(string_retain)]
#![feature(pattern)]
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
extern crate serde;
#[macro_use]
extern crate serde_derive;

pub mod arch;
pub mod fs;
pub mod node;
pub mod process;

pub use arch::Architecture;

use alloc::rc::Rc;
use allocator::LockedHoleAllocator;
use fs::{ramdisk::Ramdisk, FileManager};
use libmessage::{kernel::KernelMessage, Message, MessageHeader, NodeId};
use node::{Node, NodeManager};
use process::ProcessMessage;

#[global_allocator]
pub static ALLOCATOR: LockedHoleAllocator = LockedHoleAllocator::empty();

struct KernelNode {}

impl KernelNode {
    fn new() -> KernelNode {
        KernelNode {}
    }
}

impl Node for KernelNode {
    type MessageType = KernelMessage;

    fn message(&mut self, sender: NodeId, message: KernelMessage) -> Result<(), ()> {
        unimplemented!();
    }
}

pub fn kernel_main<A>(architecture: &mut A) -> !
where
    A: Architecture,
{
    trace!("Control passed to kernel crate");
    let mut node_manager = NodeManager::new();
    let mut file_manager = FileManager::new();

    // Create the kernel node
    node_manager.add_node(box KernelNode::new());

    // Register ramdisk
    let ramdisk_mapping = architecture
        .get_module_mapping("ramdisk")
        .expect("Couldn't load ramdisk");
    file_manager.mount("/ramdisk", Rc::new(Ramdisk::new(&ramdisk_mapping)));

    let test_file = file_manager.open("/ramdisk/test_file").unwrap();
    info!(
        "Test file contents: {}",
        core::str::from_utf8(&test_file.read().expect("Failed to read test file")).unwrap()
    );

    let test_process_image = file_manager.open("/ramdisk/test_process.elf").unwrap();
    let test_process = architecture.create_process(&test_process_image);

    let test_process_id = node_manager.add_node(test_process);
    node_manager
        .get_node(test_process_id)
        .unwrap()   // TODO: handle proplerly
        .message(NodeId(0), ProcessMessage::DropToUsermode); // TODO: use kernel's node id

    loop {}
}
