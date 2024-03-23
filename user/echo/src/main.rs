use log::info;
use std::poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_PROVIDER},
    channel::Channel,
    early_logger::EarlyLogger,
    syscall,
    syscall::GetMessageError,
};

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Echo running!");

    let echo_service_channel = Channel::register_service("echo").unwrap();
    let mut subscribers = Vec::new();

    loop {
        syscall::yield_to_kernel();

        /*
         * Check if any of our subscribers have sent us any messages, and if they have, echo them back.
         * NOTE: we don't support handles.
         */
        for subscriber in subscribers.iter() {
            let mut bytes = [0u8; 256];
            loop {
                match syscall::get_message(*subscriber, &mut bytes, &mut []) {
                    Ok((bytes, _handles)) => {
                        info!("Echoing message: {:x?}", bytes);
                        syscall::send_message(*subscriber, bytes, &[]).unwrap();
                    }
                    Err(GetMessageError::NoMessage) => break,
                    Err(err) => panic!("Error while echoing message: {:?}", err),
                }
            }
        }

        if let Some(subscriber_handle) = echo_service_channel.try_receive().unwrap() {
            info!("Task subscribed to our service!");
            subscribers.push(subscriber_handle);
        }
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_PROVIDER, CAP_PADDING, CAP_PADDING]);
