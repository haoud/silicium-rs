use core::ops::{Deref, DerefMut};
use core::ptr::copy_nonoverlapping;
use core::sync::atomic::Ordering;

use alloc::sync::Arc;
use bitflags::bitflags;
use log::trace;
use spin::Lazy;

use crate::mm::frame::{AllocationFlags, Allocator, Frame};
use crate::mm::{frame, FRAME_ALLOCATOR, KERNEL_BASE};
use crate::{mm, Spinlock, EARLY};

use x86_64::address::{Physical, Virtual};
use x86_64::cpu;
use x86_64::paging::PageEntry;
use x86_64::paging::PageEntryFlags;
use x86_64::paging::PageFaultErrorCode;
use x86_64::paging::PageTable;
use x86_64::paging::{self, PAGE_MASK};

use super::address::{phys_to_virt, virt_to_phys};

pub type MapFlags = PageEntryFlags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapError {
    OutOfMemory,
    AlreadyMapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageFaultType {
    LazyTlbInvalidation,
    DemandPaging,
}

bitflags! {
    pub struct PageFaultError: u64 {
        /// Set if the kernel wasn't able to determine the cause of the fault
        const UNKNOWN = 0;

        /// Set if the fault was caused by a page not present in memory
        const MISSING_PAGE = 1 << 1;

        /// Set if the fault was caused by a protection violation, i.e. the page was present but the
        /// access was not allowed (user tried to access a kernel page)
        const PROTECTION_VIOLATION = 1 << 2;

        /// Set if the fault was caused by a write to a read-only page
        const WRITE_PROTECTED = 1 << 3;

        /// Set if the fault was caused instruction fetch from a page that is marked as not
        /// executable
        const NOT_EXECUTABLE = 1 << 4;

        /// Set if we ran out of memory while handling the fault
        const OUT_OF_MEMORY = 1 << 5;

        /// Set if the demand paging failed because we cannot map the page safely
        const NOT_MAPPABLE = 1 << 6;

        /// Set if the page is already mapped when it should not be
        const ALREADY_MAPPED = 1 << 7;
    }
}

/// Represents a root page table (PML4). This is a convenience wrapper around a `PageTable` that
/// allocates a frame for the table and provides a `Deref` implementation to access the table.
///
/// We cannot directly use a `PageTable` to represent a root page table because we need to
/// allocate a frame for it, to avoid overflowing the stack (by allocating a `PageTable` on the
/// stack, witch is 4 KiB large) but most importantly because we need to be able to have a fixed
/// address for the root page table, so that we can load it into the CR3 register without having
/// fear of it being moved by the compiler/allocator.
#[derive(Debug)]
pub struct TableRoot {
    addr: Virtual,
    frame: Frame,
}

impl TableRoot {
    /// Allocates a new root page table. This create an empty user space, but copy the current
    /// kernel space (wich is the same for all process).
    #[must_use]
    pub fn new() -> Self {
        let frame = unsafe {
            FRAME_ALLOCATOR
                .lock()
                .allocate(AllocationFlags::KERNEL | AllocationFlags::ZEROED)
                .unwrap()
        };

        // Copy the kernel space. We use the `INIT_TABLE` to copy the kernel space, because we have
        // preallocated all kernel pml4 entries in the `INIT_TABLE`, so we can just copy the
        // kernel space from there, and avoid locking the `ACTIVE_TABLE` (which should be more
        // efficient because it is less likely to be used by other threads).
        unsafe {
            copy_nonoverlapping(
                INIT_TABLE.lock().as_ptr().offset(256),
                frame.start().as_mut_ptr::<PageEntry>().offset(256),
                256,
            );
        }

        Self {
            addr: phys_to_virt(frame.start()),
            frame,
        }
    }

    /// Creates a new root page table from a physical address that is already mapped.
    ///
    /// # Safety
    /// This function is unsafe because it can lead to undefined behavior if (non-exhaustive list):
    /// - The physical address dropped before the `TableRoot` is dropped
    /// - The physical address is not allocated with the frame allocator AND no precautions are
    ///   taken to ensure that the frame can be deallocated (with `FRAME_ALLOCATOR.lock().deallocate()`)
    ///   when the `TableRoot` is dropped
    #[must_use]
    pub unsafe fn from(phys: Frame) -> Self {
        Self {
            addr: phys_to_virt(phys.start()),
            frame: phys,
        }
    }
}

impl Default for TableRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TableRoot {
    fn clone(&self) -> Self {
        let frame = unsafe {
            FRAME_ALLOCATOR
                .lock()
                .allocate(AllocationFlags::KERNEL | AllocationFlags::ZEROED)
                .expect("Failed to allocate a frame for the pml4 entry")
        };

        // Copy all the entries
        unsafe {
            copy_nonoverlapping(
                self.frame.start().as_ptr::<PageTable>(),
                frame.start().as_mut_ptr(),
                1,
            );
        }

        Self {
            addr: phys_to_virt(frame.start()),
            frame,
        }
    }
}

impl Deref for TableRoot {
    type Target = PageTable;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.addr.as_ptr() }
    }
}

