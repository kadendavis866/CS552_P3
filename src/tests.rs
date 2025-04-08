//tests
#[cfg(test)]
mod tests {
    use crate::*;

    const AVAIL_SIZE: usize = size_of::<Avail>();

    /// Tests to make sure that the buddy_pool struct is created properly with the correct size
    #[test]
    fn test_create_destroy() {
        for k in MIN_K..DEFAULT_K {
            let num_bytes = (1u64 << k) as usize;
            let pool: BuddyPool = BuddyPool::new(num_bytes).unwrap();
            assert_eq!(pool.base.len(), num_bytes);
        }
    }

    /// Tests to make sure that the struct buddy_pool is correct and all fields have been properly
    /// set kval_m, avail[kval_m], and base pointer after a call to init
    #[test]
    fn test_init() {
        //Loop through all kval MIN_k-DEFAULT_K and make sure we get the correct amount allocated.
        //We will check all the pointer offsets to ensure the pool is all configured correctly
        for k in MIN_K..DEFAULT_K {
            let num_bytes = (1u64 << k) as usize;
            let mut pool: BuddyPool = BuddyPool::new(num_bytes).unwrap();
            pool.init();
            assert_eq!(pool.kval_m, k);
            check_buddy_pool_full(&pool);
        }
    }

    /// Tests that the b_to_k function produces the correct values
    #[test]
    fn test_b_to_k() {
        assert_eq!(b_to_k(0), 0);
        assert_eq!(b_to_k(1), 0);
        assert_eq!(b_to_k(2), 1);
        assert_eq!(b_to_k(3), 2);
        assert_eq!(b_to_k(4), 2);
        assert_eq!(b_to_k(5), 3);
    }

    /// Test allocating 1 byte to make sure we split the blocks all the way down to MIN_K size.
    /// Then free the block and ensure we end up with a full memory pool again.
    #[test]
    fn test_malloc_one_byte() {
        let size = (1u64 << MIN_K) as usize;
        let mut pool = BuddyPool::new(size).unwrap();
        pool.init();
        assert_eq!(pool.kval_m, MIN_K);
        let mem = pool.malloc(1).unwrap();
        //Make sure correct kval was allocated
        let min_kval = b_to_k(1 + AVAIL_SIZE);
        for k in min_kval..pool.kval_m {
            assert_eq!(get_size_and_validate(&pool.avail[k]), 1);
        }
        assert_eq!(get_size_and_validate(&pool.avail[pool.kval_m]), 0);

        // Check that memory is usable
        unsafe {
            *mem = 0u8;
        }
        let m = unsafe { mem.as_mut().unwrap() };
        unsafe {
            assert_eq!(*mem, 0);
        }
        *m = 1;
        assert_eq!(*m, 1);
        unsafe {
            assert_eq!(*mem, 1);
        }

        // Free the memory
        pool.free(mem);
        check_buddy_pool_full(&pool);
    }

    /**
    * Tests the allocation of one massive block that should consume the entire memory
    * pool and makes sure that after the pool is empty we correctly fail subsequent
    calls.
    */
    #[test]
    fn test_buddy_malloc_one_large() {
        let bytes = (1u64 << MIN_K) as usize;
        let mut pool = BuddyPool::new(bytes).unwrap();
        pool.init();
        //Ask for an exact K value to be allocated. This test makes assumptions on
        //the internal details of buddy_init.
        let ask = bytes - AVAIL_SIZE;
        let mem = pool.malloc(ask).unwrap();
        //Move the pointer back and make sure we got what we expected
        unsafe {
            let tmp = &*(mem.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(tmp.kval, MIN_K);
            assert_eq!(tmp.tag, BLOCK_RESERVED);
        }
        check_buddy_pool_empty(&pool);
        //Verify that a call on an empty pool fails as expected
        let fail = pool.malloc(5);
        assert!(fail.is_err());
        assert_eq!(fail, Err(BuddyError::NoMemory));
        //Free the memory and then check to make sure everything is OK
        pool.free(mem);
        check_buddy_pool_full(&pool);
    }

    /// Tests that the allocation of multiple blocks of memory works correctly even when they are of
    /// different sizes.
    #[test]
    fn test_2malloc() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem1 = pool.malloc(1).unwrap();
        let mem2 = pool.malloc(128).unwrap() as *mut u128;
        let mem1_kval = b_to_k(1 + AVAIL_SIZE);
        let mem2_kval = b_to_k(128 + AVAIL_SIZE); // large enough to be a different kval
        unsafe {
            let avail1 = &*(mem1.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            let avail2 = &*((mem2 as *mut u8).offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail1.kval, mem1_kval);
            assert_eq!(avail2.kval, mem2_kval);
            assert_eq!(avail1.tag, BLOCK_RESERVED);
            assert_eq!(avail2.tag, BLOCK_RESERVED);
        }
        pool.free(mem1);
        pool.free(mem2 as *mut u8);
        check_buddy_pool_full(&pool);
    }

    /// Tests that the buddy allocator can correctly allocate and free 100 small blocks of memory
    #[test]
    fn test_many_malloc() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mut mems: [*mut u8; 100] = [ptr::null_mut(); 100];
        for i in 0..100 {
            mems[i] = pool.malloc(i).unwrap();
            let kval = b_to_k(i + AVAIL_SIZE);
            unsafe {
                let avail = &*(mems[i].offset(-(AVAIL_SIZE as isize)) as *mut Avail);
                assert_eq!(avail.kval, kval);
                assert_eq!(avail.tag, BLOCK_RESERVED);
            }
        }
        //Check to make sure that all pointers are unique
        for i in 0..100 {
            assert_eq!(mems[i+1..100].contains(&mems[i]), false)
        }
        for mem in mems {
            pool.free(mem);
        }
        check_buddy_pool_full(&pool);
    }

