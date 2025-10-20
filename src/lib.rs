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

use std::cell::UnsafeCell;
use std::ops::{Index, IndexMut};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
pub struct AppendOnlyVec<T> {
    count: AtomicUsize,
    reserved: AtomicUsize,
    data: [UnsafeCell<*mut T>; BITS_USED - 1 - 3],
}

unsafe impl<T: Send> Send for AppendOnlyVec<T> {}
unsafe impl<T: Sync + Send> Sync for AppendOnlyVec<T> {}

const BITS: usize = std::mem::size_of::<usize>() * 8;

#[cfg(target_arch = "x86_64")]
const BITS_USED: usize = 48;
#[cfg(all(not(target_arch = "x86_64"), target_pointer_width = "64"))]
const BITS_USED: usize = 64;
#[cfg(target_pointer_width = "32")]
const BITS_USED: usize = 32;

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

fn spin_wait(failures: &mut usize) {
    *failures += 1;
    if *failures <= 3 {
        // If there haven't been many failures yet, then we optimistically
        // spinloop.
        for _ in 0..(1 << *failures) {
            std::hint::spin_loop();
        }
    } else {
        // If there have been many failures, then continuing to spinloop will
        // probably just waste CPU, and whoever we are waiting for has been
        // preempted then spinning could actively delay completion of the task.
        // So instead, we cooperatively yield to the OS scheduler.
        std::thread::yield_now();
    }
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

impl<T> Default for AppendOnlyVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AppendOnlyVec<T> {
    /// Return an `Iterator` over the elements of the vec.
    /// 
    /// Note that any elements pushed to the vector after `iter()` was
    /// called won't be returned by the iterator.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        // FIXME this could be written to be a little more efficient probably,
        // if we made it read each pointer only once.  On the other hand, that
        // could make a reversed iterator less efficient?
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
    /// Internal-only function either allocates data for a given index,
    /// or waits until another thread allocated this data. Requires that
    /// idx has been reserved, and no other thread will try to write to
    /// this specific index, otherwise it will cause UB.
    /// 
    /// This returns the pointer to the allocated slot, without initializing
    /// it. The initialization as well as adjusting the size of the vec must
    /// be done by the caller.
    unsafe fn ensure_slot_allocated(&self, idx: usize) -> *mut T {
        let (array, offset) = indices(idx);
        let ptr = if self.len() < 1 + idx - offset {
            // We are working on a new array, which may not have been allocated...
            if offset == 0 {
                // It is our job to allocate the array! The size of the array
                // is determined in the self.layout method, which needs to be
                // consistent with the indices function.
                let layout = self.layout(array);
                let ptr = unsafe { std::alloc::alloc(layout) } as *mut T;
                unsafe {
                    *self.data[array as usize].get() = ptr;
                }
                ptr
            } else {
                // We need to wait for the array to be allocated.
                let mut failures = 0;
                while self.len() < 1 + idx - offset {
                    spin_wait(&mut failures);
                }
                // The Ordering::Acquire semantics of self.len() ensures that
                // this pointer read will get the non-null pointer allocated
                // above.
                unsafe { *self.data[array as usize].get() }
            }
        } else {
            // The Ordering::Acquire semantics of self.len() ensures that
            // this pointer read will get the non-null pointer allocated
            // above.
            unsafe { *self.data[array as usize].get() }
        };
        return unsafe { ptr.add(offset) };
    }
    /// Internal-only function requests a slot and puts data into it.
    ///
    /// However this does not update the size of the vec, which *must* be done
    /// in order for either the value to be readable *or* for future pushes to
    /// actually terminate.
    fn pre_push(&self, val: T) -> usize {
        let idx = self.reserved.fetch_add(1, Ordering::Relaxed);
        // we may call ensure_slot_allocated since we got idx
        // from fetch_add above, which will never return two times
        // the same index 
        let ptr = unsafe { self.ensure_slot_allocated(idx) };
        // The contents of this offset are guaranteed to be unused (so far)
        // because we got the idx from our fetch_add above, and ptr is
        // guaranteed to be valid because of ensure_slot_allocated
        unsafe { ptr.write(val) };
        idx
    }
    // Computes a value from its index to push into the vec.
    ///
    /// The function will be called on `index` where `index` is the
    /// current length of the vector. If the function returns `Some(value)`, it
    /// is guaranteed that value will be written to the `index`-th entry of the
    /// vector. If the function returns `None` then the `AppendOnlyVec` remains 
    /// unchanged.  `compute_next()` will return `Some(index)` if a value was
    /// pushed at this index, and `None` if the vector remained unchanged.
    /// 
    /// Note that the given function may be called multiple times, in a compare-exchange
    /// loop. In particular, the longer the function runs, the more likely it is
    /// that the contents of the vector have changed in the meantime, and it has to
    /// be called again. Thus, for best performance, it is recommended to only use
    /// `compute_next()` when the function is very cheap to compute. If this is not
    /// the case, it may be faster to use a `RwLock<AppendOnlyVec<T>>` instead.
    /// 
    /// # Example
    /// 
    /// A common use case is when added elements depend on their index, for example
    /// when lazily computing a sequence of elements:
    /// ```
    /// # use append_only_vec::*;
    /// struct CachedSequence<F, T>
    ///     where F: Fn(usize) -> T
    /// {
    ///     compute_ith: F,
    ///     cache: AppendOnlyVec<T>
    /// }
    /// impl<F, T> CachedSequence<F, T>
    ///     where F: Fn(usize) -> T
    /// {
    ///     fn new(compute_ith: F) -> Self {
    ///         CachedSequence { compute_ith: compute_ith, cache: AppendOnlyVec::new() }
    ///     }
    /// 
    ///     fn get(&self, index: usize) -> &T {
    ///         while self.cache.len() <= index {
    ///             let len = self.cache.len();
    ///             self.cache.compute_next(|i| Some((self.compute_ith)(i)));
    ///         }
    ///         return &self.cache[index];
    ///     }
    /// }
    /// let seq = CachedSequence::new(|i| i + 1);
    /// assert_eq!(1, *seq.get(0));
    /// assert_eq!(1, *seq.get(0));
    /// assert_eq!(2, *seq.get(1)); 
    /// ```
    pub fn compute_next(&self, mut f: impl FnMut(usize) -> Option<T>) -> Option<usize> {
        let mut current_reserved = self.reserved.load(Ordering::Relaxed);
        while let Some(value) = f(current_reserved) {
            // as in `push`, we assume now overflow happens; in practice, this
            // is justified, since memory will run out before we reach the limit
            // of `usize`
            let new_reserved = current_reserved + 1;
            match self.reserved.compare_exchange(current_reserved, new_reserved, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => {
                    let idx = current_reserved;
                    // we may call ensure_slot_allocated since we got idx
                    // from compare_exchange above, which will never return two times
                    // the same index 
                    let ptr = unsafe { self.ensure_slot_allocated(idx) };
                    // The contents of this offset are guaranteed to be unused (so far)
                    // because we got the idx from our fetch_add above, and ptr is
                    // guaranteed to be valid because of ensure_slot_allocated
                    unsafe { ptr.write(value) };
                    
                    // Now we need to increase the size of the vec, so it can get read. 
                    // We do this exactly as in push, see that for explanations
                    while self
                        .count
                        .compare_exchange(idx, idx + 1, Ordering::Release, Ordering::Relaxed)
                        .is_err()
                    {
                        std::hint::spin_loop();
                    }
                    return Some(idx);
                },
                Err(read_value) => {
                    // another thread reserved an entry in the meantime, try again
                    current_reserved = read_value;
                }
            }
        }
        return None;
    }
    /// Append an element to the array
    ///
    /// This is notable in that it doesn't require a `&mut self`, because it
    /// does appropriate atomic synchronization.
    ///
    /// The return value is the index tha was pushed to.
    pub fn push(&self, val: T) -> usize {
        let idx = self.pre_push(val);

        // Now we need to increase the size of the vec, so it can get read.  We
        // use Release upon success, to ensure that the value which we wrote is
        // visible to any thread that has confirmed that the count is big enough
        // to read that element.  In case of failure, we can be relaxed, since
        // we don't do anything with the result other than try again.
        let mut failures = 0;
        while self
            .count
            .compare_exchange(idx, idx + 1, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            // This means that someone else *started* pushing before we started,
            // but hasn't yet finished.  We have to wait for them to finish
            // pushing before we can update the count.
            spin_wait(&mut failures);
        }
        idx
    }
    /// Extend the vec with the contents of an iterator.
    ///
    /// Note: this is currently no more efficient than calling `push` for each
    /// element of the iterator.
    pub fn extend(&self, iter: impl IntoIterator<Item = T>) {
        for val in iter {
            self.push(val);
        }
    }
    /// Append an element to the array with exclusive access
    ///
    /// This is slightly more efficient than [`AppendOnlyVec::push`] since it
    /// doesn't need to worry about concurrent access.
    ///
    /// The return value is the new size of the array.
    pub fn push_mut(&mut self, val: T) -> usize {
        let idx = self.pre_push(val);
        // We do not need synchronization here because no one else has access to
        // this data, and if it is passed to another thread that will involve
        // the appropriate memory barrier.
        self.count.store(idx + 1, Ordering::Relaxed);
        idx
    }
    const EMPTY: UnsafeCell<*mut T> = UnsafeCell::new(std::ptr::null_mut());
    /// Allocate a new empty array
    pub const fn new() -> Self {
        AppendOnlyVec {
            count: AtomicUsize::new(0),
            reserved: AtomicUsize::new(0),
            data: [Self::EMPTY; BITS_USED - 1 - 3],
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
        // already visible, since self.len() used Ordering::Acquire on
        // `self.count` which synchronizes with the Ordering::Release write in
        // `self.push`.
        let ptr = *self.data[array as usize].get();
        &*ptr.add(offset)
    }

    /// Convert into a standard `Vec`
    pub fn into_vec(self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.len());

        for idx in 0..self.len() {
            let (array, offset) = indices(idx);
            // We use a Relaxed load of the pointer, because the loop above (which
            // ends before `self.len()`) should ensure that the data we want is
            // already visible, since it Acquired `self.count` which synchronizes
            // with the write in `self.push`.
            let ptr = unsafe { *self.data[array as usize].get() };

            // Copy the element value. The copy remaining in the array must not
            // be used again (i.e. make sure we do not drop it)
            let value = unsafe { ptr.add(offset).read() };

            vec.push(value);
        }

        // Prevent dropping the copied-out values by marking the count as 0 before
        // our own drop is run
        self.count.store(0, Ordering::Relaxed);

        vec
    }
}
impl<T> std::fmt::Debug for AppendOnlyVec<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T> Index<usize> for AppendOnlyVec<T> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.len()); // this includes the required ordering memory barrier
        let (array, offset) = indices(idx);
        // The ptr value below is safe, because the length check above will
        // ensure that the data we want is already visible, since it used
        // Ordering::Acquire on `self.count` which synchronizes with the
        // Ordering::Release write in `self.push`.
        let ptr = unsafe { *self.data[array as usize].get() };
        unsafe { &*ptr.add(offset) }
    }
}

