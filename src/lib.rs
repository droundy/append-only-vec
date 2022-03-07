//! AppendOnlyVec
//!
//! This is a pretty simple type, which is a vector that you can push into, but
//! cannot modify the elements of.  The data structure never moves an element
//! once allocated, so you can push to the vec even while holding references to
//! elements that have already been pushed.
//!
//! ### Scaling
//!
//! 1. Accessing an element is O(1), but slightly more expensive than for a
//!    standard `Vec`.
//!
//! 2. Pushing a new element amortizes to O(1), but may require allocation of a
//!    new chunk.
//!
//! ### Example
//!
//! ```
//! use append_only_vec::AppendOnlyVec;
//! static v: AppendOnlyVec<String> = AppendOnlyVec::<String>::new();
//! let mut threads = Vec::new();
//! for thread_num in 0..10 {
//!     threads.push(std::thread::spawn(move || {
//!          for n in 0..100 {
//!               let s = format!("thread {} says {}", thread_num, n);
//!               let which = v.push(s.clone());
//!               assert_eq!(&v[which], &s);
//!          }
//!     }));
//! }
//! for t in threads {
//!    t.join();
//! }
//! assert_eq!(v.len(), 1000);
//! ```

use std::ops::Index;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicPtr, AtomicUsize};
pub struct AppendOnlyVec<T> {
    count: AtomicUsize,
    reserved: AtomicUsize,
    data: [AtomicPtr<T>; BITS - 1],
}

unsafe impl<T: Send> Send for AppendOnlyVec<T> {}
unsafe impl<T: Sync> Sync for AppendOnlyVec<T> {}

const BITS: usize = std::mem::size_of::<usize>() * 8;

// This takes an index into a vec, and determines which data array will hold it
// (the first return value), and what the index will be into that data array
// (second return value)
//
// The ith data array holds 1<<i values.
fn indices(i: usize) -> (u32, usize) {
    let i = i + 1;
    let bin = BITS as u32 - 1 - i.leading_zeros();
    let offset = i - (1 << bin);
    (bin, offset)
}

impl<T> AppendOnlyVec<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.len()).map(|i| &self[i])
    }
    /// Find the length of the array.
    #[inline]
    pub fn len(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
    /// Append an element to the array
    ///
    /// This is notable in that it doesn't require a `&mut self`, because it
    /// does appropriate atomic synchronization.
    pub fn push(&self, val: T) -> usize {
        let idx = self.reserved.fetch_add(1, Ordering::Relaxed);
        let (array, offset) = indices(idx);

        // First we get the pointer to the relevant array.  This requires
        // allocating it atomically if it is null.
        let mut ptr = self.data[array as usize].load(Ordering::Acquire);
        if ptr.is_null() {
            // The first array has size 2, and after that they are more regular.
            let n = if array == 0 { 2 } else { 1 << array };
            let layout = std::alloc::Layout::array::<T>(n).unwrap();
            ptr = unsafe { std::alloc::alloc(layout) } as *mut T;
            match self.data[array as usize].compare_exchange(
                std::ptr::null_mut(),
                ptr,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Nothing to do.
                }
                Err(p) => {
                    unsafe { std::alloc::dealloc(ptr as *mut u8, layout) };
                    ptr = p; // Someone allocated this already.
                }
            }
        }
        unsafe {
            (ptr.add(offset)).write(val);
        }

        // Now we need to increase the size of the vec, so it can get read.
        while self
            .count
            .compare_exchange(idx, idx + 1, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            // This means that someone else *started* pushing before we started,
            // but hasn't yet finished.  We have to wait for them to finish
            // pushing before we can update the count.  Note that using a
            // spinloop here isn't really ideal, but except when allocating a
            // new array, the window between reserving space and using it is
            // pretty small, so contention will hopefully be rare, and having a
            // context switch during that interval will hopefully be vanishingly
            // unlikely.
            std::hint::spin_loop();
        }
        idx
    }
    /// Allocate a new empty array
    pub const fn new() -> Self {
        AppendOnlyVec {
            count: AtomicUsize::new(0),
            reserved: AtomicUsize::new(0),
            data: [
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
                AtomicPtr::new(std::ptr::null_mut()),
            ],
        }
    }
}

impl<T> Index<usize> for AppendOnlyVec<T> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.len()); // this includes the required ordering memory barrier
        let (array, offset) = indices(idx);
        let ptr = self.data[array as usize].load(Ordering::Acquire);
        unsafe { &*ptr.add(offset) }
    }
}

#[test]
fn test_indices() {
    assert_eq!(indices(0), (0, 0));
    assert_eq!(indices(1), (1, 0));
    assert_eq!(indices(2), (1, 1));
    assert_eq!(indices(3), (2, 0));
    assert_eq!(indices(4), (2, 1));
    assert_eq!(indices(5), (2, 2));
    assert_eq!(indices(6), (2, 3));
    assert_eq!(indices(7), (3, 0));
}

#[test]
fn test_pushing_and_indexing() {
    let v = AppendOnlyVec::<usize>::new();

    for n in 0..50 {
        v.push(n);
        assert_eq!(v.len(), n + 1);
        for i in 0..(n + 1) {
            assert_eq!(v[i], i);
        }
    }

    let vec: Vec<usize> = v.iter().copied().collect();
    let ve2: Vec<usize> = (0..50).collect();
    assert_eq!(vec, ve2);
}
