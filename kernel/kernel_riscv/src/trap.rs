use crate::interrupts;
use core::arch::naked_asm;
use hal::memory::VAddr;
use hal_riscv::{
    hw::csr::{Scause, Sepc, Stvec},
    platform::kernel_map,
};
use tracing::info;

/// Install the proper trap handler. This handler is able to take traps from both S-mode and
/// U-mode, but requires the `sscratch` context to be correctly installed to facilitate switching
/// to the kernel's stack correctly. It therefore cannot be used during early initialization.
pub fn install_full_handler() {
    Stvec::set(VAddr::new(trap_handler_shim as extern "C" fn() -> ! as usize));
}

#[no_mangle]
extern "C" fn trap_handler(trap_frame: &mut TrapFrame, scause: usize, stval: usize) {
    match Scause::try_from(scause) {
        Ok(Scause::UEnvironmentCall) => {
            // TODO: it'd be cool to be finer-grained with allowing user memory access? We have
            // `UserSlice` types etc for this sort of thing. (assuming a CSR write isn't too
            // expensive for multiple xs per syscall?)
            hal_riscv::hw::csr::Sstatus::enable_user_memory_access();
            trap_frame.a0 = kernel::syscall::handle_syscall(
                crate::SCHEDULER.get(),
                crate::KERNEL_PAGE_TABLES.get(),
                trap_frame.a0,
                trap_frame.a1,
                trap_frame.a2,
                trap_frame.a3,
                trap_frame.a4,
                trap_frame.a5,
            );
            hal_riscv::hw::csr::Sstatus::disable_user_memory_access();
            trap_frame.sepc += 4;
        }
        Ok(Scause::SupervisorExternalInterrupt) => {
            interrupts::handle_external_interrupt();
        }
        Ok(Scause::SupervisorTimerInterrupt) => {
            crate::SCHEDULER.get().tasklet_scheduler.advance_timer(1);
            // Schedule the next tick in 20ms time (TODO: I have no idea what a sensible interval
            // should be). `Timer::advance` returns a `Turn` struct that tells us when the next
            // deadline is - the most efficient thing if this is all we need the timer interrupt
            // for would be to wait til then?
            sbi_rt::set_timer(hal_riscv::hw::csr::Time::read() as u64 + 0x989680 / 50).unwrap();
        }
        Ok(other) => {
            info!("Trap! Cause = {:?}. Stval = {:#x?}", other, stval);
            if trap_frame.sepc < usize::from(kernel_map::KERNEL_ADDRESS_SPACE_START) {
                let cpu_scheduler = crate::SCHEDULER.get().for_this_cpu();
                info!("Trap occurred in user task: {}", cpu_scheduler.running_task.as_ref().unwrap().name);
            }
            info!("Trap frame: {:#x?}", trap_frame);
            panic!();
        }
        Err(()) => panic!("Unrecognised trap cause!"),
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct TrapFrame {
    sepc: usize,
    ra: usize,
    sp: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    s0: usize,
    s1: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
}

#[repr(align(4))]
#[naked]
extern "C" fn trap_handler_shim() -> ! {
    unsafe {
        naked_asm!(
            "
            .attribute arch, \"rv64imac\"

            // Swap `sscratch` and `t6` to provide an initial working register
            csrrw t6, sscratch, t6

            // Save the current stack pointer and move to the kernel stack
            sd sp, 24(t6)
            ld sp, 0(t6)

            // Push a trap frame
            addi sp, sp, -256
            sd ra, 8(sp)
            ld ra, 24(t6)    // Use `ra` now it's been saved to retrieve the user `sp`
            sd ra, 16(sp)
            sd gp, 24(sp)
            sd tp, 32(sp)
            sd t0, 40(sp)
            sd t1, 48(sp)
            sd t2, 56(sp)
            sd s0, 64(sp)
            sd s1, 72(sp)
            sd a0, 80(sp)
            sd a1, 88(sp)
            sd a2, 96(sp)
            sd a3, 104(sp)
            sd a4, 112(sp)
            sd a5, 120(sp)
            sd a6, 128(sp)
            sd a7, 136(sp)
            sd s2, 144(sp)
            sd s3, 152(sp)
            sd s4, 160(sp)
            sd s5, 168(sp)
            sd s6, 176(sp)
            sd s7, 184(sp)
            sd s8, 192(sp)
            sd s9, 200(sp)
            sd s10, 208(sp)
            sd s11, 216(sp)
            sd t3, 224(sp)
            sd t4, 232(sp)
            sd t5, 240(sp)
            // NOTE: `t6` still contains the contents of `sscratch` so we can't save it here
            
            // Load the kernel's thread and global pointers so we can call into kernel code
            ld tp, 8(t6)
            ld gp, 16(t6)

            // Jiggle stuff around again to save `t6` and restore `sscratch`
            csrrw t6, sscratch, t6
            sd t6, 248(sp)

            // Save `sepc` - we store & restore this so the trap handler can change it to skip instructions
            csrr t6, sepc
            sd t6, 0(sp)

            // Load arguments - trap frame in `a0`, and `scause` and `stval` while we're here
            mv a0, sp
            csrr a1, scause
            csrr a2, stval

            call trap_handler

            // Restore `sepc`
            ld t6, 0(sp)
            csrw sepc, t6

            // Restore registers
            ld ra, 8(sp)
            // Skip `sp` - we're still using it
            ld gp, 24(sp)
            ld tp, 32(sp)
            ld t0, 40(sp)
            ld t1, 48(sp)
            ld t2, 56(sp)
            ld s0, 64(sp)
            ld s1, 72(sp)
            ld a0, 80(sp)
            ld a1, 88(sp)
            ld a2, 96(sp)
            ld a3, 104(sp)
            ld a4, 112(sp)
            ld a5, 120(sp)
            ld a6, 128(sp)
            ld a7, 136(sp)
            ld s2, 144(sp)
            ld s3, 152(sp)
            ld s4, 160(sp)
            ld s5, 168(sp)
            ld s6, 176(sp)
            ld s7, 184(sp)
            ld s8, 192(sp)
            ld s9, 200(sp)
            ld s10, 208(sp)
            ld s11, 216(sp)
            ld t3, 224(sp)
            ld t4, 232(sp)
            ld t5, 240(sp)
            ld t6, 248(sp)

            // Synchronise pending atomic reservations
            sc.d zero, zero, 0(sp)

            // Restore `sp`
            ld sp, 16(sp)

            sret
            "
        )
    }
}

/// Install the early trap handler - this should be installed very early in the kernel, and swapped
/// out once the task-scheduling infrastructure is up, as it uses a more advanced mechanism to
/// handle traps from both S-mode and U-mode.
pub fn install_early_handler() {
    Stvec::set(VAddr::new(early_trap_handler as extern "C" fn() -> ! as usize));
}

#[repr(align(4))]
extern "C" fn early_trap_handler() -> ! {
    let scause = Scause::read();
    let sepc = Sepc::read();
    panic!("Trap! Scause = {:?}, sepc = {:?}", scause, sepc);
}