impl DerefMut for TableRoot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.addr.as_mut_ptr() }
    }
}

impl Drop for TableRoot {
    fn drop(&mut self) {
        unsafe { FRAME_ALLOCATOR.lock().deallocate(self.frame) }
    }
}

/// Represents the table page when the kernel is loaded with Limine. This simply wrap the boot
/// pml4 frame into a `TableRoot` to keep a consistent interface.
pub static INIT_TABLE: Lazy<Arc<Spinlock<TableRoot>>> = Lazy::new(|| unsafe {
    let boot_pml4 = Frame::new(virt_to_phys(Virtual::new(active_table() as u64)));
    x86_64::irq::without(|| {
        FRAME_ALLOCATOR.lock().reference(boot_pml4);
    });
    Arc::new(Spinlock::new(TableRoot::from(boot_pml4)))
});

#[thread_local]
pub static ACTIVE_TABLE: Lazy<Arc<Spinlock<TableRoot>>> = Lazy::new(|| unsafe {
    let pml4 = INIT_TABLE.clone();
    change_table(&pml4.lock());
    pml4
});

/// Sets up the pagination system. This function does not many things, as the as most of the work
/// has been done by Limine. It only preallocate all the kernel pml4 entries and enable the NXE bit
/// and the WP bit.
///
/// ## Why we need to preallocate all the kernel pml4 entries ?
/// Because it will make our life easier
/// when we will implement user space. Indeed, when we will have many address spaces, we will need
/// to map some pages in the kernel address space. If we don't preallocate all the kernel pml4
/// entries, we will need at some point to allocate a new pml4 entry. But since we have several
/// address spaces, we will need to synchronize the pml4 entries between all address spaces. This is
/// not efficient nor easy to implement. So we just preallocate all the kernel pml4 entries, and
/// this and this comes with a nice bonus: to create a new address space, we just need to copy the
/// kernel pml4 entries and voilà, we have a new empty user address space.
pub fn setup() {
    // Preallocate all the kernel pml4 entries
    let mut table = ACTIVE_TABLE.lock();
    let start = Virtual::new(KERNEL_BASE).pml4_offset();
    let end = PageTable::COUNT as u64;

    for i in start..end {
        let pml4_entry = &mut table[i];
        if !pml4_entry.is_present() {
            unsafe {
                let frame = FRAME_ALLOCATOR
                    .lock()
                    .allocate(AllocationFlags::KERNEL)
                    .expect("Failed to allocate a frame for the kernel pml4 entry");
                let flags = PageEntryFlags::PRESENT | PageEntryFlags::WRITABLE;

                pml4_entry.set_address(frame.start());
                pml4_entry.set_flags(flags);
            }
        }
    }

    // All flags (NXE, WP...) are already set by Limine, no need to set them again
}

/// Sets up the pagination system for the current CPU. This function is called by the APs when
/// they are started. It just forces the `ACTIVE_TABLE` lazy static to be initialized.
pub fn ap_setup() {
    Lazy::force(&ACTIVE_TABLE);
}

/// Maps the given physical address to the given virtual address. If the given physical address is
/// null, this function allocates a new frame and maps it to the given virtual address.
///
/// # Errors
/// - `MapError::OutOfMemory`: There is no more memory available to create the page table that
///  maps the given virtual address.
/// - `MapError::AlreadyMapped`: The given virtual address is already mapped.
///
/// # Safety
/// This function is unsafe because it can lead to many, many undefined behaviors if used
/// incorrectly.
/// - You should not map the same physical address to two different virtual addresses in the kernel
/// space, because it could violate the memory safety of Rust (for user space, it's fine, because
/// the user space is not involved by the Rust memory safety).
pub unsafe fn map(
    table: &mut PageTable,
    at: Virtual,
    frame: Frame,
    flags: MapFlags,
) -> Result<(), MapError> {
    let pte = creat_and_fetch_pte(table, paging::Level::PageMapLevel4, at);
    if let Some(pte) = pte {
        if pte.is_present() {
            return Err(MapError::AlreadyMapped);
        }

        // If no frame is given, allocate one
        let frame = if frame.start().is_null() {
            x86_64::irq::without(|| {
                FRAME_ALLOCATOR
                    .lock()
                    .allocate(AllocationFlags::KERNEL | AllocationFlags::ZEROED)
                    .ok_or(MapError::OutOfMemory)
            })?
        } else {
            frame
        };

        // Here, we don't need to flush the TLB because we are creating a new entry and we can
        // use a lazy TLB invalidation. Indeed, the TLB is flushed only when a page fault occurs
        // (because the entry in the TLB is still to "not present"), and the page fault handler will
        // flush the TLB accordingly.
        // This is useful to avoid flushing the TLB too many times and saturating other cores with
        // TLB invalidation requests.
        pte.set_address(frame.start());
        pte.set_flags(flags);
        return Ok(());
    }
    Err(MapError::OutOfMemory)
}

