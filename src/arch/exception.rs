use crate::arch::paging;
use x86_64::address::Virtual;
use x86_64::cpu::Privilege;
use x86_64::idt::Descriptor;
use x86_64::idt::DescriptorFlags;
use x86_64::paging::PageFaultErrorCode;
use x86_64::{cpu::State, interrupt_handler};

pub fn setup() {
    register_exception_handler(0, divide_by_zero);
    register_exception_handler(1, debug);
    register_exception_handler(2, non_maskable_interrupt);
    register_exception_handler(3, breakpoint);
    register_exception_handler(4, overflow);
    register_exception_handler(5, bound_range_exceeded);
    register_exception_handler(6, invalid_opcode);
    register_exception_handler(7, device_not_available);
    register_exception_handler(8, double_fault);
    register_exception_handler(9, coprocessor_segment_overrun);
    register_exception_handler(10, invalid_tss);
    register_exception_handler(11, segment_not_present);
    register_exception_handler(12, stack_segment_fault);
    register_exception_handler(13, general_protection_fault);
    register_exception_handler(14, page_fault);
    register_exception_handler(15, reserved_1);
    register_exception_handler(16, x87_floating_point);
    register_exception_handler(17, alignment_check);
    register_exception_handler(18, machine_check);
    register_exception_handler(19, simd);
    register_exception_handler(20, virtualization);
    register_exception_handler(21, control_protection);
    register_exception_handler(22, reserved_2);
    register_exception_handler(23, reserved_3);
    register_exception_handler(24, reserved_4);
    register_exception_handler(25, reserved_5);
    register_exception_handler(26, reserved_6);
    register_exception_handler(27, reserved_7);
    register_exception_handler(28, hypervisor_injection);
    register_exception_handler(29, virtualization);
    register_exception_handler(30, security_exception);
    register_exception_handler(31, reserved_8);
}

#[allow(clippy::fn_to_numeric_cast)]
fn register_exception_handler(index: u8, handler: unsafe extern "C" fn()) {
    let mut idt = crate::arch::idt::IDT.lock();
    let flags = DescriptorFlags::new()
        .set_privilege_level(Privilege::KERNEL)
        .present(true)
        .build();
    let descriptor = Descriptor::new()
        .set_handler_addr(handler as u64)
        .set_options(flags)
        .build();
    idt.set_descriptor(index, descriptor);
}

pub extern "C" fn divide_by_zero_handler(_state: State) {
    panic!("Divide by zero exception");
}

pub extern "C" fn debug_handler(_state: State) {
    panic!("Debug exception");
}

pub extern "C" fn non_maskable_interrupt_handler(_state: State) {
    // Just freeze the CPU. This is used by the panic function to halt other core.
    // This is a temporary solution, but it works.
    x86_64::cpu::freeze();
}

pub extern "C" fn breakpoint_handler(_state: State) {
    panic!("Breakpoint exception");
}

pub extern "C" fn overflow_handler(_state: State) {
    panic!("Overflow exception");
}

pub extern "C" fn bound_range_exceeded_handler(_state: State) {
    panic!("Bound range exceeded exception");
}

pub extern "C" fn invalid_opcode_handler(_state: State) {
    panic!("Invalid opcode exception");
}

pub extern "C" fn device_not_available_handler(_state: State) {
    panic!("Device not available exception");
}

pub extern "C" fn double_fault_handler(_state: State) {
    panic!("Double fault");
}

pub extern "C" fn coprocessor_segment_overrun_handler(_state: State) {
    panic!("Coprocessor segment overrun exception");
}

pub extern "C" fn invalid_tss_handler(_state: State) {
    panic!("Invalid TSS exception");
}

pub extern "C" fn segment_not_present_handler(_state: State) {
    panic!("Segment not present exception");
}

pub extern "C" fn stack_segment_fault_handler(_state: State) {
    panic!("Stack segment fault exception");
}

pub extern "C" fn general_protection_fault_handler(state: State) {
    panic!(
        "General protection fault (error code: 0x{:02x})",
        state.code
    );
}

