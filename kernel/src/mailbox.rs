use alloc::collections::VecDeque;
use libpebble::syscall::mailbox::Mail;

/// A `Mailbox` is a kernel object that allows the kernel to send simple messages for a task to deal with. It
/// employs a simple message-pumping system, where a queue of "mails" are kept in the kernel, and the task is
/// expected to handle them by calling a blocking system call.
pub struct Mailbox {
    queue: VecDeque<Mail>,
}

impl Mailbox {
    pub fn new() -> Mailbox {
        Mailbox { queue: VecDeque::new() }
    }

    pub fn add(&mut self, mail: Mail) {
        self.queue.push_back(mail);
    }

    pub fn get_next(&mut self) -> Option<Mail> {
        self.queue.pop_front()
    }
}
