use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free double buffer: single writer, multiple readers.
///
/// Only ONE writer thread may call `write` at any time.
/// Multiple readers may call `read` concurrently without blocking the writer,
/// except that the writer will spin-wait if readers are still borrowing the
/// buffer it needs to reclaim.
pub struct DoubleBuffer<T> {
    buffers: [UnsafeCell<Vec<T>>; 2],
    /// Index of the buffer currently visible to readers (0 or 1).
    active_buffer: AtomicUsize,
    /// Per-buffer reader counts. A reader increments on entry and decrements
    /// on exit so the writer knows when the old active buffer is safe to reuse.
    reader_count: [AtomicUsize; 2],
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
            reader_count: [AtomicUsize::new(0), AtomicUsize::new(0)],
        }
    }

    /// Multiple readers may call this concurrently.
    /// Returns a guard that derefs to `&[T]`; the borrow is tracked so the
    /// writer will not mutate this buffer while any guard is alive.
    pub fn read(&self) -> ReadGuard<'_, T> {
        loop {
            let idx = self.active_buffer.load(Ordering::Acquire);
            self.reader_count[idx].fetch_add(1, Ordering::AcqRel);

            // Re-check: if active_buffer changed between our load and the
            // increment, we registered on the wrong buffer — undo and retry.
            if self.active_buffer.load(Ordering::Acquire) == idx {
                return ReadGuard { buffer: self, idx };
            }

            self.reader_count[idx].fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Only safe if called by a SINGLE WRITER thread.
    pub fn write<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut Vec<T>),
    {
        let active_idx = self.active_buffer.load(Ordering::Acquire);
        let inactive_idx = 1 - active_idx;

        // Spin until all readers that were using the inactive buffer (from two
        // swaps ago) have dropped their guards.
        while self.reader_count[inactive_idx].load(Ordering::Acquire) != 0 {
            std::hint::spin_loop();
        }

        // SAFETY: No readers hold references into the inactive buffer and the
        // single-writer invariant guarantees exclusive access.
        unsafe {
            let inactive_buffer = &mut *self.buffers[inactive_idx].get();
            update_fn(inactive_buffer);
        }

        // Publish the newly-written buffer as active.
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
        let guard = self.read();
        guard.len()
    }

    pub fn is_empty(&self) -> bool {
        let guard = self.read();
        guard.is_empty()
    }
}

/// RAII guard returned by [`DoubleBuffer::read`]. Derefs to `&[T]` and
/// decrements the reader count on drop.
pub struct ReadGuard<'a, T> {
    buffer: &'a DoubleBuffer<T>,
    idx: usize,
}

impl<T> std::ops::Deref for ReadGuard<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        // SAFETY: While this guard is alive the writer will not touch
        // buffers[self.idx] because reader_count[self.idx] > 0.
        unsafe { &*self.buffer.buffers[self.idx].get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        self.buffer.reader_count[self.idx].fetch_sub(1, Ordering::AcqRel);
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
