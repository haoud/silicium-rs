use alloc::{collections::BTreeMap, vec::Vec};
use bitflags::bitflags;
use log::trace;
use x86_64::{
    address::{Virtual, VirtualRange},
    paging::{PageTable, PAGE_SIZE},
};

use crate::{
    arch::paging::{self, map, MapError, MapFlags, PageFaultError, ACTIVE_TABLE},
    Spinlock,
};

use super::{
    frame::{self, Allocator, Frame},
    FRAME_ALLOCATOR,
};

bitflags! {
    pub struct AllocationFlags : u64 {
        const NONE = 0;

        /// Perform an atomic allocation, i.e the function will not block.
        /// If the flags `MAP` and `ATOMIC` are set, the function will map all the range before
        /// returning (and therefore, disable demand paging for this range). If an blocking
        /// operation is required, the function will return `None`.
        const ATOMIC = 1 << 1;

        /// When set, the allocated memory will be mapped in the kernel's address space.
        const MAP = 1 << 2;

        /// When set, and only when `MAP` is set, the mapped memory will be zeroed.
        const ZEROED = 1 << 3;
    }

    struct Flags : u64 {
        const NONE = 0;
        const MAP = AllocationFlags::MAP.bits;
        const ZEROED = AllocationFlags::ZEROED.bits;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    /// An allocation failed because there is no free vma that can fit the requested size, or
    /// if a physical frame could not be allocated.
    OutOfMemory,

    /// An allocation failed because it would block and the `ATOMIC` flag was set.
    WouldBlock,
}

/// A virtual memory area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualArea {
    range: VirtualRange,
    flags: Flags,
}

impl VirtualArea {
    pub fn new(range: VirtualRange, flags: Flags) -> Self {
        Self { range, flags }
    }
}

static FREE_VMA: Spinlock<BTreeMap<usize, Vec<VirtualArea>>> = Spinlock::new(BTreeMap::new());
static USED_VMA: Spinlock<BTreeMap<Virtual, VirtualArea>> = Spinlock::new(BTreeMap::new());

/// Set the allocator of virtual memory.
pub fn setup() {
    insert_free_vma(VirtualArea::new(
        VirtualRange::new(
            Virtual::new(super::VMALLOC_START),
            Virtual::new(super::VMALLOC_END),
        ),
        Flags::NONE,
    ));
}

/// Find a free vma that can fit the given size. If the size is not page aligned, it will be
/// rounded up to the next multiple of 4096.
///
/// # Errors
/// This function will return the range allocated if it succeeds, or an error if it fails :
/// - `AllocationError::OutOfMemory`: No free vma can fit the given size.
pub fn allocate(size: usize, flags: AllocationFlags) -> Result<VirtualRange, AllocationError> {
    // Align the size to the next multiple of 4096
    let aligned_size = (size.wrapping_add(0xFFF)) & !0xFFF;
    let mut vma = find_free_first_fit(aligned_size).ok_or(AllocationError::OutOfMemory)?;

    if flags.contains(AllocationFlags::ATOMIC) {
        unimplemented!("Atomic allocation is not implemented yet.");
    }

    vma.flags = Flags::from_bits_truncate(flags.bits);
    insert_used_vma(vma);
    Ok(vma.range)
}

/// Deallocate a vma. The parameter `base` must be the start of the vma, and therefore should be
/// page aligned.
///
/// # Panics
/// This function will panic if the vma is not found in the used vma list. It can be caused by:
/// - The vma simply does not exist.
/// - The vma was already deallocated.
/// - The base address is not the start of the vma.
pub fn deallocate(range: VirtualRange) {
    let vma = x86_64::irq::without(|| {
        let mut used_vmas = USED_VMA.lock();
        used_vmas.remove(&range.start()).unwrap()
    });

    if vma.flags.contains(Flags::MAP) {
        // Unmap the range of the vma
        for page in vma.range.iter().step_by(PAGE_SIZE) {
            unsafe {
                let current = &mut ACTIVE_TABLE.lock();
                let frame = paging::unmap(current, page);
                if let Some(frame) = frame {
                    x86_64::irq::without(|| {
                        FRAME_ALLOCATOR.lock().deallocate(Frame::new(frame));
                    });
                }
            }
        }
    }
    // TODO: Merge with adjacent free vma
    insert_free_vma(vma);
}

