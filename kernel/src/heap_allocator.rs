use core::{
    alloc::{AllocErr, GlobalAlloc, Layout},
    cmp::max,
    mem::{self, size_of},
    ops::Deref,
};
use spin::Mutex;
use x86_64::memory::VirtualAddress;

pub struct HoleAllocator {
    heap_bottom: VirtualAddress,
    heap_size: usize,
    holes: Option<HoleList>,
}

impl HoleAllocator {
    /// Create a new, uninitialized `HoleAllocator`. Before heap allocations can be made, `init`
    /// must be called.
    pub const fn new_uninitialized() -> HoleAllocator {
        HoleAllocator {
            heap_bottom: unsafe { VirtualAddress::new_unchecked(0) },
            heap_size: 0,
            holes: None,
        }
    }

    /// Initialise the `HoleAllocator`. This should only be called once, and constructs the
    /// `HoleList` from the address range.
    pub unsafe fn init(&mut self, heap_bottom: VirtualAddress, heap_top: VirtualAddress) {
        assert!(self.holes.is_none());
        self.heap_bottom = heap_bottom;
        self.heap_size = usize::from(heap_top) - usize::from(self.heap_bottom);
        self.holes = Some(HoleList::new(self.heap_bottom, self.heap_size));
    }
}

pub struct LockedHoleAllocator(Mutex<HoleAllocator>);

impl LockedHoleAllocator {
    pub const fn new_uninitialized() -> LockedHoleAllocator {
        LockedHoleAllocator(Mutex::new(HoleAllocator::new_uninitialized()))
    }
}

impl Deref for LockedHoleAllocator {
    type Target = Mutex<HoleAllocator>;

    fn deref(&self) -> &Mutex<HoleAllocator> {
        &self.0
    }
}

