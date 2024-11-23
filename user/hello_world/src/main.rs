use service_host::ServiceHostClient;

fn main() {
    std::poplar::syscall::early_log("Hello, World!").unwrap();
    // println!("Hello, world!");

    let service_host = ServiceHostClient::new();
    let service_channel = service_host.register_service("hello_world").unwrap();
}
