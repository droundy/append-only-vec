use std::ops::Index;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicPtr, AtomicUsize};
pub struct AppendOnlyVec<T> {
    count: AtomicUsize,
    reserved: AtomicUsize,
    data: [AtomicPtr<T>; BITS - 1],
}

const BITS: usize = std::mem::size_of::<usize>() * 8;

fn indices(i: usize) -> (u32, usize) {
    let i = i + 1; //
    let bin = BITS as u32 - 1 - i.leading_zeros();
    let offset = i - (1 << bin);
    (bin, offset)
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

impl<T> Index<usize> for AppendOnlyVec<T> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.len()); // this includes the required ordering memory barrier
        let (array, offset) = indices(idx);
        let ptr = self.data[array as usize].load(Ordering::Acquire);
        unsafe { &*ptr.add(offset) }
    }
}

impl<T> AppendOnlyVec<T> {
    pub fn iter(&self) -> impl Iterator<Item=&T> {
        (0..self.len()).map(|i| &self[i])
    }
    pub fn len(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
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
            std::hint::spin_loop();
        }
        unsafe {
            *(ptr.add(offset)) = val;
        }

        // Now we need to increase the size of the vec, so it can get read.
        while self
            .count
            .compare_exchange(idx, idx + 1, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            // This means that someone else *started* pushing before we started,
            // but hasn't yet finished.  We have to wait for them to finish
            // pushing before we can update the .
            std::hint::spin_loop();
        }
        idx
    }
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