impl<T> IndexMut<usize> for AppendOnlyVec<T> {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        assert!(idx < self.len()); // this includes the required ordering memory barrier
        let (array, offset) = indices(idx);
        // The ptr value below is safe, because the length check above will
        // ensure that the data we want is already visible, since it used
        // Ordering::Acquire on `self.count` which synchronizes with the
        // Ordering::Release write in `self.push`.
        let ptr = unsafe { *self.data[array as usize].get() };

        // `&mut` is safe because there can be no access to data owned by
        // `self` except via `self`, and we have `&mut` on `self`
        unsafe { &mut *ptr.add(offset) }
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
            let ptr = unsafe { *self.data[array as usize].get() };
            unsafe {
                std::ptr::drop_in_place(ptr.add(offset));
            }
        }
        // Now we will free all the arrays.
        for array in 0..self.data.len() as u32 {
            // This load is relaxed because no other thread can have a reference
            // to Self because we have a &mut self.
            let ptr = unsafe { *self.data[array as usize].get() };
            if !ptr.is_null() {
                let layout = self.layout(array);
                unsafe { std::alloc::dealloc(ptr as *mut u8, layout) };
            } else {
                break;
            }
        }
    }
}

impl<T> Clone for AppendOnlyVec<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        // FIXME this could be optimized to avoid reloading pointers.
        self.iter().cloned().collect()
    }
}

