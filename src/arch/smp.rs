use core::{
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};

use limine::LimineSmpInfo;
use x86_64::{address::Virtual, cpu::msr};

use crate::{
    config::MAX_CPU,
    mm::vmm::{self, AllocationFlags},
};

/// Represent the thread local information for a CPU. This structure is used by the compiler to
/// access the TLS (the `self_ptr` field is only to make the TLS work, it is not used by the kernel).
/// It contains also the LAPIC id, the CPU id and the base address of the TLS for the current CPU.
#[repr(C)]
pub struct ThreadLocalInfo {
    /// Pointer to the TLS info, used by the compiler to access the TLS: Do not move !
    self_ptr: *const ThreadLocalInfo,
    pub tls_base: Virtual,
    pub lapic_id: u32,
    pub cpu_id: u32,
}

/// Represent the number of CPUs that have started. After the initialization of the kernel, this
/// variable could be used to determine the number of CPUs in the system.
pub static CPU_COUNT: AtomicU64 = AtomicU64::new(1);

/// Allocate the thread local storage for the current CPU. The caller CPU must be the BSP, otherwise
/// the behavior is undefined.
pub fn bsp_setup() {
    let reponse = crate::LIMINE_SMP.get_response().get_mut().unwrap();
    let smp_info = reponse
        .cpus()
        .iter()
        .find(|cpu| cpu.processor_id == 0)
        .unwrap();
    unsafe {
        allocate_thread_local_storage(smp_info);
    }
}

/// This function is called by the APs when they start. It will initialize the current core and
/// signal to the BSP when the core is ready.
///
/// This function should not be called directly, but only by the `_ap_start` function.
pub fn ap_start(smp_info: &LimineSmpInfo) -> ! {
    super::gdt::reload();
    super::idt::reload();
    unsafe {
        allocate_thread_local_storage(smp_info);
        x86_64::lapic::enable();
    }
    super::tss::install(smp_info.processor_id as usize);

    // Signal to the BSP that the AP is ready and freeze the core (for now)
    CPU_COUNT.fetch_add(1, Ordering::Relaxed);
    x86_64::cpu::freeze();
}

/// Start all the APs and wait for them before returning. If an AP fails to start, this function
/// will be stuck forever in an infinite loop. I think this is the best behavior, because if an AP
/// fails to start, it means that something is wrong with the system and the kernel should not
/// continue to run.
pub fn start_cpus() {
    let reponse = crate::LIMINE_SMP.get_response().get_mut().unwrap();
    assert!(!reponse.cpus().is_empty(), "No core found");
    assert!(reponse.cpus().len() <= MAX_CPU, "Too many core found");
    for cpu in reponse.cpus().iter_mut().filter(|cpu| cpu.lapic_id != 0) {
        log::debug!("Starting AP {}", cpu.lapic_id);
        cpu.goto_address = crate::arch::_ap_start;
    }

    // Wait for all APs to start
    while CPU_COUNT.load(Ordering::Relaxed) != reponse.cpus().len() as u64 {
        core::hint::spin_loop();
    }
    log::info!("All APs started");
}

/// Get the thread local structure for the current CPU. See `ThreadLocalInfo` for more information
/// about this structure.
#[must_use]
pub fn get_cpu_info() -> &'static ThreadLocalInfo {
    // SAFETY: This is safe because the pointer is valid (never freed during the lifetime of the
    // kernel) and the structure is properly initialized. We also only deliver a reference to it,
    // so the caller can't modify it.
    unsafe { &*(msr::read(msr::Register::KernelGsBase) as *const ThreadLocalInfo) }
}

/// Allocate the thread local storage for the current CPU
///
/// # Safety
/// This function is unsafe because it heavy relies on raw pointers manipulation and some concepts
/// that Rust doesn't really like (like self-referential structs), but this is safe because we know
/// that this pointer will be only be used by the compiler to access thread-local variables, and we
/// ensure that the pointer is valid and everything is properly initialized.
unsafe fn allocate_thread_local_storage(smp_info: &LimineSmpInfo) {
    extern "C" {
        static __per_cpu_start: u64;
        static __per_cpu_end: u64;
    }

    let per_cpu_start = core::ptr::addr_of!(__per_cpu_start) as usize;
    let per_cpu_end = core::ptr::addr_of!(__per_cpu_end) as usize;
    let per_cpu_size = per_cpu_end - per_cpu_start;

    let alloc_flags = AllocationFlags::MAP | AllocationFlags::ZEROED;
    let alloc_size = per_cpu_size + size_of::<ThreadLocalInfo>();

    // Allocate the memory for the TLS
    let data = match vmm::allocate(alloc_size, alloc_flags) {
        Ok(x) => x,
        Err(e) => panic!(
            "Failed to allocate {} bytes for thread local storage: {:?}",
            alloc_size, e
        ),
    };

    // Initialize the TLS info structure
    let tls_info = (data.start() + per_cpu_size).as_u64() as *mut ThreadLocalInfo;
    (*tls_info).cpu_id = smp_info.processor_id;
    (*tls_info).lapic_id = smp_info.lapic_id;
    (*tls_info).tls_base = data.start();
    (*tls_info).self_ptr = tls_info;

    // Copy the per-cpu data from the kernel to the allocated memory
    core::ptr::copy_nonoverlapping(
        per_cpu_start as *const u8,
        data.start().as_u64() as *mut u8,
        per_cpu_size,
    );

    // Set the GS Kernel Base MSR to the address of the TLS info
    //
    // Unfortunately, we must also set the FS Base MSR to the same address, because the Rust
    // compiler uses the FS register to access thread-local variables (as user applications do), but
    // this is problematic our kernel use the FS register for the TLS info too. So we must set the
    // FS Base MSR to the same address as the GS Kernel Base MSR, and save/restore the FS register
    // when switching between kernel and user mode.
    msr::write(msr::Register::KernelGsBase, tls_info as u64);
    msr::write(msr::Register::FsBase, tls_info as u64);
}
