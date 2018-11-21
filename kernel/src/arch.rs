use alloc::boxed::Box;
// use fs::File;
// use process::ProcessId;

/// This represents a memory address, whatever that might be on the given architecture. It is
/// always the maximum integer width for the current platform, so should be guaranteed to hold a
/// memory address (on normal architectures, at least).
///
/// On architectures with paging, this may represent a physical or virtual memory address.
pub type MemoryAddress = usize;

/// This represents a module loaded into memory by the bootloader. On architectures with paging,
/// this contains the correct physical and virtual mappings of the module. Otherwise, these
/// addresses are equivelent.
#[derive(Clone, Copy)]
pub struct ModuleMapping {
    pub physical_start: MemoryAddress,
    pub physical_end: MemoryAddress,

    pub virtual_start: MemoryAddress,
    pub virtual_end: MemoryAddress,
}

/// This trait is implemented by a type in each architecture crate. It provides a common interface
/// to platform-specific operations and types for the rest of the kernel to use.
pub trait Architecture {
    // fn get_module_mapping(&self, module_name: &str) -> Option<ModuleMapping>;

    // /// Create a new process. The representation is platform-specific, so the architecture layer is
    // /// responsible for keeping track of the mapping between the given process id, and whatever
    // /// information it needs to keep track of for the process.
    // fn create_process(&mut self, id: ProcessId, image: &File);

    // /// Create a new process. The representation is platform-specific, and so it's just required to
    // /// be a node with the correct message type (`ProcessMessage`).
    // fn create_process(&mut self, image: &File) -> Box<Node<MessageType = ProcessMessage>>;
}
