use core::ops::{Add, AddAssign, Sub, SubAssign};

/// A canonical 64-bit virtual memory address.
///
/// On `x86_64`, only the 48 lower bits of a virtual address can be used. This type guarantees that 
/// the address is always canonical, i.e. that the top 17 bits are either all 0 or all 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Virtual(u64);

/// An invalid virtual address.
/// 
/// This type is used to represent an invalid virtual address. It is returned by [`Virtual::try_new`]
/// when the given address is not canonical (see [`Virtual`] for more information).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InvalidVirtual(u64);

impl Virtual {
    /// Creates a new canonical virtual address.
    /// 
    /// # Panics
    /// This function panics if the given address is not canonical.
    #[must_use]
    pub const fn new(address: u64) -> Self {
        match Self::try_new(address) {
            Ok(addr) => addr,
            Err(InvalidVirtual(_)) => panic!("Invalid virtual address: non canonical"),
        }
    }

    /// Tries to create a new canonical virtual address.
    /// 
    /// # Errors
    /// This function returns an [`InvalidVirtual`] error if the given address is not canonical, or
    /// a sign extension is performed if 48th bit is set and all bits from 49 to 63 are set to 0.
    pub const fn try_new(address: u64) -> Result<Self, InvalidVirtual> {
        match (address & 0xFFFF_8000_0000_0000) >> 47 {
            0 | 0x1FFFF => Ok(Self(address)),
            1 => Ok(Self::new_truncate(address)),
            _ => Err(InvalidVirtual(address)),
        }
    }

    /// Creates a new canonical virtual address, truncating the address if necessary.
    /// A sign extension is performed if 48th bit is set and all bits from 49 to 63 are set to 0, 
    /// and set those bits to 1 in order to make the address canonical.
    #[must_use]
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub const fn new_truncate(addr: u64) -> Self {
        // Some magic with sign extension on signed 64-bit integer
        // It set the sign bit to the 48th bit, and then shift to the right by 16 bits: all bits
        // from 48 to 63 are set to the sign bit
        Self(((addr << 16) as i64 >> 16) as u64)
    }

    /// Creates a new canonical virtual address without checking if it is canonical.
    /// 
    /// # Safety
    /// This function is unsafe because it does not check if the given address is canonical. If the
    /// address is not canonical, the behavior is undefined.
    #[must_use]
    pub const unsafe fn new_unchecked(address: u64) -> Self {
        Self(address)
    }

    /// Checks if the given address is canonical.
    #[must_use]
    pub const fn is_canonical(address: u64) -> bool {
        matches!((address & 0xFFFF_8000_0000_0000) >> 47, 0 | 0x1FFFF)
    }

    #[must_use]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    #[must_use]
    pub const fn as_ptr<T>(self) -> *const T {
        self.as_u64() as *const T
    }