/// Unmaps the given virtual address and returns the physical address of the unmapped page. If the
/// given virtual address is not mapped, this function does nothing and returns `None`, otherwise
/// it returns the physical address of the unmapped page, and it is the responsibility of the caller
/// to free the physical frame.
///
/// # Safety
/// This function is unsafe because it can lead to undefined behavior if a page in unmapped while
/// it is still in use. The caller must ensure that the page is not in use anymore (except if it is
/// the desired behavior, but this is probably not common.
pub unsafe fn unmap(table: &mut PageTable, at: Virtual) -> Option<Physical> {
    let pte = unsafe { fetch_pte_mut(table, paging::Level::PageMapLevel4, at) };
    if let Some(pte) = pte {
        if pte.is_present() {
            // Unmap the page and return the physical address
            let addr = pte.address().unwrap();
            let offset = at.as_u64() & 0xFFF;
            // Update the page table entry and flush the TLB with interrupts disabled
            // I flush the whole TLB because I don't know how to correctly
            // flush one entry with `invlpg`: do I need to invalidate the mapped virtual
            // address or the virtual address of the page table ?
            // TODO: Only flush one entry of the TLB
            x86_64::irq::without(|| {
                pte.clear();
                tlb::shootdown();
            });
            return Some(Physical::new(addr.as_u64() + offset));
        }
    }
    None
}

/// Returns the protection of the given virtual address. If the given virtual address is not mapped,
/// this function returns `None`, otherwise it returns the protection of the given virtual address.
pub fn protection(table: &mut PageTable, at: Virtual) -> Option<PageEntryFlags> {
    let pte = unsafe { fetch_pte(table, paging::Level::PageMapLevel4, at) };
    if let Some(pte) = pte {
        if pte.is_present() {
            return Some(pte.flags());
        }
    }
    None
}

/// Changes the protection of the given virtual address, and returns the old protection. If the given
/// virtual address is not mapped, this function does nothing and returns `None`, otherwise it
/// returns the old protection of the given virtual address.
///
/// # Safety
/// This function is unsafe because change the protection of a page can lead to undefined behavior
/// if the page is still in use.
pub fn change_protection(
    table: &mut PageTable,
    at: Virtual,
    flags: PageEntryFlags,
) -> Option<PageEntryFlags> {
    let pte = unsafe { fetch_pte_mut(table, paging::Level::PageMapLevel4, at) };
    if let Some(pte) = pte {
        if pte.is_present() {
            let old = pte.flags();
            // Update the page table entry and flush the TLB with interrupts disabled
            // I flush the whole TLB because I don't know how to correctly
            // flush one entry with `invlpg`: do I need to invalidate the mapped virtual
            // address or the virtual address of the page table ?
            // TODO: Only flush one entry of the TLB
            // TODO: Use a lazy TLB invalidation
            x86_64::irq::without(|| {
                pte.set_flags(flags);
                tlb::shootdown();
            });
            return Some(old);
        }
    }
    None
}

/// Translates the given virtual address to a physical address. If the given virtual address is not
/// mapped, `None` is returned, otherwise it returns the physical address of the given virtual
#[must_use]
pub fn translate(table: &PageTable, at: Virtual) -> Option<Physical> {
    let pte = unsafe { fetch_pte(table, paging::Level::PageMapLevel4, at) };
    if let Some(pte) = pte {
        if pte.is_present() {
            let addr = pte.address().unwrap();
            let offset = at.as_u64() & 0xFFF;
            Some(Physical::new(addr.as_u64() + offset))
        } else {
            None
        }
    } else {
        None
    }
}

