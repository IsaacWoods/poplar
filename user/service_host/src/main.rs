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
 *  - add a channel to each new task for task discovery + management
 *  - kernel will fill in a manifest for each new task detailing its handles (incl our channel)
 *  - provide task registration and discovery through the channel
 *  - provide a special task ourselves for the console to e.g. list tasks running, get system
 *    status, etc.
 *  - move PCI info + objects, kernel framebuffer, etc. to be passed to this task and then onwards
 *  - thinking: we need a mechanism for tasks to be able to ask us for specific objects (e.g.
 *    PCI info to platform_bus)
 */

use log::{info, warn};
use service_host::{ServiceChannelMessage, ServiceHostRequest, ServiceHostResponse};
use std::{
    collections::btree_map::BTreeMap,
    poplar::{channel::Channel, early_logger::EarlyLogger, manifest::BootstrapManifest, Handle},
};

pub struct Task {
    name: String,
    address_space: Handle,
    segments: Vec<(Handle, usize)>,
    task: Handle,
    task_channel: Channel<ServiceHostResponse, ServiceHostRequest>,
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

    let mut tasks = Vec::new();
    let mut services: BTreeMap<String, Channel<ServiceChannelMessage, ()>> = BTreeMap::new();

    for task in &manifest.boot_tasks {
        info!("Spawning task '{}'", task.name);
        let address_space = std::poplar::syscall::create_address_space().unwrap();
        let mut segments = Vec::new();
        for (map_at, memory_object) in &task.segments {
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

        // Create a channel to communicate with the task through
        let (task_channel, channel_handle) = Channel::create().unwrap();

        let spawned_task =
            std::poplar::syscall::spawn_task(&task.name, address_space, task.entry_point, &[channel_handle])
                .unwrap();
        tasks.push(Task { name: task.name.clone(), address_space, segments, task: spawned_task, task_channel });
    }

    // Monitor each task's channel for requests
    // TODO: this should probs be async in the future
    loop {
        std::poplar::syscall::yield_to_kernel();
        for task in &tasks {
            if let Some(request) = task.task_channel.try_receive().unwrap() {
                match request {
                    ServiceHostRequest::RegisterService { name } => {
                        // TODO: check for service name conflicts and send back an error
                        info!("Task '{}' registering new service '{}'", task.name, name);
                        let (service_channel, channel_handle) = Channel::create().unwrap();
                        task.task_channel.send(&ServiceHostResponse::ServiceRegistered(channel_handle)).unwrap();
                        services.insert(name, service_channel);
                    }
                    ServiceHostRequest::SubscribeService(name) => {
                        info!("Task '{}' subscribing to service called '{}'", task.name, name);
                        if let Some(ref service_channel) = services.get(&name) {
                            let (channel_a, channel_b) = std::poplar::syscall::create_channel().unwrap();
                            service_channel
                                .send(&ServiceChannelMessage::NewClient {
                                    name: task.name.clone(),
                                    channel: channel_a,
                                })
                                .unwrap();
                            task.task_channel.send(&ServiceHostResponse::SubscribedToService(channel_b)).unwrap();
                        } else {
                            /*
                             * Now there's more to service registration, we probs need to actually
                             * handle this... I wonder if we should keep a list of 'waiting' tasks
                             * that want access to a service, and check it when a new service is
                             * registered. We defo can't just ignore it (but this should be
                             * customizable behaviour. Some clients might just want to check if a
                             * service is available, but not block on it becoming available).
                             */
                            warn!("Tried to subscribe to service but it has not been registered!");
                        }
                    }
                    ServiceHostRequest::RequestResource(name) => todo!(),
                }
            }
        }
    }
}