    #[must_use]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.as_ptr::<T>() as *mut T
    }

    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Align the address up to the given alignment. If the address is already aligned, this function
    /// does nothing.
    /// 
    /// # Panics
    /// This function panics if the given alignment is not a power of two.
    #[must_use]
    pub fn align_up<T>(self, alignment: T) -> Self
    where
        T: Into<u64>,
    {
        let align: u64 = alignment.into();
        assert!(align.is_power_of_two());
        Self::new_truncate(
            (self.0.checked_add(align - 1)).expect("Overflow during aligning up a virtual address")
                & !(align - 1),
        )
    }

    /// Align the address down to the given alignment. If the address is already aligned, this
    /// function does nothing.
    /// 
    /// # Panics
    /// This function panics if the given alignment is not a power of two.
    #[must_use]
    pub fn align_down<T>(self, alignment: T) -> Self
    where
        T: Into<u64>,
    {
        let align: u64 = alignment.into();
        assert!(align.is_power_of_two());
        Self::new_truncate(self.0 & !(align - 1))
    }

    /// Checks if the address is aligned to the given alignment.
    /// 
    /// # Panics
    /// This function panics if the given alignment is not a power of two.
    #[must_use]
    pub fn is_aligned<T>(self, alignment: T) -> bool
    where
        T: Into<u64>,
    {
        let align: u64 = alignment.into();
        assert!(align.is_power_of_two());
        self.0 & (align - 1) == 0
    }

    /// Align the address up to a page boundary (4 KiB). If the address is already aligned, this
    /// function does nothing.
    #[must_use]
    pub fn page_align_up(&mut self) -> Self {
        self.align_up::<u64>(4096)
    }

    /// Align the address down to a page boundary (4 KiB). If the address is already aligned, this
    /// function does nothing.
    #[must_use]
    pub fn page_align_down(&mut self) -> Self {
        self.align_down::<u64>(4096)
    }

    /// Checks if the address is aligned to a page boundary (4 KiB).
    #[must_use]
    pub fn is_page_aligned(&mut self) -> bool {
        self.is_aligned::<u64>(4096)
    }

    #[must_use]
    pub const fn page_offset(self) -> u64 {
        self.0 & 0xFFF
    }

    #[must_use]
    pub const fn page_index(self, level: u64) -> u64 {
        assert!(level >= 1 && level <= 5);
        self.0 >> 12 >> ((level - 1) * 9) & 0x1FF
    }

    #[must_use]
    pub const fn pt_offset(self) -> u64 {
        self.page_index(1)
    }

    #[must_use]
    pub const fn pd_offset(self) -> u64 {
        self.page_index(2)
    }

    #[must_use]
    pub const fn pdpt_offset(self) -> u64 {
        self.page_index(3)
    }

    #[must_use]
    pub const fn pml4_offset(self) -> u64 {
        self.page_index(4)
    }

    #[must_use]
    pub const fn pml5_offset(self) -> u64 {
        self.page_index(5)
    }

    /// Checks if the address is in the kernel address space.
    #[must_use]
    pub const fn is_kernel(self) -> bool {
        self.0 >= 0xFFFF_8000_0000_0000
    }

    /// Checks if the address is in the user address space.
    #[must_use]
    pub const fn is_user(self) -> bool {
        !self.is_kernel()
    }
}

impl Add<Virtual> for Virtual {
    type Output = Virtual;

    fn add(self, rhs: Virtual) -> Self::Output {
        Self::new(self.0 + rhs.0)
    }
}

impl Add<u64> for Virtual {
    type Output = Virtual;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

impl Add<usize> for Virtual {
    type Output = Virtual;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.0 + rhs as u64)
    }
}

impl AddAssign<Virtual> for Virtual {
    fn add_assign(&mut self, rhs: Virtual) {
        self.0 += rhs.0;
    }
}

impl AddAssign<u64> for Virtual {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl AddAssign<usize> for Virtual {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs as u64;
    }
}

impl Sub<Virtual> for Virtual {
    type Output = Virtual;

    fn sub(self, rhs: Virtual) -> Self::Output {
        Self::new(self.0 - rhs.0)
    }
}

impl Sub<u64> for Virtual {
    type Output = Virtual;

    fn sub(self, rhs: u64) -> Self::Output {
        Self::new(self.0 - rhs)
    }
}

impl Sub<usize> for Virtual {
    type Output = Virtual;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::new(self.0 - rhs as u64)
    }
}

impl SubAssign<Virtual> for Virtual {
    fn sub_assign(&mut self, rhs: Virtual) {
        self.0 -= rhs.0;
    }
}

impl SubAssign<u64> for Virtual {
    fn sub_assign(&mut self, rhs: u64) {
        self.0 -= rhs;
    }
}

