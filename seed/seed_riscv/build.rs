fn main() {
    // TODO: wonder if we can do this based on a platform feature?
    println!("cargo:rerun-if-changed=rv64_virt.ld");
    println!("cargo:rerun-if-changed=mq_pro.ld");
}
