#[derive(Debug, Clone, Copy)]
#[repr(C, packed(4))]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub stack_table: [u64; 3],
    reserved_2: u64,
    pub interrupt_stack_table: [u64; 7],
    reserved_3: u64,
    reserved_4: u16,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn new() -> Self {
        Self {
            reserved_1: 0,
            stack_table: [0; 3],
            reserved_2: 0,
            interrupt_stack_table: [0; 7],
            reserved_3: 0,
            reserved_4: 0,
            iomap_base: core::mem::size_of::<TaskStateSegment>() as u16,
        }
    }

    #[must_use]
    pub const fn as_ptr(&self) -> *const Self {
        self as *const Self
    }
}

#[cfg(test)]
mod test {
    use core::mem::size_of;

    #[test]
    fn struct_size_checks() {
        assert_eq!(size_of::<super::TaskStateSegment>(), 104);
    }
}
