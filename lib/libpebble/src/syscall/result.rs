use crate::KernelObjectId;
use bit_field::BitField;
use core::convert::TryFrom;

pub fn result_from_syscall_repr<E>(result: usize) -> Result<KernelObjectId, E>
where
    E: TryFrom<u32, Error = ()>,
{
    let status = result.get_bits(32..64);
    if status == 0 {
        Ok(KernelObjectId::from_syscall_repr(result))
    } else {
        Err(E::try_from(status as u32).expect("System call returned invalid status"))
    }
}

pub fn result_to_syscall_repr<E>(result: Result<KernelObjectId, E>) -> usize
where
    E: Into<u32>,
{
    match result {
        Ok(id) => KernelObjectId::to_syscall_repr(id),
        Err(err) => {
            let mut value = 0usize;
            value.set_bits(32..64, err.into() as usize);
            value
        }
    }
}