/// Changes the current page table to the given one.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior/page fault if the given page
/// table is not valid or correctly initialized.
/// Furthermore, this function is unsafe because the caller must ensure that the given page table
/// is not dropped before the next page table change.
unsafe fn change_table(table: &TableRoot) {
    cpu::cr3::write(table.frame.start().as_u64());
}

/// Returns the current page table.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior, because the returned page table
/// is not guaranteed to be always valid. The caller must ensure that the page table is not dropped
/// while it is used.
///
/// This function should only be used during the initialization of the kernel, when there is no
/// other way to access the active page table (per-cpu variable are not initialized yet and therefore
/// it is impossible to use `ACTIVE_TABLE`).
#[must_use]
unsafe fn active_table() -> *mut PageTable {
    let addr = Physical::new(cpu::cr3::read() & PAGE_MASK as u64);
    phys_to_virt(addr).as_mut_ptr()
}

/// Fetches the page table entry of the given virtual address. If a entry is not present, it is
/// created and initialized (except for the [`paging::Level::PageTable`] level, which must be
/// initialized by the caller).
/// If an entry cannot be created (e.g. because we ran out of memory), `None` is returned.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior/page fault.
/// The caller must ensure that no modification of the page table and and its sub-tables are done
/// while this function is running (e.g. by locking the page table).
unsafe fn creat_and_fetch_pte(
    table: &mut PageTable,
    level: paging::Level,
    at: Virtual,
) -> Option<&mut PageEntry> {
    let entry = &mut table[at.page_index(level as u64)];
    if !entry.is_present() && level != paging::Level::PageTable {
        let frame = x86_64::irq::without(|| {
            FRAME_ALLOCATOR
                .lock()
                .allocate(frame::AllocationFlags::KERNEL | frame::AllocationFlags::ZEROED)
        })?;

        // Here, we use `PageEntryFlags::WRITABLE` even if the future mapping is not writable.
        // This is because if the `PageEntryFlags::WRITABLE` (and maybe the `PageEntryFlags::USER`)
        // are not set in intermediate page tables, the complete range of the virtual address space
        // are read-only and will cause a page fault if a write is attempted, even if the page entry
        // in the last level is marked as writable.
        // TODO: Check if this is correct for `PageEntryFlags::USER` too
        entry.add_flags(PageEntryFlags::PRESENT | PageEntryFlags::WRITABLE);
        entry.set_address(frame.start());
    }

    // Check if we are at the last level
    if let Some(level) = level.next() {
        let next_table = &mut *(phys_to_virt(entry.address().unwrap()).as_u64() as *mut PageTable);
        creat_and_fetch_pte(next_table, level, at)
    } else {
        Some(entry)
    }
}

/// Fetches the page table entry of the given virtual address and returns a reference to it. If a
/// entry is not present, `None` is returned.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior/page fault.
/// The caller must ensure that no modification of the page table and and its sub-tables are done
/// while this function is running (e.g. by locking the page table).
unsafe fn fetch_pte(table: &PageTable, level: paging::Level, at: Virtual) -> Option<&PageEntry> {
    let entry = &table[at.page_index(level as u64)];
    if entry.is_present() {
        // Check if we are at the last level
        if let Some(level) = level.next() {
            let next_table =
                &*(phys_to_virt(entry.address().unwrap()).as_u64() as *const PageTable);
            return fetch_pte(next_table, level, at);
        }
        return Some(entry);
    }
    None
}

/// Fetches the page table entry of the given virtual address and returns a mutable reference to it.
/// If a entry is not present, `None` is returned.
///
/// # Safety
/// This function is unsafe because it can cause undefined behavior/page fault.
/// The caller must ensure that no modification of the page table and and its sub-tables are done
/// while this function is running (e.g. by locking the page table).
unsafe fn fetch_pte_mut(
    table: &mut PageTable,
    level: paging::Level,
    at: Virtual,
) -> Option<&mut PageEntry> {
    let entry = &mut table[at.page_index(level as u64)];
    if entry.is_present() {
        if let Some(level) = level.next() {
            let next_table =
                &mut *(phys_to_virt(entry.address().unwrap()).as_u64() as *mut PageTable);
            return fetch_pte_mut(next_table, level, at);
        }
        return Some(entry);
    }
    None
}

/// Called when a page fault occurs. This function does almost nothing (the core function for
/// handling page faults is [`handle_page_fault`]), it simply detect if we are in the early stage
/// of the kernel initialization and call the right function to fetch the active page table.
///
/// # Errors
/// See [`handle_page_fault`] for more information.
pub fn page_fault(
    code: PageFaultErrorCode,
    addr: Virtual,
) -> Result<PageFaultType, PageFaultError> {
    if EARLY.load(Ordering::Relaxed) {
        let table = unsafe { &mut *active_table() };
        handle_page_fault(table, code, addr)
    } else {
        let mut table = ACTIVE_TABLE.lock();
        handle_page_fault(&mut table, code, addr)
    }
}

