pub mod address_space;
pub mod channel;
pub mod event;
pub mod memory_object;
pub mod task;

use core::sync::atomic::{AtomicU64, Ordering};
use mulch::{downcast::DowncastSync, impl_downcast};

/// Each kernel object is assigned a unique 64-bit ID, which is never reused. An ID of `0` is never allocated, and
/// is used as a sentinel value.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct KernelObjectId(u64);

/// A kernel object ID of `0` is reserved as a sentinel value that will never point to a real kernel object. It is
/// used to mark things like the `owner` of a kernel object being the kernel itself.
pub const SENTINEL_KERNEL_ID: KernelObjectId = KernelObjectId(0);

/// The next available `KernelObjectId`. It is shared between all the CPUs, and so is incremented atomically.
static KERNEL_OBJECT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn alloc_kernel_object_id() -> KernelObjectId {
    // TODO: this wraps, so we should manually detect when it wraps around and panic to prevent ID reuse
    KernelObjectId(KERNEL_OBJECT_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KernelObjectType {
    AddressSpace,
    Task,
    MemoryObject,
    Channel,
    Event,
}

/// This trait should be implemented by all types that implement kernel objects, and allows common code to
/// be generic over all kernel objects. Kernel objects are generally handled as `Arc<T>` where `T` is the type
/// implementing `KernelObject`, and so interior mutability should be used for data that needs to be mutable within
/// the kernel object.
pub trait KernelObject: DowncastSync {
    fn id(&self) -> KernelObjectId;
    fn typ(&self) -> KernelObjectType;
    // fn owner(&self) -> KernelObjectId;
}

impl_downcast!(sync KernelObject);