/// An `Iterator` for the values contained in the `AppendOnlyVec`
#[derive(Debug)]
pub struct IntoIter<T>(std::vec::IntoIter<T>);

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> IntoIterator for AppendOnlyVec<T> {
    type Item = T;

    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.into_vec().into_iter())
    }
}

impl<T> FromIterator<T> for AppendOnlyVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let out = Self::new();
        for x in iter {
            let idx = out.pre_push(x);
            // We can be relaxed here because no one else has access to
            // this data, and if it is passed to another thread that will involve
            // the appropriate memory barrier.
            out.count.store(idx + 1, Ordering::Relaxed);
        }
        out
    }
}

impl<T> From<Vec<T>> for AppendOnlyVec<T> {
    fn from(value: Vec<T>) -> Self {
        value.into_iter().collect()
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

#[test]
fn test_parallel_pushing() {
    use std::sync::Arc;
    let v = Arc::new(AppendOnlyVec::<u64>::new());
    let mut threads = Vec::new();
    const N: u64 = 100;
    for thread_num in 0..N {
        let v = v.clone();
        threads.push(std::thread::spawn(move || {
            let which1 = v.push(thread_num);
            let which2 = v.push(thread_num);
            assert_eq!(v[which1 as usize], thread_num);
            assert_eq!(v[which2 as usize], thread_num);
        }));
    }
    for t in threads {
        t.join().ok();
    }
    for thread_num in 0..N {
        assert_eq!(2, v.iter().copied().filter(|&x| x == thread_num).count());
    }
}

#[test]
fn test_parallel_pushing_len_restriction() {
    use std::sync::Arc;
    let v = Arc::new(AppendOnlyVec::<u64>::new());
    let mut threads = Vec::new();
    const N: u64 = 100;
    for thread_num in 0..N {
        let v = v.clone();
        threads.push(std::thread::spawn(move || {
            let index = v.compute_next(|i| Some(((i as u64) << 32) | (thread_num as u64)));
            assert!(index.is_some());
            let expected = ((index.unwrap() as u64) << 32) | (thread_num as u64);
            assert_eq!(v[index.unwrap()], expected);
        }));
    }
    for t in threads {
        t.join().ok();
    }
    assert_eq!(N, v.len() as u64);
    for i in 0..N {
        assert_eq!(i, v[i as usize] >> 32);
    }
    for thread_num in 0..N {
        assert_eq!(1, v.iter().copied().filter(|&x| x & ((1 << 32) - 1) == thread_num).count());
    }
}

#[test]
fn test_into_vec() {
    struct SafeToDrop(bool);

    impl Drop for SafeToDrop {
        fn drop(&mut self) {
            assert!(self.0);
        }
    }

    let v = AppendOnlyVec::new();

    for _ in 0..50 {
        v.push(SafeToDrop(false));
    }

    let mut v = v.into_vec();

    for i in v.iter_mut() {
        i.0 = true;
    }
}

#[test]
fn test_push_then_index_mut() {
    let mut v = AppendOnlyVec::<usize>::new();
    for i in 0..1024 {
        v.push(i);
    }
    for i in 0..1024 {
        v[i] += i;
    }
    for i in 0..1024 {
        assert_eq!(v[i], 2 * i);
    }
}

#[test]
fn test_from_vec() {
    for v in [vec![5_i32, 4, 3, 2, 1], Vec::new(), vec![1]] {
        let aov: AppendOnlyVec<i32> = v.clone().into();
        assert_eq!(v, aov.into_vec());
    }
}

#[test]
fn test_clone() {
    let v = AppendOnlyVec::<String>::new();
    for i in 0..1024 {
        v.push(format!("{}", i));
    }
    let v2 = v.clone();

    assert_eq!(v.len(), v2.len());
    for i in 0..1024 {
        assert_eq!(v[i], v2[i]);
    }
}

#[test]
fn test_push_mut() {
    let mut v = AppendOnlyVec::new();
    for i in 0..1024 {
        v.push_mut(format!("{}", i));
    }
    assert_eq!(v.len(), 1024);
}