/// Handles a page fault. This function is called by [`page_fault`] and does the actual work.
///
/// # Errors
/// This function returns the type of the page fault (see [`PageFaultType`]) if the page fault was
/// handled successfully. Otherwise, it returns an error (see [`PageFaultError`]) describing the
/// cause of the error.
fn handle_page_fault(
    table: &mut PageTable,
    code: PageFaultErrorCode,
    addr: Virtual,
) -> Result<PageFaultType, PageFaultError> {
    let pte = unsafe { fetch_pte(table, paging::Level::PageMapLevel4, addr) };
    let present = pte.map_or(false, PageEntry::is_present);
    let mut error = PageFaultError::UNKNOWN;
    if pte.is_some() {
        // Check if the page fault was caused by a lazy TLB invalidation
        // If it is the case, the error code will specify that the page was not present, but when we
        // will try to fetch the page table entry, it will be marked as present. We juste have to
        // flush the TLB and return.
        if present && !code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            trace!("Lazy TLB invalidation at {:016x}", addr.as_u64());
            tlb::shootdown();
            return Ok(PageFaultType::LazyTlbInvalidation);
        }
    }

    // If the page fault was caused by a page not present in memory, we will try to handle it by
    // demand paging.
    if !present && !code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
        match handle_demand_paging(table, addr) {
            Ok(_) => return Ok(PageFaultType::DemandPaging),
            Err(e) => error |= e,
        }
    }

    // Here, we ran into a unrecoverable page fault. To facilitate debugging, we will compute the
    // reasons of the page fault and return them as an error.
    let pte = unsafe { fetch_pte(table, paging::Level::PageMapLevel4, addr) };
    if code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
        if let Some(pte) = pte {
            if !pte.is_writable() && code.contains(PageFaultErrorCode::WRITE_ACCESS) {
                error |= PageFaultError::WRITE_PROTECTED;
            } else if !pte.is_executable() && code.contains(PageFaultErrorCode::INSTRUCTION_FETCH) {
                error |= PageFaultError::NOT_EXECUTABLE;
            } else {
                error |= PageFaultError::PROTECTION_VIOLATION;
            }
        }
    } else {
        error |= PageFaultError::MISSING_PAGE;
    }

    // TODO: Handle other errors
    Err(error)
}

/// Handles a demand paging page fault.
///
/// # Errors
/// If the page fault cannot be handled, returns `PageFaultError::UNKNOWN` if the page fault was
/// not caused by a demand paging, or `PageFaultError::OUT_OF_MEMORY` if we ran out of memory
/// while trying to handle the page fault.
/// It is the caller's responsibility to determine the reason of the page fault, and correctly
/// handle it.
fn handle_demand_paging(table: &mut PageTable, addr: Virtual) -> Result<(), PageFaultError> {
    if addr.as_u64() >= mm::HEAP_START && addr.as_u64() < mm::HEAP_END {
        return crate::mm::allocator::handle_demand_paging(table, addr);
    } else if addr.as_u64() >= mm::VMALLOC_START && addr.as_u64() < mm::VMALLOC_END {
        return crate::mm::vmm::handle_demand_paging(table, addr);
    }
    Err(PageFaultError::UNKNOWN)
}

pub mod tlb {
    use crate::arch::acpi::TLB_SHOOTDOWN_VECTOR;
    use x86_64::{
        cpu,
        lapic::{self, IpiDestination},
    };

    /// Flushes the TLB on all cores.
    pub fn shootdown() {
        flush_all();
        if lapic::initialized() {
            unsafe {
                lapic::send_ipi(
                    IpiDestination::OtherCores,
                    lapic::IpiPriority::Normal,
                    TLB_SHOOTDOWN_VECTOR,
                );
            }
        }
    }

    /// Flushes the entire TLB. This is done by writing the current value of the CR3 register to it.
    /// This function should be used only when necessary, because the execution after this function
    /// will be slowed, as the number of TLB misses will increase dramatically.
    pub fn flush_all() {
        unsafe {
            cpu::cr3::reload();
        }
    }

    /// Flushes the TLB entry for the page containing the given virtual address.
    pub fn flush(addr: u64) {
        unsafe {
            cpu::invlpg(addr);
        }
    }
}