impl SubAssign<usize> for Virtual {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs as u64;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Physical(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InvalidPhysical(u64);

impl Physical {
    /// Creates a new physical address.
    /// 
    /// # Panics
    /// If the address is not valid (bits 52-63 must be 0), this function panics.
    #[must_use]
    pub const fn new(address: u64) -> Self {
        match Self::try_new(address) {
            Ok(addr) => addr,
            Err(InvalidPhysical(_)) => panic!("Physical address is not valid (must be 52 bits)"),
        }
    }

    /// Try to create a new physical address.
    /// 
    /// # Errors
    /// If the address is not valid (bits 52-63 must be 0), this function returns an error, 
    /// containing the invalid address.
    pub const fn try_new(address: u64) -> Result<Self, InvalidPhysical> {
        if address > 0x000F_FFFF_FFFF_FFFF {
            Err(InvalidPhysical(address))
        } else {
            Ok(Self(address))
        }
    }

    /// Creates a new physical address. Bits 52-63 are truncated to 0 if they are set.
    #[must_use]
    pub const fn new_truncate(addr: u64) -> Self {
        // Only keep the lower 52 bits
        Self(addr & 0x000F_FFFF_FFFF_FFFF)
    }

    /// Creates a new physical address without checking if it is valid.
    /// 
    /// # Safety
    /// The address must be valid (bits 52-63 must be 0). If the address is not valid, the behavior
    /// is undefined.
    #[must_use]
    pub const unsafe fn new_unchecked(address: u64) -> Self {
        Self(address)
    }

    #[must_use]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as u64)
    }

    #[must_use]
    pub const fn as_ptr<T>(self) -> *const T {
        self.as_u64() as *const T
    }

    #[must_use]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.as_ptr::<T>() as *mut T
    }

    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub fn align_up<T>(self, alignment: T) -> Self
    where
        T: Into<u64>,
    {
        let align: u64 = alignment.into();
        assert!(align.is_power_of_two());
        Self::new_truncate(
            (self.0.checked_add(align - 1))
                .expect("Overflow during aligning up a physical address")
                & !(align - 1),
        )
    }

    #[must_use]
    pub fn align_down<T>(self, alignment: T) -> Self
    where
        T: Into<u64>,
    {
        let align: u64 = alignment.into();
        assert!(align.is_power_of_two());
        Self::new_truncate(self.0 & !(align - 1))
    }

    #[must_use]
    pub fn is_aligned<T>(self, alignment: T) -> bool
    where
        T: Into<u64>,
    {
        let align: u64 = alignment.into();
        assert!(align.is_power_of_two());
        self.0 & (align - 1) == 0
    }

    #[must_use]
    pub fn page_align_up(&mut self) -> Self {
        self.align_up::<u64>(4096)
    }

    #[must_use]
    pub fn page_align_down(&mut self) -> Self {
        self.align_down::<u64>(4096)
    }

    #[must_use]
    pub fn is_page_aligned(&mut self) -> bool {
        self.is_aligned::<u64>(4096)
    }
}

impl Add<Physical> for Physical {
    type Output = Physical;

    fn add(self, rhs: Physical) -> Self::Output {
        Self::new(self.0 + rhs.0)
    }
}

impl Add<u64> for Physical {
    type Output = Physical;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

impl Add<usize> for Physical {
    type Output = Physical;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.0 + rhs as u64)
    }
}

impl AddAssign<Physical> for Physical {
    fn add_assign(&mut self, rhs: Physical) {
        self.0 += rhs.0;
    }
}

impl AddAssign<u64> for Physical {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl AddAssign<usize> for Physical {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs as u64;
    }
}

impl Sub<Physical> for Physical {
    type Output = Physical;

    fn sub(self, rhs: Physical) -> Self::Output {
        Self::new(self.0 - rhs.0)
    }
}

impl Sub<u64> for Physical {
    type Output = Physical;

    fn sub(self, rhs: u64) -> Self::Output {
        Self::new(self.0 - rhs)
    }
}

impl Sub<usize> for Physical {
    type Output = Physical;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::new(self.0 - rhs as u64)
    }
}

impl SubAssign<Physical> for Physical {
    fn sub_assign(&mut self, rhs: Physical) {
        self.0 -= rhs.0;
    }
}

impl SubAssign<u64> for Physical {
    fn sub_assign(&mut self, rhs: u64) {
        self.0 -= rhs;
    }
}

impl SubAssign<usize> for Physical {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs as u64;
    }
}

#[cfg(test)]
mod test {
    use core::hint::black_box;
    pub use core::mem::size_of;

    #[test]
    fn struct_size_checks() {
        assert_eq!(size_of::<super::InvalidPhysical>(), 8);
        assert_eq!(size_of::<super::InvalidVirtual>(), 8);
        assert_eq!(size_of::<super::Physical>(), 8);
        assert_eq!(size_of::<super::Virtual>(), 8);
    }

