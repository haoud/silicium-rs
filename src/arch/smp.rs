use core::mem::size_of;

use x86_64::cpu::msr;

use crate::mm::vmm::{self, AllocationFlags};

#[repr(C)]
struct ThreadLocalInfo {
    // Pointer to the TLS info, DO NOT MOVE (used by the compiler)
    self_ptr: *const ThreadLocalInfo,
    cpu_id: u64,
}

pub fn bsp_setup() {
    unsafe {
        allocate_thread_local_storage();
    }
}

pub fn ap_setup() {}

pub fn start_ap() {}

/// Allocate the thread local storage for the current CPU
///
/// # Safety
/// This function is unsafe because it heavy relies on raw pointers manipulation and some concepts
/// that Rust doesn't really like (like self-referential structs), but this is safe because we know
/// that this pointer will be only be used by the compiler to access thread-local variables, and we
/// ensure that the pointer is valid and everything is properly initialized.
unsafe fn allocate_thread_local_storage() {
    extern "C" {
        static __per_cpu_start: u64;
        static __per_cpu_end: u64;
    }

    let per_cpu_start = core::ptr::addr_of!(__per_cpu_start) as usize;
    let per_cpu_end = core::ptr::addr_of!(__per_cpu_end) as usize;
    let per_cpu_size = per_cpu_end - per_cpu_start;

    let alloc_flags = AllocationFlags::MAP | AllocationFlags::ZEROED;
    let alloc_size = per_cpu_size + size_of::<ThreadLocalInfo>();

    let Ok(data) = vmm::allocate(alloc_size, alloc_flags) else {
        panic!("Failed to allocate {} bytes for thread local storage", alloc_size);
    };

    let tls_info = (data.start() + per_cpu_size).as_u64() as *mut ThreadLocalInfo;
    (*tls_info).self_ptr = tls_info;
    (*tls_info).cpu_id = 0;

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
