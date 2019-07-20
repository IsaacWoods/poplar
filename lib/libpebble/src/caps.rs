#[derive(Debug)]
pub enum Capability {
    /*
     * Capabilities related to kernel objects.
     */
    CreateAddressSpace,
    CreateMemoryObject,
    CreateTask,

    /*
     * Capabilities specific to tasks running on x86_64.
     */
    X86_64AccessIoPort(u16),

    /*
     * Capabilities that are owned by drivers / support services.
     */
    MapFramebuffer,
    EarlyLogging,
}
