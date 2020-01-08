use super::result::{define_error_type, result_from_syscall_repr, status_from_syscall_repr};
use crate::KernelObjectId;

pub type MailType = u16;

pub const MAIL_TYPE_INTERRUPT: MailType = 0;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct InterruptMailRepr {
    // TODO
}

#[repr(C)]
pub union MailPayload {
    interrupt: InterruptMailRepr,
}

#[repr(C)]
pub struct MailRepr {
    pub mail_type: MailType,
    pub payload: MailPayload,
}

#[derive(Clone, Copy, Debug)]
pub enum Mail {
    Interrupt,
}

impl Mail {
    fn from_repr(repr: &MailRepr) -> Result<Mail, MailboxError> {
        match repr.mail_type {
            MAIL_TYPE_INTERRUPT => Ok(Mail::Interrupt),
            _ => Err(MailboxError::InvalidMailType),
        }
    }

    pub fn to_syscall_repr(&self) -> MailRepr {
        match self {
            Mail::Interrupt => MailRepr {
                mail_type: MAIL_TYPE_INTERRUPT,
                payload: MailPayload { interrupt: InterruptMailRepr {} },
            },
        }
    }
}

define_error_type!(MailboxError {
    NotAMailbox => 1,
    InvalidMailType => 2,
});

pub fn create_mailbox() -> Result<KernelObjectId, MailboxError> {
    result_from_syscall_repr(unsafe { super::raw::syscall0(super::SYSCALL_CREATE_MAILBOX) })
}

pub fn wait_for_mail(mailbox: KernelObjectId) -> Result<Mail, MailboxError> {
    use core::mem::MaybeUninit;

    let mut mail: MaybeUninit<MailRepr> = MaybeUninit::uninit();
    unsafe {
        status_from_syscall_repr(super::raw::syscall2(
            super::SYSCALL_WAIT_FOR_MAIL,
            mailbox.to_syscall_repr(),
            mail.as_mut_ptr() as usize,
        ))?;
    }

    // The system call succeeded, so `mail` should now be initialized
    Mail::from_repr(&unsafe { mail.assume_init() })
}
