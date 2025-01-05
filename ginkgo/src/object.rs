use std::{
    alloc::{Allocator, Global, Layout},
    mem,
    ptr,
    str,
};

pub trait GinkgoObj {
    const TYP: ObjType;
}

#[repr(C)]
pub struct ObjHeader {
    pub typ: ObjType,
}

impl ObjHeader {
    pub unsafe fn as_typ<'a, T: GinkgoObj>(self: *const Self) -> Option<&'a T> {
        if unsafe { (*self).typ == T::TYP } {
            Some(unsafe { &*(self as *const T) })
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
pub enum ObjType {
    GinkgoString,
}

#[repr(C)]
pub struct GinkgoString {
    obj: ObjHeader,
    capacity: usize,
    length: usize,
    // data: str,
}

impl GinkgoString {
    pub fn new(data: &str) -> *const GinkgoString {
        let (layout, str_offset) = Layout::new::<GinkgoString>().extend(Layout::for_value(data)).unwrap();
        let base = Global.allocate(layout).unwrap().as_ptr() as *mut GinkgoString;
        unsafe {
            ptr::write(&raw mut (*base).obj, ObjHeader { typ: ObjType::GinkgoString });
            ptr::write(&raw mut (*base).capacity, data.len());
            ptr::write(&raw mut (*base).length, data.len());
            ptr::copy(data.as_bytes().as_ptr(), base.byte_add(str_offset) as *mut u8, data.len());
        }
        base as *const GinkgoString
    }

    pub fn as_str(&self) -> &str {
        unsafe {
            let base = (self as *const Self as *const u8).byte_add(mem::size_of::<GinkgoString>());
            str::from_raw_parts(base, self.length)
        }
    }
}

impl GinkgoObj for GinkgoString {
    const TYP: ObjType = ObjType::GinkgoString;
}

pub fn object_eq(l: *const ObjHeader, r: *const ObjHeader) -> bool {
    match unsafe { (*l).typ } {
        ObjType::GinkgoString => {
            let l = unsafe { l.as_typ::<GinkgoString>().unwrap() };
            if let Some(r) = unsafe { r.as_typ::<GinkgoString>() } {
                l.as_str() == r.as_str()
            } else {
                false
            }
        }
        _ => false,
    }
}
