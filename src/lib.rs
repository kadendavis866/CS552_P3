//! # Buddy Memory Allocator
#![no_std]
mod buddy_error;
mod tests;

use crate::buddy_error::BuddyError;
use core::{array, ptr};
use errno::*;
use memmap2::MmapMut;

/// The default amount of memory that this memory manger will manage unless explicitly set. The
/// number of bytes is calculated as 2^DEFAULT_K
const DEFAULT_K: usize = 30;

/// The minimum size of the buddy memory pool.
const MIN_K: usize = 20;

/// The maximum size of the buddy memory pool. This is 1 larger than needed to allow indexes 1-N
/// instead of 0-N. Internally the maximum amount of memory is MAX_K-1
const MAX_K: usize = 48;

const BLOCK_AVAIL: u8 = 1; // Block is available to allocate
const BLOCK_RESERVED: u8 = 0; // Block has been handed to user
const BLOCK_UNUSED: u8 = 3; // Block is not used at all

/// The error code for ENOMEM as defined in the POSIX standard
const ENOMEM: Errno = Errno(12);

/// Struct to represent the table of all available blocks
struct Avail {
    tag: u8,          // Tag for block status BLOCK_AVAIL, BLOCK_RESERVED
    kval: usize,      // The kval of this block
    next: *mut Avail, // next memory block
    prev: *mut Avail, // prev memory block
}

impl Avail {
    /// Create a new Avail struct with default values
    ///
    /// # Returns
    /// a new Avail struct
    fn new() -> Avail {
        Avail {
            tag: BLOCK_UNUSED,
            kval: 0,
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        }
    }
}

/// The buddy memory pool.
pub struct BuddyPool {
    kval_m: usize,         // The max kval of this pool
    base: MmapMut,         // Base address used to scale memory for buddy calculations
    avail: [Avail; MAX_K], // The array of available memory blocks
}

impl BuddyPool {
    /// Create a new memory pool using the buddy algorithm. Internally, this function uses memmap2
    /// to get a block of memory to manage so should be portable to any system that implements mmap
    /// as well as Windows systems. This function will round up to the nearest power of two. So if
    /// the user requests 503MiB it will be rounded up to 512MiB.
    ///
    /// Note that if a 0 is passed as an argument then it initializes the memory pool to be of the
    /// default size of DEFAULT_K. If the caller specifies an unreasonably small size, then the
    /// buddy system may not be able to satisfy any requests.
    ///
    /// For the pool to be usable, the caller must call the init function to initialize the pool's
    /// internal data structures.
    ///
    /// NOTE: Memory pools returned by this function can not be intermingled. Calling buddy_malloc
    /// with pool A and then calling buddy_free with pool B will result in undefined behavior.
    ///
    /// # Arguments
    /// * size - The size of the pool in bytes
    pub fn new(size: usize) -> Result<BuddyPool, BuddyError> {
        let mut kval: usize;
        if size == 0 {
            kval = DEFAULT_K;
        } else {
            kval = b_to_k(size);
        }
        if kval < MIN_K {
            kval = MIN_K;
        }
        if kval > MAX_K {
            kval = MAX_K - 1;
        }

        let kval_m = kval;

        let numbytes = (1u64 << kval) as usize;
        //Memory map a block of raw memory to manage
        let base = MmapMut::map_anon(numbytes).or_else(|_| {
            set_errno(ENOMEM);
            return Err(BuddyError::NoMemory);
        })?;

        let pool = BuddyPool {
            kval_m,
            base,
            avail: array::from_fn::<_, MAX_K, _>(|_| Avail::new()),
        };
        Ok(pool)
    }

    /// Initialize the buddy memory pool. This function must be called before any other functions
    /// for the pool to function. This was not handled in new because the avail array requires
    /// memory locations to be fixed before initialization.
    pub fn init(&mut self) {
        // Initialize the avail list
        for i in 0..=self.kval_m {
            self.avail[i].next = &mut self.avail[i] as *mut Avail;
            self.avail[i].prev = &mut self.avail[i] as *mut Avail;
            self.avail[i].kval = i;
            self.avail[i].tag = BLOCK_UNUSED;
        }

        //Add in the first block
        let base_ptr = self.base.as_ptr() as *mut Avail;
        self.avail[self.kval_m].next = base_ptr;
        self.avail[self.kval_m].prev = base_ptr;

        let m = unsafe { &mut *base_ptr };
        m.tag = BLOCK_AVAIL;
        m.kval = self.kval_m;
        m.next = &mut self.avail[self.kval_m] as *mut Avail;
        m.prev = &mut self.avail[self.kval_m] as *mut Avail;
    }