unsafe impl GlobalAlloc for LockedHoleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = align_up(
            max(layout.size() as usize, HoleList::get_min_size()),
            mem::align_of::<Hole>() as usize,
        );
        let layout = Layout::from_size_align(size as usize, layout.align()).unwrap();

        match self.0.lock().holes {
            Some(ref mut holes) => holes.allocate_first_fit(layout).unwrap_or(0x0 as *mut u8),
            None => panic!("Tried to allocate on the heap before initializing allocator!"),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = align_up(
            max(layout.size() as usize, HoleList::get_min_size()),
            mem::align_of::<Hole>() as usize,
        );
        let layout = Layout::from_size_align(size as usize, layout.align()).unwrap();

        match self.0.lock().holes {
            Some(ref mut holes) => holes.free(ptr, layout),
            None => panic!("Tried to allocate on the heap before initializing allocator!"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HoleInfo {
    addr: VirtualAddress,
    size: usize,
}

pub struct Hole {
    size: usize,
    next: Option<&'static mut Hole>,
}

impl Hole {
    fn info(&self) -> HoleInfo {
        HoleInfo {
            // Safe to unwrap because we know `self` points to a valid address
            addr: VirtualAddress::new(self as *const _ as usize).unwrap(),
            size: self.size,
        }
    }
}

pub struct HoleList {
    first: Hole,
}

impl HoleList {
    /// Create a new `HoleList` that contains the given hole. Unsafe because it is undefined
    /// bahaviour if the address passes is invalid or if [hole_addr, hole_addr+size) is used
    /// somewhere.
    pub unsafe fn new(hole_addr: VirtualAddress, hole_size: usize) -> HoleList {
        assert!(size_of::<Hole>() == Self::get_min_size());

        let ptr = hole_addr.mut_ptr() as *mut Hole;
        mem::replace(&mut *ptr, Hole { size: hole_size, next: None });

        HoleList { first: Hole { size: 0, next: Some(&mut *ptr) } }
    }

    /// Search for a big enough hole for the given `Layout` with its required alignment. This uses
    /// the first-fit strategy, and so is O(n)
    pub fn allocate_first_fit(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        assert!(layout.size() >= Self::get_min_size());

        allocate_first_fit(&mut self.first, layout).map(|allocation| {
            if let Some(padding) = allocation.front_padding {
                free(&mut self.first, padding.addr, padding.size);
            }

            if let Some(padding) = allocation.back_padding {
                free(&mut self.first, padding.addr, padding.size);
            }

            allocation.info.addr.mut_ptr() as *mut u8
        })
    }

    /// Free an allocation defined by `ptr` and `layout`. Unsafe because if `ptr` was not returned
    /// by a call to `allocate_first_fit`, undefined behaviour may occur. Deallocates by walking the
    /// list and inserts the given hole at the correct position. If the freed block is adjacent to
    /// another one, they are merged.
    pub unsafe fn free(&mut self, ptr: *mut u8, layout: Layout) {
        free(&mut self.first, VirtualAddress::from(ptr), layout.size());
    }

    pub fn get_min_size() -> usize {
        (size_of::<VirtualAddress>() + size_of::<usize>()) as usize
    }
}

#[derive(Clone, Debug)]
struct Allocation {
    info: HoleInfo,
    front_padding: Option<HoleInfo>,
    back_padding: Option<HoleInfo>,
}

/// Split the given hole into (front_padding,hole,back_padding) if it's big enough to hold the given
/// layout with the required alignment.
///     - Front padding occurs when the required alignment is higher than that of the hole.
///     - Back padding occurs when the layout's size is smaller than the hole.
fn split_hole(hole: HoleInfo, required_layout: Layout) -> Option<Allocation> {
    let required_size = required_layout.size();
    let required_align = required_layout.align();

    let (aligned_addr, front_pad) = if hole.addr == hole.addr.align_up(required_align) {
        (hole.addr, None) // Hole already has correct alignment
    } else {
        /*
         * We need to add front padding to correctly align the data
         * in the hole.
         */
        let aligned_addr = (hole.addr + HoleList::get_min_size()).unwrap().align_up(required_align);

        (
            aligned_addr,
            Some(HoleInfo {
                addr: hole.addr,
                size: usize::from(aligned_addr) - usize::from(hole.addr),
            }),
        )
    };

    let aligned_hole = if aligned_addr + required_size > hole.addr + hole.size {
        return None; // Hole is too small
    } else {
        HoleInfo {
            addr: aligned_addr,
            size: hole.size - (usize::from(aligned_addr) - usize::from(hole.addr)),
        }
    };

    let back_pad = if aligned_hole.size == required_size {
        None // Exactly the right size, don't add back padding
    } else if (aligned_hole.size - required_size) < HoleList::get_min_size() {
        return None; // We can't use this hole; it would leave a too small new one
    } else {
        /*
         * The hole is too big for the allocation, so add some back padding to
         * use the extra space.
         */
        Some(HoleInfo {
            addr: (aligned_hole.addr + required_size).unwrap(),
            size: aligned_hole.size - required_size,
        })
    };

    Some(Allocation {
        info: HoleInfo { addr: aligned_hole.addr, size: required_size },
        front_padding: front_pad,
        back_padding: back_pad,
    })
}

fn allocate_first_fit(mut previous: &mut Hole, layout: Layout) -> Result<Allocation, AllocErr> {
    loop {
        let allocation: Option<Allocation> =
            previous.next.as_mut().and_then(|current| split_hole(current.info(), layout.clone()));

        match allocation {
            Some(allocation) => {
                // Remove the allocated hole from the list
                previous.next = previous.next.as_mut().unwrap().next.take();
                return Ok(allocation);
            }

            None if previous.next.is_some() => {
                // Try the next hole
                // XXX: `{x}` forces the reference `x` to move instead of be reborrowed
                previous = { previous }.next.as_mut().unwrap();
            }

            None => {
                // This is the last hole, so no hole is big enough for this allocation :(
                return Err(AllocErr {});
            }
        }
    }
}

/// Walk the list, starting at `hole` and free the allocation given by `(addr,size)`
fn free(mut hole: &mut Hole, addr: VirtualAddress, mut size: usize) {
    loop {
        assert!(size >= HoleList::get_min_size());

        /*
         * If the size is 0, it's the dummy hole, so just set the address to 0
         */
        let hole_addr = if hole.size == 0 {
            VirtualAddress::new(0).unwrap()
        } else {
            VirtualAddress::new(hole as *mut _ as usize).unwrap()
        };
        assert!(
            (hole_addr + hole.size).unwrap() <= addr,
            "Invalid deallocation (probable double free)"
        );
        let next_hole_info = hole.next.as_ref().map(|next| next.info());

        match next_hole_info {
            Some(next) if hole_addr + hole.size == Some(addr) && addr + size == Some(next.addr) => {
                /*
                 * The block exactly fills the gap between this hole and the next:
                 *      Before: ___XXX____YYYY___    (X=this hole, Y=next hole)
                 *      After : ___XXXFFFFYYYY___    (F=freed block)
                 */
                hole.size += size; // Merge F into X
                hole.size += next.size; // Merge Y into X
                hole.next = hole.next.as_mut().unwrap().next.take(); // Remove Y
            }

            _ if hole_addr + hole.size == Some(addr) => {
                /*
                 * The block is right behind this hole but there is used memory after it:
                 *      Before: ___XXX______YYYY___ (X=this hole, Y=next hole)
                 *      After : ___XXXFFFF__YYYY___ (F=freed block)
                 *
                 * Or block is right behind this hole and this is the last hole:
                 *      Before: ___XXX_____________
                 *      After : ___XXXFFFF_________
                 */
                hole.size += size; // Merge F into X
            }

            Some(next) if addr + size == Some(next.addr) => {
                /*
                 * The block is right before the next hole but there is used memory before it:
                 *      Before: ___XXX______YYYY___
                 *      After : ___XXX__FFFFYYYY___
                 */
                hole.next = hole.next.as_mut().unwrap().next.take(); // Remove Y
                size += next.size; // Free the merged F/Y block
                continue;
            }

            Some(next) if next.addr <= addr => {
                /*
                 * The block is behind the next hole, so delegate it to the next hole
                 *      Before: ___XXX___YYYY________
                 *      After : ___XXX___YYYY__FFFF__
                 */
                hole = { hole }.next.as_mut().unwrap(); // Start next iteration at next hole
                continue;
            }

            _ => {
                /*
                 * The block is between this and the next hole:
                 *      Before: ___XXX________YYYY___
                 *      After : ___XXX__FFFF__YYYY___
                 *
                 *  Or this is the last hole:
                 *      Before: ___XXX________
                 *      After : ___XXX__FFFF__
                 */
                let new_hole = Hole {
                    size,
                    next: hole.next.take(), // Ref to Y (if it exists)
                };

                // Write the new hole into the freed memory block
                let ptr = addr.mut_ptr() as *mut Hole;
                unsafe {
                    ptr.write(new_hole);
                }

                // Add F as the next block of X
                hole.next = Some(unsafe { &mut *ptr });
            }
        }
        break;
    }
}

/// Get the greatest x with the given alignment such that x <= the given address. The alignment
/// must be a power of two.
pub fn align_down(addr: usize, align: usize) -> usize {
    assert!(align == 0 || align.is_power_of_two(), "Can only align to a power of two");

    if align.is_power_of_two() {
        /*
         * E.g.
         *      align       =   0b00001000
         *      align-1     =   0b00000111
         *      !(align-1)  =   0b11111000
         *                             ^^^ Masks the address to the value below it with the
         *                                 correct alignment
         */
        addr & !(align - 1)
    } else {
        assert!(align == 0);
        addr
    }
}

/// Get the smallest x with the given alignment such that x >= the given address.
pub fn align_up(addr: usize, align: usize) -> usize {
    align_down(addr + align - 1, align)
}

#[cfg(not(test))]
#[alloc_error_handler]
fn handle_alloc_error(layout: Layout) -> ! {
    panic!("Alloc error: {:?}", layout);
}
