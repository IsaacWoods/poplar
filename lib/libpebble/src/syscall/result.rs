use crate::KernelObjectId;
use bit_field::BitField;

pub fn result_from_syscall_repr<E>(result: usize) -> Result<KernelObjectId, E>
where
    E: From<u32>,
{
    let status = result.get_bits(32..64);
    if status == 0 {
        Ok(KernelObjectId::from_syscall_repr(result))
    } else {
        Err(E::from(status as u32))
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
