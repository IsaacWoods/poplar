use crate::Handle;
use bit_field::BitField;

pub(super) macro define_error_type($error_name:ident {
    $($(#[$attrib:meta])*$name:ident => $repr_num:expr),*$(,)?
}) {
    #[derive(Clone, Copy, Debug)]
    pub enum $error_name {
        $(
            $(#[$attrib])*
            $name,
         )*
    }

    impl TryFrom<usize> for $error_name {
        type Error = ();

        fn try_from(status: usize) -> Result<Self, Self::Error> {
            match status {
                $(
                    $repr_num => Ok(Self::$name),
                 )*
                _ => Err(()),
            }
        }
    }

    impl Into<usize> for $error_name {
        fn into(self) -> usize {
            match self {
                $(
                    Self::$name => $repr_num,
                 )*
            }
        }
    }
}

pub fn status_from_syscall_repr<E>(status: usize) -> Result<(), E>
where
    E: TryFrom<usize, Error = ()>,
{
    if status == 0 {
        Ok(())
    } else {
        Err(E::try_from(status).expect("System call returned invalid status"))
    }
}

pub fn status_to_syscall_repr<E>(result: Result<(), E>) -> usize
where
    E: Into<usize>,
{
    match result {
        Ok(()) => 0,
        Err(err) => err.into(),
    }
}

/// Convert a `Result` that carries a custom status on success. It is the producer's responsibility that the
/// success status can be differentiated from an error, if needed.
pub fn status_with_payload_to_syscall_repr<E>(result: Result<usize, E>) -> usize
where
    E: Into<usize>,
{
    match result {
        Ok(status) => status,
        Err(err) => err.into(),
    }
}

pub fn handle_from_syscall_repr<E>(result: usize) -> Result<Handle, E>
where
    E: TryFrom<usize, Error = ()>,
{
    let status = result.get_bits(0..32);
    if status == 0 {
        Ok(Handle(result.get_bits(32..64) as u32))
    } else {
        Err(E::try_from(status).expect("System call returned invalid result status"))
    }
}

pub fn handle_to_syscall_repr<E>(result: Result<Handle, E>) -> usize
where
    E: Into<usize>,
{
    match result {
        Ok(handle) => {
            let mut value = 0usize;
            value.set_bits(32..64, handle.0 as usize);
            value
        }
        Err(err) => {
            let mut value = 0usize;
            value.set_bits(0..32, err.into());
            value
        }
    }
}
