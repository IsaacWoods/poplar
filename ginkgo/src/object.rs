use crate::vm::Chunk;
use core::fmt;
use std::{
    alloc::{Allocator, Global, Layout},
    mem,
    ptr,
    str,
};

pub struct Gc<T>
where
    T: GinkgoObj,
{
    inner: *mut T,
}

impl<T> Gc<T>
where
    T: GinkgoObj,
{
    pub fn new(inner: T) -> Gc<T> {
        Gc { inner: Box::leak(Box::new(inner)) }
    }

    pub fn erase(self) -> ErasedGc {
        ErasedGc { inner: self.inner as *mut ObjHeader }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ErasedGc {
    pub inner: *mut ObjHeader,
}

impl ErasedGc {
    pub fn typ(&self) -> ObjType {
        unsafe { (*self.inner).typ }
    }

    pub unsafe fn as_typ<T: GinkgoObj>(&self) -> Option<&T> {
        if unsafe { (*self.inner).typ == T::TYP } {
            Some(unsafe { &*(self.inner as *const T) })
        } else {
            None
        }
    }
}

impl fmt::Debug for ErasedGc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.typ() {
            ObjType::GinkgoString => {
                let value = unsafe { self.as_typ::<GinkgoString>().unwrap() };
                write!(f, "GingkoString({:?})", value.as_str())
            }
            ObjType::GinkgoFunction => {
                let value = unsafe { self.as_typ::<GinkgoFunction>().unwrap() };
                write!(f, "GinkgoFunction {{ name: {:?}, ... }}", value.name)
            }
        }
    }
}

pub trait GinkgoObj {
    const TYP: ObjType;
}

#[repr(C)]
pub struct ObjHeader {
    pub typ: ObjType,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
pub enum ObjType {
    GinkgoString,
    GinkgoFunction,
}

pub fn object_eq(l: &ErasedGc, r: &ErasedGc) -> bool {
    match unsafe { (*l.inner).typ } {
        ObjType::GinkgoString => {
            let l = unsafe { l.as_typ::<GinkgoString>().unwrap() };
            if let Some(r) = unsafe { r.as_typ::<GinkgoString>() } {
                l.as_str() == r.as_str()
            } else {
                false
            }
        }
        ObjType::GinkgoFunction => todo!(),
        _ => false,
    }
}

#[repr(C)]
pub struct GinkgoString {
    header: ObjHeader,
    capacity: usize,
    length: usize,
    // data: str,
}

impl GinkgoString {
    pub fn new(data: &str) -> Gc<GinkgoString> {
        let (layout, str_offset) = Layout::new::<GinkgoString>().extend(Layout::for_value(data)).unwrap();
        let base = Global.allocate(layout).unwrap().as_ptr() as *mut GinkgoString;
        unsafe {
            ptr::write(&raw mut (*base).header, ObjHeader { typ: ObjType::GinkgoString });
            ptr::write(&raw mut (*base).capacity, data.len());
            ptr::write(&raw mut (*base).length, data.len());
            ptr::copy(data.as_bytes().as_ptr(), base.byte_add(str_offset) as *mut u8, data.len());
        }
        Gc { inner: base as *mut GinkgoString }
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

#[repr(C)]
pub struct GinkgoFunction {
    header: ObjHeader,
    pub name: String,
    pub arity: usize,
    pub chunk: Chunk,
}

impl GinkgoFunction {
    pub fn new(name: String, arity: usize, chunk: Chunk) -> Gc<GinkgoFunction> {
        Gc::new(GinkgoFunction { header: ObjHeader { typ: ObjType::GinkgoFunction }, name, arity, chunk })
    }
}

impl GinkgoObj for GinkgoFunction {
    const TYP: ObjType = ObjType::GinkgoFunction;
}