    /// Tests that a call to realloc with a size of 0 will free the memory
    #[test]
    fn test_realloc_0() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem = pool.malloc(16).unwrap();
        let _ = pool.realloc(mem, 0).unwrap();
        check_buddy_pool_full(&pool);
    }

    /// Tests that a call to realloc with a smaller size is successful
    #[test]
    fn test_realloc_smaller() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem = pool.malloc(256).unwrap();
        unsafe {
            let avail = &*(mem.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(256 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }
        let mem2 = pool.realloc(mem, 8).unwrap();
        unsafe {
            let avail = &*(mem2.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(8 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }
        pool.free(mem2);
        check_buddy_pool_full(&pool);
    }

    /// Tests that a call to realloc with a larger size is successful and that the data is preserved
    /// in the new block
    #[test]
    fn test_realloc_larger() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem = pool.malloc(16).unwrap();
        unsafe {
            let avail = &*(mem.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(16 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }

        let m = unsafe { mem.as_mut().unwrap() };
        *m = 123;

        let mem2 = pool.realloc(mem, 128).unwrap();
        unsafe {
            let avail = &*(mem2.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(128 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }

        let m = unsafe { mem.as_mut().unwrap() };
        assert_eq!(*m, 123);

        pool.free(mem2);
        check_buddy_pool_full(&pool);
    }

    /// Tests that a call to realloc which results in the same kval is successful
    #[test]
    fn test_realloc_same() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem = pool.malloc(128).unwrap();
        unsafe {
            let avail = &*(mem.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(128 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }
        let mem2 = pool.realloc(mem, 128).unwrap();
        unsafe {
            let avail = &*(mem2.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(128 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }
        let mem3 = pool.realloc(mem2, 129).unwrap();
        unsafe {
            let avail = &*(mem3.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(128 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }
        pool.free(mem2);
        check_buddy_pool_full(&pool);
    }

    /// Tests that a call to realloc with a null pointer behaves like malloc
    #[test]
    fn test_realloc_null() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem = pool.realloc(ptr::null_mut(), 128).unwrap();
        unsafe {
            let avail = &*(mem.offset(-(AVAIL_SIZE as isize)) as *mut Avail);
            assert_eq!(avail.kval, b_to_k(128 + AVAIL_SIZE));
            assert_eq!(avail.tag, BLOCK_RESERVED);
        }
        pool.free(mem);
        check_buddy_pool_full(&pool);
    }

    /// Tests that allocating a block larger than the pool size fails and sets errno to ENOMEM
    #[test]
    fn test_alloc_too_large() {
        let mut pool = BuddyPool::new((1u64 << MIN_K) as usize).unwrap();
        pool.init();
        let mem = pool.malloc(1 << (pool.kval_m + 1));
        assert!(mem.is_err());
        assert_eq!(mem, Err(BuddyError::NoMemory));
        assert_eq!(errno(), ENOMEM);
        check_buddy_pool_full(&pool);
    }

    /// A test which fails if the pool has any available blocks
    ///
    /// # Arguments
    /// *pool - The buddy pool to check
    fn check_buddy_pool_empty(pool: &BuddyPool) {
        for i in 0..=pool.kval_m {
            assert_eq!(pool.avail[i].kval, i);
            assert_eq!(get_size_and_validate(&pool.avail[i]), 0);
        }
    }

    /// A test which fails if the pool has any blocks that are not available
    ///
    /// # Arguments
    /// *pool - The buddy pool to check
    fn check_buddy_pool_full(pool: &BuddyPool) {
        //A full pool should have all values 0-(kval-1) as empty
        for i in 0..pool.kval_m {
            assert_eq!(pool.avail[i].kval, i);
            assert_eq!(get_size_and_validate(&pool.avail[i]), 0);
        }
        //The avail array at kval should have the base block
        assert_eq!(pool.avail[pool.kval_m].kval, pool.kval_m);
        assert_eq!(get_size_and_validate(&pool.avail[pool.kval_m]), 1);
        assert_eq!(pool.avail[pool.kval_m].next, pool.avail[pool.kval_m].prev);
        //Check to make sure the base address points to the starting pool
        //If this fails either buddy_init is wrong or we have corrupted the
        //buddy_pool struct.
        assert_eq!(
            pool.avail[pool.kval_m].next as *const Avail,
            pool.base.as_ptr() as *const Avail
        );
    }

    /// Tests that an Avail list has the correct values and returns the number of free blocks of
    /// that size. The list is also checked in reverse to ensure that it is circular.
    ///
    /// # Arguments
    /// *list - The avail list to check, which should be a pointer to the first block in the list
    ///
    /// # Returns
    /// * The number of blocks in the list
    fn get_size_and_validate(list: &Avail) -> usize {
        let kval = list.kval;
        assert_eq!(list.tag, BLOCK_UNUSED);
        let mut count = 0;
        let mut current = list.next as *const Avail;
        while current != list {
            count += 1;
            let a = unsafe { current.as_ref().unwrap() };
            assert_eq!(a.tag, BLOCK_AVAIL);
            assert_eq!(a.kval, kval);
            current = a.next;
        }

        let mut count_rev = 0;
        current = list.prev;
        while current != list {
            count_rev += 1;
            let a = unsafe { current.as_ref().unwrap() };
            assert_eq!(a.tag, BLOCK_AVAIL);
            assert_eq!(a.kval, kval);
            current = a.prev;
        }
        assert_eq!(count, count_rev);
        count
    }
}
