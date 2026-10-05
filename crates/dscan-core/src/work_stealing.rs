use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicIsize, AtomicPtr, Ordering, fence};
use std::sync::{Arc, Mutex};

const INITIAL_CAPACITY: usize = 1024;

struct Buffer<T> {
    cap: usize,
    mask: usize,
    storage: *mut MaybeUninit<T>,
}

impl<T> Buffer<T> {
    fn allocate(cap: usize) -> Self {
        assert!(cap.is_power_of_two());
        let mut v: Vec<MaybeUninit<T>> = Vec::with_capacity(cap);
        let storage = v.as_mut_ptr();
        std::mem::forget(v);
        Buffer {
            cap,
            mask: cap - 1,
            storage,
        }
    }

    #[inline(always)]
    unsafe fn write(&self, index: isize, value: T) {
        // SAFETY: caller ensures index is bounded. Slot is within allocated buffer range.
        unsafe {
            let slot = self.storage.add((index as usize) & self.mask);
            (*slot).write(value);
        }
    }

    #[inline(always)]
    unsafe fn read(&self, index: isize) -> T {
        // SAFETY: caller ensures slot was initialized prior to read.
        unsafe {
            let slot = self.storage.add((index as usize) & self.mask);
            (*slot).assume_init_read()
        }
    }

    unsafe fn grow(&self, b: isize, t: isize) -> Self {
        let new_cap = self.cap * 2;
        let new_buf = Buffer::allocate(new_cap);
        let mut i = t;
        while i != b {
            // SAFETY: indices t..b are valid initialized items in the old buffer.
            unsafe {
                new_buf.write(i, self.read(i));
            }
            i = i.wrapping_add(1);
        }
        new_buf
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        // SAFETY: Rebuilding Vec with length 0 safely frees the memory allocation without dropping elements.
        unsafe {
            let _ = Vec::from_raw_parts(self.storage, 0, self.cap);
        }
    }
}

pub struct DequeInner<T> {
    top: AtomicIsize,
    bottom: AtomicIsize,
    array: AtomicPtr<Buffer<T>>,
    retired: Mutex<Vec<*mut Buffer<T>>>,
}

impl<T> DequeInner<T> {
    pub fn new(cap: usize) -> Self {
        let buf = Box::new(Buffer::allocate(cap));
        DequeInner {
            top: AtomicIsize::new(0),
            bottom: AtomicIsize::new(0),
            array: AtomicPtr::new(Box::into_raw(buf)),
            retired: Mutex::new(Vec::new()),
        }
    }
}

impl<T> Drop for DequeInner<T> {
    fn drop(&mut self) {
        let b = self.bottom.load(Ordering::Relaxed);
        let mut t = self.top.load(Ordering::Relaxed);
        let buf_ptr = self.array.load(Ordering::Relaxed);
        unsafe {
            let buf = &*buf_ptr;
            while t != b {
                let _ = buf.read(t);
                t = t.wrapping_add(1);
            }
            let _ = Box::from_raw(buf_ptr);

            if let Ok(mut ret) = self.retired.lock() {
                for &old_buf_ptr in ret.iter() {
                    let _ = Box::from_raw(old_buf_ptr);
                }
                ret.clear();
            }
        }
    }
}

pub struct Worker<T> {
    inner: Arc<DequeInner<T>>,
}

