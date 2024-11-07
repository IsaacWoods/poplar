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
use std::poplar::{early_logger::EarlyLogger, manifest::BootstrapManifest, Handle};

pub struct Service {
    name: String,
    address_space: Handle,
    segments: Vec<(Handle, usize)>,
    task: Handle,
}

fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("ServiceHost is running!");

    let manifest: BootstrapManifest = {
        const MANIFEST_ADDRESS: usize = 0x20000000;
        let manifest_len = unsafe { core::ptr::read(MANIFEST_ADDRESS as *const u32) };
        let data =
            unsafe { core::slice::from_raw_parts((MANIFEST_ADDRESS + 4) as *const u8, manifest_len as usize) };
        ptah::from_wire(data, &[]).unwrap()
    };

    let mut services = Vec::new();

    for service in &manifest.boot_services {
        info!("Spawning service '{}'", service.name);
        let address_space = std::poplar::syscall::create_address_space().unwrap();
        let mut segments = Vec::new();
        for (map_at, memory_object) in &service.segments {
            let memory_object = Handle(*memory_object);
            unsafe {
                std::poplar::syscall::map_memory_object(
                    memory_object,
                    address_space,
                    Some(*map_at),
                    0x0 as *mut _,
                )
                .unwrap();
            }
            segments.push((memory_object, *map_at));
        }

        let task = std::poplar::syscall::spawn_task(&service.name, address_space, service.entry_point).unwrap();
        services.push(Service { name: service.name.clone(), address_space, segments, task });
    }
}
