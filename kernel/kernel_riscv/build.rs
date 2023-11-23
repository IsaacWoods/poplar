fn main() {
    println!("cargo:rerun-if-changed=rv64_virt.ld");
    println!("cargo:rerun-if-changed=mq_pro.ld");
}