    #[test]
    fn physical_align_tests() {
        // Test 1: Align up a physical address
        assert_eq!(
            super::Physical::new(0x1001u64).align_up(0x1000u64),
            super::Physical::new(0x2000u64)
        );

        // Test 2: An already aligned physical address should not be aligned up
        assert_eq!(
            super::Physical::new(0x1000u64).align_up(0x1000u64),
            super::Physical::new(0x1000u64)
        );

        // Test 3: Align down a physical address
        assert_eq!(
            super::Physical::new(0x1000u64).align_down(0x1000u64),
            super::Physical::new(0x1000u64)
        );

        // Test 4: An already aligned physical address should not be aligned down
        assert_eq!(
            super::Physical::new(0x1000u64).align_down(0x1000u64),
            super::Physical::new(0x1000u64)
        );

        // Test 5: Check if a physical address is aligned
        assert!(super::Physical::new(0x1000).is_aligned(0x1000u64));
        assert!(!super::Physical::new(0x1001u64).is_aligned(0x1000u64));
    }

    #[test]
    fn virtual_align_tests() {
        // Test 1: Align up a physical address
        assert_eq!(
            super::Virtual::new(0x1001u64).align_up(0x1000u64),
            super::Virtual::new(0x2000u64)
        );

        // Test 2: An already aligned physical address should not be aligned up
        assert_eq!(
            super::Virtual::new(0x1000u64).align_up(0x1000u64),
            super::Virtual::new(0x1000u64)
        );

        // Test 3: Align down a physical address
        assert_eq!(
            super::Virtual::new(0x1000u64).align_down(0x1000u64),
            super::Virtual::new(0x1000u64)
        );

        // Test 4: An already aligned physical address should not be aligned down
        assert_eq!(
            super::Virtual::new(0x1000u64).align_down(0x1000u64),
            super::Virtual::new(0x1000u64)
        );

        // Test 5: Check if a physical address is aligned
        assert!(super::Virtual::new(0x1000u64).is_aligned(0x1000u64));
        assert!(!super::Virtual::new(0x1001u64).is_aligned(0x1000u64));
    }

    #[test]
    fn physical_add_checks() {
        // Test 1: Add an physical address to another physical address
        assert_eq!(
            super::Physical::new(0x1000) + super::Physical::new(0x2000),
            super::Physical::new(0x3000)
        );

        // Test 2: Add an physical address to a u64
        assert_eq!(
            super::Physical::new(0x1000) + 0x2000u64,
            super::Physical::new(0x3000)
        );

        // Test 3: Add an physical address to a usize
        assert_eq!(
            super::Physical::new(0x1000) + 0x2000usize,
            super::Physical::new(0x3000)
        );

        // Test 4 Add asign an physical address to another physical address
        let mut x = super::Physical::new(0x1000);
        x += super::Physical::new(0x2000);
        assert_eq!(x, super::Physical::new(0x3000));

        // Test 5: Add asign an physical address to a u64
        let mut x = super::Physical::new(0x1000);
        x += 0x2000u64;
        assert_eq!(x, super::Physical::new(0x3000));

        // Test 6: Add asign an physical address to a usize
        let mut x = super::Physical::new(0x1000);
        x += 0x2000usize;
        assert_eq!(x, super::Physical::new(0x3000));
    }

    #[test]
    fn physical_sub_checks() {
        // Test 1: Subtract an physical address from another physical address
        assert_eq!(
            super::Physical::new(0x3000) - super::Physical::new(0x2000),
            super::Physical::new(0x1000)
        );

        // Test 2: Subtract an physical address from a u64
        assert_eq!(
            super::Physical::new(0x3000) - 0x2000u64,
            super::Physical::new(0x1000)
        );

        // Test 3: Subtract an physical address from a usize
        assert_eq!(
            super::Physical::new(0x3000) - 0x2000usize,
            super::Physical::new(0x1000)
        );

        // Test 4: Subtract asign an physical address from another physical address
        let mut x = super::Physical::new(0x3000);
        x -= super::Physical::new(0x2000);
        assert_eq!(x, super::Physical::new(0x1000));

        // Test 5: Subtract asign an physical address from a u64
        let mut x = super::Physical::new(0x3000);
        x -= 0x2000u64;
        assert_eq!(x, super::Physical::new(0x1000));

        // Test 6: Subtract asign an physical address from a usize
        let mut x = super::Physical::new(0x3000);
        x -= 0x2000usize;
        assert_eq!(x, super::Physical::new(0x1000));
    }

