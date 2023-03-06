/// Determines the maximum number of CPUs that can be used by the kernel. If more CPUs are detected,
/// the kernel will panic. The limit is a little arbitrary, but it is set to 32 to avoid using too
/// much memory for per-cpu data, and should be enough for most use cases.
pub const MAX_CPU: usize = 32;
pub const IRQ_BASE: u8 = 32;