/// Handle a page fault occuring in vmalloc space.
///
/// # Errors
/// - `PageFaultError::MISSING_PAGE`: The page fault occured in an unused vma.
/// - `PageFaultError::NOT_MAPPABLE`: The page fault occured in a vma that is not mappable.
/// - `PageFaultError::OUT_OF_MEMORY`: The page fault occured in a vma that is mappable, but the
///    allocation of a frame failed.
pub fn handle_demand_paging(table: &mut PageTable, addr: Virtual) -> Result<(), PageFaultError> {
    let addr = addr.page_align_down();
    // Find the vma that contains the address
    let vma = x86_64::irq::without(|| {
        let used_vmas = USED_VMA.lock();
        used_vmas
            .iter()
            .find(|(_, vma)| vma.range.contains(addr))
            .map(|(_, vma)| *vma)
            .ok_or(PageFaultError::MISSING_PAGE)
    })?;

    if !vma.flags.contains(Flags::MAP) {
        return Err(PageFaultError::NOT_MAPPABLE);
    }

    unsafe {
        let paging_flags = MapFlags::PRESENT | MapFlags::WRITABLE;
        let frame_flags = if vma.flags.contains(Flags::ZEROED) {
            frame::AllocationFlags::ZEROED
        } else {
            frame::AllocationFlags::NONE
        };
        let frame = x86_64::irq::without(|| {
            FRAME_ALLOCATOR
                .lock()
                .allocate(frame_flags)
                .ok_or(PageFaultError::OUT_OF_MEMORY)
        })?;
        trace!(
            "Page fault handler: demand paging: {:016x} -> {:016x}",
            addr,
            frame.start()
        );
        match map(table, addr, frame, paging_flags) {
            Ok(_) => Ok(()),
            Err(e) => match e {
                MapError::OutOfMemory => Err(PageFaultError::OUT_OF_MEMORY),
                MapError::AlreadyMapped => panic!("Page already mapped"),
            },
        }
    }
}

/// Insert a vma in the free vma list.
fn insert_free_vma(vma: VirtualArea) {
    x86_64::irq::without(|| {
        let mut free_vmas = FREE_VMA.lock();
        if let Some(vmas) = free_vmas.get_mut(&vma.range.size()) {
            vmas.push(vma);
        } else {
            let length = vma.range.size();
            let vmas = alloc::vec![vma];
            free_vmas.insert(length, vmas);
        }
    });
}

/// Insert a vma in the used vma list.
fn insert_used_vma(vma: VirtualArea) {
    x86_64::irq::without(|| {
        USED_VMA.lock().insert(vma.range.start(), vma);
    });
}

/// Find the first free vma that is big enough to allocate the requested size, remove it from
/// the free vma list, split it if necessary and replace it in the free vma list, and return the
/// allocated vma.
///
/// # Returns
/// The first free vma that is big enough to allocate the requested size, or `None` if no such vma
/// exists.
fn find_free_first_fit(size: usize) -> Option<VirtualArea> {
    let mut free_vmas = FREE_VMA.lock();
    let mut vma = free_vmas
        .iter_mut()
        .find(|(len, vec)| **len >= size && !vec.is_empty())
        .map(|(_, vma_list)| vma_list)?
        .pop()
        .unwrap();

    // If the vma is bigger than the requested size, split it
    if vma.range.size() > size {
        let split = VirtualArea::new(
            VirtualRange::new(vma.range.start() + size, vma.range.end()),
            Flags::NONE,
        );
        vma.range = VirtualRange::new(vma.range.start(), vma.range.start() + size);

        // Insert the split vma in the free vma list
        if let Some(vmas) = free_vmas.get_mut(&split.range.size()) {
            vmas.push(split);
        } else {
            let length = split.range.size();
            let vmas = alloc::vec![split];
            free_vmas.insert(length, vmas);
        }
    }

    Some(vma)
}
