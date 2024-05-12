#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u32)]
pub enum CtrlType {
    // 2D commands
    CmdGetDisplayInfo = 0x100,
    CmdResourceCreate2D,
    CmdResourceUnref,
    CmdSetScanout,
    CmdResourceFlush,
    CmdTransferToHost2D,
    CmdResourceAttachBacking,
    CmdResourceDetachBacking,
    CmdGetCapsetInfo,
    CmdGetEdid,
    CmdResourceAssignUuid,
    CmdResourceCreateBlob,
    CmdSetScanoutBlob,

    // 3D commands
    CmdCtxCreate = 0x200,
    CmdCtxDestroy,
    CmdCtxAttachResource,
    CmdCtxDetachResource,
    CmdResourceCreate3D,
    CmdTransferToHost3D,
    CmdTransferFromHost3D,
    CmdSubmit3D,
    CmdResourceMapBlob,
    CmdResourceUnmapBlob,

    // Cursor commands
    CmdUpdateCursor = 0x300,
    CmdMoveCursor,

    // Success responses
    OkNoData = 0x1100,
    OkDisplayInfo,
    OkCapsetInfo,
    OkCapset,
    OkEdid,
    OkResourceUuid,
    OkMapInfo,

    // Error responses
    ErrUnspecified = 0x1200,
    ErrOutOfMemory,
    ErrInvalidScanoutId,
    ErrInvalidResourceId,
    ErrInvalidContextId,
    ErrInvalidParameter,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CtrlHeader {
    pub typ: CtrlType,
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub ring_id: u8,
    _padding: [u8; 3],
}

impl CtrlHeader {
    pub fn new(typ: CtrlType) -> CtrlHeader {
        CtrlHeader { typ, flags: 0, fence_id: 0, ctx_id: 0, ring_id: 0, _padding: [0, 0, 0] }
    }
}

const MAX_SCANOUTS: usize = 16;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct DisplayMode {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub enabled: u32,
    pub flags: u32,
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct DisplayInfo {
    pub header: CtrlHeader,
    pub modes: [DisplayMode; MAX_SCANOUTS],
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u32)]
pub enum VirtioGpuFormat {
    B8G8R8A8Unorm = 1,
    B8G8R8X8Unorm = 2,
    A8R8G8B8Unorm = 3,
    X8R8G8B8Unorm = 4,
    R8G8B8A8Unorm = 67,
    X8B8G8R8Unorm = 68,
    A8B8G8R8Unorm = 121,
    R8G8B8X8Unorm = 134,
}

#[repr(C)]
pub struct CreateResource2D {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub format: VirtioGpuFormat,
    pub width: u32,
    pub height: u32,
}

impl CreateResource2D {
    pub fn new(id: u32, format: VirtioGpuFormat, width: u32, height: u32) -> CreateResource2D {
        CreateResource2D {
            header: CtrlHeader::new(CtrlType::CmdResourceCreate2D),
            resource_id: id,
            format,
            width,
            height,
        }
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct BackingMemoryEntry {
    pub address: u64,
    pub length: u32,
    _padding: u32,
}

impl BackingMemoryEntry {
    pub fn new(address: u64, length: u32) -> BackingMemoryEntry {
        BackingMemoryEntry { address, length, _padding: 0 }
    }
}

/// Command structure to attach backing pages to a GPU resource. This header is followed by a
/// number of `BackingMemoryEntry` structures.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct ResourceAttachBacking {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub num_entries: u32,
}

// This is a bit of a hacky way to easily create a `ResourceAttachBacking` with a single memory
// region.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct SimpleResourceAttachBacking {
    pub header: CtrlHeader,
    pub resource_id: u32,
    pub num_entries: u32,
    pub mem_entry: BackingMemoryEntry,
}

impl SimpleResourceAttachBacking {
    pub fn new(resource_id: u32, address: u64, length: u32) -> SimpleResourceAttachBacking {
        SimpleResourceAttachBacking {
            header: CtrlHeader::new(CtrlType::CmdResourceAttachBacking),
            resource_id,
            num_entries: 1,
            mem_entry: BackingMemoryEntry::new(address, length),
        }
    }
}

#[repr(C)]
pub struct SetScanout {
    pub header: CtrlHeader,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub scanout_id: u32,
    pub resource_id: u32,
}

impl SetScanout {
    pub fn new(width: u32, height: u32, scanout_id: u32, resource_id: u32) -> SetScanout {
        SetScanout {
            header: CtrlHeader::new(CtrlType::CmdSetScanout),
            x: 0,
            y: 0,
            width,
            height,
            scanout_id,
            resource_id,
        }
    }
}

#[repr(C)]
pub struct TransferToHost2D {
    pub header: CtrlHeader,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub offset: u64,
    pub resource_id: u32,
    _padding: u32,
}

impl TransferToHost2D {
    pub fn new(width: u32, height: u32, offset: u64, resource_id: u32) -> TransferToHost2D {
        TransferToHost2D {
            header: CtrlHeader::new(CtrlType::CmdTransferToHost2D),
            x: 0,
            y: 0,
            width,
            height,
            offset,
            resource_id,
            _padding: 0,
        }
    }
}

#[repr(C)]
pub struct FlushResource {
    pub header: CtrlHeader,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub resource_id: u32,
    _padding: u32,
}

impl FlushResource {
    pub fn new(resource_id: u32, width: u32, height: u32) -> FlushResource {
        FlushResource {
            header: CtrlHeader::new(CtrlType::CmdResourceFlush),
            x: 0,
            y: 0,
            width,
            height,
            resource_id,
            _padding: 0,
        }
    }
}
