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
//! static V: AppendOnlyVec<String> = AppendOnlyVec::<String>::new();
//! let mut threads = Vec::new();
//! for thread_num in 0..10 {
//!     threads.push(std::thread::spawn(move || {
//!          for n in 0..100 {
//!               let s = format!("thread {} says {}", thread_num, n);
//!               let which = V.push(s.clone());
//!               assert_eq!(&V[which], &s);
//!          }
//!     }));
//! }
//! for t in threads {
//!    t.join();
//! }
//! assert_eq!(V.len(), 1000);
//! ```

use std::ops::Index;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicPtr, AtomicUsize};
pub struct AppendOnlyVec<T> {
    count: AtomicUsize,
    reserved: AtomicUsize,
    data: [AtomicPtr<T>; BITS - 1 - 3],
}

unsafe impl<T: Send> Send for AppendOnlyVec<T> {}
unsafe impl<T: Sync + Send> Sync for AppendOnlyVec<T> {}

const BITS: usize = std::mem::size_of::<usize>() * 8;

// This takes an index into a vec, and determines which data array will hold it
// (the first return value), and what the index will be into that data array
// (second return value)
//
// The ith data array holds 1<<i values.
const fn indices(i: usize) -> (u32, usize) {
    let i = i + 8;
    let bin = BITS as u32 - 1 - i.leading_zeros();
    let bin = bin - 3;
    let offset = i - bin_size(bin);
    (bin, offset)
}
const fn bin_size(array: u32) -> usize {
    (1 << 3) << array
}

#[test]
fn test_indices() {
    for i in 0..32 {
        println!("{:3}: {} {}", i, indices(i).0, indices(i).1);
    }
    let mut array = 0;
    let mut offset = 0;
    let mut index = 0;
    while index < 1000 {
        index += 1;
        offset += 1;
        if offset >= bin_size(array) {
            offset = 0;
            array += 1;
        }
        assert_eq!(indices(index), (array, offset));
    }
}

impl<T> AppendOnlyVec<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.len()).map(|i| unsafe { self.get_unchecked(i) })
    }
    /// Find the length of the array.
    #[inline]
    pub fn len(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    fn layout(&self, array: u32) -> std::alloc::Layout {
        std::alloc::Layout::array::<T>(bin_size(array)).unwrap()
    }
    /// Append an element to the array
    ///
    /// This is notable in that it doesn't require a `&mut self`, because it
    /// does appropriate atomic synchronization.
    pub fn push(&self, val: T) -> usize {
        let idx = self.reserved.fetch_add(1, Ordering::Relaxed);
        let (array, offset) = indices(idx);

        // First we get the pointer to the relevant array.  This requires
        // allocating it atomically if it is null.  We use an Acquire load
        // because although we don't read any data that exists behind the
        // pointer, the data may not be correctly writeable if we just allocated
        // the pointer?
        let mut ptr = self.data[array as usize].load(Ordering::Acquire);
        if ptr.is_null() {
            // The first array has size 2, and after that they are more regular.
            let layout = self.layout(array);
            ptr = unsafe { std::alloc::alloc(layout) } as *mut T;
            // We use a Release compare_exchange to ensure that the allocated
            // array is valid to write to by any other thread that might be
            // pushing.
            match self.data[array as usize].compare_exchange(
                std::ptr::null_mut(),
                ptr,
                Ordering::Release,
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

        // Now we need to increase the size of the vec, so it can get read.  We
        // use Release upon success, to ensure that the value which we wrote is
        // visible to any thread that has confirmed that the count is big enough
        // to read that element.  In case of failure, we can be relaxed, since
        // we don't do anything with the result other than try again.
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
            ],
        }
    }

    /// Index the vec without checking the bounds.
    ///
    /// To use this correctly, you *must* first ensure that the `idx <
    /// self.len()`.  This not only prevents overwriting the bounds, but also
    /// creates the memory barriers to ensure that the data is visible to the
    /// current thread.  In single-threaded code, however, it is not needed to
    /// call `self.len()` explicitly (if e.g. you have counted the number of
    /// elements pushed).
    unsafe fn get_unchecked(&self, idx: usize) -> &T {
        let (array, offset) = indices(idx);
        // We use a Relaxed load of the pointer, because the length check (which
        // was supposed to be performed) should ensure that the data we want is
        // already visible, since it Acquired `self.count` which synchronizes
        // with the write in `self.push`.
        let ptr = self.data[array as usize].load(Ordering::Relaxed);
        &*ptr.add(offset)
    }
}

impl<T> Index<usize> for AppendOnlyVec<T> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.len()); // this includes the required ordering memory barrier
        let (array, offset) = indices(idx);
        // We use a Relaxed load of the pointer, because the length check above
        // will ensure that the data we want is already visible, since it
        // Acquired `self.count` which synchronizes with the write in
        // `self.push`.
        let ptr = self.data[array as usize].load(Ordering::Relaxed);
        unsafe { &*ptr.add(offset) }
    }
}

impl<T> Drop for AppendOnlyVec<T> {
    fn drop(&mut self) {
        // First we'll drop all the `T` in a slightly sloppy way.  FIXME this
        // could be optimized to avoid reloading the `ptr`.
        for idx in 0..self.len() {
            let (array, offset) = indices(idx);
            // We use a Relaxed load of the pointer, because the loop above (which
            // ends before `self.len()`) should ensure that the data we want is
            // already visible, since it Acquired `self.count` which synchronizes
            // with the write in `self.push`.
            let ptr = self.data[array as usize].load(Ordering::Relaxed);
            unsafe {
                std::ptr::drop_in_place(ptr.add(offset));
            }
        }
        // Now we will free all the arrays.
        for array in 0..self.data.len() as u32 {
            // This load is relaxed because no other thread can have a reference
            // to Self because we have a &mut self.
            let ptr = self.data[array as usize].load(Ordering::Relaxed);
            if !ptr.is_null() {
                let layout = self.layout(array);
                unsafe { std::alloc::dealloc(ptr as *mut u8, layout) };
            } else {
                break;
            }
        }
    }
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
