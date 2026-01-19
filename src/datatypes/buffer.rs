use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// NOTE: Only ONE writer can ever write to a DoubleBuffer at anytime;
/// having multiple writer threads for a singular DoubleBuffer is considered
/// UB.
pub struct DoubleBuffer<T> {
    buffers: [UnsafeCell<Vec<T>>; 2], // UnsafeCell for interior mutability
    active_buffer: AtomicUsize,
    current_size: AtomicUsize,
    maximum_size: usize,
}

impl<T: Clone + Default> DoubleBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffers: [
                UnsafeCell::new(vec![T::default(); capacity]),
                UnsafeCell::new(vec![T::default(); capacity]),
            ],
            active_buffer: AtomicUsize::new(0),
            current_size: AtomicUsize::new(capacity),
            maximum_size: capacity,
        }
    }

    /// multiple readers can call concurrently without locking the writer out
    pub fn read(&self) -> &[T] {
        let active_idx = self.active_buffer.load(Ordering::Acquire);
        let size = self.current_size.load(Ordering::Acquire);

        // SAFETY: We only read from the active buffer, and the single writer
        // only modifies the inactive buffer
        unsafe {
            let buffer = &*self.buffers[active_idx].get();
            &buffer[..size]
        }
    }

    /// only safe if called by a SINGLE WRITER thread
    pub fn write_batch_resize<F>(&self, new_size: usize, update_fn: F)
    where
        F: FnOnce(&mut [T]),
    {
        if new_size > self.maximum_size {
            panic!(
                "Requested size {} exceeds max capacity {}",
                new_size, self.maximum_size
            );
        }

        let active_idx = self.active_buffer.load(Ordering::Acquire);
        let inactive_idx = 1 - active_idx;

        // SAFETY: Single writer guarantee means only we access the inactive buffer
        unsafe {
            let inactive_buffer = &mut *self.buffers[inactive_idx].get();
            update_fn(&mut inactive_buffer[..new_size]);
        }

        // Atomic swap: update size first, then swap buffers
        self.current_size.store(new_size, Ordering::Release);
        self.active_buffer.store(inactive_idx, Ordering::Release);
    }

    pub fn shrink_size(&self, new_size: usize) {
        let current = self.current_size.load(Ordering::Acquire);
        if new_size > current {
            panic!(
                "Requested size {} exceeds current_size {}",
                new_size, current
            );
        }
        self.current_size.store(new_size, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for DoubleBuffer<T> {}
unsafe impl<T: Send + Sync> Sync for DoubleBuffer<T> {}
