//! `service_host` is the base of a normal Poplar userspace, and is in some ways similar to `init`
//! on a UNIX. It is responsible for bootstrapping userspace by configuring and starting other
//! tasks, providing userspace service discovery, and a range of other tasks required for a
//! basic Poplar system to function.

/*
 * TODO:
 *  - start as a new userspace task with special status
 *  - add all other userspace task's memory objects to this task
 *  - take a manifest from the kernel detailing all the handles it's giving us
 *  - create new tasks for each of the other userspace tasks (in future we'll monitor and restart
 *    them if crashed, according to some policy)
 *  - add a channel to each new task for service discovery + management
 *  - kernel will fill in a manifest for each new task detailing its handles (incl our channel)
 *  - provide service registration and discovery through the channel
 *  - provide a special service ourselves for the console to e.g. list services running, get system
 *    status, etc.
 *  - move PCI info + objects, kernel framebuffer, etc. to be passed to this task and then onwards
 *  - thinking: we need a mechanism for services to be able to ask us for specific objects (e.g.
 *    PCI info to platform_bus)
 */

use log::info;
use std::poplar::early_logger::EarlyLogger;

fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("ServiceHost is running!");
}