    #[test]
    fn virtual_add_checks() {
        // Test 1: Add an virtual address to another virtual address
        assert_eq!(
            super::Virtual::new(0x1000) + super::Virtual::new(0x2000),
            super::Virtual::new(0x3000)
        );

        // Test 2: Add an virtual address to a u64
        assert_eq!(
            super::Virtual::new(0x1000) + 0x2000u64,
            super::Virtual::new(0x3000)
        );

        // Test 3: Add an virtual address to a usize
        assert_eq!(
            super::Virtual::new(0x1000) + 0x2000usize,
            super::Virtual::new(0x3000)
        );

        // Test 4 Add asign an virtual address to another virtual address
        let mut x = super::Virtual::new(0x1000);
        x += super::Virtual::new(0x2000);
        assert_eq!(x, super::Virtual::new(0x3000));

        // Test 5: Add asign an virtual address to a u64
        let mut x = super::Virtual::new(0x1000);
        x += 0x2000u64;
        assert_eq!(x, super::Virtual::new(0x3000));

        // Test 6: Add asign an virtual address to a usize
        let mut x = super::Virtual::new(0x1000);
        x += 0x2000usize;
        assert_eq!(x, super::Virtual::new(0x3000));
    }

    #[test]
    fn virtual_sub_checks() {
        // Test 1: Subtract an virtual address from another virtual address
        assert_eq!(
            super::Virtual::new(0x3000) - super::Virtual::new(0x2000),
            super::Virtual::new(0x1000)
        );
        // Test 2: Subtract an virtual address from a u64
        assert_eq!(
            super::Virtual::new(0x3000) - 0x2000u64,
            super::Virtual::new(0x1000)
        );
        // Test 3: Subtract an virtual address from a usize
        assert_eq!(
            super::Virtual::new(0x3000) - 0x2000usize,
            super::Virtual::new(0x1000)
        );

        // Test 4: Subtract asign an virtual address from another virtual address
        let mut x = super::Virtual::new(0x3000);
        x -= super::Virtual::new(0x2000);
        assert_eq!(x, super::Virtual::new(0x1000));

        // Test 5: Subtract asign an virtual address from a u64
        let mut x = super::Virtual::new(0x3000);
        x -= 0x2000u64;
        assert_eq!(x, super::Virtual::new(0x1000));

        // Test 6: Subtract asign an virtual address from a usize
        let mut x = super::Virtual::new(0x3000);
        x -= 0x2000usize;
        assert_eq!(x, super::Virtual::new(0x1000));
    }

    #[test]
    fn virtual_truncate_test() {
        assert_eq!(super::Virtual::new_truncate(0), super::Virtual(0));
        assert_eq!(
            super::Virtual::new_truncate(1 << 47),
            super::Virtual(0xFFFFF << 47)
        );
        assert_eq!(super::Virtual::new_truncate(0xFF), super::Virtual(0xFF));
        assert_eq!(
            super::Virtual::new_truncate(0xFF << 47),
            super::Virtual(0xFFFFF << 47)
        );
    }

    #[test]
    fn virtual_page_index_checks() {
        let address = 0xFFFF_8000_DEAF_BEEF;
        let v = super::Virtual::new(address);
        assert_eq!(v.page_offset(), address & 0xFFF);
        assert_eq!(v.pt_offset(), (address >> 12) & 0x1FF);
        assert_eq!(v.pd_offset(), (address >> 21) & 0x1FF);
        assert_eq!(v.pdpt_offset(), (address >> 30) & 0x1FF);
        assert_eq!(v.pml4_offset(), (address >> 39) & 0x1FF);
        assert_eq!(v.pml5_offset(), (address >> 48) & 0x1FF);
    }

    #[test]
    #[should_panic]
    fn physical_invalid_high_low() {
        black_box(super::Physical::new(0x0010_0000_0000_0000));
    }

    #[test]
    #[should_panic]
    fn physical_invalid_high_new() {
        black_box(super::Physical::new(0xFFFF_FFFF_FFFF_FFFF));
    }

    #[test]
    #[should_panic]
    fn virtual_invalid_low_address() {
        black_box(super::Virtual::new(0x000F_8000_0000_0000));
    }

    #[test]
    #[should_panic]
    fn virtual_invalid_high_address() {
        black_box(super::Virtual::new(0xFFFF_7FFF_FFFF_FFFF));
    }
}