pub extern "C" fn page_fault_handler(state: State) {
    let code = PageFaultErrorCode::from_bits_truncate(state.code);
    let addr = Virtual::new(x86_64::cpu::read_cr2());

    if let Err(reason) = paging::handle_page_fault(code, addr) {
        panic!(
            "Unrecoverable page fault at {:016x}: {:?}",
            addr.as_u64(),
            reason
        );
    }
}

pub extern "C" fn reserved_handler(_state: State) {
    panic!("Reserved exception");
}

pub extern "C" fn x87_floating_point_handler(_state: State) {
    panic!("x87 floating point exception");
}

pub extern "C" fn alignment_check_handler(_state: State) {
    panic!("Alignment check exception");
}

pub extern "C" fn machine_check_handler(_state: State) {
    panic!("Machine check exception");
}

pub extern "C" fn simd_floating_point_handler(_state: State) {
    panic!("SIMD floating point exception");
}

pub extern "C" fn virtualization_handler(_state: State) {
    panic!("Virtualization exception");
}

pub extern "C" fn control_protection_handler(_state: State) {
    panic!("Control protection exception");
}

pub extern "C" fn hypervisor_injection_handler(_state: State) {
    panic!("Hypervisor injection exception");
}

pub extern "C" fn vmm_communication_handler(_state: State) {
    panic!("Hypervisor injection exception");
}

pub extern "C" fn security_exception_handler(_state: State) {
    panic!("Security exception");
}

interrupt_handler!(0, divide_by_zero, divide_by_zero_handler, 0);
interrupt_handler!(1, debug, debug_handler, 0);
interrupt_handler!(2, non_maskable_interrupt, non_maskable_interrupt_handler, 0);
interrupt_handler!(3, breakpoint, breakpoint_handler, 0);
interrupt_handler!(4, overflow, overflow_handler, 0);
interrupt_handler!(5, bound_range_exceeded, bound_range_exceeded_handler, 0);
interrupt_handler!(6, invalid_opcode, invalid_opcode_handler, 0);
interrupt_handler!(7, device_not_available, device_not_available_handler, 0);
interrupt_handler!(8, double_fault, double_fault_handler);
#[rustfmt::skip]
interrupt_handler!(9,coprocessor_segment_overrun, coprocessor_segment_overrun_handler, 0);
interrupt_handler!(10, invalid_tss, invalid_tss_handler);
interrupt_handler!(11, segment_not_present, segment_not_present_handler);
interrupt_handler!(12, stack_segment_fault, stack_segment_fault_handler);
#[rustfmt::skip]
interrupt_handler!(13,general_protection_fault, general_protection_fault_handler);
interrupt_handler!(14, page_fault, page_fault_handler);
interrupt_handler!(15, reserved_1, reserved_handler, 0);
interrupt_handler!(16, x87_floating_point, x87_floating_point_handler, 0);
interrupt_handler!(17, alignment_check, alignment_check_handler);
interrupt_handler!(18, machine_check, machine_check_handler, 0);
interrupt_handler!(19, simd, simd_floating_point_handler, 0);
interrupt_handler!(20, virtualization, virtualization_handler, 0);
interrupt_handler!(21, control_protection, control_protection_handler);
interrupt_handler!(22, reserved_2, reserved_handler, 0);
interrupt_handler!(23, reserved_3, reserved_handler, 0);
interrupt_handler!(24, reserved_4, reserved_handler, 0);
interrupt_handler!(25, reserved_5, reserved_handler, 0);
interrupt_handler!(26, reserved_6, reserved_handler, 0);
interrupt_handler!(27, reserved_7, reserved_handler, 0);
interrupt_handler!(28, hypervisor_injection, hypervisor_injection_handler, 0);
interrupt_handler!(29, vmm_communication, vmm_communication_handler);
interrupt_handler!(30, security_exception, security_exception_handler);
interrupt_handler!(31, reserved_8, reserved_handler, 0);