    /// Find the buddy of a given pointer and kval relative to the base address we got from memmap2
    ///
    /// # Arguments
    /// * buddy - The memory block that we want to find the buddy for
    ///
    /// # Returns
    /// a pointer to the buddy
    fn buddy_calc(&self, avail: &Avail) -> *mut Avail {
        let mut addr = (avail as *const Avail).addr();
        addr -= self.base.as_ptr().addr();
        let mask = (1u64 << avail.kval) as usize;
        unsafe { self.base.as_ptr().offset((addr ^ mask) as isize) as *mut Avail }
    }

    /// Allocates a block of size bytes of memory, returning a pointer to the beginning of the
    /// block. The content of the newly allocated block of memory is not initialized, remaining with
    /// indeterminate values.
    ///
    /// # Arguments
    /// * size - The size of the user requested memory block in bytes
    ///
    /// # Returns
    /// a pointer to the memory block
    pub fn malloc(&mut self, size: usize) -> Result<*mut u8, BuddyError> {
        let avail_size = size_of::<Avail>();
        let kval = b_to_k(size + avail_size);
        unsafe { Ok((self.malloc_kval(kval)? as *mut u8).offset(avail_size as isize)) }
    }

    /// Allocates a block of memory of size 2^k bytes, returning a pointer to the Avail struct at
    /// the start of the block. This is in contrast to the malloc function which returns a pointer
    /// to the start of usable user memory.
    ///
    /// # Arguments
    /// * kval - The size of the requested block in K values
    ///
    /// # Returns
    /// a pointer to the Avail struct at the start of the block
    unsafe fn malloc_kval(&mut self, kval: usize) -> Result<*mut Avail, BuddyError> {
        if kval > self.kval_m {
            set_errno(ENOMEM);
            return Err(BuddyError::NoMemory);
        }
        if self.avail[kval].next as *const Avail != &self.avail[kval] {
            let block = self.avail[kval].next;
            self.remove_from_avail(&mut *block);
            return Ok(block);
        }
        //No blocks available at this kval, try to split a larger block
        let larger_block = self.malloc_kval(kval + 1)?;
        Ok(self.split(&mut *larger_block))
    }

