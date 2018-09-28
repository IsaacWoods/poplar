pub type NotImplemented = &'static mut ();

pub type Event = usize;
pub type Handle = *mut ();

const ERROR_BIT: usize = 1 << 63;

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UefiStatus {
    Success = 0,
    UnknownGlyph = 1,
    Unsupported = ERROR_BIT | 3,
    DeviceError = ERROR_BIT | 7,
}

impl ::core::ops::Try for UefiStatus {
    type Ok = Self;
    type Error = Self;

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        match self {
            success @ UefiStatus::Success => Ok(success),
            error @ _ => Err(error),
        }
    }

    fn from_error(status: Self::Error) -> Self {
        status
    }

    fn from_ok(status: Self::Ok) -> Self {
        status
    }
}
