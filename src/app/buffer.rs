use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// NOTE: Only ONE writer can ever write to a DoubleBuffer at anytime;
/// having multiple writer threads for a singular DoubleBuffer is considered
/// UB.
pub struct DoubleBuffer<T> {
    buffers: [UnsafeCell<Vec<T>>; 2],
    active_buffer: AtomicUsize,
}

impl<T> DoubleBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self::with_capacity(capacity)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffers: [
                UnsafeCell::new(Vec::with_capacity(capacity)),
                UnsafeCell::new(Vec::with_capacity(capacity)),
            ],
            active_buffer: AtomicUsize::new(0),
        }
    }

    /// multiple readers can call concurrently without locking the writer out
    pub fn read(&self) -> &[T] {
        let active_idx = self.active_buffer.load(Ordering::Acquire);

        // SAFETY: We only read from the active buffer, and the single writer
        // only modifies the inactive buffer
        unsafe { &*self.buffers[active_idx].get() }
    }

    /// only safe if called by a SINGLE WRITER thread
    /// The closure should fully update the provided inactive buffer.
    pub fn write<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut Vec<T>),
    {
        let active_idx = self.active_buffer.load(Ordering::Acquire);
        let inactive_idx = 1 - active_idx;

        // SAFETY: Single writer guarantee means only we access the inactive buffer
        unsafe {
            let inactive_buffer = &mut *self.buffers[inactive_idx].get();
            update_fn(inactive_buffer);
        }

        // Atomic swap publishes the new inactive buffer as active.
        self.active_buffer.store(inactive_idx, Ordering::Release);
    }

    /// Moves all values from `data` into the inactive buffer and swaps atomically.
    pub fn write_from_vec(&self, mut data: Vec<T>) {
        self.write(|inactive| {
            inactive.clear();
            inactive.append(&mut data);
        });
    }

    pub fn len(&self) -> usize {
        self.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }
}

impl<T: Clone> DoubleBuffer<T> {
    pub fn write_from_slice(&self, data: &[T]) {
        self.write(|inactive| {
            inactive.clear();
            inactive.extend_from_slice(data);
        });
    }
}

impl<T: Default> DoubleBuffer<T> {
    /// Backward-compatible API for callers that write into a sized slice.
    pub fn write_batch_resize<F>(&self, new_size: usize, update_fn: F)
    where
        F: FnOnce(&mut [T]),
    {
        self.write(|inactive| {
            if inactive.len() < new_size {
                inactive.resize_with(new_size, T::default);
            } else {
                inactive.truncate(new_size);
            }
            update_fn(&mut inactive[..new_size]);
        });
    }
}

unsafe impl<T: Send> Send for DoubleBuffer<T> {}
unsafe impl<T: Send + Sync> Sync for DoubleBuffer<T> {}