    /// Splits a block of memory into two smaller blocks. This function will return a pointer to the
    /// block with the lowest address, the other block will be added to the Avail list. The returned
    /// block will be tagged as reserved and not added to the avail list.
    ///
    /// # Arguments
    /// * avail - The block of memory to split
    ///
    /// # Returns
    /// a pointer to the block with the lowest address after the split
    fn split<'a>(&mut self, avail: &'a mut Avail) -> &'a mut Avail {
        let kval = avail.kval;
        avail.kval -= 1;
        avail.tag = BLOCK_RESERVED;
        let buddy = self.buddy_calc(avail);
        unsafe {
            ptr::write(buddy, Avail::new());
            let buddy = &mut *buddy;
            buddy.kval = kval - 1;
            buddy.tag = BLOCK_AVAIL;
            self.add_to_avail(buddy);
        }
        avail
    }

    /// A block of memory previously allocated by a call to malloc, realloc is
    /// deallocated, making it available again for further allocations.
    ///
    /// If ptr does not point to a block of memory allocated with the above functions, it causes
    /// undefined behavior.
    ///
    /// If ptr is a null pointer, the function does nothing. Notice that this function does not
    /// change the value of ptr itself, hence it still points to the same (now invalid) location.
    ///
    /// # Arguments
    /// * ptr - Pointer to the memory block to free
    pub fn free(&mut self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            let avail = (ptr.offset(-(size_of::<Avail>() as isize)) as *mut Avail)
                .as_mut()
                .unwrap();
            self.free_avail(avail);
        }
    }

    /// Frees a block of memory previously allocated by a call to malloc, realloc. This function
    /// should only be used internally as it takes as an argument the reference to the Avail struct,
    /// not the pointer to user memory.
    unsafe fn free_avail(&mut self, avail: &mut Avail) {
        let mut avail = avail;
        let mut buddy_o = self.get_avail_buddy(avail);
        while buddy_o.is_some() {
            let buddy = buddy_o.unwrap() as *mut Avail;
            self.remove_from_avail(&mut *buddy);
            if (avail as *mut Avail) < buddy {
                avail.kval += 1;
            } else {
                (*buddy).kval += 1;
                avail = &mut *buddy;
            }
            buddy_o = self.get_avail_buddy(avail);
        }
        self.add_to_avail(avail);
    }

    /// Adds a block of memory to the avail list and tags it as available.
    ///
    /// # Arguments
    /// * avail - The block of memory to add to the avail list
    fn add_to_avail(&mut self, avail: &mut Avail) {
        let kval = avail.kval;
        avail.prev = self.avail[kval].prev;
        avail.next = &mut self.avail[kval];
        unsafe {
            (*self.avail[kval].prev).next = avail;
        }
        self.avail[kval].prev = avail;
        avail.tag = BLOCK_AVAIL;
    }

    /// Removes a block of memory from the avail list and tags it as reserved.
    ///
    /// # Arguments
    /// * avail - The block of memory to remove from the avail list
    fn remove_from_avail(&mut self, avail: &mut Avail) {
        unsafe {
            (*avail.next).prev = avail.prev;
            (*avail.prev).next = avail.next;
        }
        avail.tag = BLOCK_RESERVED;
        avail.next = ptr::null_mut();
        avail.prev = ptr::null_mut();
    }

    /// Gets the buddy of a block of memory. This function will return None if the buddy is not
    /// tagged as available. This is most useful in coalescing blocks during a free operation.
    ///
    /// # Arguments
    /// * avail - The block of memory to get the buddy for
    ///
    /// # Returns
    /// a reference to the buddy block if it is available, otherwise None
    fn get_avail_buddy(&self, avail: &Avail) -> Option<&mut Avail> {
        if avail.kval == self.kval_m {
            return None;
        }
        let buddy = unsafe { self.buddy_calc(avail).as_mut().unwrap() };
        if buddy.tag != BLOCK_AVAIL {
            return None;
        }
        if buddy.kval != avail.kval {
            return None;
        }
        Some(buddy)
    }

    /// Changes the size of the memory block pointed to by ptr. The function may move the memory
    /// block to a new location (whose address is returned by the function). The content of the
    /// memory block is preserved up to the lesser of the new and old sizes, even if the block is
    /// moved to a new location. If the new size is larger, the value of the newly allocated portion
    /// is indeterminate.
    ///
    /// In case that ptr is a null pointer, the function behaves like malloc, assigning a new block
    /// of size bytes and returning a pointer to its beginning.
    ///
    /// If size is equal to zero, and ptr is not NULL, then the call is equivalent to free(ptr)
    ///
    /// # Arguments
    /// ptr - Pointer to a memory block
    /// size - The new size of the memory block
    ///
    /// # Returns
    /// a pointer to the new memory block
    pub fn realloc(&mut self, ptr: *mut u8, size: usize) -> Result<*mut u8, BuddyError> {
        if ptr.is_null() {
            return self.malloc(size);
        }
        let target_kval = b_to_k(size + size_of::<Avail>());
        // case - requested size too large
        if target_kval > self.kval_m {
            set_errno(ENOMEM);
            return Err(BuddyError::NoMemory);
        }
        // case - current kval fits size
        let mut old_avail = unsafe {
            (ptr.offset(-(size_of::<Avail>() as isize)) as *mut Avail)
                .as_mut()
                .ok_or(BuddyError::CorruptedMemoryPool)?
        };
        let old_kval = old_avail.kval;
        if target_kval == old_kval {
            return Ok(ptr);
        }
        // case - requested size is 0
        if size == 0 {
            self.free(ptr);
            return Ok(ptr);
        }
        // case - reduce size
        while target_kval < old_avail.kval {
            old_avail = self.split(old_avail);
        }
        // case - increase size
        let mut ptr = ptr;
        if target_kval > old_avail.kval {
            unsafe {
                let new_avail = self.malloc_kval(target_kval)?;
                let new_block = (new_avail as *mut u8).offset(size_of::<Avail>() as isize);
                let old_size = (1u64 << old_avail.kval) as usize;
                new_block.copy_from_nonoverlapping(ptr, old_size);
                self.free(ptr);
                ptr = new_block;
            }
        }
        Ok(ptr)
    }
}

impl Drop for BuddyPool {
    /// Inverse of buddy_init. Releases all memory allocated by the buddy allocator. This function
    /// will be called automatically when the BuddyPool goes out of scope.
    ///
    /// Notice that this function does not change the value of pool itself, hence it still points to
    /// the same (now invalid) location.
    fn drop(&mut self) {
        let _ = self.base.flush();
    }
}

/// Converts bytes to its equivalent K value defined as bytes <= 2^K
///
/// # Arguments
/// * bytes - the number of bytes
///
/// # Returns
/// the K value that will fit bytes
fn b_to_k(mut bytes: usize) -> usize {
    if bytes == 0 {
        return 0;
    }
    let mut k: usize = 0;
    bytes -= 1;
    while bytes > 0 {
        bytes >>= 1;
        k += 1;
    }
    k
}
