use crate::vm::{Chunk, Value};
use alloc::alloc::Global;
use core::{
    alloc::{Allocator, Layout},
    fmt,
    mem,
    ops::Deref,
    ptr,
    str,
};

pub struct Gc<T>
where
    T: GinkgoObj,
{
    pub(crate) inner: *mut T,
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

impl<T> Deref for Gc<T>
where
    T: GinkgoObj,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.inner) }
    }
}

impl<T> Clone for Gc<T>
where
    T: GinkgoObj,
{
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
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

    pub unsafe fn as_gc_typ<T: GinkgoObj>(&self) -> Option<Gc<T>> {
        if unsafe { (*self.inner).typ == T::TYP } { Some(Gc { inner: (self.inner as *mut T) }) } else { None }
    }

    pub unsafe fn as_typ<T: GinkgoObj>(&self) -> Option<&T> {
        if unsafe { (*self.inner).typ == T::TYP } { Some(unsafe { &*(self.inner as *const T) }) } else { None }
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
            ObjType::GinkgoNativeFunction => {
                let value = unsafe { self.as_typ::<GinkgoNativeFunction>().unwrap() };
                write!(f, "GinkgoNativeFunction {{ name: {:?}, ... }}", value.name)
            }
            ObjType::GinkgoClosure => {
                let value = unsafe { self.as_typ::<GinkgoClosure>().unwrap() };
                write!(f, "GinkgoClosure {{ callable: {:?}, ... }}", value.callable.clone().erase())
            }
            ObjType::GinkgoUpvalue => {
                write!(f, "GinkgoUpvalue")
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
    GinkgoNativeFunction,
    GinkgoClosure,
    GinkgoUpvalue,
}

pub fn object_eq(l: &ErasedGc, r: &ErasedGc) -> bool {
    match unsafe { (*l.inner).typ } {
        ObjType::GinkgoString => {
            let l = unsafe { l.as_typ::<GinkgoString>().unwrap() };
            if let Some(r) = unsafe { r.as_typ::<GinkgoString>() } { l.as_str() == r.as_str() } else { false }
        }
        ObjType::GinkgoFunction => todo!(),
        ObjType::GinkgoNativeFunction => todo!(),
        ObjType::GinkgoClosure => todo!(),
        ObjType::GinkgoUpvalue => todo!(),
    }
}

#[repr(C)]
pub struct GinkgoString {
    header: ObjHeader,
    capacity: usize,
    length: usize,
    // data: str
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
    pub num_upvalues: usize,
    pub chunk: Chunk,
}

impl GinkgoFunction {
    pub fn new(name: String, arity: usize, num_upvalues: usize, chunk: Chunk) -> Gc<GinkgoFunction> {
        Gc::new(GinkgoFunction {
            header: ObjHeader { typ: ObjType::GinkgoFunction },
            name,
            arity,
            num_upvalues,
            chunk,
        })
    }
}

impl GinkgoObj for GinkgoFunction {
    const TYP: ObjType = ObjType::GinkgoFunction;
}

#[repr(C)]
pub struct GinkgoNativeFunction {
    header: ObjHeader,
    pub name: String,
    pub func: Box<dyn Fn(&[Value]) -> Value>,
}

impl GinkgoNativeFunction {
    pub fn new<F>(name: String, func: F) -> Gc<GinkgoNativeFunction>
    where
        F: Fn(&[Value]) -> Value + 'static,
    {
        Gc::new(GinkgoNativeFunction {
            header: ObjHeader { typ: ObjType::GinkgoNativeFunction },
            name,
            func: Box::new(func),
        })
    }
}

impl GinkgoObj for GinkgoNativeFunction {
    const TYP: ObjType = ObjType::GinkgoNativeFunction;
}

#[repr(C)]
pub struct GinkgoClosure {
    header: ObjHeader,
    pub callable: Gc<GinkgoFunction>,
    pub upvalues: Vec<Gc<GinkgoUpvalue>>,
}

impl GinkgoClosure {
    pub fn new(callable: Gc<GinkgoFunction>, upvalues: Vec<Gc<GinkgoUpvalue>>) -> Gc<GinkgoClosure> {
        Gc::new(GinkgoClosure { header: ObjHeader { typ: ObjType::GinkgoClosure }, callable, upvalues })
    }
}

impl GinkgoObj for GinkgoClosure {
    const TYP: ObjType = ObjType::GinkgoClosure;
}

#[repr(C)]
pub struct GinkgoUpvalue {
    header: ObjHeader,
    pub value: *mut Value,
}

impl GinkgoUpvalue {
    pub fn new(value: *mut Value) -> Gc<GinkgoUpvalue> {
        Gc::new(GinkgoUpvalue { header: ObjHeader { typ: ObjType::GinkgoUpvalue }, value })
    }
}

impl GinkgoObj for GinkgoUpvalue {
    const TYP: ObjType = ObjType::GinkgoUpvalue;
}