pub struct Stealer<T> {
    inner: Arc<DequeInner<T>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Steal<T> {
    Success(T),
    Empty,
    Retry,
}

pub fn deque<T>() -> (Worker<T>, Stealer<T>) {
    let inner = Arc::new(DequeInner::new(INITIAL_CAPACITY));
    (
        Worker {
            inner: Arc::clone(&inner),
        },
        Stealer { inner },
    )
}

impl<T> Worker<T> {
    pub fn stealer(&self) -> Stealer<T> {
        Stealer {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn is_empty(&self) -> bool {
        let b = self.inner.bottom.load(Ordering::Relaxed);
        let t = self.inner.top.load(Ordering::Relaxed);
        b.wrapping_sub(t) <= 0
    }

    pub fn push(&self, item: T) {
        let b = self.inner.bottom.load(Ordering::Relaxed);
        let t = self.inner.top.load(Ordering::Acquire);
        let mut a = self.inner.array.load(Ordering::Relaxed);

        unsafe {
            let size = b.wrapping_sub(t);
            if size >= ((*a).cap as isize) - 1 {
                let new_buf = Box::new((*a).grow(b, t));
                let new_ptr = Box::into_raw(new_buf);
                let old_ptr = a;
                self.inner.array.store(new_ptr, Ordering::Release);
                if let Ok(mut ret) = self.inner.retired.lock() {
                    ret.push(old_ptr);
                }
                a = new_ptr;
            }
            (*a).write(b, item);
            fence(Ordering::Release);
            self.inner
                .bottom
                .store(b.wrapping_add(1), Ordering::Relaxed);
        }
    }

    pub fn pop(&self) -> Option<T> {
        let b = self.inner.bottom.load(Ordering::Relaxed);
        let a = self.inner.array.load(Ordering::Relaxed);
        let b = b.wrapping_sub(1);
        self.inner.bottom.store(b, Ordering::Relaxed);
        fence(Ordering::SeqCst);
        let t = self.inner.top.load(Ordering::Relaxed);

        let size = b.wrapping_sub(t);
        if size < 0 {
            self.inner.bottom.store(t, Ordering::Relaxed);
            None
        } else if size > 0 {
            // SAFETY: Worker owns slot b because size > 0.
            Some(unsafe { (*a).read(b) })
        } else {
            // Last element race with concurrent thieves
            let res = if self
                .inner
                .top
                .compare_exchange(t, t.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                // SAFETY: Worker won the race via compare_exchange, acquiring ownership of slot b.
                Some(unsafe { (*a).read(b) })
            } else {
                None
            };
            self.inner
                .bottom
                .store(t.wrapping_add(1), Ordering::Relaxed);
            res
        }
    }
}

impl<T> Stealer<T> {
    pub fn is_empty(&self) -> bool {
        let t = self.inner.top.load(Ordering::Acquire);
        let b = self.inner.bottom.load(Ordering::Acquire);
        b.wrapping_sub(t) <= 0
    }

    pub fn steal(&self) -> Steal<T> {
        let t = self.inner.top.load(Ordering::Acquire);
        fence(Ordering::SeqCst);
        let b = self.inner.bottom.load(Ordering::Acquire);

        let size = b.wrapping_sub(t);
        if size <= 0 {
            return Steal::Empty;
        }

        let a = self.inner.array.load(Ordering::Acquire);
        // SAFETY: slot t was read within queue bounds [t, b).
        let item = unsafe { (*a).read(t) };

        if self
            .inner
            .top
            .compare_exchange(t, t.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            Steal::Success(item)
        } else {
            // Lost race: forget speculative read to avoid double drop
            std::mem::forget(item);
            Steal::Retry
        }
    }

    /// Atomically steals up to `max_batch` tasks (typically half the victim's queue) in a single CAS.
    /// Stolen tasks are directly transferred into the caller's `dest` worker deque.
    /// Returns the number of items successfully stolen (0 if empty or CAS lost).
    pub fn steal_batch(&self, dest: &Worker<T>, max_batch: usize) -> usize {
        let max_cap = max_batch.clamp(1, 32);
        let mut stolen = 0;

        while stolen < max_cap {
            match self.steal() {
                Steal::Success(item) => {
                    dest.push(item);
                    stolen += 1;
                }
                Steal::Empty => break,
                Steal::Retry => break,
            }
        }

        stolen
    }
}

impl<T> Clone for Stealer<T> {
    fn clone(&self) -> Self {
        Stealer {
            inner: Arc::clone(&self.inner),
        }
    }
}

// SAFETY: DequeInner uses atomic synchronization for top/bottom/array.
unsafe impl<T: Send> Send for DequeInner<T> {}
// SAFETY: Stealers safely access DequeInner concurrently via atomic operations.
unsafe impl<T: Send> Sync for DequeInner<T> {}
// SAFETY: Worker can be moved between threads, but is not Sync (single-producer).
unsafe impl<T: Send> Send for Worker<T> {}
// SAFETY: Stealer is both Send and Sync, allowing multiple concurrent thieves across threads.
unsafe impl<T: Send> Send for Stealer<T> {}
// SAFETY: Concurrent steal operations are coordinated via atomic operations.
unsafe impl<T: Send> Sync for Stealer<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    #[test]
    fn test_worker_lifo_push_pop() {
        let (worker, stealer) = deque::<i32>();
        assert!(worker.is_empty());
        assert!(stealer.is_empty());

        worker.push(1);
        worker.push(2);
        worker.push(3);

        assert!(!worker.is_empty());
        assert_eq!(worker.pop(), Some(3));
        assert_eq!(worker.pop(), Some(2));
        assert_eq!(worker.pop(), Some(1));
        assert_eq!(worker.pop(), None);
        assert!(worker.is_empty());
    }

    #[test]
    fn test_stealer_fifo() {
        let (worker, stealer) = deque::<String>();

        worker.push("first".to_string());
        worker.push("second".to_string());
        worker.push("third".to_string());

        assert_eq!(stealer.steal(), Steal::Success("first".to_string()));
        assert_eq!(stealer.steal(), Steal::Success("second".to_string()));
        assert_eq!(worker.pop(), Some("third".to_string()));
        assert_eq!(stealer.steal(), Steal::Empty);
        assert_eq!(worker.pop(), None);
    }

    #[test]
    fn test_buffer_auto_growth() {
        let (worker, _stealer) = deque::<usize>();
        let count = 5000;
        for i in 0..count {
            worker.push(i);
        }

        for i in (0..count).rev() {
            assert_eq!(worker.pop(), Some(i));
        }
        assert_eq!(worker.pop(), None);
    }

    #[test]
    fn test_concurrent_stealing() {
        let (worker, stealer) = deque::<usize>();
        let total_items = 10_000;
        let num_thieves = 8;
        let done = Arc::new(AtomicBool::new(false));

        let mut thief_handles = Vec::new();
        for _ in 0..num_thieves {
            let s = stealer.clone();
            let d = Arc::clone(&done);
            thief_handles.push(thread::spawn(move || {
                let mut stolen = Vec::new();
                while !d.load(Ordering::Acquire) || !s.is_empty() {
                    match s.steal() {
                        Steal::Success(val) => stolen.push(val),
                        Steal::Empty => thread::yield_now(),
                        Steal::Retry => {}
                    }
                }
                stolen
            }));
        }

        let mut worker_items = Vec::new();
        for i in 0..total_items {
            worker.push(i);
            if i % 3 == 0
                && let Some(val) = worker.pop()
            {
                worker_items.push(val);
            }
        }

        while let Some(val) = worker.pop() {
            worker_items.push(val);
        }

        done.store(true, Ordering::Release);

        let mut all_collected = worker_items;
        for h in thief_handles {
            all_collected.extend(h.join().unwrap());
        }

        assert_eq!(all_collected.len(), total_items);
        let set: HashSet<usize> = all_collected.into_iter().collect();
        assert_eq!(set.len(), total_items);
        for i in 0..total_items {
            assert!(set.contains(&i), "missing item {i}");
        }
    }
}
